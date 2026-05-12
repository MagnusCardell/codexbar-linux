import Adw from 'gi://Adw';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Gtk from 'gi://Gtk';

import {ExtensionPreferences} from 'resource:///org/gnome/Shell/Extensions/js/extensions/prefs.js';

import {
    BUS_NAME,
    DBUS_TIMEOUT_MS,
    INTERFACE_NAME,
    OBJECT_PATH,
    PANEL_MODES,
    RESET_TIME_FORMATS,
    THEMES,
} from './src/constants.js';
import {
    DEFAULT_DAEMON_SETTINGS,
    buildProviderSettingsPatch,
    effectiveProviderSettings,
    providerCatalog,
} from './src/providerSettings.js';

const STRING_REPLY = GLib.VariantType.new('(s)');
const PANEL_MODE_VALUES = PANEL_MODES;
const RESET_TIME_FORMAT_VALUES = RESET_TIME_FORMATS;
const THEME_VALUES = THEMES;
const PANEL_MODE_TITLES = ['Merged meters', 'Provider detail', 'Minimal icon'];
const RESET_TIME_FORMAT_TITLES = ['Countdown', 'Absolute time', 'Both'];
const THEME_TITLES = ['System', 'Compact', 'High contrast'];
const SOURCE_VALUES = ['auto', 'upstream_cli', 'off'];
const SOURCE_TITLES = ['Automatic', 'Upstream CLI', 'Off'];
const REFRESH_INTERVAL_VALUES = [0, 60, 120, 300, 900, 1800];
const REFRESH_INTERVAL_TITLES = ['Manual', '1m', '2m', '5m', '15m', '30m'];
const REFRESH_INTERVAL_CUSTOM_INDEX = REFRESH_INTERVAL_VALUES.length;
class DaemonClient {
    callString(method, parameters = null) {
        return new Promise((resolve, reject) => {
            Gio.DBus.session.call(
                BUS_NAME,
                OBJECT_PATH,
                INTERFACE_NAME,
                method,
                parameters,
                STRING_REPLY,
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                null,
                (_connection, result) => {
                    try {
                        const variant = Gio.DBus.session.call_finish(result);
                        const [value] = variant.deep_unpack();
                        resolve(value);
                    } catch (error) {
                        reject(error);
                    }
                }
            );
        });
    }

    getDaemonInfo() {
        return this.callString('GetDaemonInfo');
    }

    getSnapshot() {
        return this.callString('GetSnapshot');
    }

    getSettings() {
        return this.callString('GetSettings');
    }

    setSettingsPatch(patch) {
        return this.callString(
            'SetSettingsPatch',
            GLib.Variant.new('(s)', [JSON.stringify(patch)])
        );
    }
}

export default class CodexBarPreferences extends ExtensionPreferences {
    fillPreferencesWindow(window) {
        const settings = this.getSettings();
        this._daemon = new DaemonClient();
        this._daemonStatusRow = null;
        this._daemonVersionRow = null;
        this._upstreamCliRow = null;
        this._refreshIntervalRow = null;
        this._refreshIntervalModel = null;
        this._refreshIntervalCustomSeconds = null;
        this._providerGroup = null;
        this._providerRows = new Map();
        this._providerCatalog = providerCatalog(DEFAULT_DAEMON_SETTINGS);
        this._panelProviderRow = null;
        this._panelProviderModel = null;
        this._panelProviderValues = [''];
        this._updatingPanelProviderModel = false;
        this._daemonSettings = DEFAULT_DAEMON_SETTINGS;
        this._applyingDaemonSettings = false;

        const page = new Adw.PreferencesPage({
            title: 'CodexBar',
            icon_name: 'preferences-system-symbolic',
        });

        page.add(this._buildGeneralGroup(settings));
        page.add(this._buildDaemonGroup());
        page.add(this._buildProviderGroup());
        window.add(page);

        this._loadDaemonState();
    }

