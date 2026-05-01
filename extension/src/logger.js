import {redactText} from './state.js';

const PREFIX = 'CodexBar';

export function debug(message) {
    console.debug(`${PREFIX}: ${redactText(message)}`);
}

export function warn(message) {
    console.warn(`${PREFIX}: ${redactText(message)}`);
}

export function error(message) {
    console.error(`${PREFIX}: ${redactText(message)}`);
}
