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
            cookie_domains: &["chatgpt.com", "openai.com"],
            dashboard_url: "https://chatgpt.com/codex/settings/usage",
            dashboard_path: "/codex/settings/usage",
            timeout: Duration::from_secs(15),
            response_size_limit: 512 * 1024,
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
        let parsed = ParsedHttpsUrl::parse(url)?;
        ensure_not_local(&parsed.host)?;
        if parsed.port.is_some() {
            return Err(WebPolicyError::PortNotAllowed);
        }
        if !self.redirect_hosts.contains(&parsed.host.as_str()) {
            return Err(WebPolicyError::HostNotAllowed);
        }
        if parsed.query_or_fragment {
            return Err(WebPolicyError::TargetNotAllowed);
        }
        Ok(())
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
