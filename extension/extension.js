import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

export default class CodexBarExtension extends Extension {
    enable() {
        this._enabled = true;
        console.debug('CodexBar GNOME bootstrap extension enabled; Task 03 implements Shell UI.');
    }

    disable() {
        // Task 00 creates no actors, timers, D-Bus proxies, or signal handlers.
        // There are therefore no Shell resources to destroy until Task 03.
        this._enabled = false;
    }
}
