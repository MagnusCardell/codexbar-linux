import St from 'gi://St';

import {diagnosticsSummaryLine} from './state.js';

export function createDiagnosticsView(diagnostics, actions, providerId = 'global') {
    return createDiagnosticsActionRow(diagnostics, actions, providerId);
}

export function createDiagnosticsActionRow(diagnostics, actions, providerId = 'global') {
    const diagnosticsProviderId = providerId || 'global';
    const activeDiagnostics = activeDiagnosticsForProvider(diagnostics, diagnosticsProviderId);
    const box = new St.BoxLayout({
        vertical: true,
        style_class: `codexbar-diagnostics-row${activeDiagnostics?.payload ? ' codexbar-diagnostics-row-loaded' : ' codexbar-diagnostics-row-collapsed'}`,
        x_expand: true,
    });

    const header = new St.BoxLayout({
        style_class: 'codexbar-section-header',
        x_expand: true,
    });
    header.add_child(new St.Label({
        text: 'Diagnostics',
        style_class: 'codexbar-diagnostics-title',
        x_expand: true,
    }));
    if (activeDiagnostics?.payload) {
        header.add_child(actionButton('Copy diagnostics', () => actions.copyDiagnostics(activeDiagnostics.payload)));
    } else {
        header.add_child(actionButton('Load diagnostics', () => actions.loadDiagnostics(diagnosticsProviderId)));
    }
    box.add_child(header);

    if (activeDiagnostics?.payload) {
        box.add_child(new St.Label({
            text: activeDiagnostics?.summary ?? diagnosticsSummaryLine(activeDiagnostics.payload),
            style_class: 'codexbar-diagnostic-line',
            x_expand: true,
        }));

        const lines = new St.BoxLayout({
            vertical: true,
            style_class: 'codexbar-diagnostic-detail-list',
            x_expand: true,
        });
        for (const line of (activeDiagnostics.lines ?? []).slice(0, 4)) {
            lines.add_child(new St.Label({
                text: line,
                style_class: 'codexbar-diagnostic-detail',
                x_expand: true,
            }));
        }
        box.add_child(lines);
    }

    return box;
}

export function createDiagnosticsButton(diagnostics, actions, providerId = 'global') {
    const diagnosticsProviderId = providerId || 'global';
    return actionButton('Load diagnostics', () => actions.loadDiagnostics(diagnosticsProviderId), {utility: true});
}

export function createDiagnosticsCopyButton(diagnostics, actions, providerId = 'global') {
    const activeDiagnostics = activeDiagnosticsForProvider(diagnostics, providerId);
    if (!activeDiagnostics?.payload)
        return null;
    return actionButton('Copy diagnostics', () => actions.copyDiagnostics(activeDiagnostics.payload), {utility: true});
}

export function createDiagnosticsDetails(diagnostics, providerId = 'global') {
    const activeDiagnostics = activeDiagnosticsForProvider(diagnostics, providerId);
    if (!activeDiagnostics?.payload)
        return null;

    const box = new St.BoxLayout({
        vertical: true,
        style_class: 'codexbar-diagnostics-detail',
        x_expand: true,
    });
    box.add_child(new St.Label({
        text: activeDiagnostics?.summary ?? diagnosticsSummaryLine(activeDiagnostics.payload),
        style_class: 'codexbar-diagnostic-line',
        x_expand: true,
    }));

    const lines = new St.BoxLayout({
        vertical: true,
        style_class: 'codexbar-diagnostic-detail-list',
        x_expand: true,
    });
    for (const line of (activeDiagnostics.lines ?? []).slice(0, 4)) {
        lines.add_child(new St.Label({
            text: line,
            style_class: 'codexbar-diagnostic-detail',
            x_expand: true,
        }));
    }
    box.add_child(lines);
    return box;
}

function activeDiagnosticsForProvider(diagnostics, providerId = 'global') {
    const diagnosticsProviderId = providerId || 'global';
    return diagnostics?.providerId === diagnosticsProviderId ? diagnostics : null;
}

function actionButton(label, callback, {utility = false} = {}) {
    const button = new St.Button({
        label,
        style_class: `codexbar-button codexbar-button-secondary${utility ? ' codexbar-button-utility' : ''}`,
        can_focus: true,
        reactive: true,
        track_hover: true,
    });
    button.connect('clicked', () => callback());
    return button;
}
