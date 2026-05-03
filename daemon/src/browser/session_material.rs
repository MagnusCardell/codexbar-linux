use std::fmt;

pub(crate) const MAX_COOKIE_NAME_LEN: usize = 256;
pub(crate) const MAX_COOKIE_VALUE_LEN: usize = 4096;
pub(crate) const MAX_COOKIE_PATH_LEN: usize = 1024;
pub(crate) const MAX_COOKIE_COUNT: usize = 128;
pub(crate) const MAX_COOKIE_HEADER_LEN: usize = 16 * 1024;
const CODEX_PROVIDER_ID: &str = "codex";
const CODEX_DASHBOARD_URL: &str = "https://chatgpt.com/codex/settings/usage";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionMaterialPolicy {
    provider: &'static str,
    target_url: &'static str,
}

impl SessionMaterialPolicy {
    pub const fn codex_dashboard() -> Self {
        Self {
            provider: CODEX_PROVIDER_ID,
            target_url: CODEX_DASHBOARD_URL,
        }
    }

    pub fn provider(&self) -> &'static str {
        self.provider
    }

    pub fn target_url(&self) -> &'static str {
        self.target_url
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CookieHeaderEligibilitySummary {
    pub total_cookies: u64,
    pub domain_matched: u64,
    pub path_matched: u64,
    pub secure_matched: u64,
    pub header_eligible: u64,
    pub rejected_provider: u64,
    pub rejected_target: u64,
    pub rejected_domain: u64,
    pub rejected_path: u64,
    pub rejected_secure: u64,
    pub header_size_class: CookieHeaderSizeClass,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CookieHeaderSizeClass {
    #[default]
    Absent,
    Present,
}

/// Provider-scoped browser session material is intentionally memory-only.
///
/// ```compile_fail
/// use codexbar_linuxd::browser::session_material::{ScopedCookie, SessionMaterial};
///
/// let material = SessionMaterial::new(
///     "codex",
///     vec![ScopedCookie::new("quota_marker", "fixture-value-1")],
/// );
/// let _ = serde_json::to_string(&material).unwrap();
/// ```
pub struct SessionMaterial {
    provider: String,
    cookies: Vec<ScopedCookie>,
}

impl SessionMaterial {
    pub fn new(provider: impl Into<String>, cookies: Vec<ScopedCookie>) -> Self {
        Self::try_new(provider, cookies).expect("session material must be provider-scoped and safe")
    }

    pub fn try_new(
        provider: impl Into<String>,
        cookies: Vec<ScopedCookie>,
    ) -> Result<Self, SessionMaterialError> {
        let provider = provider.into();
        if !crate::config::is_safe_id(&provider) {
            return Err(SessionMaterialError::InvalidProvider);
        }
        if cookies.len() > MAX_COOKIE_COUNT {
            return Err(SessionMaterialError::TooManyCookies);
        }
        Self { provider, cookies }.validate_header_bound()
    }

    pub fn cookie_count(&self) -> usize {
        self.cookies.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cookies.is_empty()
    }

    pub fn provider(&self) -> &str {
        &self.provider
    }

    pub fn cookie_header_eligibility_for_policy(
        &self,
        policy: SessionMaterialPolicy,
    ) -> CookieHeaderEligibilitySummary {
        let total_cookies = self.cookies.len() as u64;
        let mut summary = CookieHeaderEligibilitySummary {
            total_cookies,
            ..CookieHeaderEligibilitySummary::default()
        };
        if self.provider != policy.provider() {
            summary.rejected_provider = total_cookies;
            return summary;
        }
        let Some(target) = CookieRequestTarget::parse(policy.target_url()) else {
            summary.rejected_target = total_cookies;
            return summary;
        };
        for cookie in &self.cookies {
            if !cookie.domain_matches(Some(target.host())) {
                summary.rejected_domain += 1;
                continue;
            }
            summary.domain_matched += 1;
            if !cookie.path_matches(Some(target.path())) {
                summary.rejected_path += 1;
                continue;
            }
            summary.path_matched += 1;
            if !cookie.secure_matches(target.is_https()) {
                summary.rejected_secure += 1;
                continue;
            }
            summary.secure_matched += 1;
            summary.header_eligible += 1;
        }
        if summary.header_eligible > 0 {
            summary.header_size_class = CookieHeaderSizeClass::Present;
        }
        summary
    }

    #[cfg(test)]
    pub(crate) fn cookie_header_value_for_url(&self, url: &str) -> Option<String> {
        self.cookie_header_for_url(url)
            .ok()
            .flatten()
            .map(|header| header.as_str().to_string())
    }

    pub(crate) fn cookie_header_for_url(
        &self,
        url: &str,
    ) -> Result<Option<CookieHeader>, CookieHeaderBuildError> {
        let Some(target) = CookieRequestTarget::parse(url) else {
            return Ok(None);
        };
        self.cookie_header_for_target(&target)
    }

    fn cookie_header_for_target(
        &self,
        target: &CookieRequestTarget,
    ) -> Result<Option<CookieHeader>, CookieHeaderBuildError> {
        if self.cookies.is_empty() {
            return Ok(None);
        }
        let mut pairs = self
            .cookies
            .iter()
            .enumerate()
            .filter(|(_, cookie)| cookie.applies_to(target))
            .map(|(index, cookie)| (cookie.path_len(), index, cookie.header_pair()))
            .collect::<Vec<_>>();
        if pairs.is_empty() {
            return Ok(None);
        }
        if pairs.len() > MAX_COOKIE_COUNT {
            return Err(CookieHeaderBuildError::TooManyCookies);
        }
        pairs.sort_by(|left, right| right.0.cmp(&left.0).then(left.1.cmp(&right.1)));
        let header_len = pairs.iter().map(|(_, _, pair)| pair.len()).sum::<usize>()
            + pairs.len().saturating_sub(1) * 2;
        if header_len > MAX_COOKIE_HEADER_LEN {
            return Err(CookieHeaderBuildError::HeaderTooLarge);
        }
        let value = pairs
            .into_iter()
            .map(|(_, _, pair)| pair)
            .collect::<Vec<_>>()
            .join("; ");
        Ok(Some(CookieHeader { value }))
    }

    fn validate_header_bound(self) -> Result<Self, SessionMaterialError> {
        let header_len = self
            .cookies
            .iter()
            .map(|cookie| cookie.name.len() + 1 + cookie.value.len())
            .sum::<usize>()
            + self.cookies.len().saturating_sub(1) * 2;
        if header_len > MAX_COOKIE_HEADER_LEN {
            return Err(SessionMaterialError::HeaderTooLarge);
        }
        Ok(self)
    }
}

impl fmt::Debug for SessionMaterial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionMaterial")
            .field("provider", &self.provider)
            .field("cookie_count", &self.cookies.len())
            .field("cookies", &"[redacted]")
            .finish()
    }
}

