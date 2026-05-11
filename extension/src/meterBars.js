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

const PANEL_METER_WIDTH = 54;
const PANEL_METER_HEIGHT = 5;
const STRIP_METER_WIDTH = 64;
const STRIP_METER_HEIGHT = 5;
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

    for (const meter of rows.slice(0, 1))
        box.add_child(createMeter(meter, {compact: true}));

    while (box.get_n_children() < 1)
        box.add_child(createMeter(null, {compact: true}));

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
        row.add_child(createMeter(meter));

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

export function createContinuousMeter(fillPercent, tone = 'unknown', {compact = false, strip = false} = {}) {
    const {width, height} = meterDimensions({compact, strip});
    const safeTone = safeMeterTone(tone);
    const fillWidth = fillWidthForPercent(fillPercent, width);
    const restWidth = Math.max(0, width - fillWidth);

    const track = new St.BoxLayout({
        vertical: false,
        style_class: meterClassNames(safeTone, {compact}),
        x_expand: !compact,
        x_align: Clutter.ActorAlign.CENTER,
        y_align: Clutter.ActorAlign.CENTER,
        style: fixedSizeStyle(width, height),
    });

    track.set_width(width);
    track.set_height(height);

    const fill = new St.Widget({
        style_class: meterFillClassNames(safeTone),
        style: [
            fixedSizeStyle(fillWidth, height),
            `background-color: ${meterColor(safeTone)}`,
        ].join('; '),
    });
    fill.set_width(fillWidth);
    fill.set_height(height);
    track.add_child(fill);

    const rest = new St.Widget({
        style: [
            fixedSizeStyle(restWidth, height),
            'background-color: transparent',
        ].join('; '),
    });
    rest.set_width(restWidth);
    rest.set_height(height);
    track.add_child(rest);

    return track;
}

export function createMeter(meter, {compact = false, strip = false, fallbackTone = 'unknown'} = {}) {
    if (meter?.kind === 'composite_availability')
        return createCompositeAvailabilityMeter(meter, {compact, strip});
    return createContinuousMeter(
        meterVisualPercent(meter),
        meter ? meterVisualTone(meter) : fallbackTone,
        {compact, strip},
    );
}

function createCompositeAvailabilityMeter(meter, {compact = false, strip = false} = {}) {
    const {width, height} = meterDimensions({compact, strip});
    const safeTone = safeMeterTone(meter?.tone);
    const weeklyWidth = fillWidthForPercent(meter?.weeklyEnvelopePercent, width);
    const effectiveWidth = Math.min(weeklyWidth, fillWidthForPercent(meter?.effectivePercent, width));
    const weeklyRestWidth = Math.max(0, weeklyWidth - effectiveWidth);
    const outerRestWidth = Math.max(0, width - weeklyWidth);

    const track = new St.BoxLayout({
        vertical: false,
        style_class: `${meterClassNames(safeTone, {compact})} codexbar-meter-composite`,
        x_expand: !compact && !strip,
        x_align: Clutter.ActorAlign.CENTER,
        y_align: Clutter.ActorAlign.CENTER,
        style: fixedSizeStyle(width, height),
    });
    track.set_width(width);
    track.set_height(height);

    const weeklyEnvelope = new St.BoxLayout({
        vertical: false,
        style_class: 'codexbar-meter-weekly-envelope',
        style: [
            fixedSizeStyle(weeklyWidth, height),
            `background-color: ${weeklyEnvelopeColor()}`,
        ].join('; '),
    });
    weeklyEnvelope.set_width(weeklyWidth);
    weeklyEnvelope.set_height(height);

    const effectiveFill = new St.Widget({
        style_class: `${meterFillClassNames(safeTone)} codexbar-meter-effective-fill`,
        style: [
            fixedSizeStyle(effectiveWidth, height),
            `background-color: ${meterColor(safeTone)}`,
        ].join('; '),
    });
    effectiveFill.set_width(effectiveWidth);
    effectiveFill.set_height(height);
    weeklyEnvelope.add_child(effectiveFill);

    const weeklyRest = new St.Widget({
        style_class: 'codexbar-meter-weekly-rest',
        style: [
            fixedSizeStyle(weeklyRestWidth, height),
            'background-color: transparent',
        ].join('; '),
    });
    weeklyRest.set_width(weeklyRestWidth);
    weeklyRest.set_height(height);
    weeklyEnvelope.add_child(weeklyRest);
    track.add_child(weeklyEnvelope);

    const outerRest = new St.Widget({
        style: [
            fixedSizeStyle(outerRestWidth, height),
            'background-color: transparent',
        ].join('; '),
    });
    outerRest.set_width(outerRestWidth);
    outerRest.set_height(height);
    track.add_child(outerRest);

    return track;
}

function meterDimensions({compact = false, strip = false} = {}) {
    if (strip)
        return {width: STRIP_METER_WIDTH, height: STRIP_METER_HEIGHT};
    if (compact)
        return {width: PANEL_METER_WIDTH, height: PANEL_METER_HEIGHT};
    return {width: PROVIDER_METER_WIDTH, height: PROVIDER_METER_HEIGHT};
}

function fillWidthForPercent(value, width) {
    const fraction = meterFillFractionFromPercent(value);
    if (fraction === null || fraction <= 0)
        return 0;
    return Math.max(1, Math.round(fraction * width));
}

function fixedSizeStyle(width, height) {
    return [
        `width: ${width}px`,
        `min-width: ${width}px`,
        `max-width: ${width}px`,
        `height: ${height}px`,
        `min-height: ${height}px`,
        `max-height: ${height}px`,
    ].join('; ');
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

function weeklyEnvelopeColor() {
    return 'rgba(122, 135, 148, 0.62)';
}
