import St from 'gi://St';

import {PRODUCT_NAME} from './constants.js';
import {
    createDiagnosticsButton,
    createDiagnosticsCopyButton,
} from './diagnosticsView.js';
import {createMeter, createProviderMeters} from './meterBars.js';
import {safeDisplay} from './state.js';

const MAX_POPOVER_PROVIDER_ITEMS = 4;

export function createProviderStrip(rows, actions) {
    const providerRows = Array.isArray(rows) ? rows : [];
    const strip = new St.BoxLayout({
        style_class: 'codexbar-provider-strip',
        x_expand: true,
    });

    if (providerRows.length === 0) {
        strip.add_child(new St.Label({
            text: 'Waiting for providers',
            style_class: 'codexbar-provider-strip-empty codexbar-muted',
            x_expand: true,
        }));
        return strip;
    }

    const visibleRows = providerRows.slice(0, MAX_POPOVER_PROVIDER_ITEMS);
    for (const row of visibleRows)
        strip.add_child(createProviderStripItem(row, actions));

    const overflow = providerRows.length - visibleRows.length;
    if (overflow > 0) {
        strip.add_child(new St.Label({
            text: `+${overflow}`,
            style_class: 'codexbar-provider-strip-overflow codexbar-muted',
        }));
    }

    return strip;
}

export function createSelectedProviderSurface(row, view, actions) {
    const surface = new St.BoxLayout({
        vertical: true,
        style_class: `codexbar-selected-provider codexbar-state-${view.state} codexbar-severity-${row?.severity ?? 'loading'}`,
        x_expand: true,
    });

    surface.add_child(createSelectedProviderTitle(row, view, actions));
    surface.add_child(createDivider());
    surface.add_child(createUsageSections(row));

    const costRows = Array.isArray(row?.costRows) ? row.costRows : [];
    if (costRows.length > 0) {
        surface.add_child(createDivider());
        surface.add_child(createCostSection(costRows));
    }

    return surface;
}

export function createDivider() {
    return new St.Widget({
        style_class: 'codexbar-divider',
        x_expand: true,
    });
}

function createProviderStripItem(row, actions) {
    const styleClasses = [
        'codexbar-provider-strip-item',
        `codexbar-state-${row.state}`,
        `codexbar-severity-${row.severity}`,
        row.selected ? 'codexbar-provider-strip-item-selected' : '',
        row.dimmed ? 'codexbar-provider-strip-item-dimmed' : '',
    ].filter(Boolean).join(' ');
    const button = new St.Button({
        style_class: styleClasses,
        can_focus: true,
        reactive: true,
        track_hover: true,
    });
    button.accessible_name = `${row.displayName}: ${row.statusLabel}`;

    const content = new St.BoxLayout({
        vertical: true,
        style_class: 'codexbar-provider-strip-item-content',
        x_expand: true,
    });
    content.add_child(new St.Label({
        text: safeDisplay(row.label || row.displayName || 'Provider'),
        style_class: 'codexbar-provider-strip-label',
        x_expand: true,
    }));
    content.add_child(createStripMeter(row.meter, row.severity));
    button.set_child(content);

    button.connect('clicked', () => actions.selectProvider(row.providerId));
    return button;
}

function createStripMeter(meter, severity) {
    return createMeter(meter, {
        compact: true,
        strip: true,
        fallbackTone: severityTone(severity),
    });
}

function createSelectedProviderTitle(row, view, actions) {
    const box = new St.BoxLayout({
        vertical: true,
        style_class: 'codexbar-selected-provider-title',
        x_expand: true,
    });
    const heading = new St.BoxLayout({
        style_class: 'codexbar-selected-provider-heading',
        x_expand: true,
    });
    heading.add_child(new St.Label({
        text: row?.displayName || PRODUCT_NAME,
        style_class: 'codexbar-provider-name',
        x_expand: true,
    }));
    if (row?.planLabel) {
        heading.add_child(new St.Label({
            text: row.planLabel,
            style_class: 'codexbar-provider-plan',
        }));
    }
    heading.add_child(refreshButton(view, actions));
    box.add_child(heading);

    box.add_child(new St.Label({
        text: selectedProviderSubtitle(row, view),
        style_class: 'codexbar-provider-subtitle',
        x_expand: true,
    }));

    if (row?.statusDescription && row.state !== 'ok') {
        box.add_child(new St.Label({
            text: row.statusDescription,
            style_class: 'codexbar-provider-message codexbar-provider-state-note',
            x_expand: true,
        }));
    }

    return box;
}

function createUsageSections(row) {
    const usageSections = Array.isArray(row?.usageSections) ? row.usageSections : [];
    const section = new St.BoxLayout({
        vertical: true,
        style_class: 'codexbar-usage-sections',
        x_expand: true,
    });

    if (usageSections.length === 0) {
        section.add_child(createProviderMeters([], {emptyText: 'Usage unavailable', limit: 1}));
        return section;
    }

    if (row?.availabilityMeter) {
        section.add_child(createUsageMeterSection({
            title: 'Availability',
            meter: row.availabilityMeter,
        }));
        section.add_child(createUsageDetailRows(usageSections));
        return section;
    }

    for (const usageSection of usageSections)
        section.add_child(createUsageMeterSection(usageSection));

    return section;
}

