import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import {MANUAL_REFRESH_OPTIONS} from '../src/constants.js';
import {
    applyProviderEventJson,
    applySnapshotJson,
    createInitialState,
    diagnosticsCopyText,
    meterRemainingPercent,
    panelMeters,
    selectProvider,
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
    assertDiagnosticsCopyRedaction();
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

function assertDiagnosticsCopyRedaction() {
    const raw = {
        token: 'Bearer abc.def',
        header: 'Authorization: secret',
        cookie: 'Cookie: session=value',
        email: 'person@example.com',
        path: '/home/test/.config/browser/Profile/Cookies',
    };
    const redacted = diagnosticsCopyText(raw);
    assert(!redacted.includes('abc.def'), 'bearer token was not redacted');
    assert(!redacted.includes('secret'), 'authorization header was not redacted');
    assert(!redacted.includes('session=value'), 'cookie was not redacted');
    assert(!redacted.includes('person@example.com'), 'email was not redacted');
    assert(!redacted.includes('/home/test'), 'home path was not redacted');
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
