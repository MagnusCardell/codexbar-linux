import Gio from 'gi://Gio';
import St from 'gi://St';

import {MANUAL_REFRESH_OPTIONS} from './constants.js';
import {diagnosticsCopyText, safeUrl} from './state.js';

export class ShellActions {
    constructor(client, store) {
        this._client = client;
        this._store = store;
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

    openDashboard(url) {
        const safe = safeUrl(url);
        if (!safe)
            return false;
        try {
            Gio.AppInfo.launch_default_for_uri(safe, null);
            return true;
        } catch (_error) {
            return false;
        }
    }

    copyDiagnostics(payload) {
        const clipboard = St.Clipboard.get_default();
        clipboard.set_text(St.ClipboardType.CLIPBOARD, diagnosticsCopyText(payload));
    }
}
