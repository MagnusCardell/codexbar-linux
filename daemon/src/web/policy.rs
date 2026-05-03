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
pub(crate) struct RedirectTargetSummary {
    target_class: RedirectTargetClass,
    scheme_class: &'static str,
    host_class: &'static str,
    query_class: &'static str,
    fragment_class: &'static str,
}

impl RedirectTargetSummary {
    const fn none() -> Self {
        Self {
            target_class: RedirectTargetClass::None,
            scheme_class: "none",
            host_class: "none",
            query_class: "none",
            fragment_class: "none",
        }
    }

    const fn invalid() -> Self {
        Self {
            target_class: RedirectTargetClass::Invalid,
            scheme_class: "invalid",
            host_class: "invalid",
            query_class: "invalid",
            fragment_class: "invalid",
        }
    }

    pub(crate) fn target_class(&self) -> RedirectTargetClass {
        self.target_class
    }

    pub(crate) fn target_class_str(&self) -> &'static str {
        self.target_class.as_str()
    }

    fn followable(&self) -> bool {
        matches!(
            self.target_class,
            RedirectTargetClass::SameHostCanonical | RedirectTargetClass::SameHostUsagePath
        ) && self.scheme_class == "https"
            && self.host_class == "allowed"
            && matches!(self.query_class, "none" | "empty" | "safe")
            && self.fragment_class == "none"
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

        let scheme_class = if parsed.scheme() == "https" {
            "https"
        } else {
            "blocked"
        };
        let fragment_class = if parsed.fragment().is_some() {
            "present"
        } else {
            "none"
        };
        let (query_class, query_allowed) = classify_redirect_query(parsed.query());
        let host = parsed.host_str().map(str::to_ascii_lowercase);
        let host_allowed = parsed.port().is_none()
            && host
                .as_deref()
                .is_some_and(|host| ensure_not_local(host).is_ok())
            && host
                .as_deref()
                .is_some_and(|host| self.redirect_hosts.contains(&host));
        let host_class = if host_allowed { "allowed" } else { "blocked" };
        let path = parsed.path();
        let target_class = if parsed.scheme() != "https"
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.port().is_some()
            || fragment_class != "none"
        {
            RedirectTargetClass::Invalid
        } else if !host_allowed {
            RedirectTargetClass::BlockedHost
        } else if host.as_deref() != Some("chatgpt.com") {
            RedirectTargetClass::AllowedHostOther
        } else if self.is_login_path(path) {
            RedirectTargetClass::SameHostLoginPath
        } else if !query_allowed {
            RedirectTargetClass::Invalid
        } else if path == self.dashboard_path || self.is_dashboard_path_with_slash(path) {
            RedirectTargetClass::SameHostCanonical
        } else if self.is_canonical_usage_path(path) {
            RedirectTargetClass::SameHostUsagePath
        } else if host.as_deref() == Some("chatgpt.com") {
            RedirectTargetClass::SameHostOther
        } else {
            RedirectTargetClass::BlockedHost
        };
        RedirectTargetSummary {
            target_class,
            scheme_class,
            host_class,
            query_class,
            fragment_class,
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

    fn is_canonical_usage_path(&self, _path: &str) -> bool {
        false
    }

    fn is_login_path(&self, path: &str) -> bool {
        let lower = path.to_ascii_lowercase();
        lower.contains("/auth/")
            || lower.contains("/login")
            || lower.contains("/log-in")
            || lower.ends_with("/auth")
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

fn classify_redirect_query(query: Option<&str>) -> (&'static str, bool) {
    let Some(query) = query else {
        return ("none", true);
    };
    if query.is_empty() {
        return ("empty", true);
    }
    if query.len() > 128 || query_is_token_like(query) {
        return ("token_like", false);
    }
    let pairs = url::form_urlencoded::parse(query.as_bytes()).collect::<Vec<_>>();
    if pairs.is_empty() || pairs.len() > 4 {
        return ("unsafe", false);
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
            return ("unsafe", false);
        }
    }
    ("safe", true)
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