    _buildGeneralGroup(settings) {
        const group = new Adw.PreferencesGroup({
            title: 'General',
        });

        group.add(this._comboRow(settings, 'panel-mode', 'Panel mode', PANEL_MODE_VALUES, PANEL_MODE_TITLES));
        group.add(this._comboRow(settings, 'reset-time-format', 'Reset time format', RESET_TIME_FORMAT_VALUES, RESET_TIME_FORMAT_TITLES));
        group.add(this._comboRow(settings, 'theme', 'Theme', THEME_VALUES, THEME_TITLES));
        group.add(this._providerComboRow(settings));

        return group;
    }

    _buildDaemonGroup() {
        const group = new Adw.PreferencesGroup({
            title: 'Daemon settings',
        });

        this._daemonStatusRow = new Adw.ActionRow({
            title: 'Daemon',
            subtitle: 'Checking D-Bus service',
        });
        const reload = new Gtk.Button({
            icon_name: 'view-refresh-symbolic',
            valign: Gtk.Align.CENTER,
        });
        reload.connect('clicked', () => this._loadDaemonState());
        this._daemonStatusRow.add_suffix(reload);
        group.add(this._daemonStatusRow);

        this._daemonVersionRow = new Adw.ActionRow({
            title: 'Version',
            subtitle: 'Unknown',
        });
        group.add(this._daemonVersionRow);

        this._upstreamCliRow = new Adw.ActionRow({
            title: 'Upstream CLI',
            subtitle: 'Unknown',
        });
        group.add(this._upstreamCliRow);

        this._refreshIntervalModel = new Gtk.StringList();
        for (const title of REFRESH_INTERVAL_TITLES)
            this._refreshIntervalModel.append(title);
        this._refreshIntervalModel.append('Custom');

        this._refreshIntervalRow = new Adw.ComboRow({
            title: 'Refresh interval',
            subtitle: 'Manual disables scheduled refresh',
            model: this._refreshIntervalModel,
            selected: refreshIntervalIndex(DEFAULT_DAEMON_SETTINGS.refresh.intervalSeconds),
        });
        this._refreshIntervalRow.connect('notify::selected', row => {
            if (this._applyingDaemonSettings)
                return;
            const intervalSeconds = row.selected === REFRESH_INTERVAL_CUSTOM_INDEX
                ? this._refreshIntervalCustomSeconds
                : REFRESH_INTERVAL_VALUES[row.selected];
            if (typeof intervalSeconds !== 'number')
                return;
            this._setDaemonPatch({
                schemaVersion: 1,
                refresh: {
                    intervalSeconds,
                },
            });
        });
        group.add(this._refreshIntervalRow);

        return group;
    }

    _buildProviderGroup() {
        const group = new Adw.PreferencesGroup({
            title: 'Providers',
        });
        this._providerGroup = group;
        this._syncProviderRows(this._providerCatalog);

        return group;
    }

    _providerComboRow(settings) {
        this._panelProviderModel = new Gtk.StringList();
        this._panelProviderRow = new Adw.ComboRow({
            title: 'Panel provider',
            model: this._panelProviderModel,
            selected: 0,
        });
        this._panelProviderRow.connect('notify::selected', combo => {
            if (this._updatingPanelProviderModel)
                return;
            const selected = this._panelProviderValues[combo.selected] ?? '';
            settings.set_string('selected-provider', selected);
        });
        this._syncPanelProviderModel(settings, this._providerCatalog);
        return this._panelProviderRow;
    }

    _syncProviderCatalog(...sources) {
        this._providerCatalog = providerCatalog(...sources);
        this._syncPanelProviderModel(this.getSettings(), this._providerCatalog);
        this._syncProviderRows(this._providerCatalog);
    }

    _syncPanelProviderModel(settings, providers) {
        if (!this._panelProviderModel || !this._panelProviderRow)
            return;
        const values = ['', ...providers.map(provider => provider.id)];
        const labels = ['Automatic', ...providers.map(provider => provider.title)];
        const selectedValue = settings.get_string('selected-provider');
        this._updatingPanelProviderModel = true;
        try {
            this._panelProviderValues = values;
            this._panelProviderModel.splice(0, this._panelProviderModel.get_n_items(), labels);
            this._panelProviderRow.selected = Math.max(0, values.indexOf(selectedValue));
        } finally {
            this._updatingPanelProviderModel = false;
        }
    }

