import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import {MANUAL_REFRESH_OPTIONS} from '../src/constants.js';
import {
    applyProviderEventJson,
    applySnapshotJson,
    createInitialState,
    diagnosticsCopyText,
    diagnosticsSummaryLine,
    meterRemainingPercent,
    normalizeViewState,
    panelMeters,
    safeUrl,
    selectProvider,
    applyDiagnosticsJson,
    stateMeta,
} from '../src/state.js';

const FIXTURE_STATES = [
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

function main() {
    assertManualRefreshOptions();
    assertSnapshotFixturesRenderStates();
    assertProviderChangedReplacesCompleteProvider();
    assertPanelMetersPreservePrimarySecondarySemantics();
    assertViewModelUsesStateCopyMap();
    assertViewModelKeepsSemanticSourceSeparateFromAdapter();
    assertDiagnosticsCopyRedaction();
    assertDiagnosticsSummaryLineIsCollapsedAndSafe();
    assertDiagnosticsRejectUnsafePayload();
    assertDiagnosticsRejectRawOutputFields();
    assertProviderUnsafeStringsFailClosed();
    assertDashboardActionHiddenForUnsafeUrls();
    assertSafeUrlRejectsLocalhost();
    assertSafeUrlRejectsPrivateAndTokenizedUrls();
    print('extension state tests passed');
}

function assertManualRefreshOptions() {
    assertEqual(MANUAL_REFRESH_OPTIONS.reason, 'manual');
    assertEqual(MANUAL_REFRESH_OPTIONS.force, true);
    assertEqual(MANUAL_REFRESH_OPTIONS.busyBehavior, 'return_existing');
    assertEqual(MANUAL_REFRESH_OPTIONS.sourceAdapterPolicy.mode, 'only');
    assertArrayEqual(MANUAL_REFRESH_OPTIONS.sourceAdapterPolicy.adapters, ['upstream_cli']);
}

function assertSnapshotFixturesRenderStates() {
    for (const stateName of FIXTURE_STATES) {
        const fixture = readJson(`fixtures/snapshots/${stateName}.json`);
        const nextState = applySnapshotJson(createInitialState(0), JSON.stringify(fixture), 0);
        const providerStates = nextState.snapshot.providers.map(provider => provider.state);
        assert(providerStates.includes(stateName), `${stateName} fixture state not preserved`);
        assertEqual(nextState.lastClientError, null);
    }
}

function assertProviderChangedReplacesCompleteProvider() {
    const ok = readJson('fixtures/snapshots/ok.json');
    const timeout = readJson('fixtures/snapshots/timeout.json');
    let state = applySnapshotJson(createInitialState(0), JSON.stringify(ok), 0);
    const event = {
        schemaVersion: 1,
        eventId: 'test-event',
        emittedAt: '2026-04-27T12:01:00Z',
        reason: 'refresh_finished',
        providerId: 'codex',
        provider: timeout.providers[0],
        diagnosticCodes: ['fixture_timeout'],
    };
    state = applyProviderEventJson(state, 'codex', JSON.stringify(event), 0);
    assertEqual(state.snapshot.providers.length, 1);
    assertEqual(state.snapshot.providers[0].state, 'timeout');
    assertEqual(state.lastClientError, null);
}

function assertPanelMetersPreservePrimarySecondarySemantics() {
    const ok = readJson('fixtures/snapshots/ok.json');
    const provider = selectProvider(ok, 'codex');
    const [primary, secondary] = panelMeters(provider);
    assertEqual(primary.label, 'Session');
    assertEqual(secondary.label, 'Weekly');
    assertEqual(meterRemainingPercent(primary), 58);
    assertEqual(meterRemainingPercent(secondary), 36);
}

function assertViewModelUsesStateCopyMap() {
    const ok = readJson('fixtures/snapshots/ok.json');
    const state = applySnapshotJson(createInitialState(0), JSON.stringify(ok), 0);
    const view = normalizeViewState(state, {panelMode: 'merged'});
    assertEqual(stateMeta('loading').label, 'Loading usage…');
    assertEqual(stateMeta('ok').label, 'Up to date');
    assertEqual(stateMeta('stale').label, 'Stale data');
    assertEqual(stateMeta('unauthenticated').label, 'Sign-in required');
    assertEqual(stateMeta('cookie_rejected').label, 'Browser session rejected');
    assertEqual(stateMeta('parse_error').label, 'Could not read provider data');
    assertEqual(stateMeta('timeout').label, 'Provider timed out');
    assertEqual(view.panelStatus, 'Up to date');
    assertEqual(view.panelLabel, 'COD');
}

function assertViewModelKeepsSemanticSourceSeparateFromAdapter() {
    const ok = readJson('fixtures/snapshots/ok.json');
    const state = applySnapshotJson(createInitialState(0), JSON.stringify(ok), 0);
    const [row] = normalizeViewState(state, {}).providerRows;
    assertEqual(row.sourceLabel, 'API');
    assertEqual(row.adapterLabel, 'Fixture');
    assertEqual(row.meters[0].label, 'Session');
    assertEqual(row.meters[1].label, 'Weekly');
}

function assertDiagnosticsCopyRedaction() {
    const raw = {
        token: 'Bearer abc.def',
        header: 'Authorization: secret',
        apiKeyHeader: 'X-API-Key: sk-api-secret',
        cookie: 'Cookie: session=value',
        sessionKey: 'sessionKey=plain-session-secret',
        email: 'person@example.com',
        path: '/home/test/.config/browser/Profile/Cookies',
    };
    const redacted = diagnosticsCopyText(raw);
    assert(!redacted.includes('abc.def'), 'bearer token was not redacted');
    assert(!redacted.includes('secret'), 'authorization header was not redacted');
    assert(!redacted.includes('sk-api-secret'), 'API key header was not redacted');
    assert(!redacted.includes('session=value'), 'cookie was not redacted');
    assert(!redacted.includes('plain-session-secret'), 'session key was not redacted');
    assert(!redacted.includes('person@example.com'), 'email was not redacted');
    assert(!redacted.includes('/home/test'), 'home path was not redacted');
}

function assertDiagnosticsSummaryLineIsCollapsedAndSafe() {
    const diagnostics = {
        schemaVersion: 1,
        scope: 'provider',
        provider: 'codex',
        generatedAt: '2026-04-27T12:00:00Z',
        events: [
            {
                code: 'fixture_warning',
                severity: 'warning',
                safeMessage: 'Safe warning',
                timestamp: '2026-04-27T12:00:00Z',
                redacted: {applied: true},
            },
            {
                code: 'fixture_error',
                severity: 'error',
                safeMessage: 'Safe error',
                timestamp: '2026-04-27T12:01:00Z',
                redacted: {applied: true},
            },
        ],
        redaction: {applied: true, policyVersion: 1},
    };
    const summary = diagnosticsSummaryLine(diagnostics);
    assertEqual(summary, 'codex · 2 events · error · fixture_error');

    const next = applyDiagnosticsJson(createInitialState(0), 'codex', JSON.stringify(diagnostics), 0);
    assertEqual(next.diagnostics.summary, summary);
    assert(!next.diagnostics.summary.includes('Safe warning'), 'collapsed diagnostics summary should not expand event messages');
}

function assertDiagnosticsRejectUnsafePayload() {
    const diagnostics = {
        schemaVersion: 1,
        scope: 'provider',
        provider: 'codex',
        generatedAt: '2026-04-27T12:00:00Z',
        events: [{
            code: 'bad_secret',
            severity: 'error',
            safeMessage: 'sessionKey=plain-session-secret',
            timestamp: '2026-04-27T12:00:00Z',
            redacted: {applied: true},
        }],
        redaction: {applied: true, policyVersion: 1},
    };
    const next = applyDiagnosticsJson(createInitialState(0), 'codex', JSON.stringify(diagnostics), 0);
    assertEqual(next.clientState, 'parse_error');
    assertEqual(next.lastClientError, 'Diagnostics payload did not match the v1 shape');
}

function assertDiagnosticsRejectRawOutputFields() {
    for (const key of ['stdout', 'stderr', 'stdoutText', 'stderrText', 'stdoutJson', 'stderrJson', 'rawOutput', 'requestHeaders']) {
        const diagnostics = {
            schemaVersion: 1,
            scope: 'provider',
            provider: 'codex',
            generatedAt: '2026-04-27T12:00:00Z',
            events: [{
                code: 'raw_output',
                severity: 'error',
                safeMessage: 'Details unavailable',
                timestamp: '2026-04-27T12:00:00Z',
                details: {[key]: 'raw upstream output'},
                redacted: {applied: true},
            }],
            redaction: {applied: true, policyVersion: 1},
        };
        const next = applyDiagnosticsJson(createInitialState(0), 'codex', JSON.stringify(diagnostics), 0);
        assertEqual(next.clientState, 'parse_error');
    }
}

function assertProviderUnsafeStringsFailClosed() {
    const ok = readJson('fixtures/snapshots/ok.json');
    ok.providers[0].status.description = 'stdout: {"raw":"payload"}';
    ok.providers[0].diagnosticsSummary = '{"rawPayload":"secret"}';
    const state = applySnapshotJson(createInitialState(0), JSON.stringify(ok), 0);
    const view = normalizeViewState(state, {});
    assertEqual(state.clientState, 'parse_error');
    assert(!view.panelStatus.includes('stdout'), 'raw stdout marker surfaced in panel status');
    assert(!JSON.stringify(view.providerRows).includes('rawPayload'), 'raw payload marker surfaced in provider rows');
}

function assertDashboardActionHiddenForUnsafeUrls() {
    const ok = readJson('fixtures/snapshots/ok.json');
    ok.providers[0].dashboardUrl = 'https://example.com/dashboard?access_token=secret';
    const state = applySnapshotJson(createInitialState(0), JSON.stringify(ok), 0);
    const [row] = normalizeViewState(state, {}).providerRows;
    assertEqual(row.dashboardUrl, '');
}

function assertSafeUrlRejectsLocalhost() {
    const http = 'http' + '://';
    const https = 'https' + '://';
    const localHost = 'local' + 'host';
    const loopback = '127.' + '0.0.1';
    const dashboard = `${https}example.com/dashboard`;

    assertEqual(safeUrl('javascript:alert(1)'), '');
    assertEqual(safeUrl(`${http}${localHost}:3000/status`), '');
    assertEqual(safeUrl(`${http}api.${localHost}/status`), '');
    assertEqual(safeUrl(`${http}${loopback}:3000/status`), '');
    assertEqual(safeUrl(`${http}[::1]:3000/status`), '');
    assertEqual(safeUrl(`${http}user:pass@example.com/status`), '');
    assertEqual(safeUrl(dashboard), dashboard);
}

function assertSafeUrlRejectsPrivateAndTokenizedUrls() {
    const http = 'http' + '://';
    const https = 'https' + '://';
    for (const url of [
        `${http}localhost./status`,
        `${http}2130706433/status`,
        `${http}0177.0.0.1/status`,
        `${http}0x7f000001/status`,
        `${http}[::ffff:127.0.0.1]/status`,
        `${http}169.254.169.254/status`,
        `${http}10.0.0.1/status`,
        `${http}192.168.1.1/status`,
        `${http}172.16.0.1/status`,
        `${https}example.com/dashboard?access_token=secret`,
    ])
        assertEqual(safeUrl(url), '');
}

function readJson(relativePath) {
    const path = GLib.build_filenamev([GLib.get_current_dir(), relativePath]);
    const file = Gio.File.new_for_path(path);
    const [ok, contents] = file.load_contents(null);
    if (!ok)
        throw new Error(`failed to load ${relativePath}`);
    return JSON.parse(new TextDecoder('utf-8').decode(contents));
}

function assert(value, message) {
    if (!value)
        throw new Error(message);
}

function assertEqual(actual, expected) {
    if (actual !== expected)
        throw new Error(`expected ${expected}, got ${actual}`);
}

function assertArrayEqual(actual, expected) {
    assertEqual(JSON.stringify(actual), JSON.stringify(expected));
}

main();
