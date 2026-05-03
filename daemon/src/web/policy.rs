use std::net::IpAddr;
use std::time::Duration;

use crate::web::client::WebRequest;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WebPolicyError {
    InvalidUrl,
    SchemeNotAllowed,
    UserInfoNotAllowed,
    PortNotAllowed,
    LocalAddressNotAllowed,
    HostNotAllowed,
    TargetNotAllowed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RedirectTargetClass {
    None,
    SameHostCanonical,
    SameHostUsagePath,
    SameHostLoginPath,
    SameHostOther,
    AllowedHostOther,
    BlockedHost,
    Invalid,
}

impl RedirectTargetClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::SameHostCanonical => "same_host_canonical",
            Self::SameHostUsagePath => "same_host_usage_path",
            Self::SameHostLoginPath => "same_host_login_path",
            Self::SameHostOther => "same_host_other",
            Self::AllowedHostOther => "allowed_host_other",
            Self::BlockedHost => "blocked_host",
            Self::Invalid => "invalid",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RedirectPathFamily {
    None,
    CodexUsage,
    CodexSettings,
    CodexOther,
    AuthLogin,
    AuthCallback,
    Root,
    StaticAsset,
    Api,
    Unknown,
    Invalid,
}

impl RedirectPathFamily {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::CodexUsage => "codex_usage",
            Self::CodexSettings => "codex_settings",
            Self::CodexOther => "codex_other",
            Self::AuthLogin => "auth_login",
            Self::AuthCallback => "auth_callback",
            Self::Root => "root",
            Self::StaticAsset => "static_asset",
            Self::Api => "api",
            Self::Unknown => "unknown",
            Self::Invalid => "invalid",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RedirectPathDepth {
    Zero,
    One,
    Two,
    Three,
    Many,
    Unknown,
}

impl RedirectPathDepth {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Zero => "zero",
            Self::One => "one",
            Self::Two => "two",
            Self::Three => "three",
            Self::Many => "many",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RedirectQueryClass {
    None,
    SafeEmpty,
    TokenLike,
    Present,
    Unknown,
}

impl RedirectQueryClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::SafeEmpty => "safe_empty",
            Self::TokenLike => "token_like",
            Self::Present => "present",
            Self::Unknown => "unknown",
        }
    }

    fn follow_safe(self) -> bool {
        matches!(self, Self::None | Self::SafeEmpty)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RedirectTargetSummary {
    target_class: RedirectTargetClass,
    path_family: RedirectPathFamily,
    path_depth: RedirectPathDepth,
    query_class: RedirectQueryClass,
    can_follow: bool,
}

impl RedirectTargetSummary {
    const fn none() -> Self {
        Self {
            target_class: RedirectTargetClass::None,
            path_family: RedirectPathFamily::None,
            path_depth: RedirectPathDepth::Unknown,
            query_class: RedirectQueryClass::None,
            can_follow: false,
        }
    }

    const fn invalid() -> Self {
        Self {
            target_class: RedirectTargetClass::Invalid,
            path_family: RedirectPathFamily::Invalid,
            path_depth: RedirectPathDepth::Unknown,
            query_class: RedirectQueryClass::Unknown,
            can_follow: false,
        }
    }

    pub(crate) fn target_class(&self) -> RedirectTargetClass {
        self.target_class
    }

    pub(crate) fn target_class_str(&self) -> &'static str {
        self.target_class.as_str()
    }

    pub(crate) fn path_family_str(&self) -> &'static str {
        self.path_family.as_str()
    }

    pub(crate) fn path_depth_str(&self) -> &'static str {
        self.path_depth.as_str()
    }

    pub(crate) fn query_class_str(&self) -> &'static str {
        self.query_class.as_str()
    }

    pub(crate) fn can_follow(&self) -> bool {
        self.can_follow
    }

    fn followable(&self) -> bool {
        self.can_follow
    }
}