    _syncProviderRows(providers) {
        if (!this._providerGroup)
            return;
        for (const provider of providers) {
            if (this._providerRows.has(provider.id))
                continue;
            const row = new Adw.ActionRow({
                title: provider.title,
                subtitle: provider.id,
            });
            const sourceModel = new Gtk.StringList();
            for (const title of SOURCE_TITLES)
                sourceModel.append(title);
            const source = new Gtk.DropDown({
                model: sourceModel,
                selected: 0,
                valign: Gtk.Align.CENTER,
            });
            source.width_request = 150;
            source.connect('notify::selected', dropdown => {
                if (this._applyingDaemonSettings)
                    return;
                const preferredSourceAdapter = SOURCE_VALUES[dropdown.selected] ?? SOURCE_VALUES[0];
                this._setDaemonPatch(buildProviderSettingsPatch(this._daemonSettings, {
                    providerId: provider.id,
                    preferredSourceAdapter,
                }));
            });

            const enabled = new Gtk.Switch({
                active: true,
                valign: Gtk.Align.CENTER,
            });
            enabled.connect('notify::active', toggle => {
                if (this._applyingDaemonSettings)
                    return;
                this._setDaemonPatch(buildProviderSettingsPatch(this._daemonSettings, {
                    providerId: provider.id,
                    enabled: toggle.get_active(),
                }));
            });

            row.add_suffix(source);
            row.add_suffix(enabled);
            row.activatable_widget = enabled;
            this._providerGroup.add(row);
            this._providerRows.set(provider.id, {enabled, source});
        }
    }

    _switchRow(settings, key, title) {
        const row = new Adw.ActionRow({title});
        const toggle = new Gtk.Switch({
            active: settings.get_boolean(key),
            valign: Gtk.Align.CENTER,
        });
        settings.bind(key, toggle, 'active', Gio.SettingsBindFlags.DEFAULT);
        row.add_suffix(toggle);
        row.activatable_widget = toggle;
        return row;
    }

    _comboRow(settings, key, title, values, labels = values) {
        const model = new Gtk.StringList();
        for (const label of labels)
            model.append(label);

        const row = new Adw.ComboRow({
            title,
            model,
            selected: Math.max(0, values.indexOf(settings.get_string(key))),
        });
        row.connect('notify::selected', combo => {
            const selected = values[combo.selected] ?? values[0];
            settings.set_string(key, selected);
        });
        return row;
    }

    async _loadDaemonState() {
        const localSettings = this._readDaemonSettings();
        this._syncProviderCatalog(localSettings);
        this._applyDaemonSettings(localSettings);
        try {
            const [infoJson, snapshotJson, settingsJson] = await Promise.all([
                this._daemon.getDaemonInfo(),
                this._daemon.getSnapshot(),
                this._daemon.getSettings(),
            ]);
            const info = JSON.parse(infoJson);
            const snapshot = JSON.parse(snapshotJson);
            const daemonSettings = JSON.parse(settingsJson);
            this._syncProviderCatalog(info, snapshot, daemonSettings);
            this._applyDaemonSettings(daemonSettings);
            this._setDaemonInfo(info, snapshot);
        } catch (error) {
            this._setDaemonUnavailable(error);
        }
    }

    _setDaemonInfo(info, snapshot) {
        if (this._daemonStatusRow)
            this._daemonStatusRow.subtitle = `${info.state ?? 'unknown'} · PID ${info.pid ?? '?'}`;
        if (this._daemonVersionRow)
            this._daemonVersionRow.subtitle = info.version ?? 'Unknown';
        if (this._upstreamCliRow) {
            const upstreamCli = info.upstreamCli ?? {};
            if (upstreamCli.available)
                this._upstreamCliRow.subtitle = upstreamCli.version ? `Available · ${upstreamCli.version}` : 'Available';
            else
                this._upstreamCliRow.subtitle = upstreamCli.diagnosticCode ?? 'Unavailable';
        }

        if (snapshot?.selectedProvider && this._providerCatalog.some(provider => provider.id === snapshot.selectedProvider))
            this.getSettings().set_string('selected-provider', snapshot.selectedProvider);
    }

