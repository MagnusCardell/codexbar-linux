import St from 'gi://St';

import {createProviderMeters} from './meterBars.js';
import {safeDisplay} from './state.js';

export function createProviderCard(row, options, actions) {
    const card = new St.BoxLayout({
        vertical: true,
        style_class: `codexbar-provider-card codexbar-state-${row.state} codexbar-severity-${row.severity}`,
        x_expand: true,
    });

    const header = new St.BoxLayout({
        style_class: 'codexbar-provider-header',
        x_expand: true,
    });
    header.add_child(new St.Label({
        text: row.shortLabel,
        style_class: 'codexbar-provider-glyph',
    }));

    const titleBox = new St.BoxLayout({
        vertical: true,
        x_expand: true,
        style_class: 'codexbar-provider-title-box',
    });
    titleBox.add_child(new St.Label({
        text: row.displayName,
        style_class: 'codexbar-provider-title',
        x_expand: true,
    }));
    const subtitlePieces = [
        row.statusLabel,
        row.identity,
        row.sourceLabel,
    ].filter(Boolean);
    titleBox.add_child(new St.Label({
        text: subtitlePieces.join(' · '),
        style_class: 'codexbar-provider-subtitle',
        x_expand: true,
    }));
    header.add_child(titleBox);
    header.add_child(new St.Label({
        text: row.updatedText,
        style_class: 'codexbar-muted codexbar-small',
    }));
    card.add_child(header);

    card.add_child(new St.Label({
        text: row.statusDescription || 'Status unavailable',
        style_class: 'codexbar-provider-message',
        x_expand: true,
    }));

    card.add_child(createProviderMeters(row.meters, options.resetTimeFormat));

    const detailPieces = [];
    if (row.resetText && row.resetText !== 'No usage data')
        detailPieces.push(row.resetText);
    if (row.diagnosticsSummary)
        detailPieces.push(row.diagnosticsSummary);
    if (row.adapterLabel)
        detailPieces.push(`Adapter: ${row.adapterLabel}`);
    card.add_child(new St.Label({
        text: detailPieces.map(safeDisplay).filter(Boolean).join(' · ') || 'Details unavailable',
        style_class: 'codexbar-muted codexbar-small',
        x_expand: true,
    }));

    const buttons = new St.BoxLayout({
        style_class: 'codexbar-card-actions',
    });
    if (row.dashboardUrl) {
        buttons.add_child(actionButton('Open', () => actions.openDashboard(row.dashboardUrl)));
    }
    buttons.add_child(actionButton('Diagnostics', () => actions.loadDiagnostics(row.providerId)));
    card.add_child(buttons);

    return card;
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