#[derive(Clone, Debug)]
pub struct CodexWebPolicy {
    request_hosts: &'static [&'static str],
    redirect_hosts: &'static [&'static str],
    cookie_domains: &'static [&'static str],
    dashboard_url: &'static str,
    dashboard_path: &'static str,
    timeout: Duration,
    response_size_limit: usize,
}

impl CodexWebPolicy {
    pub const fn new() -> Self {
        Self {
            request_hosts: &["chatgpt.com"],
            redirect_hosts: &["chatgpt.com"],
            cookie_domains: &["chatgpt.com"],
            dashboard_url: "https://chatgpt.com/codex/settings/usage",
            dashboard_path: "/codex/settings/usage",
            timeout: Duration::from_secs(15),
            response_size_limit: 512 * 1024,
        }
    }

    #[doc(hidden)]
    pub fn with_redirect_hosts_for_tests(redirect_hosts: &'static [&'static str]) -> Self {
        Self {
            redirect_hosts,
            ..Self::new()
        }
    }

    pub fn dashboard_url(&self) -> &'static str {
        self.dashboard_url
    }

    pub fn request_hosts(&self) -> &'static [&'static str] {
        self.request_hosts
    }

    pub fn redirect_hosts(&self) -> &'static [&'static str] {
        self.redirect_hosts
    }

    pub fn cookie_domains(&self) -> &'static [&'static str] {
        self.cookie_domains
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    pub fn response_size_limit(&self) -> usize {
        self.response_size_limit
    }

    pub fn dashboard_request(&self) -> WebRequest {
        WebRequest::new(self.dashboard_url)
            .with_timeout(self.timeout)
            .with_response_size_limit(self.response_size_limit)
    }

    pub fn redirect_follow_request(&self, url: &str) -> Result<WebRequest, WebPolicyError> {
        self.validate_follow_redirect_url(url)?;
        Ok(WebRequest::new_policy_validated_redirect(url.to_string())
            .with_timeout(self.timeout)
            .with_response_size_limit(self.response_size_limit))
    }

    pub fn validate_dashboard_url(&self, url: &str) -> Result<(), WebPolicyError> {
        let parsed = ParsedHttpsUrl::parse(url)?;
        ensure_not_local(&parsed.host)?;
        if parsed.port.is_some() {
            return Err(WebPolicyError::PortNotAllowed);
        }
        if !self.request_hosts.contains(&parsed.host.as_str()) {
            return Err(WebPolicyError::HostNotAllowed);
        }
        if parsed.path != self.dashboard_path || parsed.query_or_fragment {
            return Err(WebPolicyError::TargetNotAllowed);
        }
        if url != self.dashboard_url {
            return Err(WebPolicyError::TargetNotAllowed);
        }
        Ok(())
    }

    pub fn validate_candidate_url(&self, url: &str) -> Result<(), WebPolicyError> {
        let parsed = ParsedHttpsUrl::parse(url)?;
        ensure_not_local(&parsed.host)?;
        if parsed.port.is_some() {
            return Err(WebPolicyError::PortNotAllowed);
        }
        if !self.request_hosts.contains(&parsed.host.as_str()) {
            return Err(WebPolicyError::HostNotAllowed);
        }
        if parsed.query_or_fragment {
            return Err(WebPolicyError::TargetNotAllowed);
        }
        Ok(())
    }

    pub fn validate_redirect_url(&self, url: &str) -> Result<(), WebPolicyError> {
        self.validate_follow_redirect_url(url)
    }

    pub fn validate_follow_redirect_url(&self, url: &str) -> Result<(), WebPolicyError> {
        if self
            .classify_redirect_target_summary(Some(url), false)
            .followable()
        {
            Ok(())
        } else {
            Err(WebPolicyError::TargetNotAllowed)
        }
    }

    pub fn validate_follow_response_url(&self, url: &str) -> Result<(), WebPolicyError> {
        self.validate_follow_redirect_url(url)
    }

    pub fn classify_redirect_target(
        &self,
        redirect_url: Option<&str>,
        redirect_invalid: bool,
    ) -> RedirectTargetClass {
        self.classify_redirect_target_summary(redirect_url, redirect_invalid)
            .target_class()
    }

    pub(crate) fn classify_redirect_target_summary(
        &self,
        redirect_url: Option<&str>,
        redirect_invalid: bool,
    ) -> RedirectTargetSummary {
        if redirect_invalid {
            return RedirectTargetSummary::invalid();
        }
        let Some(url) = redirect_url else {
            return RedirectTargetSummary::none();
        };
        let Ok(parsed) = url::Url::parse(url) else {
            return RedirectTargetSummary::invalid();
        };

        let fragment_present = parsed.fragment().is_some();
        let query_class = classify_redirect_query(parsed.query());
        let host = parsed.host_str().map(str::to_ascii_lowercase);
        let explicit_port = url_has_explicit_authority_port(url);
        let host_allowed = !explicit_port
            && host
                .as_deref()
                .is_some_and(|host| ensure_not_local(host).is_ok())
            && host
                .as_deref()
                .is_some_and(|host| self.redirect_hosts.contains(&host));
        let path = parsed.path();
        let path_depth = classify_redirect_path_depth(path);
        let invalid_shape = parsed.scheme() != "https"
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || explicit_port
            || fragment_present;
        let path_family = if invalid_shape {
            RedirectPathFamily::Invalid
        } else if host_allowed && host.as_deref() == Some("chatgpt.com") {
            self.classify_redirect_path_family(path)
        } else {
            RedirectPathFamily::Unknown
        };
        let target_class = if invalid_shape {
            RedirectTargetClass::Invalid
        } else if !host_allowed {
            RedirectTargetClass::BlockedHost
        } else if host.as_deref() != Some("chatgpt.com") {
            RedirectTargetClass::AllowedHostOther
        } else if query_class == RedirectQueryClass::TokenLike
            || path_family == RedirectPathFamily::Invalid
        {
            RedirectTargetClass::Invalid
        } else if matches!(
            path_family,
            RedirectPathFamily::AuthLogin | RedirectPathFamily::AuthCallback
        ) {
            RedirectTargetClass::SameHostLoginPath
        } else if path_family == RedirectPathFamily::CodexUsage {
            RedirectTargetClass::SameHostUsagePath
        } else if path_family == RedirectPathFamily::CodexSettings {
            RedirectTargetClass::SameHostCanonical
        } else {
            RedirectTargetClass::SameHostOther
        };
        let can_follow = !invalid_shape
            && host_allowed
            && host.as_deref() == Some("chatgpt.com")
            && matches!(
                target_class,
                RedirectTargetClass::SameHostCanonical | RedirectTargetClass::SameHostUsagePath
            )
            && matches!(
                path_family,
                RedirectPathFamily::CodexUsage | RedirectPathFamily::CodexSettings
            )
            && query_class.follow_safe();
        RedirectTargetSummary {
            target_class,
            path_family,
            path_depth,
            query_class,
            can_follow,
        }
    }

    pub fn should_follow_redirect(&self, redirect_url: &str) -> bool {
        self.classify_redirect_target_summary(Some(redirect_url), false)
            .followable()
    }

    fn is_dashboard_path_with_slash(&self, path: &str) -> bool {
        path.len() == self.dashboard_path.len() + 1
            && path.starts_with(self.dashboard_path)
            && path.ends_with('/')
    }

    fn classify_redirect_path_family(&self, path: &str) -> RedirectPathFamily {
        let lower = path.to_ascii_lowercase();
        if lower == "/" {
            return RedirectPathFamily::Root;
        }
        if path_has_auth_callback_segment(&lower) {
            return RedirectPathFamily::AuthCallback;
        }
        if path_has_auth_login_segment(&lower) {
            return RedirectPathFamily::AuthLogin;
        }
        if path_is_api_family(&lower) {
            return RedirectPathFamily::Api;
        }
        if path_is_static_asset_family(&lower) {
            return RedirectPathFamily::StaticAsset;
        }
        if path == self.dashboard_path
            || self.is_dashboard_path_with_slash(path)
            || path == "/codex/cloud/settings/usage"
            || lower == "/codex/usage"
            || lower == "/codex/usage/"
        {
            return RedirectPathFamily::CodexUsage;
        }
        if lower == "/codex/settings" || lower == "/codex/settings/" {
            return RedirectPathFamily::CodexSettings;
        }
        if lower == "/codex" || lower == "/codex/" || lower.starts_with("/codex/") {
            return RedirectPathFamily::CodexOther;
        }
        RedirectPathFamily::Unknown
    }
}