function createUsageMeterSection(usageSection) {
    const box = new St.BoxLayout({
        vertical: true,
        style_class: 'codexbar-usage-section',
        x_expand: true,
    });
    box.add_child(createProviderMeters([usageSection.meter], {
        emptyText: `${usageSection.title || 'Usage'} unavailable`,
        limit: 1,
    }));
    return box;
}

function createUsageDetailRows(usageSections) {
    const rows = new St.BoxLayout({
        vertical: true,
        style_class: 'codexbar-usage-detail-list',
        x_expand: true,
    });

    for (const usageSection of usageSections) {
        const meter = usageSection?.meter ?? null;
        const row = new St.BoxLayout({
            style_class: 'codexbar-usage-detail-row',
            x_expand: true,
        });
        row.add_child(new St.Label({
            text: safeDisplay(usageSection?.title || meter?.label || 'Usage'),
            style_class: 'codexbar-usage-detail-label',
            x_expand: true,
        }));
        row.add_child(new St.Label({
            text: usageDetailText(meter),
            style_class: 'codexbar-usage-detail-value',
        }));
        rows.add_child(row);
    }

    return rows;
}

function usageDetailText(meter) {
    return [meter?.detail, meter?.resetText]
        .filter(text => typeof text === 'string' && text.length > 0)
        .map(text => safeDisplay(text))
        .join(' · ') || 'Usage unavailable';
}

function createCostSection(costRows) {
    const box = new St.BoxLayout({
        vertical: true,
        style_class: 'codexbar-cost-section',
        x_expand: true,
    });
    box.add_child(sectionTitle('Cost'));
    for (const row of costRows) {
        const line = new St.BoxLayout({
            style_class: 'codexbar-cost-row',
            x_expand: true,
        });
        line.add_child(new St.Label({
            text: safeDisplay(row.label || 'Cost'),
            style_class: 'codexbar-cost-label',
            x_expand: true,
        }));
        line.add_child(new St.Label({
            text: safeDisplay(row.value || 'Unavailable'),
            style_class: 'codexbar-cost-value',
        }));
        box.add_child(line);
    }
    return box;
}

function sectionTitle(text) {
    const header = new St.BoxLayout({
        style_class: 'codexbar-section-header',
        x_expand: true,
    });
    header.add_child(new St.Label({
        text,
        style_class: 'codexbar-diagnostics-title',
        x_expand: true,
    }));
    return header;
}

export function createSecondaryActionRow(view, actions) {
    const row = new St.BoxLayout({
        style_class: 'codexbar-utility-action-row',
        x_expand: true,
    });
    const buttons = [
        createDiagnosticsButton(view.diagnostics, actions, view.selectedProviderId),
    ];
    const copyButton = createDiagnosticsCopyButton(view.diagnostics, actions, view.selectedProviderId);
    if (copyButton)
        buttons.push(copyButton);
    if (actions?.canOpenSettings)
        buttons.push(actionButton('Settings', () => actions.openSettings(), {utility: true}));

    for (const button of buttons) {
        if (row.get_n_children() > 0)
            row.add_child(utilitySeparator());
        row.add_child(button);
    }

    return row;
}

function utilitySeparator() {
    return new St.Label({
        text: '·',
        style_class: 'codexbar-utility-separator',
    });
}

function actionButton(label, callback, {primary = false, reactive = true, utility = false} = {}) {
    const button = new St.Button({
        label,
        style_class: `codexbar-button ${primary ? 'codexbar-button-primary' : 'codexbar-button-secondary'}${utility ? ' codexbar-button-utility' : ''}`,
        can_focus: true,
        reactive,
        track_hover: true,
    });
    if (reactive)
        button.connect('clicked', () => callback());
    return button;
}

function refreshButton(view, actions) {
    return actionButton(
        view.titleAction?.label ?? view.refreshLabel ?? (view.refreshing ? 'Refreshing' : 'Refresh'),
        () => {
            if (view.titleAction?.action === 'retryDaemon')
                actions.retryDaemon();
            else
                actions.refresh();
        },
        {
            primary: true,
            reactive: view.titleAction?.reactive ?? !view.refreshing,
        },
    );
}

function selectedProviderSubtitle(row, view) {
    if (!row)
        return view.headerStatus || view.stateDescription || 'Waiting for usage data';

    return [
        row.titleStatusText,
        row.identity,
    ].filter(Boolean).join(' · ') || row.resetText || view.headerStatus || 'Usage state unavailable';
}

function severityTone(severity) {
    if (severity === 'error')
        return 'danger';
    if (severity === 'warning' || severity === 'loading')
        return 'warning';
    if (severity === 'ok')
        return 'ok';
    return 'unknown';
}
