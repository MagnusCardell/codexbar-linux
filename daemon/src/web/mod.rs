pub mod client;
pub mod diagnostics;
pub mod policy;
pub mod providers;

use std::collections::BTreeMap;

use crate::browser::session_material::{ScopedCookie, SessionMaterial};
use crate::config::is_safe_id;
use crate::error::AppResult;
use crate::model::{
    DaemonState, DiagnosticEvent, DiagnosticSeverity, EventRedaction, PreferredSourceAdapter,
    Provider, ProviderState, ProviderStatus, SemanticSource, Settings, Snapshot, SnapshotDaemon,
    SourceAdapter, UpstreamCliInfo, Usage,
};

use self::client::WebClient;

#[derive(Debug)]
pub struct WebRefreshRequest {
    pub refresh_id: String,
    pub started_at: String,
    pub finished_at: String,
    pub providers: Vec<String>,
    pub selected_provider: Option<String>,
    pub upstream_cli: UpstreamCliInfo,
    pub sessions: BTreeMap<String, SessionMaterial>,
    pub session_diagnostic_codes: BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Debug)]
pub struct WebRefresh {
    pub snapshot: Snapshot,
    pub diagnostics: Vec<DiagnosticEvent>,
}

pub fn target_providers(settings: &Settings, requested: &[String]) -> Vec<String> {
    let candidates = if requested.is_empty() {
        vec!["codex".to_string()]
    } else {
        requested.to_vec()
    };
    candidates
        .into_iter()
        .filter(|provider| {
            is_safe_id(provider)
                && settings.providers.get(provider).is_none_or(|settings| {
                    settings.enabled
                        && settings.allow_browser_import
                        && settings.preferred_source_adapter != PreferredSourceAdapter::Off
                })
        })
        .collect()
}

pub fn refresh_with_client<C>(request: WebRefreshRequest, client: &C) -> AppResult<WebRefresh>
where
    C: WebClient,
{
    let mut providers = Vec::new();
    let mut diagnostics = Vec::new();
    for provider_id in &request.providers {
        let session_codes = request
            .session_diagnostic_codes
            .get(provider_id)
            .cloned()
            .unwrap_or_default();
        diagnostics.extend(session_diagnostic_events(
            provider_id,
            &session_codes,
            &request.finished_at,
        ));
        if provider_id == providers::codex::PROVIDER_ID {
            let result = providers::codex::fetch_dashboard_with_client_with_session_codes(
                client,
                request.sessions.get(provider_id),
                &request.finished_at,
                None,
                &session_codes,
            );
            providers.push(result.provider);
            diagnostics.extend(result.diagnostics);
        } else {
            providers.push(unavailable_provider(
                provider_id,
                "provider_web_adapter_disabled",
                "Provider web adapter is not implemented for this provider",
                &request.finished_at,
            ));
            diagnostics.push(web_diagnostic(
                "provider_web_adapter_disabled",
                "Provider web adapter is not implemented for this provider",
                provider_id,
                &request.finished_at,
                DiagnosticSeverity::Warning,
            ));
        }
    }
    if providers.is_empty() {
        providers.push(unavailable_provider(
            "codex",
            "provider_web_adapter_disabled",
            "Provider web adapter is not implemented for this provider",
            &request.finished_at,
        ));
    }
    Ok(WebRefresh {
        snapshot: snapshot_from_providers(request, providers),
        diagnostics,
    })
}

pub fn disabled_refresh(request: WebRefreshRequest) -> AppResult<WebRefresh> {
    let mut providers = request
        .providers
        .iter()
        .map(|provider| {
            unavailable_provider(
                provider,
                "linux_web_live_http_disabled",
                "Linux web adapter has no live HTTP client configured",
                &request.finished_at,
            )
        })
        .collect::<Vec<_>>();
    let diagnostics = request
        .providers
        .iter()
        .map(|provider| {
            web_diagnostic(
                "linux_web_live_http_disabled",
                "Linux web adapter has no live HTTP client configured",
                provider,
                &request.finished_at,
                DiagnosticSeverity::Warning,
            )
        })
        .collect::<Vec<_>>();
    if providers.is_empty() {
        providers.push(unavailable_provider(
            "codex",
            "linux_web_live_http_disabled",
            "Linux web adapter has no live HTTP client configured",
            &request.finished_at,
        ));
    }
    Ok(WebRefresh {
        snapshot: snapshot_from_providers(request, providers),
        diagnostics,
    })
}

