pub mod chromium;
pub mod cookie_store;
pub mod diagnostics;
pub mod keyring;
pub mod profile;
pub mod session_material;

use std::collections::{BTreeMap, BTreeSet};

use crate::browser::chromium::discover_chromium_profiles;
use crate::browser::cookie_store::{
    read_profile_cookies, read_profile_session_material, CookieQuery,
};
use crate::browser::keyring::{FakeCookieDecryptor, FakeDecryptorMode};
use crate::browser::profile::{is_safe_profile_id, BrowserDiscoveryRoots};
use crate::browser::session_material::SessionMaterial;
use crate::model::{
    BrowserImportOptions, BrowserImportPolicy, BrowserImportResult, BrowserImportStatus,
    BrowserProfileResult, BrowserProviderResult, BrowserProviderStatus, BrowserSourceAdapter,
    PreferredSourceAdapter, Settings,
};

#[derive(Clone, Debug)]
pub struct BrowserImportRequest {
    pub options: BrowserImportOptions,
    pub settings: Settings,
    pub roots: Option<BrowserDiscoveryRoots>,
    pub decryptor_mode: FakeDecryptorMode,
    pub tested_at: String,
}

#[derive(Clone, Debug)]
pub struct BrowserSessionRequest {
    pub providers: Vec<String>,
    pub settings: Settings,
    pub roots: Option<BrowserDiscoveryRoots>,
    pub decryptor_mode: FakeDecryptorMode,
}

#[derive(Debug)]
pub struct BrowserSessionCollection {
    pub sessions: BTreeMap<String, SessionMaterial>,
    pub provider_diagnostic_codes: BTreeMap<String, Vec<String>>,
    pub diagnostic_codes: Vec<String>,
}

