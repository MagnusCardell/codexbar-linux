use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::model::BrowserFamily;

#[derive(Clone, Eq, PartialEq)]
pub struct BrowserDiscoveryRoots {
    home: PathBuf,
    xdg_config_home: PathBuf,
}

impl fmt::Debug for BrowserDiscoveryRoots {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrowserDiscoveryRoots")
            .field("home", &"[redacted]")
            .field("xdg_config_home", &"[redacted]")
            .finish()
    }
}

impl BrowserDiscoveryRoots {
    pub fn new(home: PathBuf, xdg_config_home: PathBuf) -> Self {
        Self {
            home,
            xdg_config_home,
        }
    }

    pub fn synthetic_home(home: PathBuf) -> Self {
        Self {
            xdg_config_home: home.join(".config"),
            home,
        }
    }

    pub fn canonicalized(&self) -> Option<Self> {
        let home = fs::canonicalize(&self.home).ok()?;
        if !home.is_dir() {
            return None;
        }

        let xdg_config_home = if self.xdg_config_home.exists() {
            fs::canonicalize(&self.xdg_config_home).ok()?
        } else {
            home.join(".config")
        };
        if xdg_config_home.exists() && !xdg_config_home.is_dir() {
            return None;
        }
        if !path_is_under(&xdg_config_home, &home) {
            return None;
        }

        Some(Self {
            home,
            xdg_config_home,
        })
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    pub fn xdg_config_home(&self) -> &Path {
        &self.xdg_config_home
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChromiumRootKind {
    Chrome,
    Chromium,
    ChromiumSnap,
    Brave,
}

impl ChromiumRootKind {
    pub fn browser_family(self) -> BrowserFamily {
        match self {
            Self::Chrome => BrowserFamily::Chrome,
            Self::Chromium | Self::ChromiumSnap => BrowserFamily::Chromium,
            Self::Brave => BrowserFamily::Brave,
        }
    }

    pub fn id_prefix(self) -> &'static str {
        match self {
            Self::Chrome => "chrome",
            Self::Chromium => "chromium",
            Self::ChromiumSnap => "chromium-snap",
            Self::Brave => "brave",
        }
    }

    pub fn display_prefix(self) -> &'static str {
        match self {
            Self::Chrome => "Chrome",
            Self::Chromium => "Chromium",
            Self::ChromiumSnap => "Chromium Snap",
            Self::Brave => "Brave",
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct BrowserProfileDescriptor {
    browser_family: BrowserFamily,
    profile_id: String,
    display_name: String,
    root_kind: ChromiumRootKind,
    profile_path: PathBuf,
}

impl BrowserProfileDescriptor {
    pub fn new(
        root_kind: ChromiumRootKind,
        profile_label: &str,
        profile_path: PathBuf,
    ) -> Option<Self> {
        let profile_slug = profile_label_to_slug(profile_label)?;
        let profile_id = format!("{}-{profile_slug}", root_kind.id_prefix());
        let display_name = format!("{} {}", root_kind.display_prefix(), profile_label);
        Some(Self {
            browser_family: root_kind.browser_family(),
            profile_id,
            display_name,
            root_kind,
            profile_path,
        })
    }

    pub fn browser_family(&self) -> BrowserFamily {
        self.browser_family
    }

    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn root_kind(&self) -> ChromiumRootKind {
        self.root_kind
    }

    pub(crate) fn profile_path(&self) -> &Path {
        &self.profile_path
    }
}

impl fmt::Debug for BrowserProfileDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrowserProfileDescriptor")
            .field("browser_family", &self.browser_family)
            .field("profile_id", &self.profile_id)
            .field("display_name", &self.display_name)
            .field("root_kind", &self.root_kind)
            .field("profile_path", &"[redacted]")
            .finish()
    }
}

pub fn is_safe_profile_id(value: &str) -> bool {
    crate::config::is_safe_id(value)
        && !value.contains('/')
        && !value.contains('\\')
        && !value.contains('~')
        && !value.contains("..")
        && {
            let lower = value.to_ascii_lowercase();
            !lower.contains(".config")
                && !lower.contains("cookies")
                && !lower.contains("login-data")
                && !lower.contains("network")
        }
}

fn profile_label_to_slug(label: &str) -> Option<String> {
    if label == "Default" {
        return Some("default".to_string());
    }
    let suffix = label.strip_prefix("Profile ")?;
    if suffix.is_empty() || !suffix.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    Some(format!("profile-{suffix}"))
}

pub(crate) fn safe_profile_label(file_name: &str) -> Option<&str> {
    if file_name == "Default" {
        return Some(file_name);
    }
    let suffix = file_name.strip_prefix("Profile ")?;
    if suffix.is_empty() || !suffix.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    Some(file_name)
}

pub(crate) fn path_is_under(path: &Path, root: &Path) -> bool {
    path == root || path.starts_with(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_id_policy_rejects_path_like_values() {
        for value in [
            "/home/user/.config/chromium/Default",
            "chromium/Default",
            "chromium\\Default",
            "~chromium",
            "chromium..default",
            "chromium-.config",
            "chromium-Network-Cookies",
        ] {
            assert!(!is_safe_profile_id(value), "{value}");
        }
        assert!(is_safe_profile_id("chromium-default"));
        assert!(is_safe_profile_id("chrome-profile-1"));
    }
}
