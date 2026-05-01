import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

import {ShellActions} from './src/actions.js';
import {CodexbarDbusClient} from './src/dbusClient.js';
import {CodexbarIndicator} from './src/indicator.js';
import {SnapshotStore} from './src/snapshotStore.js';
import {SETTINGS_KEYS} from './src/constants.js';

export default class CodexBarExtension extends Extension {
    enable() {
        this._settings = this.getSettings();
        this._settingsSignals = [];
        this._store = new SnapshotStore();
        this._client = new CodexbarDbusClient();
        const openSettings = typeof this.openPreferences === 'function'
            ? () => this.openPreferences()
            : null;
        this._actions = new ShellActions(this._client, this._store, this._settings, openSettings);
        this._indicator = new CodexbarIndicator(this._actions);
        this._indicator.addToPanel();

        const store = this._store;
        const client = this._client;
        const current = callback => (...args) => {
            if (this._store !== store || this._client !== client)
                return;
            callback(...args);
        };

        this._storeSubscription = this._store.subscribe(() => this._render());
        for (const key of Object.values(SETTINGS_KEYS))
            this._settingsSignals.push(this._settings.connect(`changed::${key}`, () => this._render()));

        this._clientSignals = [
            client.on('snapshot', current(snapshotJson => store.applySnapshotJson(snapshotJson))),
            client.on('daemon-info', current(infoJson => store.applyDaemonInfoJson(infoJson))),
            client.on('provider-changed', current((providerId, eventJson) => store.applyProviderEventJson(providerId, eventJson))),
            client.on('refresh-started', current(refreshId => store.applyRefreshStarted(refreshId))),
            client.on('refresh-finished', current((refreshId, resultJson) => store.applyRefreshFinishedJson(refreshId, resultJson))),
            client.on('parse-error', current(error => store.applyClientError(error, 'parse_error', 'dbus_signal_parse_error'))),
            client.on('unavailable', current(error => store.applyClientError(error, 'daemon_unavailable', 'daemon_unavailable'))),
        ];
        client.start().catch(error => {
            if (this._store !== store || this._client !== client)
                return;
            store.applyClientError(error, 'daemon_unavailable', 'daemon_unavailable');
        });
    }

    disable() {
        for (const signalId of this._settingsSignals ?? [])
            this._settings?.disconnect(signalId);
        this._settingsSignals = [];

        this._storeSubscription?.disconnect();
        this._storeSubscription = null;

        for (const signal of this._clientSignals ?? [])
            signal.disconnect();
        this._clientSignals = [];

        this._client?.destroy();
        this._client = null;

        this._indicator?.destroy();
        this._indicator = null;

        this._store?.destroy();
        this._store = null;

        this._actions = null;
        this._settings = null;
    }

    _render() {
        if (!this._indicator || !this._store || !this._settings)
            return;
        const options = readUiOptions(this._settings);
        this._indicator.update(this._store.view(options), options);
    }
}

function readUiOptions(settings) {
    return {
        startDaemonOnLogin: settings.get_boolean(SETTINGS_KEYS.startDaemonOnLogin),
        panelMode: settings.get_string(SETTINGS_KEYS.panelMode),
        resetTimeFormat: settings.get_string(SETTINGS_KEYS.resetTimeFormat),
        theme: settings.get_string(SETTINGS_KEYS.theme),
        selectedProvider: settings.get_string(SETTINGS_KEYS.selectedProvider),
    };
}
