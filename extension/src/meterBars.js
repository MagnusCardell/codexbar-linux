import Clutter from 'gi://Clutter';
import St from 'gi://St';

import {
    meterFillFractionFromPercent,
    meterClassNames,
    meterFillClassNames,
    meterTone,
    meterRemainingPercent,
    safeMeterTone,
    safeDisplay,
} from './state.js';

const PANEL_METER_WIDTH = 30;
const PANEL_METER_HEIGHT = 3;
const PROVIDER_METER_WIDTH = 330;
const PROVIDER_METER_HEIGHT = 6;

export function createMicroMeterStack(meters) {
    const rows = Array.isArray(meters) ? meters : [];

    const box = new St.BoxLayout({
        vertical: true,
        style_class: 'codexbar-panel-meter-stack',
        x_align: Clutter.ActorAlign.CENTER,
        y_align: Clutter.ActorAlign.CENTER,
    });

    for (const meter of rows.slice(0, 2)) {
        box.add_child(createContinuousMeter(
            meterVisualPercent(meter),
            meterVisualTone(meter),
            {compact: true}
        ));
    }

    while (box.get_n_children() < 2)
        box.add_child(createContinuousMeter(null, 'unknown', {compact: true}));

    return box;
}

function meterVisualPercent(meter) {
    if (Number.isFinite(meter?.fillPercent))
        return meter.fillPercent;
    if (Number.isFinite(meter?.remainingPercent))
        return meter.remainingPercent;
    if (Number.isFinite(meter?.usedPercent))
        return 100 - meter.usedPercent;
    return null;
}

function meterVisualTone(meter) {
    if (meter?.tone)
        return meter.tone;
    return meterTone(meter);
}

export function createProviderMeters(meterRows, {emptyText = 'Usage unavailable', limit = 4} = {}) {
    const rows = Array.isArray(meterRows) ? meterRows : [];
    const box = new St.BoxLayout({
        vertical: true,
        style_class: 'codexbar-provider-meters',
        x_expand: true,
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
            x_expand: true,
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
    const height = compact ? PANEL_METER_HEIGHT : PROVIDER_METER_HEIGHT;
    const safeTone = safeMeterTone(tone);

    if (!compact)
        return createResponsiveMeter(fillPercent, safeTone, width, height);

    // Compact: pixel-precise fixed-width approach for the panel bar
    const fillWidth = fillWidthForPercent(fillPercent, width);
    const restWidth = Math.max(0, width - fillWidth);

    const track = new St.BoxLayout({
        vertical: false,
        style_class: meterClassNames(safeTone, {compact}),
        x_expand: false,
        x_align: Clutter.ActorAlign.CENTER,
        y_align: Clutter.ActorAlign.CENTER,
        style: [
            `width: ${width}px`,
            `min-width: ${width}px`,
            `max-width: ${width}px`,
            `height: ${height}px`,
            `min-height: ${height}px`,
            `max-height: ${height}px`,
        ].join('; '),
    });

    track.set_width(width);
    track.set_height(height);

    if (fillWidth > 0) {
        const fill = new St.Widget({
            style_class: meterFillClassNames(safeTone),
            style: [
                `width: ${fillWidth}px`,
                `min-width: ${fillWidth}px`,
                `max-width: ${fillWidth}px`,
                `height: ${height}px`,
                `min-height: ${height}px`,
                `max-height: ${height}px`,
                `background-color: ${meterColor(safeTone)}`,
            ].join('; '),
        });
        fill.set_width(fillWidth);
        fill.set_height(height);
        track.add_child(fill);
    }

    if (restWidth > 0) {
        const rest = new St.Widget({
            style: [
                `width: ${restWidth}px`,
                `min-width: ${restWidth}px`,
                `max-width: ${restWidth}px`,
                `height: ${height}px`,
                `min-height: ${height}px`,
                `max-height: ${height}px`,
                'background-color: transparent',
            ].join('; '),
        });
        rest.set_width(restWidth);
        rest.set_height(height);
        track.add_child(rest);
    }

    return track;
}

function createResponsiveMeter(fillPercent, safeTone, minWidth, height) {
    const fraction = meterFillFractionFromPercent(fillPercent);
    const fillPct = fraction !== null ? Math.max(0, Math.min(100, Math.round(fraction * 100))) : 0;
    const fillClr = meterColor(safeTone);
    const emptyClr = 'rgba(255, 255, 255, 0.12)';

    let bgStyle = '';
    if (fillPct >= 100)
        bgStyle = `background-color: ${fillClr};`;
    else if (fillPct > 0)
        bgStyle = `background-image: linear-gradient(to right, ${fillClr} ${fillPct}%, ${emptyClr} ${fillPct}%); background-color: ${emptyClr};`;

    const track = new St.Widget({
        style_class: meterClassNames(safeTone, {compact: false}),
        x_expand: true,
        x_align: Clutter.ActorAlign.CENTER,
        y_align: Clutter.ActorAlign.CENTER,
        style: `min-width: ${minWidth}px; height: ${height}px; min-height: ${height}px; max-height: ${height}px; ${bgStyle}`,
    });
    track.set_height(height);
    return track;
}

function fillWidthForPercent(value, width) {
    const fraction = meterFillFractionFromPercent(value);
    if (fraction === null || fraction <= 0)
        return 0;
    return Math.max(1, Math.round(fraction * width));
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
