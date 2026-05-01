import {
    applyDaemonInfoJson,
    applyDiagnosticsJson,
    applyProviderEventJson,
    applyRefreshFinishedJson,
    applyRefreshStarted,
    applySnapshotJson,
    createInitialState,
    normalizeUiOptions,
    normalizeViewState,
    withClientError,
} from './state.js';

export class SnapshotStore {
    constructor() {
        this._state = createInitialState();
        this._listeners = [];
    }

    destroy() {
        this._listeners = [];
        this._state = createInitialState();
    }

    subscribe(callback) {
        this._listeners.push(callback);
        callback(this._state);
        return {
            disconnect: () => {
                this._listeners = this._listeners.filter(item => item !== callback);
            },
        };
    }

    state() {
        return this._state;
    }

    view(options = {}) {
        return normalizeViewState(this._state, normalizeUiOptions(options));
    }

    applySnapshotJson(snapshotJson) {
        this._set(applySnapshotJson(this._state, snapshotJson));
    }

    applyDaemonInfoJson(infoJson) {
        this._set(applyDaemonInfoJson(this._state, infoJson));
    }

    applyProviderEventJson(providerId, eventJson) {
        this._set(applyProviderEventJson(this._state, providerId, eventJson));
    }

    applyRefreshStarted(refreshId) {
        this._set(applyRefreshStarted(this._state, refreshId));
    }

    applyRefreshFinishedJson(refreshId, resultJson) {
        this._set(applyRefreshFinishedJson(this._state, refreshId, resultJson));
    }

    applyDiagnosticsJson(providerId, diagnosticsJson) {
        this._set(applyDiagnosticsJson(this._state, providerId, diagnosticsJson));
    }

    applyClientError(error, state = 'daemon_unavailable', code = 'daemon_unavailable') {
        this._set(withClientError(this._state, error, Date.now(), state, code));
    }

    _set(nextState) {
        this._state = nextState;
        for (const listener of this._listeners)
            listener(this._state);
    }
}