pub struct ScopedCookie {
    domain: Option<CookieDomain>,
    path: Option<String>,
    secure: bool,
    name: String,
    value: String,
}

impl ScopedCookie {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self::try_new(name, value).expect("cookie name and value must be header-safe")
    }

    pub fn try_new(
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, SessionMaterialError> {
        Self::try_new_scoped(None, None, false, name, value)
    }

    pub fn try_new_for_domain(
        domain: impl AsRef<str>,
        path: impl AsRef<str>,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, SessionMaterialError> {
        Self::try_new_for_domain_with_secure(domain, path, true, name, value)
    }

    pub fn try_new_for_domain_with_secure(
        domain: impl AsRef<str>,
        path: impl AsRef<str>,
        secure: bool,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, SessionMaterialError> {
        Self::try_new_scoped(
            Some(normalize_cookie_domain(domain.as_ref())?),
            Some(normalize_cookie_path(path.as_ref())?),
            secure,
            name,
            value,
        )
    }

    fn try_new_scoped(
        domain: Option<CookieDomain>,
        path: Option<String>,
        secure: bool,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, SessionMaterialError> {
        let name = name.into();
        let value = value.into();
        validate_cookie_name(&name)?;
        validate_cookie_value(&value)?;
        Self {
            domain,
            path,
            secure,
            name,
            value,
        }
        .pipe(Ok)
    }

    fn header_pair(&self) -> String {
        format!("{}={}", self.name, self.value)
    }

    fn path_len(&self) -> usize {
        self.path.as_ref().map_or(0, String::len)
    }

    fn applies_to(&self, target: &CookieRequestTarget) -> bool {
        self.domain_matches(Some(target.host()))
            && self.path_matches(Some(target.path()))
            && self.secure_matches(target.is_https())
    }

    fn domain_matches(&self, request_host: Option<&str>) -> bool {
        if let Some(domain) = &self.domain {
            let Some(host) = request_host.map(|host| host.trim_matches('.').to_ascii_lowercase())
            else {
                return true;
            };
            if !domain.matches_host(&host) {
                return false;
            }
        }
        true
    }

    fn path_matches(&self, request_path: Option<&str>) -> bool {
        if let Some(cookie_path) = &self.path {
            let request_path = request_path.unwrap_or("/");
            if !path_matches(cookie_path, request_path) {
                return false;
            }
        }
        true
    }

    fn secure_matches(&self, request_is_https: bool) -> bool {
        !self.secure || request_is_https
    }
}

fn path_matches(cookie_path: &str, request_path: &str) -> bool {
    if request_path == cookie_path {
        return true;
    }
    let Some(rest) = request_path.strip_prefix(cookie_path) else {
        return false;
    };
    cookie_path.ends_with('/') || rest.starts_with('/')
}

pub(crate) fn cookie_path_matches(cookie_path: &str, request_path: &str) -> bool {
    path_matches(cookie_path, request_path)
}

pub(crate) fn cookie_domain_matches(host_key: &str, request_host: &str) -> bool {
    let Ok(domain) = normalize_cookie_domain(host_key) else {
        return false;
    };
    let request_host = request_host.trim_matches('.').to_ascii_lowercase();
    normalize_cookie_domain(&request_host).is_ok() && domain.matches_host(&request_host)
}

#[derive(Clone)]
pub(crate) struct CookieHeader {
    value: String,
}

impl CookieHeader {
    pub(crate) fn as_str(&self) -> &str {
        &self.value
    }

    pub(crate) fn len(&self) -> usize {
        self.value.len()
    }
}

impl fmt::Debug for CookieHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CookieHeader")
            .field("bytes", &self.value.len())
            .field("value", &"[redacted]")
            .finish()
    }
}

impl Drop for CookieHeader {
    fn drop(&mut self) {
        self.value.clear();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CookieHeaderBuildError {
    TooManyCookies,
    HeaderTooLarge,
}

pub(crate) struct CookieRequestTarget {
    host: String,
    path: String,
    is_https: bool,
}

impl CookieRequestTarget {
    pub(crate) fn parse(url: &str) -> Option<Self> {
        let (is_https, rest) = if let Some(rest) = url.strip_prefix("https://") {
            (true, rest)
        } else if let Some(rest) = url.strip_prefix("http://") {
            (false, rest)
        } else {
            return None;
        };
        let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
        let authority = &rest[..authority_end];
        if authority.is_empty() || authority.contains('@') {
            return None;
        }
        let host = authority
            .split_once(':')
            .map_or(authority, |(host, _port)| host)
            .trim_matches('.')
            .to_ascii_lowercase();
        normalize_cookie_domain(&host).ok()?;
        let suffix = &rest[authority_end..];
        let path = if suffix.starts_with('/') {
            let path_end = suffix.find(['?', '#']).unwrap_or(suffix.len());
            suffix[..path_end].to_string()
        } else {
            "/".to_string()
        };
        Some(Self {
            host,
            path,
            is_https,
        })
    }

    pub(crate) fn host(&self) -> &str {
        &self.host
    }

    pub(crate) fn path(&self) -> &str {
        &self.path
    }

    pub(crate) fn is_https(&self) -> bool {
        self.is_https
    }
}

impl fmt::Debug for ScopedCookie {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScopedCookie")
            .field("name", &"[redacted]")
            .field("value", &"[redacted]")
            .finish()
    }
}

impl Drop for ScopedCookie {
    fn drop(&mut self) {
        if let Some(domain) = &mut self.domain {
            domain.clear();
        }
        if let Some(path) = &mut self.path {
            path.clear();
        }
        self.name.clear();
        self.value.clear();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionMaterialError {
    InvalidProvider,
    EmptyCookieName,
    CookieNameTooLong,
    InvalidCookieName,
    CookieValueTooLong,
    InvalidCookieValue,
    InvalidCookieDomain,
    InvalidCookiePath,
    TooManyCookies,
    HeaderTooLarge,
}

fn validate_cookie_name(name: &str) -> Result<(), SessionMaterialError> {
    if name.is_empty() {
        return Err(SessionMaterialError::EmptyCookieName);
    }
    if name.len() > MAX_COOKIE_NAME_LEN {
        return Err(SessionMaterialError::CookieNameTooLong);
    }
    if name.bytes().any(|byte| {
        byte <= 0x20
            || byte >= 0x7f
            || matches!(
                byte,
                b'(' | b')'
                    | b'<'
                    | b'>'
                    | b'@'
                    | b','
                    | b';'
                    | b':'
                    | b'\\'
                    | b'"'
                    | b'/'
                    | b'['
                    | b']'
                    | b'?'
                    | b'='
                    | b'{'
                    | b'}'
            )
    }) {
        return Err(SessionMaterialError::InvalidCookieName);
    }
    Ok(())
}

fn validate_cookie_value(value: &str) -> Result<(), SessionMaterialError> {
    if value.len() > MAX_COOKIE_VALUE_LEN {
        return Err(SessionMaterialError::CookieValueTooLong);
    }
    // Strict RFC 6265 cookie-octet policy. The daemon does not serialize
    // quoted cookie values, so comma, semicolon, DQUOTE, backslash, whitespace,
    // and control bytes are rejected before a Cookie header is built.
    if value.bytes().any(|byte| {
        byte <= 0x20 || byte == 0x7f || matches!(byte, b',' | b';' | b'"' | b'\\' | b'\r' | b'\n')
    }) {
        return Err(SessionMaterialError::InvalidCookieValue);
    }
    Ok(())
}

#[derive(Clone, Eq, PartialEq)]
struct CookieDomain {
    host: String,
    host_only: bool,
}

impl CookieDomain {
    fn matches_host(&self, host: &str) -> bool {
        if self.host_only {
            return host == self.host;
        }
        host == self.host
            || host
                .strip_suffix(&self.host)
                .is_some_and(|prefix| prefix.ends_with('.'))
    }

    fn clear(&mut self) {
        self.host.clear();
    }
}

fn normalize_cookie_domain(domain: &str) -> Result<CookieDomain, SessionMaterialError> {
    let domain = domain.trim();
    let host_only = !domain.starts_with('.');
    let host = domain.trim_start_matches('.').to_ascii_lowercase();
    if host.is_empty()
        || host.len() > 253
        || host == "localhost"
        || host.ends_with(".localhost")
        || host.contains("..")
        || host.parse::<std::net::IpAddr>().is_ok()
    {
        return Err(SessionMaterialError::InvalidCookieDomain);
    }
    for label in host.split('.') {
        if label.is_empty()
            || label.starts_with('-')
            || label.ends_with('-')
            || !label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err(SessionMaterialError::InvalidCookieDomain);
        }
    }
    Ok(CookieDomain { host, host_only })
}

fn normalize_cookie_path(path: &str) -> Result<String, SessionMaterialError> {
    if path.is_empty()
        || !path.starts_with('/')
        || path.len() > MAX_COOKIE_PATH_LEN
        || path
            .bytes()
            .any(|byte| byte <= 0x1f || byte == 0x7f || matches!(byte, b'\r' | b'\n'))
    {
        return Err(SessionMaterialError::InvalidCookiePath);
    }
    Ok(path.to_string())
}

trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}

impl<T> Pipe for T {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_redacts_session_material() {
        let material = SessionMaterial::new(
            "codex",
            vec![ScopedCookie::new("quota_marker", "fixture-value-1")],
        );
        let debug = format!("{material:?}");
        assert!(debug.contains("SessionMaterial"));
        assert!(debug.contains("cookie_count"));
        assert!(!debug.contains("quota_marker"));
        assert!(!debug.contains("fixture-value-1"));
    }

    #[test]
    fn cookie_header_filters_by_domain_and_path() {
        let material = SessionMaterial::new(
            "codex",
            vec![
                ScopedCookie::try_new_for_domain(
                    ".chatgpt.com",
                    "/codex",
                    "chatgpt_safe",
                    "fixture-value-1",
                )
                .expect("chatgpt cookie"),
                ScopedCookie::try_new_for_domain(
                    ".openai.com",
                    "/",
                    "openai_safe",
                    "fixture-value-2",
                )
                .expect("openai cookie"),
            ],
        );

        let header = material
            .cookie_header_value_for_url("https://chatgpt.com/codex/settings/usage")
            .expect("chatgpt header");
        assert!(header.contains("chatgpt_safe=fixture-value-1"));
        assert!(!header.contains("openai_safe"));
        assert!(material
            .cookie_header_value_for_url("https://chatgpt.com/other")
            .is_none());
    }

    #[test]
    fn codex_policy_builds_static_dashboard_header_and_counts_eligibility() {
        let material = SessionMaterial::new(
            "codex",
            vec![
                ScopedCookie::try_new_for_domain(
                    ".chatgpt.com",
                    "/codex",
                    "codex_dash",
                    "fixture-value-1",
                )
                .expect("codex dashboard cookie"),
                ScopedCookie::try_new_for_domain(
                    ".chatgpt.com",
                    "/auth",
                    "codex_auth",
                    "fixture-value-2",
                )
                .expect("codex auth cookie"),
                ScopedCookie::try_new_for_domain(
                    ".openai.com",
                    "/",
                    "openai_only",
                    "fixture-value-3",
                )
                .expect("openai cookie"),
            ],
        );

        let header = material
            .cookie_header_value_for_url(SessionMaterialPolicy::codex_dashboard().target_url())
            .expect("codex header");
        let summary =
            material.cookie_header_eligibility_for_policy(SessionMaterialPolicy::codex_dashboard());
        let summary_json = serde_json::to_string(&summary).expect("summary json");

        assert_eq!(header, "codex_dash=fixture-value-1");
        assert_eq!(summary.total_cookies, 3);
        assert_eq!(summary.domain_matched, 2);
        assert_eq!(summary.path_matched, 1);
        assert_eq!(summary.secure_matched, 1);
        assert_eq!(summary.header_eligible, 1);
        assert_eq!(summary.rejected_domain, 1);
        assert_eq!(summary.rejected_path, 1);
        assert_eq!(summary.header_size_class, CookieHeaderSizeClass::Present);
        assert!(!summary_json.contains("codex_dash"));
        assert!(!summary_json.contains("fixture-value"));
        assert!(!summary_json.contains("chatgpt"));
        assert!(!summary_json.contains("openai"));
        crate::redact::validate_public_json_text(&summary_json).expect("summary is public-safe");
    }

    #[test]
    fn policy_summary_rejects_provider_mismatch_without_header_material() {
        let material = SessionMaterial::new(
            "claude",
            vec![ScopedCookie::try_new_for_domain(
                ".chatgpt.com",
                "/codex",
                "codex_dash",
                "fixture-value-1",
            )
            .expect("codex dashboard cookie")],
        );

        let summary =
            material.cookie_header_eligibility_for_policy(SessionMaterialPolicy::codex_dashboard());

        assert_eq!(summary.total_cookies, 1);
        assert_eq!(summary.rejected_provider, 1);
        assert_eq!(summary.header_eligible, 0);
        assert_eq!(summary.header_size_class, CookieHeaderSizeClass::Absent);
    }

    #[test]
    fn host_only_cookie_does_not_match_subdomains_but_domain_cookie_does() {
        let material = SessionMaterial::new(
            "codex",
            vec![
                ScopedCookie::try_new_for_domain(
                    "chatgpt.com",
                    "/",
                    "host_only",
                    "fixture-value-1",
                )
                .expect("host-only cookie"),
                ScopedCookie::try_new_for_domain(
                    ".chatgpt.com",
                    "/",
                    "domain_cookie",
                    "fixture-value-2",
                )
                .expect("domain cookie"),
            ],
        );

        let exact_header = material
            .cookie_header_value_for_url("https://chatgpt.com/codex/settings/usage")
            .expect("exact host header");
        let subdomain_header = material
            .cookie_header_value_for_url("https://sub.chatgpt.com/codex/settings/usage")
            .expect("subdomain header");

        assert!(exact_header.contains("host_only=fixture-value-1"));
        assert!(exact_header.contains("domain_cookie=fixture-value-2"));
        assert_eq!(subdomain_header, "domain_cookie=fixture-value-2");
    }

    #[test]
    fn secure_cookies_are_not_sent_to_http_targets() {
        let material = SessionMaterial::new(
            "codex",
            vec![
                ScopedCookie::try_new_for_domain_with_secure(
                    ".chatgpt.example.invalid",
                    "/codex",
                    true,
                    "secure_cookie",
                    "fixture-value-1",
                )
                .expect("secure cookie"),
                ScopedCookie::try_new_for_domain_with_secure(
                    ".chatgpt.example.invalid",
                    "/codex",
                    false,
                    "plain_cookie",
                    "fixture-value-2",
                )
                .expect("plain cookie"),
            ],
        );

        assert_eq!(
            material
                .cookie_header_value_for_url("http://chatgpt.example.invalid/codex/settings/usage")
                .as_deref(),
            Some("plain_cookie=fixture-value-2")
        );
        assert_eq!(
            material
                .cookie_header_value_for_url("https://chatgpt.example.invalid/codex/settings/usage")
                .as_deref(),
            Some("secure_cookie=fixture-value-1; plain_cookie=fixture-value-2")
        );
    }

    #[test]
    fn cookie_header_orders_longer_paths_first_with_stable_fallback() {
        let material = SessionMaterial::new(
            "codex",
            vec![
                ScopedCookie::try_new_for_domain(
                    ".chatgpt.example.invalid",
                    "/",
                    "root_cookie",
                    "fixture-value-1",
                )
                .expect("root cookie"),
                ScopedCookie::try_new_for_domain(
                    ".chatgpt.example.invalid",
                    "/codex/settings",
                    "settings_cookie",
                    "fixture-value-2",
                )
                .expect("settings cookie"),
                ScopedCookie::try_new_for_domain(
                    ".chatgpt.example.invalid",
                    "/codex",
                    "codex_cookie",
                    "fixture-value-3",
                )
                .expect("codex cookie"),
            ],
        );

        assert_eq!(
            material
                .cookie_header_value_for_url(
                    "https://chatgpt.example.invalid/codex/settings/usage"
                )
                .as_deref(),
            Some(
                "settings_cookie=fixture-value-2; codex_cookie=fixture-value-3; root_cookie=fixture-value-1"
            )
        );
    }

    #[test]
    fn cookie_header_uses_segment_aware_path_matching() {
        let material = SessionMaterial::new(
            "codex",
            vec![ScopedCookie::try_new_for_domain(
                ".chatgpt.com",
                "/codex/setting",
                "chatgpt_safe",
                "fixture-value-1",
            )
            .expect("chatgpt cookie")],
        );

        assert!(material
            .cookie_header_value_for_url("https://chatgpt.com/codex/setting")
            .is_some());
        assert!(material
            .cookie_header_value_for_url("https://chatgpt.com/codex/setting/details")
            .is_some());
        assert!(material
            .cookie_header_value_for_url("https://chatgpt.com/codex/settings/usage")
            .is_none());
    }

    #[test]
    fn cookie_header_allows_empty_cookie_values() {
        let material = SessionMaterial::new(
            "codex",
            vec![
                ScopedCookie::try_new_for_domain(".chatgpt.com", "/", "empty_value", "")
                    .expect("empty value cookie"),
            ],
        );

        assert_eq!(
            material
                .cookie_header_value_for_url("https://chatgpt.com/codex/settings/usage")
                .as_deref(),
            Some("empty_value=")
        );
    }

    #[test]
    fn invalid_cookie_material_is_rejected_before_header_construction() {
        assert_eq!(
            ScopedCookie::try_new("bad;name", "fixture").unwrap_err(),
            SessionMaterialError::InvalidCookieName
        );
        assert_eq!(
            ScopedCookie::try_new("good_name", "bad\r\nvalue").unwrap_err(),
            SessionMaterialError::InvalidCookieValue
        );
        assert_eq!(
            ScopedCookie::try_new("good_name", "bad,value").unwrap_err(),
            SessionMaterialError::InvalidCookieValue
        );
        assert_eq!(
            ScopedCookie::try_new_for_domain("127.0.0.1", "/", "good_name", "fixture").unwrap_err(),
            SessionMaterialError::InvalidCookieDomain
        );
        assert_eq!(
            SessionMaterial::try_new("codex/unsafe", vec![]).unwrap_err(),
            SessionMaterialError::InvalidProvider
        );
    }
}
