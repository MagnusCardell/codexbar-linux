import {
    MAX_PROVIDER_INDICATORS,
    PANEL_MODES,
    PRODUCT_NAME,
    PROVIDER_STATES,
    RESET_TIME_FORMATS,
    STATE_LABELS,
    THEMES,
} from './constants.js';
import {
    DEFAULT_DAEMON_SETTINGS,
    SUPPORTED_PROVIDERS,
    effectiveProviderSettings,
} from './providerSettings.js';

export {PANEL_MODES, RESET_TIME_FORMATS, THEMES};

const PRODUCT_PANEL_PLACEHOLDER = PRODUCT_NAME;

const STATE_META = {
    loading: {
        label: STATE_LABELS.loading,
        severity: 'loading',
        description: 'Waiting for usage data',
        iconName: 'view-refresh-symbolic',
    },
    ok: {
        label: STATE_LABELS.ok,
        severity: 'ok',
        description: 'Usage data is current',
        iconName: 'emblem-ok-symbolic',
    },
    stale: {
        label: STATE_LABELS.stale,
        severity: 'warning',
        description: 'Showing cached data.',
        iconName: 'appointment-soon-symbolic',
    },
    unauthenticated: {
        label: STATE_LABELS.unauthenticated,
        severity: 'warning',
        description: 'Sign in with the upstream CLI, then refresh.',
        iconName: 'dialog-password-symbolic',
    },
    cookie_rejected: {
        label: 'Session unavailable',
        severity: 'warning',
        description: 'Use upstream CLI setup, then refresh.',
        iconName: 'dialog-warning-symbolic',
    },
    missing_dependency: {
        label: 'CLI setup needed',
        severity: 'warning',
        description: 'Install or select the upstream CodexBar CLI, then refresh.',
        iconName: 'dialog-warning-symbolic',
    },
    provider_unavailable: {
        label: STATE_LABELS.provider_unavailable,
        severity: 'warning',
        description: 'Check provider setup in the upstream CLI, then refresh.',
        iconName: 'network-offline-symbolic',
    },
    parse_error: {
        label: 'Provider data unreadable',
        severity: 'error',
        description: 'Update or rerun the upstream CLI, then refresh.',
        iconName: 'dialog-error-symbolic',
    },
    timeout: {
        label: 'Refresh timed out',
        severity: 'warning',
        description: 'Try Refresh again. If it repeats, open diagnostics.',
        iconName: 'alarm-symbolic',
    },
    error: {
        label: STATE_LABELS.error,
        severity: 'error',
        description: 'Refresh failed. Try Refresh again or open diagnostics.',
        iconName: 'dialog-error-symbolic',
    },
    no_providers: {
        label: STATE_LABELS.no_providers,
        severity: 'warning',
        description: 'Enable a provider in Preferences, then refresh.',
        iconName: 'dialog-warning-symbolic',
    },
    daemon_unavailable: {
        label: STATE_LABELS.daemon_unavailable,
        severity: 'error',
        description: 'Start the CodexBar daemon, then refresh.',
        iconName: 'network-offline-symbolic',
    },
};

const SOURCE_LABELS = {
    api: 'API',
    local: 'Local',
    web: 'Unsupported source',
    unknown: 'Unknown',
};

const ADAPTER_LABELS = {
    upstream_cli: 'Upstream CLI',
    linux_web: 'Unsupported adapter',
    cache: 'Cache fallback',
    fixture: 'Fixture',
    synthetic: 'Synthetic',
    none: 'None',
};

const SNAPSHOT_REQUIRED_KEYS = ['schemaVersion', 'generatedAt', 'stale', 'daemon', 'providers'];
const SNAPSHOT_OPTIONAL_KEYS = ['selectedProvider'];
const PROVIDER_REQUIRED_KEYS = ['provider', 'displayName', 'state', 'source', 'sourceAdapter', 'updatedAt', 'usage'];
const PROVIDER_OPTIONAL_KEYS = [
    'version',
    'staleSince',
    'credits',
    'identity',
    'status',
    'cost',
    'dashboardUrl',
    'diagnosticsSummary',
    'diagnosticCodes',
];
const SNAPSHOT_DAEMON_REQUIRED_KEYS = ['version', 'state'];
const SNAPSHOT_DAEMON_OPTIONAL_KEYS = ['lastRefreshId', 'lastRefreshStartedAt', 'lastRefreshFinishedAt', 'upstreamCli'];
const UPSTREAM_CLI_REQUIRED_KEYS = ['available'];
const UPSTREAM_CLI_OPTIONAL_KEYS = ['path', 'version', 'diagnosticCode'];
const USAGE_REQUIRED_KEYS = ['primary', 'secondary', 'tertiary'];
const METER_REQUIRED_KEYS = ['usedPercent', 'remainingPercent', 'windowMinutes', 'resetsAt', 'label'];
const METER_OPTIONAL_KEYS = ['detail'];
const IDENTITY_OPTIONAL_KEYS = [
    'providerAccountIdHash',
    'accountEmailDisplay',
    'accountEmailHash',
    'accountOrganizationDisplay',
    'accountOrganizationHash',
    'loginMethod',
];
const STATUS_REQUIRED_KEYS = ['indicator', 'description', 'updatedAt'];
const STATUS_OPTIONAL_KEYS = ['url'];
const CREDITS_REQUIRED_KEYS = ['remaining', 'remainingPercent', 'updatedAt'];
const CREDITS_OPTIONAL_KEYS = ['unit'];
const COST_REQUIRED_KEYS = ['updatedAt', 'currency', 'total', 'items'];
const COST_OPTIONAL_KEYS = ['periodStartAt', 'periodEndAt', 'diagnosticCodes'];
const COST_ITEM_REQUIRED_KEYS = ['label', 'amount', 'currency'];
const COST_ITEM_OPTIONAL_KEYS = ['detail'];
const DAEMON_STATES = ['starting', 'ok', 'refreshing', 'degraded', 'error'];
const SEMANTIC_SOURCES = ['api', 'local', 'web', 'unknown'];
const SOURCE_ADAPTERS = ['upstream_cli', 'linux_web', 'cache', 'fixture', 'synthetic', 'none'];
const DIAGNOSTIC_SCOPES = ['global', 'provider', 'browser_import', 'upstream_cli', 'settings'];
const DIAGNOSTICS_REQUIRED_KEYS = ['schemaVersion', 'generatedAt', 'scope', 'events', 'redaction'];
const DIAGNOSTICS_OPTIONAL_KEYS = ['provider'];
const DIAGNOSTIC_EVENT_REQUIRED_KEYS = ['code', 'severity', 'safeMessage', 'timestamp', 'redacted'];
const DIAGNOSTIC_EVENT_OPTIONAL_KEYS = ['provider', 'sourceAdapter', 'recoverable', 'details'];
const DIAGNOSTIC_REDACTION_REQUIRED_KEYS = ['applied', 'policyVersion'];
const DIAGNOSTIC_REDACTION_OPTIONAL_KEYS = ['notes'];
const DIAGNOSTIC_EVENT_REDACTED_REQUIRED_KEYS = ['applied'];
const DIAGNOSTIC_EVENT_REDACTED_OPTIONAL_KEYS = ['classes'];
const REFRESH_RESULT_STATUSES = ['ok', 'partial', 'error', 'busy', 'noop'];
const REFRESH_REASONS = ['manual', 'scheduled', 'startup', 'settings_changed', 'retry', 'test'];
const REFRESH_PROVIDER_STATUSES = [
    'ok',
    'stale',
    'skipped',
    'unauthenticated',
    'cookie_rejected',
    'missing_dependency',
    'provider_unavailable',
    'parse_error',
    'timeout',
    'error',
];
const REFRESH_RESULT_REQUIRED_KEYS = [
    'schemaVersion',
    'refreshId',
    'status',
    'startedAt',
    'finishedAt',
    'durationMs',
    'reason',
    'providers',
    'cacheWritten',
];
const REFRESH_RESULT_OPTIONAL_KEYS = ['snapshotGeneratedAt', 'diagnosticCodes'];
const REFRESH_PROVIDER_REQUIRED_KEYS = ['provider', 'status'];
const REFRESH_PROVIDER_OPTIONAL_KEYS = ['sourceAdapter', 'diagnosticCodes'];
const DIAGNOSTIC_SEVERITY_RANK = {
    info: 1,
    warning: 2,
    error: 3,
};
const FORBIDDEN_PUBLIC_KEYS = new Set([
    'accountemail',
    'email',
    'accountorganization',
    'organization',
    'raw',
    'rawpayload',
    'rawresponse',
    'rawstdout',
    'rawstderr',
    'rawoutput',
    'providerpayload',
    'headers',
    'requestheaders',
    'responseheaders',
    'authorization',
    'cookie',
    'cookies',
    'setcookie',
    'apikey',
    'accesstoken',
    'refreshtoken',
    'sessiontoken',
    'sessionkey',
    'stdout',
    'stderr',
    'stdouttext',
    'stderrtext',
    'stdoutjson',
    'stderrjson',
    'stdoutpath',
    'stderrpath',
    'token',
]);