    _setDaemonUnavailable(error) {
        if (this._daemonStatusRow)
            this._daemonStatusRow.subtitle = error?.message ?? 'Unavailable';
        if (this._daemonVersionRow)
            this._daemonVersionRow.subtitle = 'Unknown';
        if (this._upstreamCliRow)
            this._upstreamCliRow.subtitle = 'Unknown';
    }

    _readDaemonSettings() {
        const path = GLib.build_filenamev([
            GLib.get_user_config_dir(),
            'codexbar-linux',
            'config.json',
        ]);
        try {
            const [ok, contents] = Gio.File.new_for_path(path).load_contents(null);
            if (!ok)
                return DEFAULT_DAEMON_SETTINGS;
            const parsed = JSON.parse(new TextDecoder('utf-8').decode(contents));
            if (parsed?.schemaVersion !== 1)
                return DEFAULT_DAEMON_SETTINGS;
            return parsed;
        } catch (_error) {
            return DEFAULT_DAEMON_SETTINGS;
        }
    }

    _applyDaemonSettings(settings) {
        const daemonSettings = settings ?? DEFAULT_DAEMON_SETTINGS;
        this._daemonSettings = daemonSettings;
        this._applyingDaemonSettings = true;
        try {
            const interval = daemonSettings.refresh?.intervalSeconds
                ?? DEFAULT_DAEMON_SETTINGS.refresh.intervalSeconds;
            this._setRefreshIntervalValue(interval);

            const effectiveProviders = effectiveProviderSettings(daemonSettings, this._providerCatalog);
            for (const provider of this._providerCatalog) {
                const row = this._providerRows.get(provider.id);
                if (!row)
                    continue;
                const providerSettings = effectiveProviders[provider.id] ?? {};
                row.enabled.set_active(providerSettings.enabled ?? false);
                row.source.selected = Math.max(0, SOURCE_VALUES.indexOf(providerSettings.preferredSourceAdapter ?? 'auto'));
            }
        } finally {
            this._applyingDaemonSettings = false;
        }
    }

    _setRefreshIntervalValue(interval) {
        const selected = refreshIntervalIndex(interval);
        if (selected === REFRESH_INTERVAL_CUSTOM_INDEX) {
            this._refreshIntervalCustomSeconds = interval;
            this._refreshIntervalModel?.splice(
                REFRESH_INTERVAL_CUSTOM_INDEX,
                1,
                [`Custom (${formatRefreshInterval(interval)})`]
            );
        } else {
            this._refreshIntervalCustomSeconds = null;
            this._refreshIntervalModel?.splice(REFRESH_INTERVAL_CUSTOM_INDEX, 1, ['Custom']);
        }
        if (this._refreshIntervalRow)
            this._refreshIntervalRow.selected = selected;
    }

    async _setDaemonPatch(patch) {
        if (this._daemonStatusRow)
            this._daemonStatusRow.subtitle = 'Saving settings';
        try {
            const settingsJson = await this._daemon.setSettingsPatch(patch);
            const settings = JSON.parse(settingsJson);
            this._syncProviderCatalog(settings);
            this._applyDaemonSettings(settings);
            if (this._daemonStatusRow)
                this._daemonStatusRow.subtitle = 'Settings saved';
        } catch (error) {
            if (this._daemonStatusRow)
                this._daemonStatusRow.subtitle = error?.message ?? 'Settings update failed';
        }
    }
}

function refreshIntervalIndex(interval) {
    const index = REFRESH_INTERVAL_VALUES.indexOf(interval);
    return index >= 0 ? index : REFRESH_INTERVAL_CUSTOM_INDEX;
}

function formatRefreshInterval(seconds) {
    if (seconds === 0)
        return 'Manual';
    if (seconds % 60 === 0)
        return `${seconds / 60}m`;
    return `${seconds}s`;
}
