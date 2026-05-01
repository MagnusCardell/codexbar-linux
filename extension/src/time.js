import {UNKNOWN_TEXT} from './constants.js';

const MINUTE_SECONDS = 60;
const HOUR_SECONDS = 60 * MINUTE_SECONDS;
const DAY_SECONDS = 24 * HOUR_SECONDS;

export function parseDate(value) {
    if (!value || typeof value !== 'string')
        return null;
    const date = new Date(value);
    if (Number.isNaN(date.getTime()))
        return null;
    return date;
}

export function formatAbsolute(value) {
    const date = parseDate(value);
    if (!date)
        return UNKNOWN_TEXT;
    return date.toLocaleString(undefined, {
        month: 'short',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit',
    });
}

export function formatRelative(value, now = new Date()) {
    const date = parseDate(value);
    if (!date)
        return UNKNOWN_TEXT;

    const seconds = Math.round((date.getTime() - now.getTime()) / 1000);
    const prefix = seconds >= 0 ? 'in ' : '';
    const suffix = seconds < 0 ? ' ago' : '';
    const abs = Math.abs(seconds);

    if (abs < MINUTE_SECONDS)
        return `${prefix}<1m${suffix}`;
    if (abs < HOUR_SECONDS)
        return `${prefix}${Math.round(abs / MINUTE_SECONDS)}m${suffix}`;
    if (abs < DAY_SECONDS)
        return `${prefix}${Math.round(abs / HOUR_SECONDS)}h${suffix}`;
    return `${prefix}${Math.round(abs / DAY_SECONDS)}d${suffix}`;
}

export function formatReset(value, mode = 'countdown', now = new Date()) {
    const relative = formatRelative(value, now);
    const absolute = formatAbsolute(value);
    if (mode === 'absolute')
        return absolute;
    if (mode === 'both')
        return `${relative} · ${absolute}`;
    return relative;
}

export function formatUpdated(value, now = new Date()) {
    if (!value)
        return 'Updated unknown';
    return `Updated ${formatRelative(value, now)}`;
}

export function formatGenerated(value, now = new Date()) {
    if (!value)
        return 'Snapshot time unknown';
    return `Snapshot ${formatRelative(value, now)}`;
}
