import {
    PANEL_MODES,
    PRODUCT_NAME,
    PROVIDER_STATES,
    RESET_TIME_FORMATS,
    STATE_LABELS,
    THEMES,
} from './constants.js';

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
        description: 'Showing cached usage data',
        iconName: 'appointment-soon-symbolic',
    },
    unauthenticated: {
        label: STATE_LABELS.unauthenticated,
        severity: 'warning',
        description: 'Provider needs a valid sign-in',
        iconName: 'dialog-password-symbolic',
    },
    cookie_rejected: {
        label: STATE_LABELS.cookie_rejected,
        severity: 'warning',
        description: 'Browser session was rejected',
        iconName: 'dialog-warning-symbolic',
    },
    missing_dependency: {
        label: STATE_LABELS.missing_dependency,
        severity: 'warning',
        description: 'Required local dependency is unavailable',
        iconName: 'dialog-warning-symbolic',
    },
    provider_unavailable: {
        label: STATE_LABELS.provider_unavailable,
        severity: 'warning',
        description: 'Provider or adapter is unavailable',
        iconName: 'network-offline-symbolic',
    },
    parse_error: {
        label: STATE_LABELS.parse_error,
        severity: 'error',
        description: 'Provider data was not readable',
        iconName: 'dialog-error-symbolic',
    },
    timeout: {
        label: STATE_LABELS.timeout,
        severity: 'warning',
        description: 'Provider refresh timed out',
        iconName: 'alarm-symbolic',
    },
    error: {
        label: STATE_LABELS.error,
        severity: 'error',
        description: 'Refresh did not complete',
        iconName: 'dialog-error-symbolic',
    },
    daemon_unavailable: {
        label: STATE_LABELS.daemon_unavailable,
        severity: 'error',
        description: 'D-Bus service is unavailable',
        iconName: 'network-offline-symbolic',
    },
};

const SOURCE_LABELS = {
    api: 'API',
    local: 'Local',
    web: 'Web',
    unknown: 'Unknown',
};

const ADAPTER_LABELS = {
    upstream_cli: 'Upstream CLI',
    linux_web: 'Linux web',
    cache: 'Cache fallback',
    fixture: 'Fixture',
    synthetic: 'Synthetic',
    none: 'None',
};