pub fn test_import(request: BrowserImportRequest) -> BrowserImportResult {
    let include_diagnostics = request.options.include_diagnostics;
    let policy = effective_policy(&request.options, &request.settings);
    let mut diagnostic_codes = vec![diagnostics::IMPORT_STARTED.to_string()];
    let provider_ids = request.options.providers.clone();

    if !request.settings.browser_import.enabled || policy == BrowserImportPolicy::Off {
        diagnostics::push_code(&mut diagnostic_codes, diagnostics::IMPORT_DISABLED);
        diagnostics::push_code(&mut diagnostic_codes, diagnostics::IMPORT_FINISHED);
        return BrowserImportResult {
            schema_version: 1,
            tested_at: request.tested_at,
            status: BrowserImportStatus::Unavailable,
            policy,
            profiles: Vec::new(),
            providers: provider_ids
                .into_iter()
                .map(|provider| {
                    provider_result(
                        provider,
                        BrowserProviderStatus::MissingDependency,
                        BrowserSourceAdapter::None,
                        vec![diagnostics::IMPORT_DISABLED.to_string()],
                        include_diagnostics,
                    )
                })
                .collect(),
            diagnostic_codes: maybe_codes(diagnostic_codes, include_diagnostics),
        };
    }

    if policy == BrowserImportPolicy::Firefox {
        diagnostics::push_code(&mut diagnostic_codes, diagnostics::FIREFOX_NOT_IMPLEMENTED);
        diagnostics::push_code(&mut diagnostic_codes, diagnostics::IMPORT_FINISHED);
        return BrowserImportResult {
            schema_version: 1,
            tested_at: request.tested_at,
            status: BrowserImportStatus::NotImplemented,
            policy,
            profiles: Vec::new(),
            providers: provider_ids
                .into_iter()
                .map(|provider| {
                    provider_result(
                        provider,
                        BrowserProviderStatus::NotImplemented,
                        BrowserSourceAdapter::None,
                        vec![diagnostics::FIREFOX_NOT_IMPLEMENTED.to_string()],
                        include_diagnostics,
                    )
                })
                .collect(),
            diagnostic_codes: maybe_codes(diagnostic_codes, include_diagnostics),
        };
    }

    let Some(roots) = request.roots else {
        diagnostics::push_code(&mut diagnostic_codes, diagnostics::LIVE_PROFILES_DISABLED);
        diagnostics::push_code(&mut diagnostic_codes, diagnostics::PROFILE_SKIPPED);
        diagnostics::push_code(&mut diagnostic_codes, diagnostics::IMPORT_FINISHED);
        return BrowserImportResult {
            schema_version: 1,
            tested_at: request.tested_at,
            status: BrowserImportStatus::Unavailable,
            policy,
            profiles: Vec::new(),
            providers: provider_ids
                .into_iter()
                .map(|provider| {
                    provider_result(
                        provider,
                        BrowserProviderStatus::MissingDependency,
                        BrowserSourceAdapter::None,
                        vec![
                            diagnostics::LIVE_PROFILES_DISABLED.to_string(),
                            diagnostics::PROFILE_SKIPPED.to_string(),
                        ],
                        include_diagnostics,
                    )
                })
                .collect(),
            diagnostic_codes: maybe_codes(diagnostic_codes, include_diagnostics),
        };
    };

    let eligible_providers = eligible_providers(&provider_ids, &request.settings);
    if !provider_ids.is_empty() && eligible_providers.is_empty() {
        diagnostics::push_code(&mut diagnostic_codes, diagnostics::PROFILE_SKIPPED);
        diagnostics::push_code(&mut diagnostic_codes, diagnostics::IMPORT_FINISHED);
        return BrowserImportResult {
            schema_version: 1,
            tested_at: request.tested_at,
            status: BrowserImportStatus::Unavailable,
            policy,
            profiles: Vec::new(),
            providers: provider_ids
                .into_iter()
                .map(|provider| {
                    provider_result(
                        provider,
                        BrowserProviderStatus::MissingDependency,
                        BrowserSourceAdapter::None,
                        vec![diagnostics::PROFILE_SKIPPED.to_string()],
                        include_diagnostics,
                    )
                })
                .collect(),
            diagnostic_codes: maybe_codes(diagnostic_codes, include_diagnostics),
        };
    }

    let discovery = discover_chromium_profiles(&roots);
    diagnostic_codes.extend(discovery.diagnostic_codes);
    let selected_profile_ids = selected_profile_ids(&request.options, &request.settings);
    let mut profiles = Vec::new();
    let mut provider_counts = BTreeMap::<String, u64>::new();
    let mut provider_codes = BTreeMap::<String, Vec<String>>::new();
    let decryptor = FakeCookieDecryptor::new(request.decryptor_mode);
    let queries = eligible_providers
        .iter()
        .map(|provider| CookieQuery::for_provider(provider))
        .collect::<Vec<_>>();

    for profile in discovery.profiles {
        if !selected_profile_ids.is_empty() && !selected_profile_ids.contains(profile.profile_id())
        {
            diagnostics::push_code(&mut diagnostic_codes, diagnostics::PROFILE_SKIPPED);
            continue;
        }
        let outcome = read_profile_cookies(&profile, &queries, &decryptor);
        diagnostic_codes.extend(outcome.diagnostic_codes.clone());
        for (provider, count) in outcome.provider_counts {
            *provider_counts.entry(provider).or_default() += count;
        }
        for (provider, codes) in outcome.provider_diagnostic_codes {
            provider_codes.entry(provider).or_default().extend(codes);
        }
        profiles.push(BrowserProfileResult {
            browser_family: profile.browser_family(),
            profile_id: profile.profile_id().to_string(),
            profile_display_name: profile.display_name().to_string(),
            available: outcome.cookies_found > 0
                || !diagnostics::contains_dependency_failure(&outcome.diagnostic_codes),
            keyring_state: outcome.keyring_state,
            cookies_found: Some(outcome.cookies_found),
            diagnostic_codes: maybe_codes(
                diagnostics::unique_codes(outcome.diagnostic_codes),
                include_diagnostics,
            ),
        });
    }

    if profiles.is_empty() {
        diagnostics::push_code(&mut diagnostic_codes, diagnostics::PROFILE_NOT_FOUND);
    }

    let providers = provider_ids
        .into_iter()
        .map(|provider| {
            if !eligible_providers.contains(&provider) {
                return provider_result(
                    provider,
                    BrowserProviderStatus::MissingDependency,
                    BrowserSourceAdapter::None,
                    vec![diagnostics::IMPORT_DISABLED.to_string()],
                    include_diagnostics,
                );
            }
            let count = provider_counts.get(&provider).copied().unwrap_or_default();
            let codes =
                diagnostics::unique_codes(provider_codes.remove(&provider).unwrap_or_default());
            if count > 0 {
                provider_result(
                    provider,
                    BrowserProviderStatus::Success,
                    BrowserSourceAdapter::LinuxWeb,
                    vec![diagnostics::COOKIE_FOUND.to_string()],
                    include_diagnostics,
                )
            } else if diagnostics::contains_dependency_failure(&codes) {
                provider_result(
                    provider,
                    provider_failure_status(&codes),
                    BrowserSourceAdapter::LinuxWeb,
                    codes,
                    include_diagnostics,
                )
            } else {
                provider_result(
                    provider,
                    BrowserProviderStatus::Unauthenticated,
                    BrowserSourceAdapter::LinuxWeb,
                    vec![diagnostics::COOKIE_MISSING.to_string()],
                    include_diagnostics,
                )
            }
        })
        .collect::<Vec<_>>();

    diagnostics::push_code(&mut diagnostic_codes, diagnostics::IMPORT_FINISHED);
    let status = aggregate_status(&profiles, &providers, &diagnostic_codes);
    BrowserImportResult {
        schema_version: 1,
        tested_at: request.tested_at,
        status,
        policy,
        profiles,
        providers,
        diagnostic_codes: maybe_codes(
            diagnostics::unique_codes(diagnostic_codes),
            include_diagnostics,
        ),
    }
}

