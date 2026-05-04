import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import {MANUAL_REFRESH_OPTIONS} from '../src/constants.js';
import {
    applyProviderEventJson,
    applyRefreshFinishedJson,
    applySnapshotJson,
    costSummaryRows,
    createInitialState,
    diagnosticsCopyText,
    diagnosticsSummaryLine,
    footerStatusText,
    meterClassNames,
    meterFillFraction,
    meterFillClassNames,
    meterFillFractionFromPercent,
    headerStatusText,
    meterRow,
    meterRemainingPercent,
    normalizeViewState,
    panelAccessibleName,
    panelButtonClassNames,
    panelContentClassNames,
    panelMeters,
    panelProviderItemClassNames,
    refreshButtonLabel,
    safeUrl,
    selectProvider,
    applyDiagnosticsJson,
    stateMeta,
    withClientError,
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
    assertViewModelBuildsProviderStripAndSelectedSurface();
    assertDefaultViewModelKeepsDiagnosticsAndDebugCopyOutOfMainLabels();
    assertPanelViewModelBoundsProviderMode();
    assertPanelViewModelStaysCompactProgressOnly();
    assertPanelClassNamesAreStable();
    assertPanelAccessibleNameIncludesHiddenUsageDetails();
    assertViewModelKeepsSemanticSourceSeparateFromAdapter();
    assertMeterRowsClampAndCostSummariesRender();
    assertMeterFractionsAreClampedAndProportional();
    assertMeterCssClassNamesMatchStylesheet();
    assertSnapshotRejectsSchemaDrift();
    assertFooterStatusIncludesCostCapability();
    assertDaemonUnavailableSyntheticStateIsConsistent();
    assertProviderStatusUsesSafeDaemonDescription();
    assertHeaderStatusPreservesNonOkStates();
    assertRefreshFinishedRejectsUnsafeResults();
    assertRefreshFinishedRejectsSchemaDrift();
    assertDiagnosticsCopyRedaction();
    assertDiagnosticsSummaryLineIsCollapsedAndSafe();
    assertDiagnosticsFailureClearsStalePayload();
    assertDiagnosticsRejectSchemaDrift();
    assertDiagnosticsRejectUnsafePayload();
    assertDiagnosticsRejectUnsafeArrayPayloads();
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
    assertEqual(stateMeta('missing_dependency').label, 'Dependency missing');
    assertEqual(stateMeta('provider_unavailable').label, 'Provider unavailable');
    assertEqual(stateMeta('parse_error').label, 'Could not read provider data');
    assertEqual(stateMeta('timeout').label, 'Provider timed out');
    assertEqual(stateMeta('error').label, 'Error');
    assertEqual(stateMeta('daemon_unavailable').label, 'Daemon unavailable');
    assertEqual(view.panelStatus, 'Up to date');
    assertEqual(view.panelLabel, 'COD');
    assertEqual(view.stateLabel, 'Up to date');
    assertEqual(view.stateDescription, 'Usage data is current');
    assertEqual(view.refreshLabel, 'Refresh');
    assertEqual(view.titleAction.label, 'Refresh');
    assertEqual(view.titleAction.action, 'refresh');
    assertEqual(view.titleAction.reactive, true);
    assertEqual(view.footerStatus, 'Daemon running · CLI available · Cost unknown · Browser import unsupported');
}

