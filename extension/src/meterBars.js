import St from 'gi://St';

import {
    meterTone,
    meterRemainingPercent,
    safeDisplay,
} from './state.js';

const PANEL_METER_WIDTH = 30;
const PROVIDER_METER_WIDTH = 330;

export function createMicroMeterStack(meters) {
    const box = new St.BoxLayout({
        vertical: true,
        style_class: 'codexbar-panel-meter-stack',
    });

    for (const meter of meters.slice(0, 2))
        box.add_child(createContinuousMeter(meterRemainingPercent(meter), meterTone(meter), {compact: true}));

    while (box.get_n_children() < 2)
        box.add_child(createContinuousMeter(null, 'unknown', {compact: true}));

    return box;
}

export function createProviderMeters(meterRows, {emptyText = 'Usage unavailable', limit = 4} = {}) {
    const rows = Array.isArray(meterRows) ? meterRows : [];
    const box = new St.BoxLayout({
        vertical: true,
        style_class: 'codexbar-provider-meters',
    });

    if (rows.length === 0) {
        box.add_child(new St.Label({
            text: emptyText,
            style_class: 'codexbar-muted codexbar-small',
        }));
        return box;
    }

    for (const meter of rows.slice(0, limit)) {
        const row = new St.BoxLayout({
            vertical: true,
            style_class: 'codexbar-meter-row',
        });

        row.add_child(new St.Label({
            text: safeDisplay(meter.label || 'Usage'),
            style_class: 'codexbar-meter-label',
            x_expand: true,
        }));
        row.add_child(createContinuousMeter(remainingVisualPercent(meter), meter.tone));

        const detail = new St.BoxLayout({
            style_class: 'codexbar-meter-detail-row',
            x_expand: true,
        });
        detail.add_child(new St.Label({
            text: meter.detail || 'Usage unavailable',
            style_class: 'codexbar-muted codexbar-small codexbar-meter-detail-left',
            x_expand: true,
        }));
        if (meter.resetText) {
            detail.add_child(new St.Label({
                text: meter.resetText,
                style_class: 'codexbar-muted codexbar-small codexbar-meter-detail-right',
            }));
        }
        row.add_child(detail);
        box.add_child(row);
    }

    return box;
}

export function createContinuousMeter(fillPercent, tone = 'unknown', {compact = false} = {}) {
    const width = compact ? PANEL_METER_WIDTH : PROVIDER_METER_WIDTH;
    const normalized = normalizedPercent(fillPercent);
    const fillWidth = normalized === null ? 0 : Math.round((normalized / 100) * width);
    const safeTone = ['ok', 'warning', 'danger', 'unknown'].includes(tone) ? tone : 'unknown';
    const box = new St.BoxLayout({
        style_class: compact
            ? `codexbar-meter codexbar-meter-compact codexbar-meter-${safeTone}`
            : `codexbar-meter codexbar-meter-${safeTone}`,
        x_expand: !compact,
        style: `width: ${width}px;`,
    });

    box.add_child(new St.Widget({
        style_class: `codexbar-meter-fill codexbar-meter-fill-${safeTone}`,
        style: `width: ${fillWidth}px; background-color: ${meterColor(safeTone)};`,
    }));

    return box;
}

function normalizedPercent(value) {
    if (!Number.isFinite(value))
        return null;
    return Math.max(0, Math.min(100, value));
}

function remainingVisualPercent(meter) {
    if (Number.isFinite(meter?.fillPercent))
        return meter.fillPercent;
    if (Number.isFinite(meter?.remainingPercent))
        return meter.remainingPercent;
    if (Number.isFinite(meter?.usedPercent))
        return 100 - meter.usedPercent;
    return null;
}

function meterColor(tone) {
    if (tone === 'danger')
        return '#d75f5f';
    if (tone === 'warning')
        return '#c9a227';
    if (tone === 'ok')
        return '#57c785';
    return '#7a8794';
}