const SECRET_PATTERNS = [
    [/\bAuthorization\s*:\s*[^\n\r,}]+/gi, 'Authorization: [redacted]'],
    [/\b(X-API-Key|X-Auth-Token)\s*:\s*[^\n\r,}]+/gi, '$1: [redacted]'],
    [/\bSet-Cookie\s*:\s*[^\n\r,}]+/gi, 'Set-Cookie: [redacted]'],
    [/\bCookie\s*:\s*[^\n\r,}]+/gi, 'Cookie: [redacted]'],
    [/\bBearer\s+[A-Za-z0-9._~+/=-]+/gi, 'Bearer [redacted]'],
    [/\bsk-(?:ant-)?[A-Za-z0-9_-]{16,}\b/g, '[redacted-token]'],
    [/\bgh[opsu]_[A-Za-z0-9_]{20,}\b/g, '[redacted-token]'],
    [/\bxox[baprs]-[A-Za-z0-9-]{16,}\b/g, '[redacted-token]'],
    [/\bAIza[0-9A-Za-z_-]{20,}\b/g, '[redacted-token]'],
    [/\beyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\b/g, '[redacted-token]'],
    [/\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b/g, '[redacted-email]'],
    [/(^|["'\s=:/?&#])\/(?:home|Users)\/[^"'\s,}&#]+/g, '$1[redacted-path]'],
    [/~\/(?:\.config|Library|AppData)\/[^"'\s,}]*/g, '[redacted-path]'],
    [/\b(?:api[_-]?key|access[_-]?token|refresh[_-]?token|session[_-]?(?:token|key))\b\s*[:=]\s*[^"'\s,}]+/gi, '[redacted]'],
];

export function createInitialState(nowMs = Date.now()) {
    return {
        snapshot: createSyntheticSnapshot('loading', {
            nowMs,
            summary: 'Waiting for daemon snapshot',
        }),
        clientState: 'loading',
        refreshing: false,
        activeRefreshId: null,
        lastRefreshResult: null,
        lastClientError: null,
        lastProviderEvent: null,
        daemonInfo: null,
        daemonSettings: DEFAULT_DAEMON_SETTINGS,
        diagnostics: null,
    };
}

export function createSyntheticSnapshot(state, options = {}) {
    const now = new Date(options.nowMs ?? Date.now()).toISOString();
    const providerId = options.providerId ?? 'codex';
    const displayName = options.displayName ?? 'Codex';
    const summary = options.summary ?? stateMeta(state).description;
    const daemonUnavailable = state === 'daemon_unavailable';
    const providerState = daemonUnavailable
        ? 'missing_dependency'
        : (PROVIDER_STATES.includes(state) ? state : 'provider_unavailable');

    return {
        schemaVersion: 1,
        generatedAt: now,
        stale: providerState === 'stale',
        selectedProvider: providerId,
        daemon: {
            version: 'unknown',
            state: daemonUnavailable ? 'error' : (providerState === 'loading' ? 'starting' : 'degraded'),
            lastRefreshId: null,
            lastRefreshStartedAt: null,
            lastRefreshFinishedAt: null,
            upstreamCli: null,
        },
        providers: [
            {
                provider: providerId,
                displayName,
                version: null,
                source: 'unknown',
                sourceAdapter: 'synthetic',
                state: providerState,
                updatedAt: providerState === 'loading' ? null : now,
                staleSince: providerState === 'stale' ? now : null,
                usage: {
                    primary: null,
                    secondary: null,
                    tertiary: null,
                },
                credits: null,
                identity: null,
                status: {
                    indicator: providerState,
                    description: summary,
                    updatedAt: providerState === 'loading' ? null : now,
                    url: null,
                },
                cost: null,
                dashboardUrl: null,
                diagnosticsSummary: summary,
                diagnosticCodes: options.diagnosticCodes ?? [],
            },
        ],
    };
}

export function applySnapshotJson(state, snapshotJson, nowMs = Date.now()) {
    const parsed = parseJsonObject(snapshotJson);
    if (!parsed.ok)
        return withClientError(state, parsed.error, nowMs, 'parse_error', 'dbus_parse_error');

    const snapshot = parsed.value;
    const validation = validateSnapshot(snapshot);
    if (!validation.ok)
        return withClientError(state, validation.error, nowMs, 'parse_error', 'dbus_parse_error');

    return {
        ...state,
        snapshot,
        clientState: deriveSnapshotState(snapshot),
        lastClientError: null,
    };
}

export function applyDaemonInfoJson(state, infoJson, nowMs = Date.now()) {
    const parsed = parseJsonObject(infoJson);
    if (!parsed.ok)
        return withClientError(state, parsed.error, nowMs, 'parse_error', 'daemon_info_parse_error');

    const daemonInfo = parsed.value;
    if (hasForbiddenPublicData(daemonInfo) || daemonInfo.schemaVersion !== 1 || !daemonInfo.capabilities || !daemonInfo.upstreamCli)
        return withClientError(state, 'Daemon info payload did not match the v1 shape', nowMs, 'parse_error', 'daemon_info_invalid');

    return {
        ...state,
        daemonInfo,
        lastClientError: null,
    };
}

export function applyDaemonSettingsJson(state, settingsJson) {
    if (typeof settingsJson !== 'string' || settingsJson.length === 0)
        return applyDaemonSettings(state, DEFAULT_DAEMON_SETTINGS);

    const parsed = parseJsonObject(settingsJson);
    if (!parsed.ok)
        return applyDaemonSettings(state, DEFAULT_DAEMON_SETTINGS);

    return applyDaemonSettings(state, parsed.value);
}

export function applyDaemonSettings(state, settings) {
    const daemonSettings = isDaemonSettingsPayload(settings)
        ? settings
        : DEFAULT_DAEMON_SETTINGS;

    return {
        ...state,
        daemonSettings,
    };
}

export function applyDiagnosticsJson(state, providerId, diagnosticsJson, nowMs = Date.now()) {
    const parsed = parseJsonObject(diagnosticsJson);
    if (!parsed.ok)
        return withClientError(state, parsed.error, nowMs, 'parse_error', 'diagnostics_parse_error');

    const diagnostics = parsed.value;
    const validation = validateDiagnosticsPayload(diagnostics);
    if (!validation.ok)
        return withClientError(state, 'Diagnostics payload did not match the v1 shape', nowMs, 'parse_error', 'diagnostics_invalid');

    return {
        ...state,
        diagnostics: {
            providerId: providerId || 'global',
            payload: diagnostics,
            copyText: diagnosticsCopyText(diagnostics),
            lines: diagnosticsDisplayLines(diagnostics),
            summary: diagnosticsSummaryLine(diagnostics),
        },
        lastClientError: null,
    };
}

export function clearDiagnostics(state) {
    return {
        ...state,
        diagnostics: null,
    };
}

export function applyProviderEventJson(state, providerId, eventJson, nowMs = Date.now()) {
    const parsed = parseJsonObject(eventJson);
    if (!parsed.ok)
        return withClientError(state, parsed.error, nowMs, 'parse_error', 'provider_event_parse_error');

    const event = parsed.value;
    const provider = event.provider;
    if (!provider || event.providerId !== providerId || provider.provider !== providerId)
        return withClientError(state, 'ProviderChanged payload identifiers did not match', nowMs, 'parse_error', 'provider_event_mismatch');
    if (hasForbiddenPublicData(event))
        return withClientError(state, 'ProviderChanged payload contained non-public fields', nowMs, 'parse_error', 'provider_event_forbidden_fields');
    if (!isFullProviderEvent(event))
        return withClientError(state, 'ProviderChanged payload did not contain a complete provider', nowMs, 'parse_error', 'provider_event_incomplete');

    const providers = Array.isArray(state.snapshot?.providers)
        ? state.snapshot.providers.slice()
        : [];
    const existingIndex = providers.findIndex(item => item.provider === providerId);
    if (existingIndex >= 0)
        providers[existingIndex] = provider;
    else
        providers.push(provider);

    return {
        ...state,
        snapshot: {
            ...state.snapshot,
            providers,
        },
        clientState: deriveSnapshotState({
            ...state.snapshot,
            providers,
        }),
        lastProviderEvent: event,
        lastClientError: null,
    };
}

export function applyRefreshStarted(state, refreshId) {
    if (!state?.refreshing && state?.lastRefreshResult?.refreshId === refreshId)
        return state;

    return {
        ...state,
        clientState: 'loading',
        refreshing: true,
        activeRefreshId: refreshId,
        snapshot: {
            ...state.snapshot,
            daemon: {
                ...(state.snapshot?.daemon ?? {}),
                state: 'refreshing',
                lastRefreshId: refreshId,
                lastRefreshStartedAt: new Date().toISOString(),
            },
        },
    };
}

export function applyRefreshFinishedJson(state, refreshId, resultJson, nowMs = Date.now()) {
    const parsed = parseJsonObject(resultJson);
    if (!parsed.ok)
        return withClientError({
            ...state,
            refreshing: false,
            activeRefreshId: null,
        }, parsed.error, nowMs, 'parse_error', 'refresh_result_parse_error');

    const result = parsed.value;
    const safeResult = refreshResultProjection(result);
    if (!safeResult)
        return withClientError({
            ...state,
            refreshing: false,
            activeRefreshId: null,
        }, 'Refresh result payload did not match the v1 shape', nowMs, 'parse_error', 'refresh_result_invalid');

    return {
        ...state,
        clientState: deriveSnapshotState(state.snapshot),
        refreshing: false,
        activeRefreshId: null,
        lastRefreshResult: safeResult,
        snapshot: {
            ...state.snapshot,
            daemon: {
                ...(state.snapshot?.daemon ?? {}),
                lastRefreshId: refreshId || safeResult.refreshId || state.snapshot?.daemon?.lastRefreshId || null,
                lastRefreshFinishedAt: safeResult.finishedAt ?? new Date(nowMs).toISOString(),
            },
        },
        lastClientError: null,
    };
}

export function withClientError(state, error, nowMs = Date.now(), clientState = 'daemon_unavailable', diagnosticCode = 'daemon_unavailable') {
    const safeMessage = redactText(errorToMessage(error));
    return {
        ...state,
        snapshot: createSyntheticSnapshot(clientState, {
            nowMs,
            summary: safeMessage || 'Daemon unavailable',
            diagnosticCodes: [diagnosticCode],
        }),
        clientState,
        refreshing: false,
        activeRefreshId: null,
        lastClientError: safeMessage,
        diagnostics: null,
    };
}

export function normalizeUiOptions(options = {}) {
    const panelMode = PANEL_MODES.includes(options.panelMode)
        ? options.panelMode
        : 'merged';
    const resetTimeFormat = RESET_TIME_FORMATS.includes(options.resetTimeFormat)
        ? options.resetTimeFormat
        : 'countdown';
    const theme = THEMES.includes(options.theme)
        ? options.theme
        : 'system';

    return {
        panelMode,
        resetTimeFormat,
        theme,
        selectedProvider: typeof options.selectedProvider === 'string' ? options.selectedProvider : '',
        startDaemonOnLogin: Boolean(options.startDaemonOnLogin),
        daemonSettings: isDaemonSettingsPayload(options.daemonSettings) ? options.daemonSettings : null,
    };
}

export function selectProvider(snapshot, selectedProvider = '') {
    const providers = Array.isArray(snapshot?.providers) ? snapshot.providers : [];
    if (providers.length === 0)
        return null;

    const requested = selectedProvider || snapshot?.selectedProvider || '';
    if (requested) {
        const match = providers.find(provider => provider.provider === requested);
        if (match)
            return match;
    }

    return mostConstrainedUsableProvider(providers)
        ?? providers.find(provider => provider.state === 'stale')
        ?? providers[0];
}

export function deriveSnapshotState(snapshot) {
    const providers = Array.isArray(snapshot?.providers) ? snapshot.providers : [];
    if (providers.length === 0)
        return 'no_providers';
    if (snapshot?.stale)
        return 'stale';
    if (providers.some(provider => provider?.state === 'ok'))
        return 'ok';
    return providers[0]?.state ?? 'error';
}

export function normalizeViewState(state, options = {}) {
    const uiOptions = normalizeUiOptions(options);
    const snapshot = state?.snapshot ?? createSyntheticSnapshot('loading');
    const daemonSettings = uiOptions.daemonSettings
        ?? state?.daemonSettings
        ?? DEFAULT_DAEMON_SETTINGS;
    const viewProviders = providersForSettings(snapshot, daemonSettings);
    const noProvidersEnabled = !hasRefreshEnabledProviders(daemonSettings);
    const selectedProvider = selectProvider({
        ...snapshot,
        providers: viewProviders,
    }, uiOptions.selectedProvider);
    const providerRows = viewProviders
        .map(provider => providerRow(provider, uiOptions));
    const selectedRow = selectedProvider ? providerRow(selectedProvider, uiOptions) : null;
    const viewState = noProvidersEnabled
        ? 'no_providers'
        : (state?.clientState ?? deriveSnapshotState(snapshot));
    const viewMeta = stateMeta(viewState);
    const providerSelectorRows = providerRows
        .map(row => providerSelectorRow(row, selectedRow?.providerId ?? ''));
    const refreshLabel = refreshButtonLabel(viewState, Boolean(state?.refreshing));

    return {
        state: viewState,
        stateLabel: viewMeta.label,
        stateDescription: viewMeta.description,
        refreshing: Boolean(state?.refreshing),
        activeRefreshId: state?.activeRefreshId ?? null,
        lastClientError: state?.lastClientError ?? null,
        daemonInfo: state?.daemonInfo ?? null,
        diagnostics: state?.diagnostics ?? null,
        snapshot,
        daemon: snapshot.daemon ?? null,
        selectedProvider,
        selectedRow,
        providerRows,
        providerSelectorRows,
        selectedProviderId: selectedRow?.providerId ?? '',
        panel: panelViewModel(providerRows, selectedRow, viewState, Boolean(snapshot.stale), uiOptions),
        panelLabel: selectedRow?.shortLabel ?? PRODUCT_PANEL_PLACEHOLDER,
        panelStatus: viewState === 'ok' && selectedRow ? selectedRow.statusLabel : viewMeta.label,
        headerStatus: headerStatusText(viewState, Boolean(snapshot.stale), Boolean(state?.refreshing), snapshot.generatedAt),
        refreshLabel,
        titleAction: {
            label: refreshLabel,
            action: viewState === 'daemon_unavailable' ? 'retryDaemon' : 'refresh',
            reactive: !state?.refreshing,
        },
        footerStatus: footerStatusText(state?.daemonInfo ?? snapshot.daemon, state?.daemonInfo?.capabilities ?? {}),
        stale: Boolean(snapshot.stale),
        generatedAt: snapshot.generatedAt ?? null,
    };
}

function providersForSettings(snapshot, daemonSettings = DEFAULT_DAEMON_SETTINGS) {
    const snapshotProviders = Array.isArray(snapshot?.providers) ? snapshot.providers : [];
    const effectiveProviders = effectiveProviderSettings(daemonSettings);
    const rows = [];
    const seen = new Set();

    for (const providerInfo of settingsProviderInfos(daemonSettings)) {
        const configured = settingsForProvider(daemonSettings, effectiveProviders, providerInfo.id);
        const snapshotProvider = snapshotProviders.find(provider => provider?.provider === providerInfo.id);
        if (snapshotProvider) {
            rows.push(!providerRefreshEnabled(configured)
                ? disabledSnapshotProvider(snapshotProvider)
                : snapshotProvider);
        } else {
            rows.push(!providerRefreshEnabled(configured)
                ? disabledProvider(providerInfo)
                : pendingProvider(providerInfo));
        }
        seen.add(providerInfo.id);
    }

    for (const provider of snapshotProviders) {
        if (!seen.has(provider?.provider))
            rows.push(provider);
    }

    return rows;
}

function hasRefreshEnabledProviders(daemonSettings) {
    const effectiveProviders = effectiveProviderSettings(daemonSettings);
    return settingsProviderInfos(daemonSettings)
        .some(providerInfo => providerRefreshEnabled(
            settingsForProvider(daemonSettings, effectiveProviders, providerInfo.id)
        ));
}

function providerRefreshEnabled(settings) {
    return settings?.enabled !== false
        && settings?.allowCliFallback !== false
        && settings?.preferredSourceAdapter !== 'off';
}

function settingsProviderInfos(daemonSettings) {
    const providerInfos = [];
    const seen = new Set();
    for (const provider of SUPPORTED_PROVIDERS) {
        providerInfos.push(provider);
        seen.add(provider.id);
    }

    if (plainObject(daemonSettings?.providers)) {
        for (const providerId of Object.keys(daemonSettings.providers)) {
            if (!seen.has(providerId) && /^[A-Za-z0-9_-]+$/.test(providerId)) {
                providerInfos.push({
                    id: providerId,
                    title: titleFromProviderId(providerId),
                });
                seen.add(providerId);
            }
        }
    }

    return providerInfos;
}

function settingsForProvider(daemonSettings, effectiveProviders, providerId) {
    if (Object.prototype.hasOwnProperty.call(effectiveProviders, providerId))
        return effectiveProviders[providerId];

    const configured = plainObject(daemonSettings?.providers?.[providerId])
        ? daemonSettings.providers[providerId]
        : {};
    return {
        enabled: typeof configured.enabled === 'boolean' ? configured.enabled : true,
        preferredSourceAdapter: typeof configured.preferredSourceAdapter === 'string'
            ? configured.preferredSourceAdapter
            : 'auto',
        allowBrowserImport: false,
        allowCliFallback: typeof configured.allowCliFallback === 'boolean'
            ? configured.allowCliFallback
            : true,
    };
}

function titleFromProviderId(providerId) {
    return providerId
        .replace(/[_-]+/g, ' ')
        .replace(/\b\w/g, letter => letter.toUpperCase());
}

function disabledSnapshotProvider(provider) {
    return {
        ...provider,
        disabled: true,
        status: {
            ...(provider?.status ?? {}),
            indicator: 'disabled',
            description: disabledProviderMeta().description,
        },
        diagnosticsSummary: disabledProviderMeta().description,
    };
}

function disabledProvider(providerInfo) {
    return {
        provider: providerInfo.id,
        displayName: providerInfo.title,
        version: null,
        source: 'unknown',
        sourceAdapter: 'none',
        state: 'provider_unavailable',
        updatedAt: null,
        staleSince: null,
        usage: {
            primary: null,
            secondary: null,
            tertiary: null,
        },
        credits: null,
        identity: null,
        status: {
            indicator: 'disabled',
            description: disabledProviderMeta().description,
            updatedAt: null,
            url: null,
        },
        cost: null,
        dashboardUrl: null,
        diagnosticsSummary: disabledProviderMeta().description,
        diagnosticCodes: ['provider_disabled_by_settings'],
        disabled: true,
    };
}

function pendingProvider(providerInfo) {
    return {
        provider: providerInfo.id,
        displayName: providerInfo.title,
        version: null,
        source: 'unknown',
        sourceAdapter: 'upstream_cli',
        state: 'loading',
        updatedAt: null,
        staleSince: null,
        usage: {
            primary: null,
            secondary: null,
            tertiary: null,
        },
        credits: null,
        identity: null,
        status: {
            indicator: 'loading',
            description: stateMeta('loading').description,
            updatedAt: null,
            url: null,
        },
        cost: null,
        dashboardUrl: null,
        diagnosticsSummary: stateMeta('loading').description,
        diagnosticCodes: [],
    };
}

export function providerRow(provider, options = {}) {
    const disabled = Boolean(provider?.disabled);
    const state = provider?.state ?? 'error';
    const meta = disabled ? disabledProviderMeta() : stateMeta(state);
    const displayName = safeDisplay(provider?.displayName || provider?.provider || 'Unknown provider');
    const providerId = safeDisplay(provider?.provider || '');
    const shortLabel = shortProviderLabel(displayName, providerId);
    const identity = disabled ? '' : safeDisplay(providerIdentityText(provider));
    const statusText = safeDisplay(providerStatusText(provider), meta.description);
    const meters = disabled ? [] : providerMeters(provider);
    const meterRows = meters.map(meter => meterRow(meter, options.resetTimeFormat));
    const costRows = disabled ? [] : costSummaryRows(provider?.cost);
    const source = disabled ? '' : sourceLabel(provider?.source);
    const adapter = disabled ? '' : adapterLabel(provider?.sourceAdapter);
    const titleStatusText = providerTitleStatusText(provider, options);
    const planLabel = providerPlanLabel(provider, source, adapter);

    return {
        provider,
        providerId,
        displayName,
        shortLabel,
        state,
        severity: meta.severity,
        statusLabel: meta.label,
        statusDescription: statusText,
        identity,
        sourceLabel: source,
        adapterLabel: adapter,
        planLabel,
        metadataText: [identity, planLabel].filter(Boolean).join(' · '),
        adapterText: '',
        titleStatusText,
        updatedText: formatUpdatedAt(provider?.updatedAt),
        resetText: meters
            .map(meter => formatMeterDetail(meter, options.resetTimeFormat))
            .join(' / ') || 'No usage data',
        meters,
        meterRows,
        usageSections: usageSectionRows(meterRows),
        costRows,
        diagnosticsSummary: safeDisplay(provider?.diagnosticsSummary || ''),
        disabled,
    };
}

export function providerSelectorRow(row, selectedProviderId = '') {
    const primaryMeter = row.meterRows[0] ?? null;
    return {
        providerId: row.providerId,
        label: row.displayName,
        displayName: row.displayName,
        state: row.state,
        severity: row.severity,
        selected: row.providerId === selectedProviderId,
        dimmed: row.disabled || !['ok', 'stale'].includes(row.state),
        disabled: row.disabled,
        statusLabel: row.statusLabel,
        meter: primaryMeter,
    };
}

export function usageSectionRows(meterRows) {
    const rows = Array.isArray(meterRows) ? meterRows : [];
    return rows
        .filter(row => ['primary', 'secondary', 'tertiary', 'credits'].includes(row.key))
        .map(row => ({
            key: row.key,
            title: usageSectionTitle(row),
            meter: row,
        }));
}

export function panelViewModel(providerRows, selectedRow, viewState, stale, options = {}) {
    const mode = PANEL_MODES.includes(options.panelMode) ? options.panelMode : 'merged';
    const visibleLimit = MAX_PROVIDER_INDICATORS;
    const activeProviderRows = providerRows.filter(row => !row.disabled);
    const visibleProviders = activeProviderRows.slice(0, visibleLimit)
        .map(row => ({
            providerId: row.providerId,
            label: row.shortLabel,
            state: row.state,
            severity: row.severity,
            meters: panelMeters(row.provider),
            compact: true,
            showText: false,
            meterCount: 2,
        }));

    return {
        mode,
        label: selectedRow?.shortLabel ?? PRODUCT_PANEL_PLACEHOLDER,
        status: stateMeta(viewState).label,
        iconName: stateMeta(viewState).iconName,
        stale,
        meters: selectedRow?.disabled ? [null, null] : panelMeters(selectedRow?.provider),
        visibleProviders,
        overflowCount: Math.max(0, activeProviderRows.length - visibleProviders.length),
        compact: true,
        showText: false,
        meterCount: 2,
    };
}

export function panelButtonClassNames(view, options = {}, {open = false} = {}) {
    const uiOptions = normalizeUiOptions(options);
    const state = normalizeStateName(view?.state);
    const classes = [
        'panel-button',
        'codexbar-panel',
        `codexbar-theme-${uiOptions.theme}`,
        `codexbar-state-${state}`,
    ];

    if (view?.stale)
        classes.push('codexbar-stale');
    if (open)
        classes.push('codexbar-panel-open');

    return classes.join(' ');
}

export function panelContentClassNames(panel) {
    const mode = PANEL_MODES.includes(panel?.mode) ? panel.mode : 'merged';
    const modeClass = {
        merged: 'codexbar-panel-content-merged',
        provider: 'codexbar-panel-content-provider',
        minimal: 'codexbar-panel-content-minimal',
    }[mode];

    return ['codexbar-panel-content', modeClass].join(' ');
}

export function panelProviderItemClassNames(row) {
    const state = normalizeStateName(row?.state);
    const severity = ['ok', 'warning', 'loading', 'error'].includes(row?.severity)
        ? row.severity
        : stateMeta(state).severity;

    return [
        'codexbar-panel-provider-item',
        `codexbar-state-${state}`,
        `codexbar-severity-${severity}`,
    ].join(' ');
}

export function panelAccessibleName(view) {
    const row = view?.selectedRow ?? null;
    const providerName = row?.displayName || view?.panel?.label || view?.panelLabel || PRODUCT_PANEL_PLACEHOLDER;
    const pieces = [
        `${PRODUCT_NAME}: ${providerName}`,
        row?.statusLabel || view?.stateLabel || view?.panelStatus,
        view?.headerStatus || row?.titleStatusText,
        ...panelAccessibleMeterTexts(row),
    ].filter(Boolean);

    return safeDisplay(uniqueStrings(pieces).join(' · '), PRODUCT_NAME);
}

export function panelMeters(provider) {
    if (!provider)
        return [null, null];

    const primary = provider.usage?.primary ?? null;
    const secondary = provider.usage?.secondary
        ?? meterFromCredits(provider.credits)
        ?? provider.usage?.tertiary
        ?? null;

    return [primary, secondary];
}

export function providerMeters(provider) {
    if (!provider)
        return [];

    const meters = [];
    for (const key of ['primary', 'secondary', 'tertiary']) {
        const meter = provider.usage?.[key];
        if (meter)
            meters.push({...meter, meterKey: key});
    }

    const creditsMeter = meterFromCredits(provider.credits);
    if (creditsMeter)
        meters.push(creditsMeter);

    return meters;
}

export function meterFromCredits(credits) {
    if (!credits)
        return null;

    return {
        usedPercent: typeof credits.remainingPercent === 'number' ? 100 - credits.remainingPercent : null,
        remainingPercent: credits.remainingPercent ?? null,
        windowMinutes: null,
        resetsAt: null,
        label: 'Credits',
        detail: formatCreditsDetail(credits),
        meterKey: 'credits',
    };
}

export function meterUsedPercent(meter) {
    if (!meter)
        return null;
    if (typeof meter.usedPercent === 'number')
        return clampPercent(meter.usedPercent);
    if (typeof meter.remainingPercent === 'number')
        return clampPercent(100 - meter.remainingPercent);
    return null;
}

export function meterRemainingPercent(meter) {
    if (!meter)
        return null;
    if (typeof meter.remainingPercent === 'number')
        return clampPercent(meter.remainingPercent);
    if (typeof meter.usedPercent === 'number')
        return clampPercent(100 - meter.usedPercent);
    return null;
}

export function meterFillFraction(meter) {
    return meterFillFractionFromPercent(meterRemainingPercent(meter));
}

export function meterFillFractionFromPercent(percent) {
    const clamped = clampPercent(percent);
    return clamped === null ? null : clamped / 100;
}

export function meterClassNames(tone, {compact = false} = {}) {
    return [
        'codexbar-meter',
        compact ? 'codexbar-meter-compact' : '',
        `codexbar-meter-${safeMeterTone(tone)}`,
    ].filter(Boolean).join(' ');
}

export function meterFillClassNames(tone) {
    return `codexbar-meter-fill codexbar-meter-fill-${safeMeterTone(tone)}`;
}

export function safeMeterTone(tone) {
    return ['ok', 'warning', 'danger', 'unknown'].includes(tone) ? tone : 'unknown';
}

export function meterTone(meter) {
    const remaining = meterRemainingPercent(meter);
    if (remaining === null)
        return 'unknown';
    if (remaining <= 15)
        return 'danger';
    if (remaining <= 35)
        return 'warning';
    return 'ok';
}

export function meterRow(meter, resetTimeFormat = 'countdown', nowMs = Date.now()) {
    const used = meterUsedPercent(meter);
    const remaining = meterRemainingPercent(meter);
    const label = safeDisplay(meter?.label || meter?.meterKey || 'Usage');
    return {
        key: meter?.meterKey ?? 'usage',
        label: label || 'Usage',
        detail: formatMeterDetail(meter, resetTimeFormat, nowMs),
        usedPercent: used,
        remainingPercent: remaining,
        fillPercent: remaining,
        fillFraction: meterFillFraction(meter),
        tone: meterTone(meter),
        resetText: formatResetTime(meter?.resetsAt, resetTimeFormat, nowMs),
    };
}

export function formatMeterDetail(meter, resetTimeFormat = 'countdown', nowMs = Date.now()) {
    if (!meter)
        return 'No usage data';

    const pieces = [];
    const detail = meter.detail ? safeDisplay(meter.detail) : '';
    if (detail)
        pieces.push(detail);
    else if (typeof meterRemainingPercent(meter) === 'number')
        pieces.push(`${Math.round(meterRemainingPercent(meter))}% remaining`);
    else if (typeof meterUsedPercent(meter) === 'number')
        pieces.push(`${Math.round(meterUsedPercent(meter))}% used`);

    return safeDisplay(pieces.join(' · ') || 'Usage available');
}

export function costSummaryRows(cost) {
    if (!cost || typeof cost !== 'object')
        return [];

    const rows = [];
    const items = Array.isArray(cost.items) ? cost.items : [];
    for (const item of items.slice(0, 2)) {
        const value = [
            typeof item.amount === 'number' ? formatMoney(item.amount, item.currency ?? cost.currency) : '',
            safeDisplay(item.detail || ''),
        ].filter(Boolean).join(' · ');
        rows.push({
            label: safeDisplay(item.label || 'Cost'),
            value: value || 'Cost unavailable',
        });
    }
    if (rows.length === 0 && typeof cost.total === 'number') {
        rows.push({
            label: 'Cost',
            value: formatMoney(cost.total, cost.currency),
        });
    }

    return rows;
}

export function formatResetTime(isoString, format = 'countdown', nowMs = Date.now()) {
    if (!isoString)
        return '';

    const date = new Date(isoString);
    if (Number.isNaN(date.getTime()))
        return '';

    const countdown = formatCountdown(date.getTime() - nowMs);
    const absolute = formatAbsolute(date);

    if (format === 'absolute')
        return absolute;
    if (format === 'both' && countdown && absolute)
        return `${countdown} (${absolute})`;
    return countdown || absolute;
}

export function formatUpdatedAt(isoString, nowMs = Date.now()) {
    if (!isoString)
        return 'Not updated yet';

    const date = new Date(isoString);
    if (Number.isNaN(date.getTime()))
        return 'Updated time unavailable';

    const diff = nowMs - date.getTime();
    if (diff < 60_000)
        return 'Updated just now';
    if (diff < 3_600_000)
        return `Updated ${Math.max(1, Math.round(diff / 60_000))}m ago`;
    if (diff < 86_400_000)
        return `Updated ${Math.round(diff / 3_600_000)}h ago`;
    return `Updated ${Math.round(diff / 86_400_000)}d ago`;
}

export function stateMeta(state) {
    return STATE_META[state] ?? STATE_META.error;
}

function disabledProviderMeta() {
    return {
        label: 'Disabled',
        severity: 'loading',
        description: 'Provider disabled in settings.',
        iconName: 'action-unavailable-symbolic',
    };
}

function normalizeStateName(state) {
    return Object.prototype.hasOwnProperty.call(STATE_META, state) ? state : 'error';
}

function panelAccessibleMeterTexts(row) {
    const meterRows = Array.isArray(row?.meterRows) ? row.meterRows : [];
    return meterRows.slice(0, 2)
        .map(meter => {
            const pieces = [
                safeDisplay(meter?.label || 'Usage'),
                safeDisplay(meter?.detail || ''),
                meter?.resetText ? `resets ${safeDisplay(meter.resetText)}` : '',
            ].filter(Boolean);
            return pieces.join(' ');
        })
        .filter(Boolean);
}

function uniqueStrings(values) {
    const seen = new Set();
    return values.filter(value => {
        if (seen.has(value))
            return false;
        seen.add(value);
        return true;
    });
}

export function sourceLabel(source) {
    return SOURCE_LABELS[source] ?? SOURCE_LABELS.unknown;
}

export function adapterLabel(adapter) {
    return ADAPTER_LABELS[adapter] ?? ADAPTER_LABELS.none;
}

export function providerStatusText(provider) {
    if (!provider)
        return 'No provider selected';
    if (provider.disabled)
        return disabledProviderMeta().description;

    const meta = stateMeta(provider.state);
    if (provider.state === 'ok')
        return '';

    return providerSetupStatusText(provider) || meta.description;
}

export function headerStatusText(state, stale, refreshing, generatedAt) {
    if (refreshing)
        return 'Refreshing…';
    if (state === 'daemon_unavailable')
        return STATE_LABELS.daemon_unavailable;

    const updated = generatedAt ? lowerInitial(formatUpdatedAt(generatedAt)) : '';
    if (stale)
        return ['Stale data', updated].filter(Boolean).join(' · ');
    if (state === 'ok')
        return updated ? capitalizeInitial(updated) : stateMeta(state).label;
    return [stateMeta(state).label, updated].filter(Boolean).join(' · ');
}

export function providerTitleStatusText(provider, options = {}) {
    if (provider?.disabled)
        return disabledProviderMeta().label;

    const state = provider?.state ?? 'loading';
    const updated = provider?.updatedAt
        ? lowerInitial(formatUpdatedAt(provider.updatedAt, options.nowMs ?? Date.now()))
        : '';
    if (state === 'ok')
        return updated ? capitalizeInitial(updated) : 'Updated just now';
    if (state === 'stale')
        return ['Stale data', updated].filter(Boolean).join(' · ');
    return [stateMeta(state).label, updated].filter(Boolean).join(' · ') || stateMeta(state).label;
}

export function refreshButtonLabel(state, refreshing) {
    if (refreshing)
        return 'Refreshing';
    if (state === 'daemon_unavailable')
        return 'Retry';
    return 'Refresh';
}

export function footerStatusText(daemon, capabilities = null) {
    const daemonState = daemon?.state ?? 'unknown';
    const upstream = daemon?.upstreamCli ?? null;
    return [
        `Daemon ${calmDaemonState(daemonState)}`,
        upstreamCliStatusText(upstream),
        capabilityStatusText('Cost', capabilities, 'cost'),
        browserImportStatusText(capabilities),
        'No web adapters',
    ].join(' · ');
}

export function providerIdentityText(provider) {
    const identity = provider?.identity;
    if (!identity)
        return '';

    const parts = [];
    if (identity.accountEmailDisplay)
        parts.push(safeDisplay(identity.accountEmailDisplay));
    if (identity.accountOrganizationDisplay)
        parts.push(safeDisplay(identity.accountOrganizationDisplay));
    if (identity.loginMethod)
        parts.push(safeDisplay(identity.loginMethod.replace(/_/g, ' ')));

    return parts.join(' · ');
}

export function diagnosticsDisplayLines(payload) {
    const diagnostics = typeof payload === 'string' ? parseJsonObject(payload).value : payload;
    if (!diagnostics || typeof diagnostics !== 'object')
        return ['Diagnostics unavailable'];

    const lines = [];
    const scope = diagnostics.provider ? `${diagnostics.scope}:${diagnostics.provider}` : diagnostics.scope;
    if (scope)
        lines.push(scope);
    if (diagnostics.generatedAt)
        lines.push(`Generated ${diagnostics.generatedAt}`);

    const events = Array.isArray(diagnostics.events) ? diagnostics.events : [];
    if (events.length === 0)
        lines.push('No diagnostic events');
    for (const event of events.slice(0, 6)) {
        const code = event.code ?? 'unknown';
        const severity = event.severity ?? 'info';
        const message = event.safeMessage ?? '';
        lines.push(`${severity}: ${code}${message ? ` - ${message}` : ''}`);
    }
    if (events.length > 6)
        lines.push(`${events.length - 6} more events`);

    return lines.map(redactText);
}

export function diagnosticsSummaryLine(payload) {
    const diagnostics = typeof payload === 'string' ? parseJsonObject(payload).value : payload;
    if (!diagnostics || typeof diagnostics !== 'object')
        return 'Diagnostics unavailable';

    const scope = safeDisplay(diagnostics.provider || diagnostics.scope || 'global') || 'global';
    const events = Array.isArray(diagnostics.events) ? diagnostics.events : [];
    if (events.length === 0)
        return 'No diagnostics';

    const highlightedEvent = strongestDiagnosticEvent(events);
    const message = safeDisplay(highlightedEvent?.safeMessage ?? '', 'Details unavailable');
    return redactText(message ? `Last issue: ${message}` : `${scope} diagnostics available`);
}

export function diagnosticsCopyText(payload) {
    const parsed = typeof payload === 'string' ? parseJsonObject(payload).value : payload;
    if (parsed && validateDiagnosticsPayload(parsed).ok)
        return redactText(JSON.stringify(diagnosticsCopyProjection(parsed), null, 2));

    return JSON.stringify({
        diagnostics: 'unavailable',
        redaction: {applied: true, policyVersion: 1},
    }, null, 2);
}

export function redactText(value) {
    let text = String(value ?? '');
    for (const [pattern, replacement] of SECRET_PATTERNS)
        text = text.replace(pattern, replacement);
    return text;
}

export function safeDisplay(value, fallback = '') {
    const text = redactText(value);
    if (!text || text === 'null' || text === 'undefined')
        return '';
    if (looksUnsafePublicString(text))
        return fallback;
    return text;
}

export function safeUrl(value) {
    if (!value || typeof value !== 'string')
        return '';
    const text = value.trim();
    if (!/^https?:\/\/[^\s]+$/i.test(text))
        return '';
    if (redactText(text) !== text)
        return '';
    if (hasUnsafeUrlPath(text))
        return '';
    if (hasSecretUrlComponent(text))
        return '';
    const host = urlHost(text);
    if (!host || isUnsafeDashboardHost(host))
        return '';
    return text;
}

export function parseJsonObject(jsonText) {
    try {
        const value = JSON.parse(jsonText);
        if (!value || typeof value !== 'object' || Array.isArray(value))
            return {ok: false, error: 'Expected a JSON object'};
        return {ok: true, value};
    } catch (error) {
        return {ok: false, error: errorToMessage(error)};
    }
}

function isFullProviderEvent(event) {
    if (event.schemaVersion !== 1)
        return false;
    for (const key of ['eventId', 'emittedAt', 'reason', 'providerId']) {
        if (typeof event[key] !== 'string' || event[key].length === 0)
            return false;
    }
    return isFullProvider(event.provider);
}

function isFullProvider(provider) {
    if (!provider || typeof provider !== 'object' || Array.isArray(provider))
        return false;
    if (!hasExactKeys(provider, PROVIDER_REQUIRED_KEYS, PROVIDER_OPTIONAL_KEYS))
        return false;

    for (const key of PROVIDER_REQUIRED_KEYS) {
        if (!Object.prototype.hasOwnProperty.call(provider, key))
            return false;
    }

    for (const key of ['provider', 'displayName', 'state', 'source', 'sourceAdapter']) {
        if (typeof provider[key] !== 'string' || provider[key].length === 0)
            return false;
    }

    if (!PROVIDER_STATES.includes(provider.state))
        return false;
    if (!SEMANTIC_SOURCES.includes(provider.source))
        return false;
    if (!SOURCE_ADAPTERS.includes(provider.sourceAdapter))
        return false;
    if (!provider.usage || typeof provider.usage !== 'object' || Array.isArray(provider.usage))
        return false;
    if (!validateUsage(provider.usage))
        return false;
    if (!isNullableDateTimeString(provider.updatedAt))
        return false;
    if (Object.prototype.hasOwnProperty.call(provider, 'version') && !isNullableString(provider.version))
        return false;
    if (Object.prototype.hasOwnProperty.call(provider, 'staleSince') && !isNullableDateTimeString(provider.staleSince))
        return false;
    if (Object.prototype.hasOwnProperty.call(provider, 'credits') && !validateCredits(provider.credits))
        return false;
    if (Object.prototype.hasOwnProperty.call(provider, 'identity') && !validateIdentity(provider.identity))
        return false;
    if (Object.prototype.hasOwnProperty.call(provider, 'status') && !validateStatus(provider.status))
        return false;
    if (Object.prototype.hasOwnProperty.call(provider, 'cost') && !validateCost(provider.cost))
        return false;
    for (const key of ['dashboardUrl', 'diagnosticsSummary']) {
        if (Object.prototype.hasOwnProperty.call(provider, key) && !isNullableString(provider[key]))
            return false;
    }
    if (Object.prototype.hasOwnProperty.call(provider, 'diagnosticCodes') && !isStringArray(provider.diagnosticCodes))
        return false;

    return true;
}

function validateSnapshot(snapshot) {
    if (!snapshot || typeof snapshot !== 'object' || Array.isArray(snapshot))
        return {ok: false, error: 'Snapshot payload was not an object'};
    if (hasForbiddenPublicData(snapshot))
        return {ok: false, error: 'Snapshot payload contained non-public fields'};
    if (!hasExactKeys(snapshot, SNAPSHOT_REQUIRED_KEYS, SNAPSHOT_OPTIONAL_KEYS))
        return {ok: false, error: 'Snapshot payload did not match the v1 shape'};
    for (const key of SNAPSHOT_REQUIRED_KEYS) {
        if (!Object.prototype.hasOwnProperty.call(snapshot, key))
            return {ok: false, error: `Snapshot payload missing ${key}`};
    }
    if (snapshot.schemaVersion !== 1)
        return {ok: false, error: 'Snapshot payload had an unsupported schema version'};
    if (!isDateTimeString(snapshot.generatedAt))
        return {ok: false, error: 'Snapshot payload had no generatedAt timestamp'};
    if (typeof snapshot.stale !== 'boolean')
        return {ok: false, error: 'Snapshot payload had no stale flag'};
    if (!validateSnapshotDaemon(snapshot.daemon))
        return {ok: false, error: 'Snapshot daemon payload did not match the v1 shape'};
    if (!Array.isArray(snapshot.providers))
        return {ok: false, error: 'Snapshot payload had no providers array'};
    if (!snapshot.providers.every(isFullProvider))
        return {ok: false, error: 'Snapshot provider payload did not match the v1 shape'};
    if (Object.prototype.hasOwnProperty.call(snapshot, 'selectedProvider') && !isNullableString(snapshot.selectedProvider))
        return {ok: false, error: 'Snapshot selected provider did not match the v1 shape'};
    return {ok: true};
}

function validateDiagnosticsPayload(diagnostics) {
    if (!diagnostics || typeof diagnostics !== 'object' || Array.isArray(diagnostics))
        return {ok: false};
    if (hasForbiddenPublicData(diagnostics))
        return {ok: false};
    if (!hasExactKeys(diagnostics, DIAGNOSTICS_REQUIRED_KEYS, DIAGNOSTICS_OPTIONAL_KEYS))
        return {ok: false};
    if (diagnostics.schemaVersion !== 1
        || !isDateTimeString(diagnostics.generatedAt)
        || !DIAGNOSTIC_SCOPES.includes(diagnostics.scope)
        || !Array.isArray(diagnostics.events))
        return {ok: false};
    if (Object.prototype.hasOwnProperty.call(diagnostics, 'provider') && !isNullableString(diagnostics.provider))
        return {ok: false};
    if (!diagnostics.redaction
        || typeof diagnostics.redaction !== 'object'
        || Array.isArray(diagnostics.redaction)
        || !hasExactKeys(diagnostics.redaction, DIAGNOSTIC_REDACTION_REQUIRED_KEYS, DIAGNOSTIC_REDACTION_OPTIONAL_KEYS)
        || diagnostics.redaction.applied !== true
        || diagnostics.redaction.policyVersion !== 1)
        return {ok: false};
    if (Object.prototype.hasOwnProperty.call(diagnostics.redaction, 'notes') && !isStringArray(diagnostics.redaction.notes))
        return {ok: false};

    for (const event of diagnostics.events) {
        if (!event || typeof event !== 'object' || Array.isArray(event))
            return {ok: false};
        if (!hasExactKeys(event, DIAGNOSTIC_EVENT_REQUIRED_KEYS, DIAGNOSTIC_EVENT_OPTIONAL_KEYS))
            return {ok: false};
        for (const key of ['code', 'severity', 'safeMessage', 'timestamp']) {
            if (typeof event[key] !== 'string' || event[key].length === 0)
                return {ok: false};
        }
        if (!isDateTimeString(event.timestamp))
            return {ok: false};
        if (!Object.prototype.hasOwnProperty.call(DIAGNOSTIC_SEVERITY_RANK, event.severity))
            return {ok: false};
        if (!event.redacted
            || typeof event.redacted !== 'object'
            || Array.isArray(event.redacted)
            || !hasExactKeys(event.redacted, DIAGNOSTIC_EVENT_REDACTED_REQUIRED_KEYS, DIAGNOSTIC_EVENT_REDACTED_OPTIONAL_KEYS)
            || event.redacted.applied !== true)
            return {ok: false};
        if (Object.prototype.hasOwnProperty.call(event.redacted, 'classes') && !isStringArray(event.redacted.classes))
            return {ok: false};
        if (Object.prototype.hasOwnProperty.call(event, 'provider') && !isNullableString(event.provider))
            return {ok: false};
        if (Object.prototype.hasOwnProperty.call(event, 'sourceAdapter')
            && event.sourceAdapter !== null
            && !SOURCE_ADAPTERS.includes(event.sourceAdapter))
            return {ok: false};
        if (Object.prototype.hasOwnProperty.call(event, 'recoverable') && typeof event.recoverable !== 'boolean')
            return {ok: false};
        if (Object.prototype.hasOwnProperty.call(event, 'details')) {
            if (!event.details || typeof event.details !== 'object' || Array.isArray(event.details))
                return {ok: false};
            if (!Object.entries(event.details).every(([key, value]) => !isForbiddenPublicKey(key) && isSafeDiagnosticDetailValue(value)))
                return {ok: false};
        }
    }

    return {ok: true};
}

function refreshResultProjection(result) {
    if (!result || typeof result !== 'object' || Array.isArray(result))
        return null;
    if (hasForbiddenPublicData(result))
        return null;
    if (!hasExactKeys(result, REFRESH_RESULT_REQUIRED_KEYS, REFRESH_RESULT_OPTIONAL_KEYS))
        return null;
    if (result.schemaVersion !== 1
        || typeof result.refreshId !== 'string'
        || !REFRESH_RESULT_STATUSES.includes(result.status)
        || typeof result.startedAt !== 'string'
        || !isDateTimeString(result.startedAt)
        || typeof result.finishedAt !== 'string'
        || !isDateTimeString(result.finishedAt)
        || !Number.isInteger(result.durationMs)
        || result.durationMs < 0
        || !REFRESH_REASONS.includes(result.reason)
        || typeof result.cacheWritten !== 'boolean'
        || !Array.isArray(result.providers))
        return null;
    if (Object.prototype.hasOwnProperty.call(result, 'snapshotGeneratedAt')
        && result.snapshotGeneratedAt !== null
        && (typeof result.snapshotGeneratedAt !== 'string' || !isDateTimeString(result.snapshotGeneratedAt)))
        return null;
    if (Object.prototype.hasOwnProperty.call(result, 'diagnosticCodes')
        && !isStringArray(result.diagnosticCodes))
        return null;

    const providers = [];
    for (const provider of result.providers) {
        if (!provider
            || typeof provider !== 'object'
            || Array.isArray(provider)
            || !hasExactKeys(provider, REFRESH_PROVIDER_REQUIRED_KEYS, REFRESH_PROVIDER_OPTIONAL_KEYS)
            || typeof provider.provider !== 'string'
            || !REFRESH_PROVIDER_STATUSES.includes(provider.status))
            return null;
        const adapter = Object.prototype.hasOwnProperty.call(provider, 'sourceAdapter')
            ? provider.sourceAdapter
            : null;
        if (adapter !== null && !SOURCE_ADAPTERS.includes(adapter))
            return null;
        if (Object.prototype.hasOwnProperty.call(provider, 'diagnosticCodes')
            && !isStringArray(provider.diagnosticCodes))
            return null;
        const diagnosticCodes = Array.isArray(provider.diagnosticCodes)
            ? provider.diagnosticCodes.map(code => safeDisplay(code)).filter(Boolean)
            : [];
        providers.push({
            provider: safeDisplay(provider.provider),
            status: provider.status,
            sourceAdapter: adapter,
            diagnosticCodes,
        });
    }

    return {
        schemaVersion: 1,
        refreshId: safeDisplay(result.refreshId),
        status: result.status,
        startedAt: result.startedAt,
        finishedAt: result.finishedAt,
        durationMs: result.durationMs,
        reason: result.reason,
        cacheWritten: result.cacheWritten,
        snapshotGeneratedAt: result.snapshotGeneratedAt ?? null,
        providers,
        diagnosticCodes: Array.isArray(result.diagnosticCodes)
            ? result.diagnosticCodes.map(code => safeDisplay(code)).filter(Boolean)
            : [],
    };
}

function hasExactKeys(value, required, optional = []) {
    const allowed = new Set([...required, ...optional]);
    for (const key of required) {
        if (!Object.prototype.hasOwnProperty.call(value, key))
            return false;
    }
    return Object.keys(value).every(key => allowed.has(key));
}

function validateSnapshotDaemon(daemon) {
    if (!daemon || typeof daemon !== 'object' || Array.isArray(daemon))
        return false;
    if (!hasExactKeys(daemon, SNAPSHOT_DAEMON_REQUIRED_KEYS, SNAPSHOT_DAEMON_OPTIONAL_KEYS))
        return false;
    if (typeof daemon.version !== 'string' || !DAEMON_STATES.includes(daemon.state))
        return false;
    for (const key of ['lastRefreshId']) {
        if (Object.prototype.hasOwnProperty.call(daemon, key) && !isNullableString(daemon[key]))
            return false;
    }
    for (const key of ['lastRefreshStartedAt', 'lastRefreshFinishedAt']) {
        if (Object.prototype.hasOwnProperty.call(daemon, key) && !isNullableDateTimeString(daemon[key]))
            return false;
    }
    if (Object.prototype.hasOwnProperty.call(daemon, 'upstreamCli') && !validateUpstreamCli(daemon.upstreamCli))
        return false;
    return true;
}

function validateUpstreamCli(upstreamCli) {
    if (upstreamCli === null)
        return true;
    if (!upstreamCli || typeof upstreamCli !== 'object' || Array.isArray(upstreamCli))
        return false;
    if (!hasExactKeys(upstreamCli, UPSTREAM_CLI_REQUIRED_KEYS, UPSTREAM_CLI_OPTIONAL_KEYS))
        return false;
    if (typeof upstreamCli.available !== 'boolean')
        return false;
    for (const key of UPSTREAM_CLI_OPTIONAL_KEYS) {
        if (Object.prototype.hasOwnProperty.call(upstreamCli, key) && !isNullableString(upstreamCli[key]))
            return false;
    }
    return true;
}

function validateUsage(usage) {
    if (!hasExactKeys(usage, USAGE_REQUIRED_KEYS))
        return false;
    return USAGE_REQUIRED_KEYS.every(key => validateMeter(usage[key]));
}

function validateMeter(meter) {
    if (meter === null)
        return true;
    if (!meter || typeof meter !== 'object' || Array.isArray(meter))
        return false;
    if (!hasExactKeys(meter, METER_REQUIRED_KEYS, METER_OPTIONAL_KEYS))
        return false;
    if (!isPercentOrNull(meter.usedPercent) || !isPercentOrNull(meter.remainingPercent))
        return false;
    if (!isNonNegativeIntegerOrNull(meter.windowMinutes))
        return false;
    if (!isNullableDateTimeString(meter.resetsAt) || !isNullableString(meter.label))
        return false;
    if (Object.prototype.hasOwnProperty.call(meter, 'detail') && !isNullableString(meter.detail))
        return false;
    return true;
}

function validateIdentity(identity) {
    if (identity === null)
        return true;
    if (!identity || typeof identity !== 'object' || Array.isArray(identity))
        return false;
    if (!hasExactKeys(identity, [], IDENTITY_OPTIONAL_KEYS))
        return false;
    return IDENTITY_OPTIONAL_KEYS.every(key => !Object.prototype.hasOwnProperty.call(identity, key) || isNullableString(identity[key]));
}

function validateStatus(status) {
    if (status === null)
        return true;
    if (!status || typeof status !== 'object' || Array.isArray(status))
        return false;
    if (!hasExactKeys(status, STATUS_REQUIRED_KEYS, STATUS_OPTIONAL_KEYS))
        return false;
    if (!isNullableString(status.indicator) || !isNullableString(status.description) || !isNullableDateTimeString(status.updatedAt))
        return false;
    if (Object.prototype.hasOwnProperty.call(status, 'url') && !isNullableString(status.url))
        return false;
    return true;
}

function validateCredits(credits) {
    if (credits === null)
        return true;
    if (!credits || typeof credits !== 'object' || Array.isArray(credits))
        return false;
    if (!hasExactKeys(credits, CREDITS_REQUIRED_KEYS, CREDITS_OPTIONAL_KEYS))
        return false;
    if (!isNullableNumber(credits.remaining) || !isPercentOrNull(credits.remainingPercent) || !isNullableDateTimeString(credits.updatedAt))
        return false;
    if (Object.prototype.hasOwnProperty.call(credits, 'unit') && !isNullableString(credits.unit))
        return false;
    return true;
}

function validateCost(cost) {
    if (cost === null)
        return true;
    if (!cost || typeof cost !== 'object' || Array.isArray(cost))
        return false;
    if (!hasExactKeys(cost, COST_REQUIRED_KEYS, COST_OPTIONAL_KEYS))
        return false;
    if (!isNullableDateTimeString(cost.updatedAt)
        || !isNullableString(cost.currency)
        || !isNullableNumber(cost.total)
        || !Array.isArray(cost.items))
        return false;
    for (const key of ['periodStartAt', 'periodEndAt']) {
        if (Object.prototype.hasOwnProperty.call(cost, key) && !isNullableDateTimeString(cost[key]))
            return false;
    }
    if (Object.prototype.hasOwnProperty.call(cost, 'diagnosticCodes') && !isStringArray(cost.diagnosticCodes))
        return false;
    return cost.items.every(validateCostItem);
}

function validateCostItem(item) {
    if (!item || typeof item !== 'object' || Array.isArray(item))
        return false;
    if (!hasExactKeys(item, COST_ITEM_REQUIRED_KEYS, COST_ITEM_OPTIONAL_KEYS))
        return false;
    if (typeof item.label !== 'string' || !isNullableNumber(item.amount) || !isNullableString(item.currency))
        return false;
    if (Object.prototype.hasOwnProperty.call(item, 'detail') && !isNullableString(item.detail))
        return false;
    return true;
}

function isStringArray(value) {
    return Array.isArray(value) && value.every(item => typeof item === 'string');
}

function isDateTimeString(value) {
    return typeof value === 'string' && !Number.isNaN(Date.parse(value));
}

function isNullableString(value) {
    return value === null || typeof value === 'string';
}

function isNullableNumber(value) {
    return value === null || (typeof value === 'number' && Number.isFinite(value));
}

function isPercentOrNull(value) {
    return value === null || (typeof value === 'number' && Number.isFinite(value) && value >= 0 && value <= 100);
}

function isNonNegativeIntegerOrNull(value) {
    return value === null || (Number.isInteger(value) && value >= 0);
}

function isNullableDateTimeString(value) {
    return value === null || isDateTimeString(value);
}

function isDaemonSettingsPayload(settings) {
    if (!plainObject(settings) || settings.schemaVersion !== 1 || hasForbiddenPublicData(settings))
        return false;
    if (!plainObject(settings.providers))
        return false;

    for (const [providerId, provider] of Object.entries(settings.providers)) {
        if (!/^[A-Za-z0-9_-]+$/.test(providerId) || !plainObject(provider))
            return false;
        if (Object.prototype.hasOwnProperty.call(provider, 'enabled') && typeof provider.enabled !== 'boolean')
            return false;
        if (
            Object.prototype.hasOwnProperty.call(provider, 'preferredSourceAdapter')
            && !['auto', 'upstream_cli', 'linux_web', 'off'].includes(provider.preferredSourceAdapter)
        )
            return false;
        if (
            Object.prototype.hasOwnProperty.call(provider, 'allowCliFallback')
            && typeof provider.allowCliFallback !== 'boolean'
        )
            return false;
        if (
            Object.prototype.hasOwnProperty.call(provider, 'allowBrowserImport')
            && typeof provider.allowBrowserImport !== 'boolean'
        )
            return false;
    }

    return true;
}

function plainObject(value) {
    return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function isSafeDiagnosticDetailValue(value) {
    return value === null || ['string', 'number', 'boolean'].includes(typeof value);
}

function hasForbiddenPublicData(value) {
    const queue = [value];
    while (queue.length > 0) {
        const current = queue.shift();
        if (typeof current === 'string' && looksUnsafePublicString(current))
            return true;
        if (!current || typeof current !== 'object')
            continue;
        if (Array.isArray(current)) {
            queue.push(...current);
            continue;
        }
        for (const [key, child] of Object.entries(current)) {
            if (isForbiddenPublicKey(key))
                return true;
            if (typeof child === 'string' && looksUnsafePublicString(child))
                return true;
            if (child && typeof child === 'object')
                queue.push(child);
        }
    }
    return false;
}

function isForbiddenPublicKey(key) {
    const fingerprint = String(key ?? '').replace(/[-_\s]/g, '').toLowerCase();
    return FORBIDDEN_PUBLIC_KEYS.has(fingerprint)
        || /^(api|access|refresh|session)?token$/i.test(fingerprint);
}

function looksUnsafePublicString(value) {
    const trimmed = String(value ?? '').trim();
    const emailMatches = value.match(/\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b/g) ?? [];
    const hasRawEmail = emailMatches.some(match => !/^[^@\s]*\*{2,}[^@\s]*@/.test(match));

    return /\bAuthorization\s*:/i.test(value)
        || /\b(?:X-API-Key|X-Auth-Token)\s*:/i.test(value)
        || /\bSet-Cookie\s*:/i.test(value)
        || /\bCookie\s*:/i.test(value)
        || /\bBearer\s+[A-Za-z0-9._~+/=-]+/i.test(value)
        || /\bsk-(?:ant-)?[A-Za-z0-9_-]{16,}\b/.test(value)
        || /\bgh[opsu]_[A-Za-z0-9_]{20,}\b/.test(value)
        || /\bxox[baprs]-[A-Za-z0-9-]{16,}\b/.test(value)
        || /\bAIza[0-9A-Za-z_-]{20,}\b/.test(value)
        || /\beyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\b/.test(value)
        || /\b(?:api[_-]?key|access[_-]?token|refresh[_-]?token|session[_-]?(?:token|key))\b\s*[:=]/i.test(value)
        || hasRawEmail
        || /(^|["'\s=:/?&#])\/(?:home|Users)\//.test(value)
        || /\b(rawPayload|rawResponse|rawOutput|stdout|stderr)\b/i.test(value)
        || /[{[]\s*["'][^"']+["']\s*:/.test(value)
        || ((trimmed.startsWith('{') && trimmed.endsWith('}')) || (trimmed.startsWith('[') && trimmed.endsWith(']')));
}

function urlHost(url) {
    const match = url.match(/^[a-z][a-z0-9+.-]*:\/\/([^/?#]+)/i);
    const authority = match?.[1] ?? '';
    if (!authority || authority.includes('@'))
        return '';

    if (authority.startsWith('[')) {
        const end = authority.indexOf(']');
        if (end < 0)
            return '';
        return authority.slice(1, end).toLowerCase();
    }

    return authority.split(':', 1)[0].toLowerCase();
}

function hasSecretUrlComponent(url) {
    const queryOrFragment = url.match(/[?#](.*)$/)?.[1] ?? '';
    if (!queryOrFragment)
        return false;
    return queryOrFragment
        .split(/[&#]/)
        .some(part => {
            const rawName = part.split('=', 1)[0] ?? '';
            let name = rawName.toLowerCase();
            try {
                name = decodeURIComponent(name.replace(/\+/g, ' ')).toLowerCase();
            } catch (_error) {
                return true;
            }
            const rawValue = part.includes('=') ? part.slice(part.indexOf('=') + 1) : '';
            let decodedValue = rawValue;
            try {
                decodedValue = decodeURIComponent(rawValue.replace(/\+/g, ' '));
            } catch (_error) {
                return true;
            }
            return /(^|[_-])(token|secret|code|key|api_key|apikey|access_token|refresh_token|session|auth|authorization)([_-]|$)/i.test(name)
                || looksUnsafePublicString(decodedValue);
        });
}

function hasUnsafeUrlPath(url) {
    const path = url.match(/^[a-z][a-z0-9+.-]*:\/\/[^/?#]+([^?#]*)/i)?.[1] ?? '';
    if (!path)
        return false;
    return path
        .split('/')
        .filter(Boolean)
        .some(segment => {
            let decoded = segment;
            try {
                decoded = decodeURIComponent(segment);
            } catch (_error) {
                return true;
            }
            return looksUnsafePublicString(decoded);
        });
}

function isUnsafeDashboardHost(host) {
    const canonical = host.toLowerCase().replace(/\.+$/, '');
    if (!canonical)
        return true;
    if (isLocalhostHost(canonical))
        return true;
    if (canonical.includes(':'))
        return true;

    const ipv4 = parseIpv4Literal(canonical);
    return ipv4 ? isUnsafeIpv4Address(ipv4) : false;
}

function isLocalhostHost(host) {
    return host === 'localhost'
        || host.endsWith('.localhost')
        || host === '0.0.0.0'
        || host === '::1'
        || host === '0:0:0:0:0:0:0:1'
        || /^127(?:\.\d{1,3}){0,3}$/.test(host);
}

function parseIpv4Literal(host) {
    const parts = host.split('.');
    if (parts.some(part => part.length === 0))
        return null;

    if (parts.length === 1) {
        const value = parseNumericHostPart(parts[0]);
        if (value === null || value > 0xffffffff)
            return null;
        return [
            (value >>> 24) & 0xff,
            (value >>> 16) & 0xff,
            (value >>> 8) & 0xff,
            value & 0xff,
        ];
    }

    if (parts.length !== 4)
        return null;

    const octets = parts.map(parseNumericHostPart);
    if (octets.some(value => value === null || value > 255))
        return null;
    return octets;
}

function parseNumericHostPart(value) {
    if (/^0x[0-9a-f]+$/i.test(value))
        return Number.parseInt(value.slice(2), 16);
    if (/^0[0-7]+$/.test(value))
        return Number.parseInt(value.slice(1), 8);
    if (/^[0-9]+$/.test(value))
        return Number.parseInt(value, 10);
    return null;
}

function isUnsafeIpv4Address(octets) {
    const [a, b, c] = octets;
    return a === 0
        || a === 10
        || a === 127
        || (a === 169 && b === 254)
        || (a === 172 && b >= 16 && b <= 31)
        || (a === 192 && b === 168)
        || (a === 192 && b === 0 && c === 0)
        || (a === 100 && b >= 64 && b <= 127)
        || (a === 198 && (b === 18 || b === 19))
        || a >= 224;
}

function diagnosticsCopyProjection(diagnostics) {
    return {
        schemaVersion: 1,
        generatedAt: diagnostics.generatedAt,
        scope: diagnostics.scope,
        provider: diagnostics.provider === null || diagnostics.provider === undefined
            ? null
            : safeDisplay(diagnostics.provider),
        events: diagnostics.events.map(event => ({
            code: safeDisplay(event.code, 'diagnostic_event'),
            severity: Object.prototype.hasOwnProperty.call(DIAGNOSTIC_SEVERITY_RANK, event.severity)
                ? event.severity
                : 'info',
            safeMessage: safeDisplay(event.safeMessage, 'Details unavailable'),
            timestamp: event.timestamp,
            provider: event.provider === null || event.provider === undefined
                ? null
                : safeDisplay(event.provider),
            sourceAdapter: event.sourceAdapter === null || event.sourceAdapter === undefined
                ? null
                : event.sourceAdapter,
            recoverable: typeof event.recoverable === 'boolean' ? event.recoverable : null,
            details: sanitizeDiagnosticsDetails(event.details),
            redacted: {applied: true, classes: safeStringList(event.redacted?.classes)},
        })),
        redaction: {applied: true, policyVersion: 1},
    };
}

function safeStringList(values) {
    if (!Array.isArray(values))
        return [];
    return values
        .map(value => (typeof value === 'string' ? safeDisplay(value) : ''))
        .filter(Boolean);
}

function sanitizeDiagnosticsDetails(details) {
    if (!details || typeof details !== 'object' || Array.isArray(details))
        return {};

    const safe = {};
    for (const [key, value] of Object.entries(details)) {
        if (isForbiddenPublicKey(key))
            continue;
        if (typeof value === 'string')
            safe[key] = safeDisplay(value, '[redacted]');
        else if (['number', 'boolean'].includes(typeof value) || value === null)
            safe[key] = value;
    }
    return safe;
}

function mostConstrainedUsableProvider(providers) {
    let selected = null;
    let selectedRemaining = Number.POSITIVE_INFINITY;

    for (const provider of providers) {
        if (!provider || provider.state !== 'ok')
            continue;

        const remainingValues = providerMeters(provider)
            .map(meterRemainingPercent)
            .filter(value => typeof value === 'number');
        const remaining = remainingValues.length > 0
            ? Math.min(...remainingValues)
            : Number.POSITIVE_INFINITY;
        if (remaining < selectedRemaining) {
            selected = provider;
            selectedRemaining = remaining;
        }
    }

    return selected ?? providers.find(provider => provider.state === 'ok') ?? null;
}

function strongestDiagnosticEvent(events) {
    let selected = null;
    let selectedRank = DIAGNOSTIC_SEVERITY_RANK.info;
    for (const event of events) {
        const severity = event?.severity;
        const rank = DIAGNOSTIC_SEVERITY_RANK[severity] ?? 0;
        if (!selected || rank > selectedRank) {
            selected = event;
            selectedRank = rank;
        }
    }
    return selected;
}

function shortProviderLabel(displayName, providerId) {
    const source = (providerId || displayName || 'cb')
        .replace(/[^A-Za-z0-9]+/g, '')
        .toUpperCase();
    return source.slice(0, 3) || 'CB';
}

function providerPlanLabel(provider, source, adapter) {
    if (provider?.disabled)
        return 'Off';
    if (provider?.source === 'local')
        return source;
    if (provider?.sourceAdapter === 'upstream_cli')
        return adapter;
    return '';
}

function providerSetupStatusText(provider) {
    if (isRateLimitedProvider(provider))
        return 'Rate limit active. Wait, then refresh.';

    const state = provider?.state ?? 'error';
    if (state === 'stale')
        return 'Showing cached data.';
    if (state === 'unauthenticated')
        return 'Sign in with the upstream CLI, then refresh.';
    if (state === 'cookie_rejected')
        return 'Use upstream CLI setup, then refresh.';
    if (state === 'missing_dependency') {
        const codes = Array.isArray(provider?.diagnosticCodes) ? provider.diagnosticCodes : [];
        if (codes.includes('daemon_unavailable'))
            return stateMeta('daemon_unavailable').description;
        return stateMeta('missing_dependency').description;
    }
    if (state === 'provider_unavailable')
        return stateMeta('provider_unavailable').description;
    if (state === 'parse_error')
        return stateMeta('parse_error').description;
    if (state === 'timeout')
        return stateMeta('timeout').description;
    if (state === 'error')
        return stateMeta('error').description;
    if (state === 'loading')
        return stateMeta('loading').description;
    return stateMeta(state).description;
}

function isRateLimitedProvider(provider) {
    const values = [
        provider?.status?.indicator,
        provider?.status?.description,
        provider?.diagnosticsSummary,
        ...(Array.isArray(provider?.diagnosticCodes) ? provider.diagnosticCodes : []),
    ]
        .filter(value => typeof value === 'string')
        .map(value => value.toLowerCase());

    return values.some(value => value.includes('rate') && value.includes('limit'));
}

function usageSectionTitle(row) {
    if (row?.key === 'primary')
        return safeDisplay(row.label || 'Session', 'Session');
    if (row?.key === 'secondary')
        return safeDisplay(row.label || 'Weekly', 'Weekly');
    if (row?.key === 'credits')
        return 'Credits';
    return safeDisplay(row?.label || 'Usage', 'Usage');
}

function formatCreditsDetail(credits) {
    const parts = [];
    if (typeof credits.remaining === 'number')
        parts.push(`${credits.remaining}${credits.unit ? ` ${credits.unit}` : ''} remaining`);
    if (typeof credits.remainingPercent === 'number')
        parts.push(`${Math.round(credits.remainingPercent)}% remaining`);
    return parts.join(' · ') || 'Credits available';
}

function formatMoney(amount, currency) {
    if (typeof amount !== 'number' || !Number.isFinite(amount))
        return 'Cost unavailable';
    if (!currency)
        return `${roundCurrency(amount)}`;
    if (currency.toUpperCase() === 'USD')
        return `$${roundCurrency(amount)}`;
    return `${roundCurrency(amount)} ${safeDisplay(currency.toUpperCase())}`;
}

function roundCurrency(amount) {
    return amount >= 100
        ? amount.toFixed(0)
        : amount.toFixed(2);
}

function calmDaemonState(state) {
    if (state === 'ok')
        return 'running';
    if (state === 'refreshing')
        return 'refreshing';
    if (state === 'starting')
        return 'starting';
    if (state === 'degraded')
        return 'degraded';
    if (state === 'error')
        return 'unavailable';
    return 'unknown';
}

function upstreamCliStatusText(upstream) {
    if (!upstream)
        return 'Upstream CLI unknown';
    return `Upstream CLI ${upstream.available ? 'available' : 'missing'}`;
}

function capabilityStatusText(label, capabilities, key) {
    if (!capabilities || !Object.prototype.hasOwnProperty.call(capabilities, key))
        return `${label} unknown`;
    return `${label} ${capabilities[key] ? 'available' : 'unavailable'}`;
}

function browserImportStatusText(capabilities) {
    if (!capabilities || !Object.prototype.hasOwnProperty.call(capabilities, 'browserImport'))
        return 'Browser import unsupported';
    return 'Browser import unsupported';
}

function lowerInitial(value) {
    if (!value)
        return '';
    return value[0].toLowerCase() + value.slice(1);
}

function capitalizeInitial(value) {
    if (!value)
        return '';
    return value[0].toUpperCase() + value.slice(1);
}

function formatCountdown(diffMs) {
    if (diffMs <= 0)
        return 'now';

    const minutes = Math.ceil(diffMs / 60_000);
    if (minutes < 60)
        return `${minutes}m`;

    const hours = Math.ceil(minutes / 60);
    if (hours < 48)
        return `${hours}h`;

    return `${Math.ceil(hours / 24)}d`;
}

function formatAbsolute(date) {
    return `${date.getFullYear()}-${pad2(date.getMonth() + 1)}-${pad2(date.getDate())} ${pad2(date.getHours())}:${pad2(date.getMinutes())}`;
}

function pad2(value) {
    return String(value).padStart(2, '0');
}

function clampPercent(value) {
    if (!Number.isFinite(value))
        return null;
    return Math.max(0, Math.min(100, value));
}

function errorToMessage(error) {
    if (typeof error === 'string')
        return error;
    if (error?.message)
        return error.message;
    return String(error ?? 'Unknown error');
}
