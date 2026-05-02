use std::fmt;

const MAX_COOKIE_NAME_LEN: usize = 256;
const MAX_COOKIE_VALUE_LEN: usize = 4096;
const MAX_COOKIE_PATH_LEN: usize = 1024;
const MAX_COOKIE_COUNT: usize = 128;
const MAX_COOKIE_HEADER_LEN: usize = 16 * 1024;

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

    pub(crate) fn cookie_header_value_for_url(&self, url: &str) -> Option<String> {
        let (host, path) = request_parts(url)?;
        self.cookie_header_value_for_request(Some(host.as_str()), Some(path.as_str()))
    }

    fn cookie_header_value_for_request(
        &self,
        request_host: Option<&str>,
        request_path: Option<&str>,
    ) -> Option<String> {
        if self.cookies.is_empty() {
            return None;
        }
        let value = self
            .cookies
            .iter()
            .filter(|cookie| cookie.applies_to(request_host, request_path))
            .map(ScopedCookie::header_pair)
            .collect::<Vec<_>>()
            .join("; ");
        if value.is_empty() || value.len() > MAX_COOKIE_HEADER_LEN {
            None
        } else {
            Some(value)
        }
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
    domain: Option<String>,
    path: Option<String>,
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
        Self::try_new_scoped(None, None, name, value)
    }

    pub fn try_new_for_domain(
        domain: impl AsRef<str>,
        path: impl AsRef<str>,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, SessionMaterialError> {
        Self::try_new_scoped(
            Some(normalize_cookie_domain(domain.as_ref())?),
            Some(normalize_cookie_path(path.as_ref())?),
            name,
            value,
        )
    }

    fn try_new_scoped(
        domain: Option<String>,
        path: Option<String>,
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
            name,
            value,
        }
        .pipe(Ok)
    }

    fn header_pair(&self) -> String {
        format!("{}={}", self.name, self.value)
    }

    fn applies_to(&self, request_host: Option<&str>, request_path: Option<&str>) -> bool {
        if let Some(domain) = &self.domain {
            let Some(host) = request_host.map(|host| host.trim_matches('.').to_ascii_lowercase())
            else {
                return true;
            };
            if host != *domain && !host.ends_with(&format!(".{domain}")) {
                return false;
            }
        }
        if let Some(cookie_path) = &self.path {
            let request_path = request_path.unwrap_or("/");
            if !request_path.starts_with(cookie_path) {
                return false;
            }
        }
        true
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
    InvalidCookieName,
    InvalidCookieValue,
    InvalidCookieDomain,
    InvalidCookiePath,
    TooManyCookies,
    HeaderTooLarge,
}

fn validate_cookie_name(name: &str) -> Result<(), SessionMaterialError> {
    if name.is_empty() || name.len() > MAX_COOKIE_NAME_LEN {
        return Err(SessionMaterialError::InvalidCookieName);
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
    if value.is_empty() || value.len() > MAX_COOKIE_VALUE_LEN {
        return Err(SessionMaterialError::InvalidCookieValue);
    }
    if value.bytes().any(|byte| {
        byte <= 0x1f || byte == 0x7f || matches!(byte, b';' | b'"' | b'\\' | b'\r' | b'\n')
    }) {
        return Err(SessionMaterialError::InvalidCookieValue);
    }
    Ok(())
}

fn normalize_cookie_domain(domain: &str) -> Result<String, SessionMaterialError> {
    let domain = domain.trim().trim_start_matches('.').to_ascii_lowercase();
    if domain.is_empty()
        || domain.len() > 253
        || domain == "localhost"
        || domain.ends_with(".localhost")
        || domain.contains("..")
        || domain.parse::<std::net::IpAddr>().is_ok()
    {
        return Err(SessionMaterialError::InvalidCookieDomain);
    }
    for label in domain.split('.') {
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
    Ok(domain)
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

fn request_parts(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("https://")?;
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
    Some((host, path))
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
            ScopedCookie::try_new_for_domain("127.0.0.1", "/", "good_name", "fixture").unwrap_err(),
            SessionMaterialError::InvalidCookieDomain
        );
        assert_eq!(
            SessionMaterial::try_new("codex/unsafe", vec![]).unwrap_err(),
            SessionMaterialError::InvalidProvider
        );
    }
}