const SNAPSHOT_REQUIRED_KEYS = ['schemaVersion', 'generatedAt', 'stale', 'daemon', 'providers'];
const PROVIDER_REQUIRED_KEYS = ['provider', 'displayName', 'state', 'source', 'sourceAdapter', 'updatedAt', 'usage'];
const DAEMON_STATES = ['starting', 'ok', 'refreshing', 'degraded', 'error'];
const SEMANTIC_SOURCES = ['api', 'local', 'web', 'unknown'];
const SOURCE_ADAPTERS = ['upstream_cli', 'linux_web', 'cache', 'fixture', 'synthetic', 'none'];
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
    [/\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b/g, '[redacted-email]'],
    [/(^|["'\s])\/(?:home|Users)\/[^"'\s,}]+/g, '$1[redacted-path]'],
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
        diagnostics: null,
    };
}

export function createSyntheticSnapshot(state, options = {}) {
    const now = new Date(options.nowMs ?? Date.now()).toISOString();
    const providerId = options.providerId ?? 'codex';
    const displayName = options.displayName ?? 'Codex';
    const summary = options.summary ?? stateMeta(state).description;
    const providerState = PROVIDER_STATES.includes(state) ? state : 'provider_unavailable';

    return {
        schemaVersion: 1,
        generatedAt: now,
        stale: providerState === 'stale',
        selectedProvider: providerId,
        daemon: {
            version: 'unknown',
            state: providerState === 'loading' ? 'starting' : 'degraded',
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
    return {
        ...state,
        clientState: deriveSnapshotState(state.snapshot),
        refreshing: false,
        activeRefreshId: null,
        lastRefreshResult: result,
        snapshot: {
            ...state.snapshot,
            daemon: {
                ...(state.snapshot?.daemon ?? {}),
                lastRefreshId: refreshId || result.refreshId || state.snapshot?.daemon?.lastRefreshId || null,
                lastRefreshFinishedAt: result.finishedAt ?? new Date(nowMs).toISOString(),
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
        return 'loading';
    if (snapshot?.stale)
        return 'stale';
    if (providers.some(provider => provider?.state === 'ok'))
        return 'ok';
    return providers[0]?.state ?? 'error';
}

export function normalizeViewState(state, options = {}) {
    const uiOptions = normalizeUiOptions(options);
    const snapshot = state?.snapshot ?? createSyntheticSnapshot('loading');
    const selectedProvider = selectProvider(snapshot, uiOptions.selectedProvider);
    const providerRows = (Array.isArray(snapshot.providers) ? snapshot.providers : [])
        .map(provider => providerRow(provider, uiOptions));
    const selectedRow = selectedProvider ? providerRow(selectedProvider, uiOptions) : null;
    const viewState = state?.clientState ?? deriveSnapshotState(snapshot);
    const viewMeta = stateMeta(viewState);

    return {
        state: viewState,
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
        panelLabel: selectedRow?.shortLabel ?? PRODUCT_PANEL_PLACEHOLDER,
        panelStatus: viewState === 'ok' && selectedRow ? selectedRow.statusLabel : viewMeta.label,
        stale: Boolean(snapshot.stale),
        generatedAt: snapshot.generatedAt ?? null,
    };
}

export function providerRow(provider, options = {}) {
    const state = provider?.state ?? 'error';
    const meta = stateMeta(state);
    const displayName = safeDisplay(provider?.displayName || provider?.provider || 'Unknown provider');
    const providerId = safeDisplay(provider?.provider || '');
    const shortLabel = shortProviderLabel(displayName, providerId);
    const identity = safeDisplay(providerIdentityText(provider));
    const statusText = safeDisplay(providerStatusText(provider), meta.description);
    const meters = providerMeters(provider);

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
        sourceLabel: sourceLabel(provider?.source),
        adapterLabel: adapterLabel(provider?.sourceAdapter),
        updatedText: formatUpdatedAt(provider?.updatedAt),
        resetText: meters
            .map(meter => formatMeterDetail(meter, options.resetTimeFormat))
            .join(' / ') || 'No usage data',
        meters,
        diagnosticsSummary: safeDisplay(provider?.diagnosticsSummary || ''),
        dashboardUrl: safeUrl(provider?.dashboardUrl || provider?.status?.url || ''),
    };
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
        label: credits.unit ? `Credits (${credits.unit})` : 'Credits',
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

export function formatMeterDetail(meter, resetTimeFormat = 'countdown', nowMs = Date.now()) {
    if (!meter)
        return 'No usage data';

    const pieces = [];
    const detail = meter.detail ? safeDisplay(meter.detail) : '';
    if (detail)
        pieces.push(detail);
    else if (typeof meter.remainingPercent === 'number')
        pieces.push(`${Math.round(meter.remainingPercent)}% remaining`);
    else if (typeof meter.usedPercent === 'number')
        pieces.push(`${Math.round(meter.usedPercent)}% used`);

    const reset = formatResetTime(meter.resetsAt, resetTimeFormat, nowMs);
    if (reset)
        pieces.push(`resets ${reset}`);

    return safeDisplay(pieces.join(' · ') || 'Usage available');
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

export function sourceLabel(source) {
    return SOURCE_LABELS[source] ?? SOURCE_LABELS.unknown;
}

export function adapterLabel(adapter) {
    return ADAPTER_LABELS[adapter] ?? ADAPTER_LABELS.none;
}

export function providerStatusText(provider) {
    if (!provider)
        return 'No provider selected';

    const meta = stateMeta(provider.state);
    if (provider.state === 'ok')
        return '';

    return meta.description;
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
        return `${scope} · no events`;

    const highlightedEvent = strongestDiagnosticEvent(events);
    const severity = DIAGNOSTIC_SEVERITY_RANK[highlightedEvent?.severity]
        ? highlightedEvent.severity
        : 'info';
    const firstCode = safeDisplay(highlightedEvent?.code ?? '');
    const pieces = [
        scope,
        `${events.length} ${events.length === 1 ? 'event' : 'events'}`,
        severity,
        firstCode,
    ].filter(Boolean);

    return redactText(pieces.join(' · '));
}

export function diagnosticsCopyText(payload) {
    const parsed = typeof payload === 'string' ? parseJsonObject(payload).value : payload;
    const text = parsed && validateDiagnosticsPayload(parsed).ok
        ? JSON.stringify(diagnosticsCopyProjection(parsed), null, 2)
        : (typeof payload === 'string' ? payload : JSON.stringify(payload, null, 2));
    return redactText(text);
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
    for (const key of ['primary', 'secondary', 'tertiary']) {
        if (!Object.prototype.hasOwnProperty.call(provider.usage, key))
            return false;
    }
    return Object.prototype.hasOwnProperty.call(provider, 'updatedAt');
}

function validateSnapshot(snapshot) {
    if (!snapshot || typeof snapshot !== 'object' || Array.isArray(snapshot))
        return {ok: false, error: 'Snapshot payload was not an object'};
    if (hasForbiddenPublicData(snapshot))
        return {ok: false, error: 'Snapshot payload contained non-public fields'};
    for (const key of SNAPSHOT_REQUIRED_KEYS) {
        if (!Object.prototype.hasOwnProperty.call(snapshot, key))
            return {ok: false, error: `Snapshot payload missing ${key}`};
    }
    if (snapshot.schemaVersion !== 1)
        return {ok: false, error: 'Snapshot payload had an unsupported schema version'};
    if (typeof snapshot.generatedAt !== 'string')
        return {ok: false, error: 'Snapshot payload had no generatedAt timestamp'};
    if (typeof snapshot.stale !== 'boolean')
        return {ok: false, error: 'Snapshot payload had no stale flag'};
    if (!snapshot.daemon || typeof snapshot.daemon !== 'object' || Array.isArray(snapshot.daemon))
        return {ok: false, error: 'Snapshot payload had no daemon state'};
    if (typeof snapshot.daemon.version !== 'string' || !DAEMON_STATES.includes(snapshot.daemon.state))
        return {ok: false, error: 'Snapshot daemon payload did not match the v1 shape'};
    if (!Array.isArray(snapshot.providers))
        return {ok: false, error: 'Snapshot payload had no providers array'};
    if (!snapshot.providers.every(isFullProvider))
        return {ok: false, error: 'Snapshot provider payload did not match the v1 shape'};
    return {ok: true};
}

function validateDiagnosticsPayload(diagnostics) {
    if (!diagnostics || typeof diagnostics !== 'object' || Array.isArray(diagnostics))
        return {ok: false};
    if (hasForbiddenPublicData(diagnostics))
        return {ok: false};
    if (diagnostics.schemaVersion !== 1 || !Array.isArray(diagnostics.events))
        return {ok: false};
    if (!diagnostics.redaction || diagnostics.redaction.applied !== true || diagnostics.redaction.policyVersion !== 1)
        return {ok: false};

    for (const event of diagnostics.events) {
        if (!event || typeof event !== 'object' || Array.isArray(event))
            return {ok: false};
        for (const key of ['code', 'severity', 'safeMessage', 'timestamp']) {
            if (typeof event[key] !== 'string' || event[key].length === 0)
                return {ok: false};
        }
        if (!Object.prototype.hasOwnProperty.call(DIAGNOSTIC_SEVERITY_RANK, event.severity))
            return {ok: false};
        if (!event.redacted || event.redacted.applied !== true)
            return {ok: false};
        if (event.details && (typeof event.details !== 'object' || Array.isArray(event.details)))
            return {ok: false};
    }

    return {ok: true};
}

function hasForbiddenPublicData(value) {
    const queue = [value];
    while (queue.length > 0) {
        const current = queue.shift();
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
        || /\b(?:api[_-]?key|access[_-]?token|refresh[_-]?token|session[_-]?(?:token|key))\b\s*[:=]/i.test(value)
        || hasRawEmail
        || /(^|["'\s])\/(?:home|Users)\//.test(value)
        || /\b(rawPayload|rawResponse|rawOutput|stdout|stderr)\b\s*[:=]/i.test(value)
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
        provider: diagnostics.provider ?? null,
        events: diagnostics.events.map(event => ({
            code: safeDisplay(event.code, 'diagnostic_event'),
            severity: Object.prototype.hasOwnProperty.call(DIAGNOSTIC_SEVERITY_RANK, event.severity)
                ? event.severity
                : 'info',
            safeMessage: safeDisplay(event.safeMessage, 'Details unavailable'),
            timestamp: event.timestamp,
            provider: event.provider ?? null,
            sourceAdapter: event.sourceAdapter ?? null,
            recoverable: typeof event.recoverable === 'boolean' ? event.recoverable : null,
            details: sanitizeDiagnosticsDetails(event.details),
            redacted: {applied: true, classes: event.redacted?.classes ?? []},
        })),
        redaction: {applied: true, policyVersion: 1},
    };
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
    const source = displayName || providerId || 'CB';
    const words = source
        .split(/[\s._-]+/)
        .map(part => part.trim())
        .filter(Boolean);
    if (words.length >= 2)
        return words.slice(0, 2).map(word => word[0]).join('').toUpperCase();
    return source.slice(0, 3).toUpperCase();
}

function formatCreditsDetail(credits) {
    const parts = [];
    if (typeof credits.remaining === 'number')
        parts.push(`${credits.remaining}${credits.unit ? ` ${credits.unit}` : ''} remaining`);
    if (typeof credits.remainingPercent === 'number')
        parts.push(`${Math.round(credits.remainingPercent)}% remaining`);
    return parts.join(' · ') || 'Credits available';
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