pub fn fake_codex_session_for_tests() -> SessionMaterial {
    SessionMaterial::new(
        providers::codex::PROVIDER_ID,
        vec![ScopedCookie::new("fixture_session", "fixture-value")],
    )
}

fn snapshot_from_providers(request: WebRefreshRequest, providers: Vec<Provider>) -> Snapshot {
    Snapshot {
        schema_version: 1,
        generated_at: request.finished_at.clone(),
        stale: false,
        selected_provider: request.selected_provider,
        daemon: SnapshotDaemon {
            version: env!("CARGO_PKG_VERSION").to_string(),
            state: if providers
                .iter()
                .any(|provider| provider.state == ProviderState::Ok)
            {
                DaemonState::Ok
            } else {
                DaemonState::Degraded
            },
            last_refresh_id: Some(request.refresh_id),
            last_refresh_started_at: Some(request.started_at),
            last_refresh_finished_at: Some(request.finished_at),
            upstream_cli: Some(request.upstream_cli),
        },
        providers,
    }
}

fn unavailable_provider(provider_id: &str, code: &str, message: &str, timestamp: &str) -> Provider {
    Provider {
        provider: provider_id.to_string(),
        display_name: display_name(provider_id),
        version: None,
        source: SemanticSource::Web,
        source_adapter: SourceAdapter::LinuxWeb,
        state: ProviderState::MissingDependency,
        updated_at: Some(timestamp.to_string()),
        stale_since: None,
        usage: Usage {
            primary: None,
            secondary: None,
            tertiary: None,
        },
        credits: None,
        identity: None,
        status: Some(ProviderStatus {
            indicator: Some("missing_dependency".to_string()),
            description: Some(message.to_string()),
            updated_at: Some(timestamp.to_string()),
            url: None,
        }),
        cost: None,
        dashboard_url: None,
        diagnostics_summary: Some(message.to_string()),
        diagnostic_codes: vec![code.to_string()],
    }
}

fn web_diagnostic(
    code: &str,
    message: &str,
    provider: &str,
    timestamp: &str,
    severity: DiagnosticSeverity,
) -> DiagnosticEvent {
    DiagnosticEvent {
        code: code.to_string(),
        severity,
        safe_message: message.to_string(),
        timestamp: timestamp.to_string(),
        provider: Some(provider.to_string()),
        source_adapter: Some(SourceAdapter::LinuxWeb),
        recoverable: true,
        details: BTreeMap::new(),
        redacted: EventRedaction {
            applied: true,
            classes: vec![
                "secrets".to_string(),
                "identity".to_string(),
                "provider_payload".to_string(),
            ],
        },
    }
}

fn session_diagnostic_events(
    provider: &str,
    codes: &[String],
    timestamp: &str,
) -> Vec<DiagnosticEvent> {
    codes
        .iter()
        .map(|code| {
            web_diagnostic(
                code,
                session_code_message(code),
                provider,
                timestamp,
                session_code_severity(code),
            )
        })
        .collect()
}

fn session_code_message(code: &str) -> &'static str {
    match code {
        "browser_cookie_found" => "Provider-scoped browser cookie material was found",
        "browser_cookie_decrypted" => "Provider-scoped browser cookie material was decrypted",
        "browser_cookie_missing" => "Provider-scoped browser cookie material was absent",
        "browser_cookie_db_locked" => "Browser cookie store was locked",
        "browser_keyring_locked" => "Browser session material could not be unlocked",
        "browser_keyring_prompt_required" => {
            "Browser session material would require an interactive keyring prompt"
        }
        "browser_profile_not_found" | "browser_not_found" => {
            "No supported throwaway browser profile was available"
        }
        "browser_live_profiles_disabled" => "Live browser profile scanning is disabled",
        _ => "Browser session preflight completed with a redacted diagnostic",
    }
}

fn session_code_severity(code: &str) -> DiagnosticSeverity {
    match code {
        "browser_cookie_found" | "browser_cookie_decrypted" | "browser_profile_discovered" => {
            DiagnosticSeverity::Info
        }
        _ => DiagnosticSeverity::Warning,
    }
}

fn display_name(provider_id: &str) -> String {
    if provider_id == providers::codex::PROVIDER_ID {
        return providers::codex::DISPLAY_NAME.to_string();
    }
    let mut chars = provider_id.chars();
    match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
        None => "Unknown".to_string(),
    }
}