function assertViewModelBuildsProviderStripAndSelectedSurface() {
    const snapshot = readJson('fixtures/snapshots/ok.json');
    const claude = cloneProvider(snapshot.providers[0], {
        provider: 'claude',
        displayName: 'Claude',
        primaryUsed: 18,
        primaryRemaining: 82,
        secondaryUsed: 91,
        secondaryRemaining: 9,
    });
    snapshot.providers.push(claude);
    snapshot.providers[1].credits = {
        remaining: 25,
        remainingPercent: 50,
        updatedAt: '2026-04-27T12:00:00Z',
        unit: 'credits',
    };
    snapshot.providers[1].cost = {
        updatedAt: '2026-04-27T12:00:00Z',
        currency: 'USD',
        total: 2.5,
        items: [
            {label: 'Today', amount: 0.5, currency: 'USD', detail: '2k tokens'},
        ],
    };
    snapshot.selectedProvider = 'claude';

    const state = applySnapshotJson(createInitialState(0), JSON.stringify(snapshot), 0);
    const view = normalizeViewState(state, {selectedProvider: 'claude', panelMode: 'merged'});

    assertEqual(view.selectedProviderId, 'claude');
    assertEqual(view.selectedRow.providerId, 'claude');
    assertEqual(view.selectedRow.displayName, 'Claude');
    assertEqual(view.selectedRow.shortLabel, 'CLA');
    assertEqual(view.selectedRow.titleStatusText.startsWith('Updated'), true);
    assertEqual(view.selectedRow.meterRows[0].label, 'Session');
    assertEqual(view.selectedRow.meterRows[0].usedPercent, 18);
    assertEqual(view.selectedRow.meterRows[0].remainingPercent, 82);
    assertEqual(view.selectedRow.meterRows[0].fillPercent, 82);
    assertEqual(view.selectedRow.meterRows[0].fillFraction, 0.82);
    assertEqual(view.selectedRow.meterRows[1].label, 'Weekly');
    assertEqual(view.selectedRow.meterRows[1].remainingPercent, 9);
    assertEqual(view.selectedRow.meterRows[1].fillPercent, 9);
    assertEqual(view.selectedRow.meterRows[1].fillFraction, 0.09);
    assertArrayEqual(
        view.selectedRow.usageSections.map(section => section.key),
        ['primary', 'secondary', 'credits'],
    );
    assertArrayEqual(
        view.selectedRow.usageSections.map(section => section.title),
        ['Session', 'Weekly', 'Credits'],
    );
    assertEqual(view.selectedRow.usageSections[2].meter.label, 'Credits');
    assert(!view.selectedRow.usageSections[2].meter.label.includes('(credits)'), 'credits label should not duplicate unit');
    assertEqual(view.selectedRow.usageSections[2].meter.fillPercent, 50);
    assertEqual(view.selectedRow.usageSections[2].meter.fillFraction, 0.5);
    assertEqual(view.selectedRow.costRows.length, 1);
    assertEqual(view.selectedRow.costRows[0].label, 'Today');
    assertEqual(view.providerSelectorRows.length, 2);
    assertEqual(view.providerSelectorRows[0].selected, false);
    assertEqual(view.providerSelectorRows[1].selected, true);
    assertEqual(view.providerSelectorRows[1].label, 'Claude');
    assertEqual(view.providerSelectorRows[1].displayName, 'Claude');
    assert(!view.providerSelectorRows[1].label.includes(view.selectedRow.shortLabel), 'provider strip should not concatenate badge and provider name');
    assertEqual(view.panel.label, 'CLA');
    assert(view.panel.label.length <= 3, 'top-bar provider label should stay compact');
    assertEqual(view.panel.meters[0].label, 'Session');
    assertEqual(view.panel.meters[1].label, 'Weekly');
}

