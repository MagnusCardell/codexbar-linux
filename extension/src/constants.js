export const BUS_NAME = 'org.codexbar.Linux1';
export const OBJECT_PATH = '/org/codexbar/Linux1';
export const INTERFACE_NAME = 'org.codexbar.Linux1';
export const DBUS_TIMEOUT_MS = 5000;

export const PANEL_MODES = ['merged', 'provider', 'minimal'];
export const RESET_TIME_FORMATS = ['countdown', 'absolute', 'both'];
export const THEMES = ['system', 'compact', 'high_contrast'];

export const SETTINGS_KEYS = {
    startDaemonOnLogin: 'start-daemon-on-login',
    panelMode: 'panel-mode',
    resetTimeFormat: 'reset-time-format',
    theme: 'theme',
    selectedProvider: 'selected-provider',
};

export const PROVIDER_STATES = [
    'loading',
    'ok',
    'stale',
    'unauthenticated',
    'cookie_rejected',
    'missing_dependency',
    'provider_unavailable',
    'parse_error',
    'timeout',
    'error',
];

export const UI_STATES = [
    ...PROVIDER_STATES,
    'no_providers',
    'daemon_unavailable',
];

export const STATE_LABELS = {
    loading: 'Loading usage…',
    ok: 'Up to date',
    stale: 'Stale data',
    unauthenticated: 'Sign-in required',
    cookie_rejected: 'Session rejected',
    missing_dependency: 'Dependency missing',
    provider_unavailable: 'Provider unavailable',
    parse_error: 'Could not read provider data',
    timeout: 'Provider timed out',
    error: 'Error',
    no_providers: 'No providers enabled',
    daemon_unavailable: 'Daemon unavailable',
};

export const MANUAL_REFRESH_OPTIONS = {
    schemaVersion: 1,
    reason: 'manual',
    force: true,
    busyBehavior: 'return_existing',
    sourceAdapterPolicy: {
        mode: 'only',
        adapters: ['upstream_cli'],
    },
};

export const SIGNALS = {
    snapshotChanged: 'SnapshotChanged',
    providerChanged: 'ProviderChanged',
    refreshStarted: 'RefreshStarted',
    refreshFinished: 'RefreshFinished',
};

export const MAX_PROVIDER_INDICATORS = 3;
export const UNKNOWN_TEXT = 'Unknown';
export const PRODUCT_NAME = 'CodexBar';
export const EXTENSION_STATUS_AREA_NAME = 'codexbar-linux';
