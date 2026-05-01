import St from 'gi://St';

import {
    createDivider,
    createProviderStrip,
    createSelectedProviderSurface,
} from './providerCard.js';

export class CodexbarPopover {
    constructor(actions) {
        this._actions = actions;
        this.actor = new St.BoxLayout({
            vertical: true,
            style_class: 'codexbar-popover',
            x_expand: true,
        });
    }

    destroy() {
        this.actor.destroy();
    }

    update(view, options) {
        this.actor.style_class = `codexbar-popover codexbar-theme-${options.theme}`;
        this.actor.destroy_all_children();
        this.actor.add_child(createProviderStrip(view.providerSelectorRows ?? [], this._actions));
        this.actor.add_child(createDivider());
        this.actor.add_child(createSelectedProviderSurface(view.selectedRow, view, this._actions));
        this.actor.add_child(createDivider());
        this.actor.add_child(this._footer(view));
    }

    _footer(view) {
        const footer = new St.BoxLayout({
            style_class: 'codexbar-footer',
            x_expand: true,
        });
        footer.add_child(new St.Label({
            text: view.footerStatus || 'Daemon status unavailable',
            style_class: 'codexbar-muted codexbar-small',
            x_expand: true,
        }));
        return footer;
    }
}