function assertDefaultViewModelKeepsDiagnosticsAndDebugCopyOutOfMainLabels() {
    const ok = readJson('fixtures/snapshots/ok.json');
    ok.providers[0].diagnosticCodes = ['upstream_cli_command_finished'];
    ok.providers[0].credits = {
        remaining: 0,
        remainingPercent: null,
        updatedAt: '2026-04-27T12:00:00Z',
        unit: 'credits',
    };
    const state = applySnapshotJson(createInitialState(0), JSON.stringify(ok), 0);
    const view = normalizeViewState(state, {panelMode: 'merged'});
    const labels = collectMainViewLabels(view);
    assertEqual(view.diagnostics, null);
    assertEqual(typeof view.footerStatus, 'string');
    assert(!view.footerStatus.includes('\n'), 'footer should be one compact status string');
    for (const label of labels) {
        for (const forbidden of [
            'Diagnostics block body',
            'upstream_cli_command_finished',
            'stdout',
            'stderr',
            'rawPayload',
            'rawResponse',
            'Partial System Degradation',
            'Stale · stale',
            'Credits (credits)',
        ]) {
            assert(!label.includes(forbidden), `default UI label leaked forbidden copy: ${forbidden}`);
        }
        assert(!/[{[]\s*["'][^"']+["']\s*:/.test(label), `default UI label looks like raw JSON: ${label}`);
    }
    assertArrayEqual(
        view.selectedRow.usageSections.map(section => section.title),
        ['Session', 'Weekly', 'Credits'],
    );
}

function assertPanelViewModelBoundsProviderMode() {
    const fourProviderSnapshot = readJson('fixtures/snapshots/ok.json');
    for (let index = 1; index <= 3; index++) {
        fourProviderSnapshot.providers.push(cloneProvider(fourProviderSnapshot.providers[0], {
            provider: `four-provider${index}`,
            displayName: `Four Provider ${index}`,
            primaryUsed: index * 10,
            primaryRemaining: 100 - (index * 10),
            secondaryUsed: index * 12,
            secondaryRemaining: 100 - (index * 12),
        }));
    }
    const fourProviderState = applySnapshotJson(createInitialState(0), JSON.stringify(fourProviderSnapshot), 0);
    const fourProviderView = normalizeViewState(fourProviderState, {panelMode: 'provider'});

    assertEqual(fourProviderView.panel.visibleProviders.length, 3);
    assertEqual(fourProviderView.panel.overflowCount, 1);

    const snapshot = readJson('fixtures/snapshots/ok.json');
    for (let index = 1; index <= 4; index++) {
        snapshot.providers.push(cloneProvider(snapshot.providers[0], {
            provider: `provider${index}`,
            displayName: `Provider ${index}`,
            primaryUsed: index * 10,
            primaryRemaining: 100 - (index * 10),
            secondaryUsed: index * 12,
            secondaryRemaining: 100 - (index * 12),
        }));
    }

    const state = applySnapshotJson(createInitialState(0), JSON.stringify(snapshot), 0);
    const view = normalizeViewState(state, {panelMode: 'provider'});

    assertEqual(view.panel.mode, 'provider');
    assertEqual(view.panel.visibleProviders.length, 3);
    assertEqual(view.panel.overflowCount, 2);
}

function assertPanelViewModelStaysCompactProgressOnly() {
    const snapshot = readJson('fixtures/snapshots/ok.json');
    for (let index = 1; index <= 2; index++) {
        snapshot.providers.push(cloneProvider(snapshot.providers[0], {
            provider: `compact-provider${index}`,
            displayName: `Compact Provider ${index}`,
            primaryUsed: index * 12,
            primaryRemaining: 100 - (index * 12),
            secondaryUsed: index * 18,
            secondaryRemaining: 100 - (index * 18),
        }));
    }

    const state = applySnapshotJson(createInitialState(0), JSON.stringify(snapshot), 0);
    const mergedView = normalizeViewState(state, {panelMode: 'merged'});
    const providerView = normalizeViewState(state, {panelMode: 'provider'});
    const minimalView = normalizeViewState(state, {panelMode: 'minimal'});

    for (const view of [mergedView, providerView, minimalView]) {
        assertEqual(view.panel.compact, true);
        assertEqual(view.panel.showText, false);
        assertEqual(view.panel.meterCount, 2);
        assertEqual(view.panel.meters.length, 2);
    }

    assertEqual(providerView.panel.visibleProviders.length, 3);
    for (const row of providerView.panel.visibleProviders) {
        assertEqual(row.compact, true);
        assertEqual(row.showText, false);
        assertEqual(row.meterCount, 2);
        assertEqual(row.meters.length, 2);
        assert(row.label.length <= 3, 'provider labels should remain bounded for accessibility only');
    }
}

function assertPanelClassNamesAreStable() {
    const ok = readJson('fixtures/snapshots/ok.json');
    const state = applySnapshotJson(createInitialState(0), JSON.stringify(ok), 0);
    const view = normalizeViewState(state, {panelMode: 'provider', theme: 'compact'});
    const panelClasses = panelButtonClassNames(view, {theme: 'compact'}, {open: true});

    for (const expected of [
        'panel-button',
        'codexbar-panel',
        'codexbar-theme-compact',
        'codexbar-state-ok',
        'codexbar-panel-open',
    ])
        assert(panelClasses.split(' ').includes(expected), `panel class missing ${expected}`);
    assert(!panelClasses.includes('undefined'), 'panel classes should not include undefined');

    assertEqual(
        panelContentClassNames(view.panel),
        'codexbar-panel-content codexbar-panel-content-provider',
    );
    assertEqual(
        panelContentClassNames({mode: 'minimal'}),
        'codexbar-panel-content codexbar-panel-content-minimal',
    );
    assertEqual(
        panelProviderItemClassNames(view.panel.visibleProviders[0]),
        'codexbar-panel-provider-item codexbar-state-ok codexbar-severity-ok',
    );
    assertEqual(
        panelProviderItemClassNames({state: 'not_real', severity: 'not_real'}),
        'codexbar-panel-provider-item codexbar-state-error codexbar-severity-error',
    );
}

function assertPanelAccessibleNameIncludesHiddenUsageDetails() {
    const ok = readJson('fixtures/snapshots/ok.json');
    const state = applySnapshotJson(createInitialState(0), JSON.stringify(ok), 0);
    const view = normalizeViewState(state, {panelMode: 'merged'});
    const label = panelAccessibleName(view);

    for (const expected of [
        'CodexBar: Codex',
        'Up to date',
        'Session',
        '58% remaining',
        'Weekly',
        '36% remaining',
        'resets',
    ])
        assert(label.includes(expected), `panel accessible name missing ${expected}`);
    assert(!label.includes('rawPayload'), 'panel accessible name must stay normalized');
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

function assertMeterRowsClampAndCostSummariesRender() {
    const clamped = meterRow({usedPercent: 150, label: 'Session'}, 'countdown', 0);
    assertEqual(clamped.usedPercent, 100);
    assertEqual(clamped.remainingPercent, 0);
    assertEqual(clamped.fillPercent, 0);
    assertEqual(clamped.fillFraction, 0);

    const missing = meterRow(null, 'countdown', 0);
    assertEqual(missing.detail, 'No usage data');
    assertEqual(missing.fillFraction, null);

    const rows = costSummaryRows({
        total: 12.345,
        currency: 'USD',
        items: [
            {label: 'Today', amount: 1.2, currency: 'USD', detail: '10k tokens'},
            {label: 'Last 30 days', amount: 11.145, currency: 'USD', detail: '80k tokens'},
            {label: 'Extra', amount: 2, currency: 'USD', detail: 'bounded'},
            {label: 'Overflow', amount: 3, currency: 'USD', detail: 'hidden'},
        ],
    });
    assertEqual(rows[0].label, 'Today');
    assertEqual(rows[0].value, '$1.20 · 10k tokens');
    assertEqual(rows[1].label, 'Last 30 days');
    assertEqual(rows[1].value, '$11.14 · 80k tokens');
    assertEqual(rows.length, 2);
    assert(!JSON.stringify(rows).includes('Total'), 'cost rows should avoid duplicate aggregate summaries when items exist');
    assert(!JSON.stringify(rows).includes('Overflow'), 'cost summary should be bounded');

    const fallbackRows = costSummaryRows({
        total: 12.345,
        currency: 'USD',
        items: [],
    });
    assertEqual(fallbackRows.length, 1);
    assertEqual(fallbackRows[0].label, 'Cost');
    assertEqual(fallbackRows[0].value, '$12.35');
}

function assertMeterFractionsAreClampedAndProportional() {
    const highRemaining = meterRow({remainingPercent: 97, usedPercent: 3, label: 'Session'}, 'countdown', 0);
    assertEqual(highRemaining.fillPercent, 97);
    assertEqual(highRemaining.fillFraction, 0.97);

    const halfRemaining = meterRow({remainingPercent: 57, usedPercent: 43, label: 'Weekly'}, 'countdown', 0);
    assertEqual(halfRemaining.fillPercent, 57);
    assertEqual(halfRemaining.fillFraction, 0.57);

    const usedOnly = meterRow({usedPercent: 25, label: 'Session'}, 'countdown', 0);
    assertEqual(usedOnly.remainingPercent, 75);
    assertEqual(usedOnly.fillPercent, 75);
    assertEqual(usedOnly.fillFraction, 0.75);

    assertEqual(meterFillFraction({remainingPercent: 120}), 1);
    assertEqual(meterFillFraction({usedPercent: 150}), 0);
    assertEqual(meterFillFraction({remainingPercent: -4}), 0);
    assertEqual(meterFillFraction(null), null);
    assertEqual(meterFillFractionFromPercent(58), 0.58);
    assertEqual(meterFillFractionFromPercent(57), 0.57);
    assertEqual(meterFillFractionFromPercent(Number.POSITIVE_INFINITY), null);
}

function assertMeterCssClassNamesMatchStylesheet() {
    const stylesheet = readText('extension/stylesheet.css');
    for (const tone of ['ok', 'warning', 'danger', 'unknown', 'invalid']) {
        const classNames = [
            ...meterClassNames(tone, {compact: true}).split(' '),
            ...meterFillClassNames(tone).split(' '),
        ];
        for (const className of classNames)
            assert(stylesheet.includes(`.${className}`), `stylesheet missing selector for ${className}`);
    }
    assert(meterClassNames('invalid').endsWith('codexbar-meter-unknown'), 'invalid meter tone should fall back to unknown');
    assert(meterFillClassNames('invalid').endsWith('codexbar-meter-fill-unknown'), 'invalid fill tone should fall back to unknown');
}

function assertSnapshotRejectsSchemaDrift() {
    const ok = readJson('fixtures/snapshots/ok.json');

    const withExtraTopLevel = {...ok, extra: 'safe-looking drift'};
    const topLevelState = applySnapshotJson(createInitialState(0), JSON.stringify(withExtraTopLevel), 0);
    assertEqual(topLevelState.clientState, 'parse_error');

    const withExtraProvider = JSON.parse(JSON.stringify(ok));
    withExtraProvider.providers[0].extra = 'safe-looking drift';
    const providerState = applySnapshotJson(createInitialState(0), JSON.stringify(withExtraProvider), 0);
    assertEqual(providerState.clientState, 'parse_error');

    const withExtraMeter = JSON.parse(JSON.stringify(ok));
    withExtraMeter.providers[0].usage.primary.extra = 'safe-looking drift';
    const meterState = applySnapshotJson(createInitialState(0), JSON.stringify(withExtraMeter), 0);
    assertEqual(meterState.clientState, 'parse_error');
}

function assertFooterStatusIncludesCostCapability() {
    assertEqual(refreshButtonLabel('daemon_unavailable', false), 'Retry');
    assertEqual(refreshButtonLabel('daemon_unavailable', true), 'Refreshing');
    assertEqual(
        footerStatusText(null, null),
        'Daemon unknown · CLI unknown · Cost unknown · Browser import unsupported',
    );
    const footer = footerStatusText(
        {state: 'degraded', upstreamCli: {available: true}},
        {cost: true, browserImport: false},
    );
    assertEqual(footer, 'Daemon degraded · CLI available · Cost available · Browser import unsupported');
    assertEqual(
        footerStatusText({state: 'ok', upstreamCli: {available: true}}, {cost: true, browserImport: true}),
        'Daemon running · CLI available · Cost available · Browser import unsupported',
    );
}

function assertDaemonUnavailableSyntheticStateIsConsistent() {
    const state = withClientError(
        createInitialState(0),
        'Daemon D-Bus service is unavailable',
        0,
        'daemon_unavailable',
        'daemon_unavailable',
    );
    const view = normalizeViewState(state, {panelMode: 'merged'});
    assertEqual(view.state, 'daemon_unavailable');
    assertEqual(view.headerStatus, 'Daemon unavailable');
    assertEqual(view.refreshLabel, 'Retry');
    assertEqual(view.footerStatus, 'Daemon unavailable · CLI unknown · Cost unknown · Browser import unsupported');
    assertEqual(view.selectedRow.state, 'missing_dependency');
    assertEqual(view.selectedRow.statusDescription, 'Daemon D-Bus service is unavailable');
}

function assertProviderStatusUsesSafeDaemonDescription() {
    const snapshot = readJson('fixtures/snapshots/missing_dependency.json');
    snapshot.providers[0].status.description = 'Configured CodexBar CLI is not executable';
    const state = applySnapshotJson(createInitialState(0), JSON.stringify(snapshot), 0);
    const view = normalizeViewState(state, {panelMode: 'merged'});
    assertEqual(view.selectedRow.statusDescription, 'Configured CodexBar CLI is not executable');

    snapshot.providers[0].status.description = '/home/user/.config/codexbar/private';
    const unsafe = applySnapshotJson(createInitialState(0), JSON.stringify(snapshot), 0);
    assertEqual(unsafe.clientState, 'parse_error');
}

function assertHeaderStatusPreservesNonOkStates() {
    const generatedAt = '2026-04-27T12:00:00Z';
    assert(headerStatusText('ok', false, false, generatedAt).startsWith('Updated'), 'ok header should show updated age');
    assert(headerStatusText('stale', true, false, generatedAt).startsWith('Stale data'), 'stale header should keep stale wording');
    assert(headerStatusText('unauthenticated', false, false, generatedAt).startsWith('Sign-in required'), 'auth header should keep auth wording');
    assert(headerStatusText('cookie_rejected', false, false, generatedAt).startsWith('Browser session rejected'), 'cookie header should keep cookie wording');
    assert(headerStatusText('parse_error', false, false, generatedAt).startsWith('Could not read provider data'), 'parse header should keep parse wording');
    assertEqual(headerStatusText('daemon_unavailable', false, false, generatedAt), 'Daemon unavailable');
    assertEqual(headerStatusText('ok', false, true, generatedAt), 'Refreshing…');
}

function assertRefreshFinishedRejectsUnsafeResults() {
    const ok = readJson('fixtures/snapshots/ok.json');
    let state = applySnapshotJson(createInitialState(0), JSON.stringify(ok), 0);
    const result = {
        schemaVersion: 1,
        refreshId: 'refresh-unsafe',
        status: 'error',
        startedAt: '2026-04-27T11:59:58Z',
        finishedAt: '2026-04-27T12:00:00Z',
        durationMs: 100,
        reason: 'manual',
        providers: [{provider: 'codex', status: 'error', diagnosticCodes: ['bad']}],
        cacheWritten: false,
        stdout: 'raw provider output',
    };
    state = applyRefreshFinishedJson(state, 'refresh-unsafe', JSON.stringify(result), 0);
    assertEqual(state.clientState, 'parse_error');
    assertEqual(state.lastRefreshResult, null);
    assert(!JSON.stringify(state).includes('raw provider output'), 'unsafe refresh result should not be retained');
}

function assertRefreshFinishedRejectsSchemaDrift() {
    const ok = readJson('fixtures/snapshots/ok.json');
    const baseState = applySnapshotJson(createInitialState(0), JSON.stringify(ok), 0);
    const baseResult = {
        schemaVersion: 1,
        refreshId: 'refresh-valid',
        status: 'ok',
        startedAt: '2026-04-27T11:59:58Z',
        finishedAt: '2026-04-27T12:00:00Z',
        durationMs: 100,
        reason: 'manual',
        providers: [{
            provider: 'codex',
            status: 'ok',
            sourceAdapter: 'upstream_cli',
            diagnosticCodes: [],
        }],
        cacheWritten: true,
        snapshotGeneratedAt: '2026-04-27T12:00:00Z',
        diagnosticCodes: [],
    };

    const accepted = applyRefreshFinishedJson(baseState, 'refresh-valid', JSON.stringify(baseResult), 0);
    assertEqual(accepted.lastRefreshResult.durationMs, 100);

    for (const patch of [
        {extra: 'safe extra field'},
        {durationMs: 100.5},
        {snapshotGeneratedAt: 'not-a-date'},
        {diagnosticCodes: ['ok', 42]},
        {providers: [{...baseResult.providers[0], extra: 'safe extra provider field'}]},
        {providers: [{...baseResult.providers[0], diagnosticCodes: ['ok', 42]}]},
    ]) {
        const result = {...baseResult, ...patch};
        const next = applyRefreshFinishedJson(baseState, 'refresh-invalid', JSON.stringify(result), 0);
        assertEqual(next.clientState, 'parse_error');
        assertEqual(next.lastRefreshResult, null);
    }
}

function assertDiagnosticsCopyRedaction() {
    const raw = {
        token: 'Bearer abc.def',
        header: 'Authorization: secret',
        apiKeyHeader: 'X-API-Key: sk-api-secret',
        cookie: 'Cookie: session=value',
        sessionKey: 'sessionKey=plain-session-secret',
        bareOpenAiToken: 'sk-proj-123456789012345678901234',
        bareAnthropicToken: 'sk-ant-123456789012345678901234',
        jwt: 'eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0In0.signaturetoken',
        email: 'person@example.com',
        path: '/home/test/.config/browser/Profile/Cookies',
    };
    const redacted = diagnosticsCopyText(raw);
    assert(!redacted.includes('abc.def'), 'bearer token was not redacted');
    assert(!redacted.includes('secret'), 'authorization header was not redacted');
    assert(!redacted.includes('sk-api-secret'), 'API key header was not redacted');
    assert(!redacted.includes('session=value'), 'cookie was not redacted');
    assert(!redacted.includes('plain-session-secret'), 'session key was not redacted');
    assert(!redacted.includes('sk-proj-123456789012345678901234'), 'OpenAI-style token was not redacted');
    assert(!redacted.includes('sk-ant-123456789012345678901234'), 'Anthropic-style token was not redacted');
    assert(!redacted.includes('eyJhbGciOiJIUzI1NiJ9'), 'JWT was not redacted');
    assert(!redacted.includes('person@example.com'), 'email was not redacted');
    assert(!redacted.includes('/home/test'), 'home path was not redacted');
    assert(!redacted.includes('rawPayload'), 'invalid diagnostics fallback should not preserve raw keys');
    assert(!redacted.includes('stdout'), 'invalid diagnostics fallback should not preserve stdout keys');
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
    assertEqual(summary, 'Last issue: Safe error');
    assert(!summary.includes('fixture_error'), 'collapsed diagnostics summary should not use diagnostic codes as primary copy');

    const next = applyDiagnosticsJson(createInitialState(0), 'codex', JSON.stringify(diagnostics), 0);
    assertEqual(next.diagnostics.summary, summary);
    assert(!next.diagnostics.summary.includes('Safe warning'), 'collapsed diagnostics summary should show only the strongest safe message');
}

function assertDiagnosticsFailureClearsStalePayload() {
    const diagnostics = safeDiagnosticsPayload();
    let state = applyDiagnosticsJson(createInitialState(0), 'codex', JSON.stringify(diagnostics), 0);
    assert(state.diagnostics?.payload, 'safe diagnostics should be loaded before failure case');

    const unsafe = safeDiagnosticsPayload();
    unsafe.events[0].safeMessage = 'sessionKey=plain-session-secret';
    state = applyDiagnosticsJson(state, 'codex', JSON.stringify(unsafe), 0);
    assertEqual(state.clientState, 'parse_error');
    assertEqual(state.diagnostics, null);
}

function assertDiagnosticsRejectSchemaDrift() {
    for (const patch of [
        {extra: 'safe-looking drift'},
        {generatedAt: 'not-a-date'},
        {scope: 'unknown'},
        {provider: {id: 'codex'}},
        {redaction: {applied: true, policyVersion: 1, extra: 'safe-looking drift'}},
        {events: [{...safeDiagnosticsPayload().events[0], extra: 'safe-looking drift'}]},
        {events: [{...safeDiagnosticsPayload().events[0], sourceAdapter: 'unknown'}]},
        {events: [{...safeDiagnosticsPayload().events[0], timestamp: 'not-a-date'}]},
        {events: [{...safeDiagnosticsPayload().events[0], details: {nested: {unsafe: 'shape'}}}]},
        {events: [{...safeDiagnosticsPayload().events[0], redacted: {applied: true, extra: 'safe-looking drift'}}]},
    ]) {
        const diagnostics = {...safeDiagnosticsPayload(), ...patch};
        const next = applyDiagnosticsJson(createInitialState(0), 'codex', JSON.stringify(diagnostics), 0);
        assertEqual(next.clientState, 'parse_error');
        assertEqual(next.diagnostics, null);
    }
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
    assertEqual(next.diagnostics, null);
}

function assertDiagnosticsRejectUnsafeArrayPayloads() {
    const diagnostics = {
        schemaVersion: 1,
        scope: 'provider',
        provider: 'codex',
        generatedAt: '2026-04-27T12:00:00Z',
        events: [{
            code: 'safe_code',
            severity: 'warning',
            safeMessage: 'Details unavailable',
            timestamp: '2026-04-27T12:00:00Z',
            redacted: {
                applied: true,
                classes: ['path=/home/test/.config/chromium/Profile/Cookies'],
            },
        }],
        redaction: {applied: true, policyVersion: 1},
    };
    const next = applyDiagnosticsJson(createInitialState(0), 'codex', JSON.stringify(diagnostics), 0);
    assertEqual(next.clientState, 'parse_error');
    assertEqual(next.diagnostics, null);
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
        assertEqual(next.diagnostics, null);
    }
}

function assertProviderUnsafeStringsFailClosed() {
    const ok = readJson('fixtures/snapshots/ok.json');
    ok.providers[0].status.description = 'parse failed: {"rawPayload":"payload"} path=/home/test/.config/chromium/Profile/Cookies';
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

    ok.providers[0].dashboardUrl = null;
    ok.providers[0].status.url = 'https://status.example.com/';
    const statusUrlState = applySnapshotJson(createInitialState(0), JSON.stringify(ok), 0);
    const [statusUrlRow] = normalizeViewState(statusUrlState, {}).providerRows;
    assertEqual(statusUrlRow.dashboardUrl, '');
    assertEqual(statusUrlRow.statusPageUrl, 'https://status.example.com/');

    ok.providers[0].status.url = 'https://status.example.com/rawPayload/secret';
    const unsafeStatusUrlState = applySnapshotJson(createInitialState(0), JSON.stringify(ok), 0);
    const [unsafeStatusUrlRow] = normalizeViewState(unsafeStatusUrlState, {}).providerRows;
    assertEqual(unsafeStatusUrlRow.statusPageUrl, '');
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
        `${https}example.com/dashboard?token=secret`,
        `${https}example.com/dashboard?secret=secret`,
        `${https}example.com/dashboard?code=secret`,
        `${https}example.com/dashboard#key=secret`,
        `${https}example.com/dashboard?redirect=/home/test/.config/chromium/Profile/Cookies`,
        `${https}example.com/dashboard#redirect=${encodeURIComponent('/home/test/.config/chromium/Profile/Cookies')}`,
        `${https}example.com/dashboard?error=${encodeURIComponent('parse failed: {"rawPayload":"payload"}')}`,
        `${https}example.com/rawPayload/secret`,
        `${https}example.com/${encodeURIComponent('{"rawPayload":"payload"}')}/dashboard`,
        `${https}example.com/${encodeURIComponent('/home/test/.config/chromium/Profile/Cookies')}`,
    ])
        assertEqual(safeUrl(url), '');
}

