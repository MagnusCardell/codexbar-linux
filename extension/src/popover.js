import St from 'gi://St';

import {PRODUCT_NAME} from './constants.js';
import {createDiagnosticsView} from './diagnosticsView.js';
import {createProviderCard} from './providerCard.js';
import {formatGenerated, formatUpdated} from './time.js';

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
        this.actor.add_child(this._header(view));

        const list = new St.BoxLayout({
            vertical: true,
            style_class: 'codexbar-provider-list',
            x_expand: true,
        });
        const rows = view.providerRows.length > 0
            ? view.providerRows
            : [view.selectedRow].filter(Boolean);
        for (const row of rows)
            list.add_child(createProviderCard(row, options, this._actions));
        this.actor.add_child(list);

        this.actor.add_child(createDiagnosticsView(view.diagnostics, this._actions));
        this.actor.add_child(this._footer(view));
    }

    _header(view) {
        const header = new St.BoxLayout({
            vertical: true,
            style_class: `codexbar-header codexbar-state-${view.state}`,
            x_expand: true,
        });

        const top = new St.BoxLayout({
            style_class: 'codexbar-header-top',
            x_expand: true,
        });
        top.add_child(new St.Label({
            text: PRODUCT_NAME,
            style_class: 'codexbar-title',
            x_expand: true,
        }));

        const refresh = new St.Button({
            label: view.state === 'daemon_unavailable' ? 'Retry' : (view.refreshing ? 'Refreshing' : 'Refresh'),
            style_class: 'codexbar-button codexbar-button-primary',
            can_focus: true,
            reactive: !view.refreshing,
            track_hover: true,
        });
        refresh.connect('clicked', () => {
            if (view.state === 'daemon_unavailable')
                this._actions.retryDaemon();
            else
                this._actions.refresh();
        });
        top.add_child(refresh);
        header.add_child(top);

        const status = view.refreshing
            ? 'Refreshing…'
            : (view.state === 'daemon_unavailable'
                ? 'Daemon unavailable'
                : (view.stale ? 'Stale data' : view.panelStatus));
        const time = view.generatedAt
            ? (view.stale ? formatGenerated(view.generatedAt) : formatUpdated(view.generatedAt))
            : '';
        const statusPieces = [status, time].filter(Boolean);
        header.add_child(new St.Label({
            text: statusPieces.join(' · ') || 'Status unavailable',
            style_class: 'codexbar-muted',
            x_expand: true,
        }));
        return header;
    }

    _footer(view) {
        const daemon = view.daemonInfo ?? view.daemon;
        const capabilities = view.daemonInfo?.capabilities ?? {};
        const upstream = view.daemonInfo?.upstreamCli ?? view.daemon?.upstreamCli ?? null;
        const daemonVersion = daemon?.version ? ` ${daemon.version}` : '';
        const upstreamVersion = upstream?.version ? ` ${upstream.version}` : '';
        const lines = [
            [
                `Daemon ${daemon?.state ?? 'unknown'}${daemonVersion}`,
                `CLI ${upstream?.available ? 'available' : 'unavailable'}${upstreamVersion}`,
                `Cost ${capabilities.cost ? 'available' : 'unavailable'}`,
                `Browser import ${capabilities.browserImport ? 'available' : 'unavailable'}`,
            ].join(' · '),
            view.daemon?.lastRefreshFinishedAt
                ? formatUpdated(view.daemon.lastRefreshFinishedAt)
                : 'Last refresh unknown',
        ];

        const footer = new St.BoxLayout({
            vertical: true,
            style_class: 'codexbar-footer',
            x_expand: true,
        });
        for (const line of lines) {
            footer.add_child(new St.Label({
                text: line,
                style_class: 'codexbar-muted codexbar-small',
                x_expand: true,
            }));
        }
        return footer;
    }
}