impl Default for CodexWebPolicy {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ParsedHttpsUrl {
    host: String,
    path: String,
    port: Option<u16>,
    query_or_fragment: bool,
}

impl ParsedHttpsUrl {
    fn parse(url: &str) -> Result<Self, WebPolicyError> {
        let rest = url
            .strip_prefix("https://")
            .ok_or(WebPolicyError::SchemeNotAllowed)?;
        let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
        let authority = &rest[..authority_end];
        if authority.is_empty() {
            return Err(WebPolicyError::InvalidUrl);
        }
        if authority.contains('@') {
            return Err(WebPolicyError::UserInfoNotAllowed);
        }
        let (host, port) = parse_authority(authority)?;
        let suffix = &rest[authority_end..];
        let query_or_fragment = suffix.contains('?') || suffix.contains('#');
        let path = if suffix.starts_with('/') {
            let path_end = suffix.find(['?', '#']).unwrap_or(suffix.len());
            suffix[..path_end].to_string()
        } else {
            "/".to_string()
        };
        Ok(Self {
            host: host.to_ascii_lowercase(),
            path,
            port,
            query_or_fragment,
        })
    }
}

fn classify_redirect_path_depth(path: &str) -> RedirectPathDepth {
    match path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .count()
    {
        0 => RedirectPathDepth::Zero,
        1 => RedirectPathDepth::One,
        2 => RedirectPathDepth::Two,
        3 => RedirectPathDepth::Three,
        4.. => RedirectPathDepth::Many,
    }
}

fn path_has_auth_callback_segment(path: &str) -> bool {
    path.split('/')
        .filter(|segment| !segment.is_empty())
        .any(|segment| {
            segment.contains("callback")
                || segment.contains("oauth")
                || segment.contains("oidc")
                || segment.contains("saml")
        })
}

fn path_has_auth_login_segment(path: &str) -> bool {
    path.split('/')
        .filter(|segment| !segment.is_empty())
        .any(|segment| {
            matches!(
                segment,
                "auth" | "login" | "signin" | "sign-in" | "log-in" | "sso"
            ) || segment.ends_with("-login")
                || segment.ends_with("_login")
                || segment.ends_with("-signin")
                || segment.ends_with("_signin")
        })
}

fn path_is_api_family(path: &str) -> bool {
    matches!(path, "/api" | "/backend-api" | "/public-api")
        || path.starts_with("/api/")
        || path.starts_with("/backend-api/")
        || path.starts_with("/public-api/")
}

fn path_is_static_asset_family(path: &str) -> bool {
    matches!(
        path,
        "/favicon.ico" | "/manifest.json" | "/robots.txt" | "/sitemap.xml"
    ) || path.starts_with("/_next/")
        || path.starts_with("/assets/")
        || path.starts_with("/static/")
        || path.starts_with("/cdn-cgi/")
        || path.ends_with(".css")
        || path.ends_with(".js")
        || path.ends_with(".map")
        || path.ends_with(".png")
        || path.ends_with(".jpg")
        || path.ends_with(".jpeg")
        || path.ends_with(".svg")
        || path.ends_with(".ico")
        || path.ends_with(".webp")
        || path.ends_with(".woff")
        || path.ends_with(".woff2")
}

fn classify_redirect_query(query: Option<&str>) -> RedirectQueryClass {
    let Some(query) = query else {
        return RedirectQueryClass::None;
    };
    if query.is_empty() {
        return RedirectQueryClass::SafeEmpty;
    }
    if query.len() > 128 || query_is_token_like(query) {
        return RedirectQueryClass::TokenLike;
    }
    let pairs = url::form_urlencoded::parse(query.as_bytes()).collect::<Vec<_>>();
    if pairs.is_empty() || pairs.len() > 4 {
        return RedirectQueryClass::Present;
    }
    for (key, value) in pairs {
        if key.is_empty()
            || key.len() > 32
            || value.len() > 64
            || !query_component_is_safe(&key)
            || !query_component_is_safe(&value)
            || query_is_token_like(&key)
            || query_is_token_like(&value)
        {
            return RedirectQueryClass::TokenLike;
        }
    }
    RedirectQueryClass::Present
}

fn url_has_explicit_authority_port(url: &str) -> bool {
    let Some((_, rest)) = url.split_once("://") else {
        return false;
    };
    authority_has_explicit_port(rest)
}

fn authority_has_explicit_port(rest: &str) -> bool {
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    let authority = authority
        .rsplit_once('@')
        .map_or(authority, |(_, authority)| authority);
    if let Some(ipv6_rest) = authority.strip_prefix('[') {
        let Some(end) = ipv6_rest.find(']') else {
            return false;
        };
        return ipv6_rest[end + 1..].starts_with(':');
    }
    authority.contains(':')
}

fn query_component_is_safe(value: &str) -> bool {
    value.bytes().all(|byte| {
        matches!(
            byte,
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~'
        )
    })
}

fn query_is_token_like(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if [
        "access", "auth", "bearer", "code", "continue", "cookie", "csrf", "id_token", "jwt", "key",
        "login", "next", "redirect", "refresh", "return", "secret", "session", "sso", "state",
        "token", "url",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        return true;
    }
    value.len() >= 16
        && value.bytes().all(|byte| {
            matches!(
                byte,
                b'A'..=b'Z'
                    | b'a'..=b'z'
                    | b'0'..=b'9'
                    | b'-'
                    | b'_'
                    | b'.'
                    | b'~'
            )
        })
}

fn parse_authority(authority: &str) -> Result<(String, Option<u16>), WebPolicyError> {
    if let Some(rest) = authority.strip_prefix('[') {
        let end = rest.find(']').ok_or(WebPolicyError::InvalidUrl)?;
        let host = &rest[..end];
        let after = &rest[end + 1..];
        let port = if let Some(port_text) = after.strip_prefix(':') {
            Some(parse_port(port_text)?)
        } else if after.is_empty() {
            None
        } else {
            return Err(WebPolicyError::InvalidUrl);
        };
        return Ok((host.to_string(), port));
    }
    if authority.matches(':').count() > 1 {
        return Err(WebPolicyError::InvalidUrl);
    }
    match authority.split_once(':') {
        Some((host, port)) if !host.is_empty() => Ok((host.to_string(), Some(parse_port(port)?))),
        Some(_) => Err(WebPolicyError::InvalidUrl),
        None => Ok((authority.to_string(), None)),
    }
}

fn parse_port(port: &str) -> Result<u16, WebPolicyError> {
    if port.is_empty() {
        return Err(WebPolicyError::InvalidUrl);
    }
    port.parse::<u16>().map_err(|_| WebPolicyError::InvalidUrl)
}

fn ensure_not_local(host: &str) -> Result<(), WebPolicyError> {
    let host = host.trim_matches('.').to_ascii_lowercase();
    if host == "localhost" || host.ends_with(".localhost") {
        return Err(WebPolicyError::LocalAddressNotAllowed);
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        if ip_is_local(ip) {
            return Err(WebPolicyError::LocalAddressNotAllowed);
        }
    }
    Ok(())
}

fn ip_is_local(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_unspecified()
                || ip.octets()[0] == 0
        }
        IpAddr::V6(ip) => {
            let first = ip.segments()[0];
            ip.is_loopback()
                || ip.is_unspecified()
                || (first & 0xfe00) == 0xfc00
                || (first & 0xffc0) == 0xfe80
        }
    }
}
