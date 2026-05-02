use std::fs;
use std::path::{Path, PathBuf};

use crate::browser::diagnostics;
use crate::browser::profile::{
    path_is_under, safe_profile_label, BrowserDiscoveryRoots, BrowserProfileDescriptor,
    ChromiumRootKind,
};

#[derive(Clone, Debug, Default)]
pub struct ChromiumDiscovery {
    pub profiles: Vec<BrowserProfileDescriptor>,
    pub diagnostic_codes: Vec<String>,
}

pub fn discover_chromium_profiles(roots: &BrowserDiscoveryRoots) -> ChromiumDiscovery {
    let mut output = ChromiumDiscovery::default();
    for (kind, root) in chromium_roots(roots) {
        discover_root(kind, root, roots, &mut output);
    }
    if output.profiles.is_empty() {
        diagnostics::push_code(&mut output.diagnostic_codes, diagnostics::NOT_FOUND);
        diagnostics::push_code(&mut output.diagnostic_codes, diagnostics::PROFILE_NOT_FOUND);
    }
    output
}

fn chromium_roots(roots: &BrowserDiscoveryRoots) -> Vec<(ChromiumRootKind, PathBuf)> {
    vec![
        (
            ChromiumRootKind::Chrome,
            roots.xdg_config_home().join("google-chrome"),
        ),
        (
            ChromiumRootKind::Chromium,
            roots.xdg_config_home().join("chromium"),
        ),
        (
            ChromiumRootKind::Brave,
            roots
                .xdg_config_home()
                .join("BraveSoftware")
                .join("Brave-Browser"),
        ),
        (
            ChromiumRootKind::ChromiumSnap,
            roots
                .home()
                .join("snap")
                .join("chromium")
                .join("common")
                .join("chromium"),
        ),
    ]
}

fn discover_root(
    kind: ChromiumRootKind,
    root: PathBuf,
    roots: &BrowserDiscoveryRoots,
    output: &mut ChromiumDiscovery,
) {
    if !root.exists() {
        return;
    }
    let canonical_root = match fs::canonicalize(&root) {
        Ok(path) => path,
        Err(_) => {
            diagnostics::push_code(
                &mut output.diagnostic_codes,
                diagnostics::PROFILE_UNREADABLE,
            );
            return;
        }
    };
    if !root_is_within_allowed_roots(&canonical_root, roots) {
        diagnostics::push_code(&mut output.diagnostic_codes, diagnostics::PROFILE_SKIPPED);
        return;
    }
    let entries = match fs::read_dir(&canonical_root) {
        Ok(entries) => entries,
        Err(_) => {
            diagnostics::push_code(
                &mut output.diagnostic_codes,
                diagnostics::PROFILE_UNREADABLE,
            );
            return;
        }
    };
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let Some(label) = safe_profile_label(file_name.as_ref()) else {
            continue;
        };
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => {
                diagnostics::push_code(&mut output.diagnostic_codes, diagnostics::PROFILE_SKIPPED);
                continue;
            }
        };
        if !file_type.is_dir() && !file_type.is_symlink() {
            continue;
        }
        let profile_path = entry.path();
        let canonical_profile = match fs::canonicalize(&profile_path) {
            Ok(path) => path,
            Err(_) => {
                diagnostics::push_code(&mut output.diagnostic_codes, diagnostics::PROFILE_SKIPPED);
                continue;
            }
        };
        if !path_is_under(&canonical_profile, &canonical_root) {
            diagnostics::push_code(&mut output.diagnostic_codes, diagnostics::PROFILE_SKIPPED);
            continue;
        }
        if let Some(profile) = BrowserProfileDescriptor::new(kind, label, canonical_profile) {
            diagnostics::push_code(
                &mut output.diagnostic_codes,
                diagnostics::PROFILE_DISCOVERED,
            );
            output.profiles.push(profile);
        }
    }
}

fn root_is_within_allowed_roots(root: &Path, roots: &BrowserDiscoveryRoots) -> bool {
    [roots.home(), roots.xdg_config_home()]
        .into_iter()
        .filter_map(|base| fs::canonicalize(base).ok())
        .any(|base| path_is_under(root, &base))
}