pub fn collect_session_material(request: BrowserSessionRequest) -> BrowserSessionCollection {
    let mut diagnostic_codes = vec![diagnostics::IMPORT_STARTED.to_string()];
    let mut provider_codes = BTreeMap::<String, Vec<String>>::new();
    let mut sessions = BTreeMap::<String, SessionMaterial>::new();
    let provider_ids = request.providers;
    let policy = match request.settings.browser_import.policy {
        BrowserImportPolicy::Auto => BrowserImportPolicy::ChromiumFamily,
        policy => policy,
    };

    let finish = |sessions: BTreeMap<String, SessionMaterial>,
                  provider_codes: BTreeMap<String, Vec<String>>,
                  mut diagnostic_codes: Vec<String>| {
        diagnostics::push_code(&mut diagnostic_codes, diagnostics::IMPORT_FINISHED);
        BrowserSessionCollection {
            sessions,
            provider_diagnostic_codes: provider_codes
                .into_iter()
                .map(|(provider, codes)| (provider, diagnostics::unique_codes(codes)))
                .collect(),
            diagnostic_codes: diagnostics::unique_codes(diagnostic_codes),
        }
    };

    if !request.settings.browser_import.enabled || policy == BrowserImportPolicy::Off {
        diagnostics::push_code(&mut diagnostic_codes, diagnostics::IMPORT_DISABLED);
        for provider in provider_ids {
            provider_codes
                .entry(provider)
                .or_default()
                .push(diagnostics::IMPORT_DISABLED.to_string());
        }
        return finish(sessions, provider_codes, diagnostic_codes);
    }

    if policy == BrowserImportPolicy::Firefox {
        diagnostics::push_code(&mut diagnostic_codes, diagnostics::FIREFOX_NOT_IMPLEMENTED);
        for provider in provider_ids {
            provider_codes
                .entry(provider)
                .or_default()
                .push(diagnostics::FIREFOX_NOT_IMPLEMENTED.to_string());
        }
        return finish(sessions, provider_codes, diagnostic_codes);
    }

    let Some(roots) = request.roots else {
        diagnostics::push_code(&mut diagnostic_codes, diagnostics::LIVE_PROFILES_DISABLED);
        diagnostics::push_code(&mut diagnostic_codes, diagnostics::PROFILE_SKIPPED);
        for provider in provider_ids {
            provider_codes.entry(provider).or_default().extend([
                diagnostics::LIVE_PROFILES_DISABLED.to_string(),
                diagnostics::PROFILE_SKIPPED.to_string(),
            ]);
        }
        return finish(sessions, provider_codes, diagnostic_codes);
    };

    let eligible_providers = eligible_providers(&provider_ids, &request.settings);
    if !provider_ids.is_empty() && eligible_providers.is_empty() {
        diagnostics::push_code(&mut diagnostic_codes, diagnostics::PROFILE_SKIPPED);
        for provider in provider_ids {
            provider_codes
                .entry(provider)
                .or_default()
                .push(diagnostics::PROFILE_SKIPPED.to_string());
        }
        return finish(sessions, provider_codes, diagnostic_codes);
    }

    let discovery = discover_chromium_profiles(&roots);
    diagnostic_codes.extend(discovery.diagnostic_codes);
    let settings_profile_ids = request
        .settings
        .browser_import
        .profile_id_allowlist
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let decryptor = FakeCookieDecryptor::new(request.decryptor_mode);
    let queries = eligible_providers
        .iter()
        .map(|provider| CookieQuery::for_live_web_provider(provider))
        .collect::<Vec<_>>();
    let mut visited_profile = false;

    for profile in discovery.profiles {
        if !settings_profile_ids.is_empty() && !settings_profile_ids.contains(profile.profile_id())
        {
            diagnostics::push_code(&mut diagnostic_codes, diagnostics::PROFILE_SKIPPED);
            continue;
        }
        visited_profile = true;
        let outcome = read_profile_session_material(&profile, &queries, &decryptor);
        diagnostic_codes.extend(outcome.profile.diagnostic_codes.clone());
        for (provider, codes) in outcome.profile.provider_diagnostic_codes {
            provider_codes.entry(provider).or_default().extend(codes);
        }
        for (provider, material) in outcome.sessions {
            if sessions.contains_key(&provider) {
                diagnostics::push_code(&mut diagnostic_codes, diagnostics::PROFILE_SKIPPED);
                provider_codes
                    .entry(provider)
                    .or_default()
                    .push(diagnostics::PROFILE_SKIPPED.to_string());
                continue;
            }
            sessions.insert(provider, material);
        }
    }

    if !visited_profile {
        diagnostics::push_code(&mut diagnostic_codes, diagnostics::PROFILE_NOT_FOUND);
    }
    for provider in eligible_providers {
        if sessions.contains_key(&provider) {
            continue;
        }
        let codes = provider_codes.entry(provider).or_default();
        if !diagnostics::contains_dependency_failure(codes) {
            diagnostics::push_code(codes, diagnostics::COOKIE_MISSING);
        }
    }

    finish(sessions, provider_codes, diagnostic_codes)
}

