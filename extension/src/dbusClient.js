import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import {
    BUS_NAME,
    DBUS_TIMEOUT_MS,
    INTERFACE_NAME,
    OBJECT_PATH,
} from './constants.js';
import * as Log from './logger.js';

const STRING_REPLY = GLib.VariantType.new('(s)');
const REFRESH_FINISHED_REPLY = GLib.VariantType.new('(ss)');

export class CodexbarDbusClient {
    constructor() {
        this._connection = Gio.DBus.session;
        this._cancellable = new Gio.Cancellable();
        this._subscriptions = [];
        this._callbacks = new Map();
        this._retrySources = [];
        this.available = false;
    }

    on(name, callback) {
        const callbacks = this._callbacks.get(name) ?? [];
        callbacks.push(callback);
        this._callbacks.set(name, callbacks);
        return {
            disconnect: () => {
                const current = this._callbacks.get(name) ?? [];
                this._callbacks.set(name, current.filter(item => item !== callback));
            },
        };
    }

    start() {
        this._subscribeSignals();
        return this.refreshSnapshot({allowRetry: true});
    }

    destroy() {
        this._cancellable.cancel();
        for (const sourceId of this._retrySources)
            GLib.Source.remove(sourceId);
        this._retrySources = [];

        for (const id of this._subscriptions)
            this._connection.signal_unsubscribe(id);
        this._subscriptions = [];
        this._callbacks.clear();
        this.available = false;
    }

    async refreshSnapshot({allowRetry = false} = {}) {
        try {
            const [snapshotJson, daemonInfoJson] = await Promise.all([
                this.getSnapshot(),
                this.getDaemonInfo(),
            ]);
            this.available = true;
            this._emit('snapshot', snapshotJson);
            this._emit('daemon-info', daemonInfoJson);
        } catch (error) {
            this.available = false;
            this._emit('unavailable', error);
            if (allowRetry)
                this._scheduleStartupRetries();
        }
    }

    getSnapshot() {
        return this._callString('GetSnapshot');
    }

    refresh(optionsJson) {
        return this._callString('Refresh', GLib.Variant.new('(s)', [optionsJson]));
    }

    getDiagnostics(providerId = 'global') {
        return this._callString('GetDiagnostics', GLib.Variant.new('(s)', [providerId || 'global']));
    }

    getDaemonInfo() {
        return this._callString('GetDaemonInfo');
    }

    _subscribeSignals() {
        this._signal('SnapshotChanged', params => {
            const [snapshotJson] = params.deep_unpack();
            this.available = true;
            this._emit('snapshot', snapshotJson);
        });
        this._signal('ProviderChanged', params => {
            const [providerId, providerEventJson] = params.deep_unpack();
            this.available = true;
            this._emit('provider-changed', providerId, providerEventJson);
        });
        this._signal('RefreshStarted', params => {
            const [refreshId] = params.deep_unpack();
            this.available = true;
            this._emit('refresh-started', refreshId);
        });
        this._signal('RefreshFinished', params => {
            const [refreshId, resultJson] = params.deep_unpack();
            this.available = true;
            this._emit('refresh-finished', refreshId, resultJson);
            this.refreshSnapshot().catch(error => Log.warn(error.message));
        });
    }

    _signal(signalName, callback) {
        const id = this._connection.signal_subscribe(
            BUS_NAME,
            INTERFACE_NAME,
            signalName,
            OBJECT_PATH,
            null,
            Gio.DBusSignalFlags.NONE,
            (_connection, _sender, _path, _iface, _signal, params) => {
                try {
                    callback(params);
                } catch (error) {
                    this._emit('parse-error', error);
                }
            }
        );
        this._subscriptions.push(id);
    }

    _callString(method, parameters = null) {
        return new Promise((resolve, reject) => {
            this._connection.call(
                BUS_NAME,
                OBJECT_PATH,
                INTERFACE_NAME,
                method,
                parameters,
                method === 'RefreshFinished' ? REFRESH_FINISHED_REPLY : STRING_REPLY,
                Gio.DBusCallFlags.NONE,
                DBUS_TIMEOUT_MS,
                this._cancellable,
                (_connection, result) => {
                    try {
                        const variant = this._connection.call_finish(result);
                        const [value] = variant.deep_unpack();
                        resolve(value);
                    } catch (error) {
                        reject(error);
                    }
                }
            );
        });
    }

    _scheduleStartupRetries() {
        if (this._retrySources.length > 0)
            return;

        for (const delayMs of [1000, 3000]) {
            const id = GLib.timeout_add(GLib.PRIORITY_DEFAULT, delayMs, () => {
                this._retrySources = this._retrySources.filter(sourceId => sourceId !== id);
                this.refreshSnapshot().catch(error => Log.warn(error.message));
                return GLib.SOURCE_REMOVE;
            });
            this._retrySources.push(id);
        }
    }

    _emit(name, ...args) {
        const callbacks = this._callbacks.get(name) ?? [];
        for (const callback of callbacks) {
            try {
                callback(...args);
            } catch (error) {
                Log.warn(error.message);
            }
        }
    }
}
