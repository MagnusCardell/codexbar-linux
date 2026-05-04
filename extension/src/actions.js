import St from 'gi://St';

import {MANUAL_REFRESH_OPTIONS, SETTINGS_KEYS} from './constants.js';
import {diagnosticsCopyText} from './state.js';

export class ShellActions {
    constructor(client, store, settings = null, openSettings = null) {
        this._client = client;
        this._store = store;
        this._settings = settings;
        this._openSettings = openSettings;
    }

    get canOpenSettings() {
        return typeof this._openSettings === 'function';
    }

    async refresh() {
        try {
            const refreshId = await this._client.refresh(JSON.stringify(MANUAL_REFRESH_OPTIONS));
            this._store.applyRefreshStarted(refreshId);
            return refreshId;
        } catch (error) {
            this._store.applyClientError(error, this._client.available ? 'error' : 'daemon_unavailable', 'manual_refresh_failed');
            return null;
        }
    }

    retryDaemon() {
        return this._client.refreshSnapshot({allowRetry: false});
    }

    selectProvider(providerId) {
        if (!this._settings || !providerId)
            return false;
        this._settings.set_string(SETTINGS_KEYS.selectedProvider, providerId);
        return true;
    }

    async loadDiagnostics(providerId = 'global') {
        try {
            const diagnosticsJson = await this._client.getDiagnostics(providerId || 'global');
            this._store.applyDiagnosticsJson(providerId || 'global', diagnosticsJson);
            return diagnosticsJson;
        } catch (error) {
            this._store.applyClientError(error, this._client.available ? 'error' : 'daemon_unavailable', 'diagnostics_failed');
            return null;
        }
    }

    copyDiagnostics(payload) {
        const clipboard = St.Clipboard.get_default();
        clipboard.set_text(St.ClipboardType.CLIPBOARD, diagnosticsCopyText(payload));
    }

    openSettings() {
        if (!this.canOpenSettings)
            return false;
        try {
            this._openSettings();
            return true;
        } catch (_error) {
            return false;
        }
    }
}
