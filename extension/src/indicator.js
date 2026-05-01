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

export const CodexbarIndicator = GObject.registerClass(
class CodexbarIndicator extends PanelMenu.Button {
    _init(actions) {
        super._init(0.0, PRODUCT_NAME, false);
        this._actions = actions;
        this._content = new St.BoxLayout({
            style_class: 'codexbar-panel-content',
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
    }

    addToPanel() {
        Main.panel.addToStatusArea(EXTENSION_STATUS_AREA_NAME, this, 0, 'right');
    }

    update(view, options) {
        this._content.destroy_all_children();
        this.style_class = `panel-button codexbar-panel codexbar-theme-${options.theme} codexbar-state-${view.state}${view.stale ? ' codexbar-stale' : ''}`;
        this.accessible_name = `${PRODUCT_NAME}: ${view.panel?.label ?? view.panelLabel} ${view.headerStatus ?? view.panelStatus}`;

        if (options.panelMode === 'minimal')
            this._renderMinimal(view);
        else if (options.panelMode === 'provider')
            this._renderProviderGroup(view);
        else
            this._renderMerged(view);

        this._popover.update(view, options);
    }

    destroy() {
        this._popover?.destroy();
        this._popover = null;
        super.destroy();
    }

    _renderMerged(view) {
        const panel = view.panel ?? {};
        this._content.add_child(new St.Widget({
            style_class: 'codexbar-panel-status-dot',
        }));
        this._content.add_child(new St.Label({
            text: panel.label ?? view.panelLabel ?? 'CB',
            style_class: 'codexbar-panel-label',
        }));
        this._content.add_child(createMicroMeterStack(panel.meters ?? []));
    }

    _renderProviderGroup(view) {
        const panel = view.panel ?? {};
        const rows = panel.visibleProviders ?? [];
        for (const row of rows) {
            const item = new St.BoxLayout({
                vertical: true,
                style_class: `codexbar-provider-dot codexbar-panel-provider-item codexbar-state-${row.state}`,
            });
            item.add_child(new St.Label({
                text: row.label,
                style_class: 'codexbar-provider-dot-label',
            }));
            item.add_child(createMicroMeterStack(row.meters ?? []));
            this._content.add_child(item);
        }
        const extra = panel.overflowCount ?? 0;
        if (extra > 0) {
            this._content.add_child(new St.Label({
                text: `+${extra}`,
                style_class: 'codexbar-panel-label codexbar-muted',
            }));
        }
        if (rows.length === 0)
            this._renderMinimal(view);
    }

    _renderMinimal(view) {
        const panel = view.panel ?? {};
        this._content.add_child(new St.Icon({
            icon_name: panel.iconName ?? 'emblem-ok-symbolic',
            style_class: 'system-status-icon codexbar-panel-icon',
        }));
    }
});
