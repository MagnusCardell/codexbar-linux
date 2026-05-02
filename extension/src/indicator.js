import Clutter from 'gi://Clutter';
import GObject from 'gi://GObject';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

import {
    EXTENSION_STATUS_AREA_NAME,
    PRODUCT_NAME,
} from './constants.js';
import {createMicroMeterStack} from './meterBars.js';
import {CodexbarPopover} from './popover.js';
import {
    panelAccessibleName,
    panelButtonClassNames,
    panelContentClassNames,
    panelProviderItemClassNames,
} from './state.js';

export const CodexbarIndicator = GObject.registerClass(
class CodexbarIndicator extends PanelMenu.Button {
    _init(actions) {
        super._init(0.0, PRODUCT_NAME, false);
        this._actions = actions;
        this._content = new St.BoxLayout({
            style_class: 'codexbar-panel-content',
            x_align: Clutter.ActorAlign.CENTER,
            y_align: Clutter.ActorAlign.CENTER,
        });
        this.add_child(this._content);

        this._popover = new CodexbarPopover(actions);
        const item = new PopupMenu.PopupBaseMenuItem({
            reactive: false,
            can_focus: false,
            style_class: 'codexbar-menu-item',
        });
        item.add_child(this._popover.actor);
        this.menu.addMenuItem(item);

        this._menuOpen = false;
        this._menuOpenSignal = this.menu.connect('open-state-changed', (_menu, open) => {
            this._menuOpen = open;
            this._syncPanelStyle();
        });
    }

    addToPanel() {
        const existing = Main.panel.statusArea[EXTENSION_STATUS_AREA_NAME];
        if (existing && existing !== this)
            existing.destroy();

        Main.panel.addToStatusArea(EXTENSION_STATUS_AREA_NAME, this, 0, 'right');
    }

    update(view, options) {
        this._view = view;
        this._options = options;
        this._content.destroy_all_children();
        this._content.style_class = panelContentClassNames(view.panel);
        this._syncPanelStyle();
        this.accessible_name = panelAccessibleName(view);

        if (options.panelMode === 'provider')
            this._renderProviderGroup(view);
        else
            this._renderMerged(view);

        this._popover.update(view, options);
    }

    destroy() {
        if (this._menuOpenSignal) {
            this.menu.disconnect(this._menuOpenSignal);
            this._menuOpenSignal = null;
        }
        this._popover?.destroy();
        this._popover = null;
        this._view = null;
        this._options = null;
        super.destroy();
    }

    _renderMerged(view) {
        const panel = view.panel ?? {};
        this._content.add_child(createMicroMeterStack(panel.meters ?? []));
    }

    _renderProviderGroup(view) {
        const panel = view.panel ?? {};
        const rows = panel.visibleProviders ?? [];
        for (const row of rows) {
            const item = new St.Bin({
                style_class: panelProviderItemClassNames(row),
                x_align: Clutter.ActorAlign.CENTER,
                y_align: Clutter.ActorAlign.CENTER,
            });
            item.set_child(createMicroMeterStack(row.meters ?? []));
            this._content.add_child(item);
        }
        if (rows.length === 0)
            this._renderMinimal(view);
    }

    _renderMinimal(view) {
        const panel = view.panel ?? {};
        this._content.add_child(createMicroMeterStack(panel.meters ?? []));
    }

    _syncPanelStyle() {
        if (!this._view || !this._options)
            return;
        this.style_class = panelButtonClassNames(this._view, this._options, {open: this._menuOpen});
    }
});