pub fn validate_profile_ids(profile_ids: &[String]) -> bool {
    profile_ids
        .iter()
        .all(|profile_id| is_safe_profile_id(profile_id))
}

fn effective_policy(options: &BrowserImportOptions, settings: &Settings) -> BrowserImportPolicy {
    if options.policy != BrowserImportPolicy::Auto {
        return options.policy;
    }
    match settings.browser_import.policy {
        BrowserImportPolicy::Auto => BrowserImportPolicy::ChromiumFamily,
        policy => policy,
    }
}

fn selected_profile_ids(options: &BrowserImportOptions, settings: &Settings) -> BTreeSet<String> {
    let option_ids = options.profile_ids.iter().cloned().collect::<BTreeSet<_>>();
    let settings_ids = settings
        .browser_import
        .profile_id_allowlist
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    match (option_ids.is_empty(), settings_ids.is_empty()) {
        (true, true) => BTreeSet::new(),
        (false, true) => option_ids,
        (true, false) => settings_ids,
        (false, false) => option_ids.intersection(&settings_ids).cloned().collect(),
    }
}

fn eligible_providers(providers: &[String], settings: &Settings) -> BTreeSet<String> {
    providers
        .iter()
        .filter(|provider| {
            settings.providers.get(*provider).is_none_or(|settings| {
                settings.enabled
                    && settings.allow_browser_import
                    && settings.preferred_source_adapter != PreferredSourceAdapter::Off
            })
        })
        .cloned()
        .collect()
}

fn provider_failure_status(codes: &[String]) -> BrowserProviderStatus {
    if codes
        .iter()
        .any(|code| code == diagnostics::COOKIE_DB_SCHEMA_UNSUPPORTED)
    {
        BrowserProviderStatus::ParseError
    } else if codes
        .iter()
        .any(|code| code == diagnostics::COOKIE_DB_LOCKED)
    {
        BrowserProviderStatus::ProviderUnavailable
    } else {
        BrowserProviderStatus::MissingDependency
    }
}

fn aggregate_status(
    profiles: &[BrowserProfileResult],
    providers: &[BrowserProviderResult],
    diagnostic_codes: &[String],
) -> BrowserImportStatus {
    if profiles.is_empty() {
        return BrowserImportStatus::Unavailable;
    }
    let successes = providers
        .iter()
        .filter(|provider| provider.status == BrowserProviderStatus::Success)
        .count();
    if !providers.is_empty() && successes == providers.len() {
        return BrowserImportStatus::Success;
    }
    if successes > 0 {
        return BrowserImportStatus::Partial;
    }
    if diagnostic_codes
        .iter()
        .any(|code| code == diagnostics::COOKIE_DB_SCHEMA_UNSUPPORTED)
    {
        BrowserImportStatus::Failure
    } else {
        BrowserImportStatus::Unavailable
    }
}

fn provider_result(
    provider: String,
    status: BrowserProviderStatus,
    source_adapter: BrowserSourceAdapter,
    codes: Vec<String>,
    include_diagnostics: bool,
) -> BrowserProviderResult {
    BrowserProviderResult {
        provider,
        status,
        source_adapter,
        diagnostic_codes: maybe_codes(codes, include_diagnostics),
    }
}

fn maybe_codes(codes: Vec<String>, include: bool) -> Vec<String> {
    if include {
        diagnostics::unique_codes(codes)
    } else {
        Vec::new()
    }
}
