import St from 'gi://St';

import {diagnosticsSummaryLine} from './state.js';

export function createDiagnosticsView(diagnostics, actions) {
    const box = new St.BoxLayout({
        vertical: true,
        style_class: `codexbar-diagnostics${diagnostics?.payload ? ' codexbar-diagnostics-loaded' : ' codexbar-diagnostics-collapsed'}`,
        x_expand: true,
    });

    const header = new St.BoxLayout({
        style_class: 'codexbar-section-header',
        x_expand: true,
    });
    header.add_child(new St.Label({
        text: 'Diagnostics',
        style_class: 'codexbar-section-title',
        x_expand: true,
    }));
    if (diagnostics?.payload) {
        header.add_child(actionButton('Copy', () => actions.copyDiagnostics(diagnostics.payload)));
    }
    box.add_child(header);

    box.add_child(new St.Label({
        text: diagnostics?.summary
            ?? (diagnostics?.payload ? diagnosticsSummaryLine(diagnostics.payload) : 'Diagnostics not loaded'),
        style_class: 'codexbar-diagnostic-line',
        x_expand: true,
    }));

    return box;
}

function actionButton(label, callback) {
    const button = new St.Button({
        label,
        style_class: 'codexbar-button codexbar-button-secondary',
        can_focus: true,
        reactive: true,
        track_hover: true,
    });
    button.connect('clicked', () => callback());
    return button;
}
