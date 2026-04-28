use crate::error::{AppError, AppResult};
use crate::model::{
    DaemonState, ProviderState, RefreshProviderResult, RefreshProviderStatus, Snapshot,
    SourceAdapter, UpstreamCliInfo,
};

const OK_SNAPSHOT: &str = include_str!("../../fixtures/snapshots/ok.json");
const LOADING_SNAPSHOT: &str = include_str!("../../fixtures/snapshots/loading.json");
const ERROR_SNAPSHOT: &str = include_str!("../../fixtures/snapshots/error.json");

pub fn refreshed_snapshot(
    refresh_id: &str,
    started_at: &str,
    finished_at: &str,
) -> AppResult<Snapshot> {
    let mut snapshot: Snapshot =
        serde_json::from_str(OK_SNAPSHOT).map_err(|_| AppError::internal_redacted())?;
    snapshot.generated_at = finished_at.to_string();
    snapshot.stale = false;
    snapshot.daemon.version = env!("CARGO_PKG_VERSION").to_string();
    snapshot.daemon.state = DaemonState::Ok;
    snapshot.daemon.last_refresh_id = Some(refresh_id.to_string());
    snapshot.daemon.last_refresh_started_at = Some(started_at.to_string());
    snapshot.daemon.last_refresh_finished_at = Some(finished_at.to_string());
    snapshot.daemon.upstream_cli = Some(task01_upstream_cli());
    for provider in &mut snapshot.providers {
        provider.source_adapter = SourceAdapter::Fixture;
        provider.updated_at = Some(finished_at.to_string());
        provider.stale_since = None;
        if let Some(status) = provider.status.as_mut() {
            status.updated_at = Some(finished_at.to_string());
        }
    }
    Ok(snapshot)
}

pub fn synthetic_loading(now: &str) -> AppResult<Snapshot> {
    let mut snapshot: Snapshot =
        serde_json::from_str(LOADING_SNAPSHOT).map_err(|_| AppError::internal_redacted())?;
    snapshot.generated_at = now.to_string();
    snapshot.daemon.version = env!("CARGO_PKG_VERSION").to_string();
    snapshot.daemon.state = DaemonState::Starting;
    snapshot.daemon.last_refresh_id = None;
    snapshot.daemon.last_refresh_started_at = None;
    snapshot.daemon.last_refresh_finished_at = None;
    snapshot.daemon.upstream_cli = Some(task01_upstream_cli());
    for provider in &mut snapshot.providers {
        provider.source_adapter = SourceAdapter::Synthetic;
        provider.updated_at = None;
        provider.stale_since = None;
    }
    Ok(snapshot)
}

pub fn unsupported_adapter_snapshot(now: &str) -> AppResult<Snapshot> {
    let mut snapshot: Snapshot =
        serde_json::from_str(ERROR_SNAPSHOT).map_err(|_| AppError::internal_redacted())?;
    snapshot.generated_at = now.to_string();
    snapshot.stale = false;
    snapshot.daemon.version = env!("CARGO_PKG_VERSION").to_string();
    snapshot.daemon.state = DaemonState::Degraded;
    snapshot.daemon.last_refresh_id = None;
    snapshot.daemon.last_refresh_started_at = None;
    snapshot.daemon.last_refresh_finished_at = None;
    snapshot.daemon.upstream_cli = Some(task01_upstream_cli());
    for provider in &mut snapshot.providers {
        provider.source_adapter = SourceAdapter::None;
        provider.state = ProviderState::MissingDependency;
        provider.updated_at = Some(now.to_string());
        provider.diagnostics_summary =
            Some("Requested source adapter is not implemented".to_string());
        provider.diagnostic_codes = vec![
            "upstream_cli_not_implemented".to_string(),
            "browser_import_not_implemented".to_string(),
        ];
        if let Some(status) = provider.status.as_mut() {
            status.indicator = Some("missing_dependency".to_string());
            status.description = Some("Requested source adapter is not implemented".to_string());
            status.updated_at = Some(now.to_string());
        }
    }
    Ok(snapshot)
}

pub fn provider_results(snapshot: &Snapshot) -> Vec<RefreshProviderResult> {
    snapshot
        .providers
        .iter()
        .map(|provider| RefreshProviderResult {
            provider: provider.provider.clone(),
            status: RefreshProviderStatus::from(provider.state),
            source_adapter: Some(provider.source_adapter),
            diagnostic_codes: provider.diagnostic_codes.clone(),
        })
        .collect()
}

pub fn task01_upstream_cli() -> UpstreamCliInfo {
    UpstreamCliInfo {
        available: false,
        path: None,
        version: None,
        diagnostic_code: Some("upstream_cli_not_implemented".to_string()),
    }
}
