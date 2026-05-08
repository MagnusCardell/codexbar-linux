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

const STRING_REPLY = GLib.VariantType.new('(s)');
const PANEL_MODE_VALUES = PANEL_MODES;
const RESET_TIME_FORMAT_VALUES = RESET_TIME_FORMATS;
const THEME_VALUES = THEMES;
const PANEL_MODE_TITLES = ['Merged meters', 'Provider detail', 'Minimal icon'];
const RESET_TIME_FORMAT_TITLES = ['Countdown', 'Absolute time', 'Both'];
const THEME_TITLES = ['System', 'Compact', 'High contrast'];
const PROVIDERS = [
    {id: 'codex', title: 'Codex'},
    {id: 'claude', title: 'Claude'},
    {id: 'gemini', title: 'Gemini'},
];
const SOURCE_VALUES = ['auto', 'upstream_cli', 'off'];
const SOURCE_TITLES = ['Automatic', 'Upstream CLI', 'Off'];
const REFRESH_INTERVAL_VALUES = [0, 60, 120, 300, 900, 1800];
const REFRESH_INTERVAL_TITLES = ['Manual', '1m', '2m', '5m', '15m', '30m'];
const REFRESH_INTERVAL_CUSTOM_INDEX = REFRESH_INTERVAL_VALUES.length;
const DEFAULT_DAEMON_SETTINGS = {
    schemaVersion: 1,
    refresh: {
        intervalSeconds: 300,
        startupRefresh: true,
        allowStaleCacheFallback: true,
    },
    providers: {},
    browserImport: {
        enabled: false,
        policy: 'off',
        profileIdAllowlist: [],
        domainAllowlistMode: 'provider_required_only',
    },
    diagnostics: {
        verbosity: 'normal',
        keepRedactedArtifacts: false,
    },
};

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
        this._providerRows = new Map();
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
        group.add(this._comboRow(
            settings,
            'selected-provider',
            'Panel provider',
            ['', ...PROVIDERS.map(provider => provider.id)],
            ['Automatic', ...PROVIDERS.map(provider => provider.title)]
        ));

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

        for (const provider of PROVIDERS) {
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
                this._setDaemonPatch({
                    schemaVersion: 1,
                    providers: {
                        [provider.id]: {
                            preferredSourceAdapter,
                        },
                    },
                });
            });

            const enabled = new Gtk.Switch({
                active: true,
                valign: Gtk.Align.CENTER,
            });
            enabled.connect('notify::active', toggle => {
                if (this._applyingDaemonSettings)
                    return;
                this._setDaemonPatch({
                    schemaVersion: 1,
                    providers: {
                        [provider.id]: {
                            enabled: toggle.get_active(),
                        },
                    },
                });
            });

            row.add_suffix(source);
            row.add_suffix(enabled);
            row.activatable_widget = enabled;
            group.add(row);
            this._providerRows.set(provider.id, {enabled, source});
        }

        return group;
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
        this._applyDaemonSettings(this._readDaemonSettings());
        try {
            const [infoJson, snapshotJson] = await Promise.all([
                this._daemon.getDaemonInfo(),
                this._daemon.getSnapshot(),
            ]);
            const info = JSON.parse(infoJson);
            const snapshot = JSON.parse(snapshotJson);
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

        if (snapshot?.selectedProvider)
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
        this._applyingDaemonSettings = true;
        try {
            const interval = daemonSettings.refresh?.intervalSeconds
                ?? DEFAULT_DAEMON_SETTINGS.refresh.intervalSeconds;
            this._setRefreshIntervalValue(interval);

            for (const provider of PROVIDERS) {
                const row = this._providerRows.get(provider.id);
                if (!row)
                    continue;
                const providerSettings = daemonSettings.providers?.[provider.id] ?? {};
                row.enabled.set_active(providerSettings.enabled ?? true);
                const preferred = providerSettings.preferredSourceAdapter === 'linux_web'
                    ? 'upstream_cli'
                    : (providerSettings.preferredSourceAdapter ?? 'auto');
                row.source.selected = Math.max(0, SOURCE_VALUES.indexOf(preferred));
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
            this._applyDaemonSettings(JSON.parse(settingsJson));
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
