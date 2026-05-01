import Adw from 'gi://Adw';
import Gio from 'gi://Gio';
import Gtk from 'gi://Gtk';

import {ExtensionPreferences} from 'resource:///org/gnome/Shell/Extensions/js/extensions/prefs.js';

const PANEL_MODE_VALUES = ['merged', 'provider', 'minimal'];
const RESET_TIME_FORMAT_VALUES = ['countdown', 'absolute', 'both'];
const THEME_VALUES = ['system', 'compact', 'high_contrast'];
const PANEL_MODE_TITLES = ['Merged meters', 'Provider detail', 'Minimal icon'];
const RESET_TIME_FORMAT_TITLES = ['Countdown', 'Absolute time', 'Both'];
const THEME_TITLES = ['System', 'Compact', 'High contrast'];

export default class CodexBarPreferences extends ExtensionPreferences {
    fillPreferencesWindow(window) {
        const settings = this.getSettings();
        const page = new Adw.PreferencesPage({
            title: 'CodexBar',
            icon_name: 'preferences-system-symbolic',
        });

        page.add(this._buildGeneralGroup(settings));
        page.add(this._buildPlaceholderGroup());
        window.add(page);
    }

    _buildGeneralGroup(settings) {
        const group = new Adw.PreferencesGroup({
            title: 'General',
        });

        group.add(this._switchRow(settings, 'start-daemon-on-login', 'Start daemon on login'));
        group.add(this._comboRow(settings, 'panel-mode', 'Panel mode', PANEL_MODE_VALUES, PANEL_MODE_TITLES));
        group.add(this._comboRow(settings, 'reset-time-format', 'Reset time format', RESET_TIME_FORMAT_VALUES, RESET_TIME_FORMAT_TITLES));
        group.add(this._comboRow(settings, 'theme', 'Theme', THEME_VALUES, THEME_TITLES));

        const selectedProvider = new Adw.EntryRow({
            title: 'Selected provider',
            text: settings.get_string('selected-provider'),
        });
        selectedProvider.connect('changed', row => {
            settings.set_string('selected-provider', row.get_text());
        });
        group.add(selectedProvider);

        return group;
    }

    _buildPlaceholderGroup() {
        const group = new Adw.PreferencesGroup({
            title: 'Daemon settings',
            description: 'Provider, browser import, refresh, and diagnostics settings are daemon-owned.',
        });

        const row = new Adw.ActionRow({
            title: 'Daemon configuration',
            subtitle: 'This vertical slice only edits Shell presentation preferences.',
        });
        row.activatable = false;
        group.add(row);

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
}
