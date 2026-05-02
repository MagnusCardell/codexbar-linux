use std::fmt;

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
        Self {
            provider: provider.into(),
            cookies,
        }
    }

    pub fn cookie_count(&self) -> usize {
        self.cookies.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cookies.is_empty()
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
    name: String,
    value: String,
}

impl ScopedCookie {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
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
        self.name.clear();
        self.value.clear();
    }
}

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
}
