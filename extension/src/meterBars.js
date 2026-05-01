import St from 'gi://St';

import {
    formatMeterDetail,
    meterTone,
    meterUsedPercent,
    safeDisplay,
} from './state.js';

const SEGMENTS = 10;

export function createMicroMeterStack(meters) {
    const box = new St.BoxLayout({
        vertical: true,
        style_class: 'codexbar-panel-meter-stack',
    });

    for (const meter of meters.slice(0, 2))
        box.add_child(createSegmentMeter(meter, {compact: true}));

    while (box.get_n_children() < 2)
        box.add_child(createSegmentMeter(null, {compact: true}));

    return box;
}

export function createProviderMeters(meters, resetTimeFormat) {
    const box = new St.BoxLayout({
        vertical: true,
        style_class: 'codexbar-provider-meters',
    });

    if (meters.length === 0) {
        box.add_child(new St.Label({
            text: 'No usage data',
            style_class: 'codexbar-muted codexbar-small',
        }));
        return box;
    }

    for (const meter of meters.slice(0, 4)) {
        const row = new St.BoxLayout({
            vertical: true,
            style_class: 'codexbar-meter-row',
        });
        row.add_child(new St.Label({
            text: safeDisplay(meter.label || meter.meterKey || 'Usage'),
            style_class: 'codexbar-meter-label',
            x_expand: true,
        }));
        row.add_child(createSegmentMeter(meter));
        row.add_child(new St.Label({
            text: formatMeterDetail(meter, resetTimeFormat),
            style_class: 'codexbar-muted codexbar-small',
            x_expand: true,
        }));
        box.add_child(row);
    }

    return box;
}

export function createSegmentMeter(meter, {compact = false} = {}) {
    const used = meterUsedPercent(meter);
    const activeSegments = used === null ? 0 : Math.round((used / 100) * SEGMENTS);
    const tone = meterTone(meter);
    const box = new St.BoxLayout({
        style_class: compact
            ? `codexbar-meter codexbar-meter-compact codexbar-meter-${tone}`
            : `codexbar-meter codexbar-meter-${tone}`,
        x_expand: !compact,
    });

    for (let index = 0; index < SEGMENTS; index++) {
        box.add_child(new St.Widget({
            style_class: index < activeSegments
                ? 'codexbar-meter-segment codexbar-meter-segment-active'
                : 'codexbar-meter-segment',
            x_expand: !compact,
        }));
    }

    return box;
}