function collectMainViewLabels(view) {
    const labels = [
        view.stateLabel,
        view.stateDescription,
        view.panelLabel,
        view.panelStatus,
        view.headerStatus,
        view.refreshLabel,
        view.titleAction?.label,
        view.footerStatus,
        view.panel?.label,
        view.panel?.status,
        view.selectedRow?.displayName,
        view.selectedRow?.statusLabel,
        view.selectedRow?.statusDescription,
        view.selectedRow?.identity,
        view.selectedRow?.metadataText,
        view.selectedRow?.titleStatusText,
        view.selectedRow?.planLabel,
    ];
    for (const row of view.providerSelectorRows ?? [])
        labels.push(row.label, row.displayName, row.statusLabel);
    for (const section of view.selectedRow?.usageSections ?? [])
        labels.push(section.title, section.meter?.label, section.meter?.detail, section.meter?.resetText);
    for (const row of view.selectedRow?.costRows ?? [])
        labels.push(row.label, row.value);
    return labels.filter(label => typeof label === 'string' && label.length > 0);
}

function readJson(relativePath) {
    return JSON.parse(readText(relativePath));
}

function readText(relativePath) {
    const path = GLib.build_filenamev([GLib.get_current_dir(), relativePath]);
    const file = Gio.File.new_for_path(path);
    const [ok, contents] = file.load_contents(null);
    if (!ok)
        throw new Error(`failed to load ${relativePath}`);
    return new TextDecoder('utf-8').decode(contents);
}

