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
        row.identity,
        row.sourceLabel ? `${row.sourceLabel} source` : '',
    ].filter(Boolean);
    titleBox.add_child(new St.Label({
        text: subtitlePieces.join(' · '),
        style_class: 'codexbar-provider-subtitle',
        x_expand: true,
    }));
    header.add_child(titleBox);

    const statusBox = new St.BoxLayout({
        vertical: true,
        style_class: 'codexbar-provider-status-box',
    });
    statusBox.add_child(new St.Label({
        text: row.statusLabel,
        style_class: `codexbar-state-pill codexbar-state-pill-${row.severity}`,
    }));
    statusBox.add_child(new St.Label({
        text: row.updatedText,
        style_class: 'codexbar-muted codexbar-small codexbar-provider-updated',
    }));
    header.add_child(statusBox);
    card.add_child(header);

    if (row.statusDescription) {
        card.add_child(new St.Label({
            text: row.statusDescription,
            style_class: 'codexbar-provider-message',
            x_expand: true,
        }));
    }

    card.add_child(createProviderMeters(row.meters, options.resetTimeFormat));

    const detailPieces = [];
    if (row.adapterLabel && row.adapterLabel !== 'None')
        detailPieces.push(`Via ${row.adapterLabel}`);
    if (detailPieces.length > 0) {
        card.add_child(new St.Label({
            text: detailPieces.map(safeDisplay).filter(Boolean).join(' · '),
            style_class: 'codexbar-muted codexbar-small',
            x_expand: true,
        }));
    }

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