function cloneProvider(provider, overrides) {
    const copy = JSON.parse(JSON.stringify(provider));
    copy.provider = overrides.provider;
    copy.displayName = overrides.displayName;
    copy.updatedAt = '2026-04-27T12:00:00Z';
    copy.identity = {
        providerAccountIdHash: `${overrides.provider}-account-hash`,
        accountEmailDisplay: `${overrides.provider}-masked-account`,
        accountEmailHash: `${overrides.provider}-email-hash`,
        accountOrganizationDisplay: null,
        accountOrganizationHash: null,
        loginMethod: 'api_key',
    };
    copy.usage.primary.usedPercent = overrides.primaryUsed;
    copy.usage.primary.remainingPercent = overrides.primaryRemaining;
    copy.usage.secondary.usedPercent = overrides.secondaryUsed;
    copy.usage.secondary.remainingPercent = overrides.secondaryRemaining;
    return copy;
}

function safeDiagnosticsPayload() {
    return {
        schemaVersion: 1,
        scope: 'provider',
        provider: 'codex',
        generatedAt: '2026-04-27T12:00:00Z',
        events: [
            {
                code: 'fixture_error',
                severity: 'error',
                safeMessage: 'Safe error',
                timestamp: '2026-04-27T12:01:00Z',
                provider: 'codex',
                sourceAdapter: 'fixture',
                recoverable: true,
                details: {attempt: 1, cached: false, note: 'safe'},
                redacted: {applied: true, classes: ['headers']},
            },
        ],
        redaction: {applied: true, policyVersion: 1, notes: ['safe']},
    };
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
