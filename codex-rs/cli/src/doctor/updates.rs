//! Diagnoses whether Codex update paths target the running installation.
//!
//! Update diagnostics combine cached version metadata, install-channel hints,
//! and bounded latest-version probes. For npm-managed launches, this module also
//! verifies that npm install -g would update the package root that launched the
//! current process, which catches PATH and prefix mismatches before the user runs
//! an update command.

use std::path::Path;

use codex_core::config::Config;
use codex_install_context::InstallContext;
use serde::Deserialize;

use super::CheckStatus;
use super::DoctorCheck;
use super::NpmRootCheck;
use super::doctor_install_context;
use super::doctor_managed_by_npm;
use super::npm_global_root_check;
use super::run_command;

const VERSION_FILE_NAME: &str = "version.json";
const GITHUB_LATEST_RELEASE_URL: &str = "https://api.github.com/repos/openai/codex/releases/latest";
const HOMEBREW_CASK_API_URL: &str = "https://formulae.brew.sh/api/cask/codex.json";

/// Builds the update-health row for the current installation.
///
/// Network failures while fetching latest-version metadata degrade the row to a
/// warning instead of failing doctor outright; update freshness is useful
/// support context but should not mask more direct install/config failures.
pub(super) fn updates_check(config: &Config) -> DoctorCheck {
    let current_exe = std::env::current_exe().ok();
    let install_context = doctor_install_context(current_exe.as_deref());
    let mut details = vec![
        format!(
            "check for update on startup: {}",
            config.check_for_update_on_startup
        ),
        format!("update action: {}", update_action_label(&install_context)),
    ];
    let version_file = config.codex_home.join(VERSION_FILE_NAME);
    push_cached_version_details(&mut details, &version_file);

    let mut status = CheckStatus::Ok;
    let mut summary = "update configuration is locally consistent".to_string();
    let mut remediation = None;

    if doctor_managed_by_npm(current_exe.as_deref()) {
        match npm_global_root_check() {
            NpmRootCheck::Match { package_root } => {
                details.push(format!("npm update target: {}", package_root.display()));
            }
            NpmRootCheck::Mismatch {
                running_package_root,
                npm_package_root,
            } => {
                status = CheckStatus::Fail;
                summary = "update would target a different npm install".to_string();
                details.push(format!(
                    "running package root: {}",
                    running_package_root.display()
                ));
                details.push(format!("npm package root: {}", npm_package_root.display()));
                remediation = Some(format!(
                    "Fix PATH or npm prefix so the running package root ({}) matches the npm global package root ({}).",
                    running_package_root.display(),
                    npm_package_root.display()
                ));
            }
            NpmRootCheck::MissingPackageRoot => {
                status = status.max(CheckStatus::Warning);
                summary = "npm update target could not be proven".to_string();
                remediation = Some(
                    "Reinstall or update Codex so the JS shim provides CODEX_MANAGED_PACKAGE_ROOT."
                        .to_string(),
                );
            }
            NpmRootCheck::NpmUnavailable(error) => {
                status = status.max(CheckStatus::Warning);
                summary = "npm update target could not be inspected".to_string();
                details.push(format!("npm root -g failed: {error}"));
            }
        }
    }

    match fetch_latest_version(&install_context) {
        Ok(latest_version) => {
            details.push(format!("latest version: {latest_version}"));
            if is_newer(&latest_version, env!("CARGO_PKG_VERSION")) == Some(true) {
                details.push("latest version status: newer version is available".to_string());
            } else {
                details.push("latest version status: current version is not older".to_string());
            }
        }
        Err(err) => {
            status = status.max(CheckStatus::Warning);
            details.push(format!("latest version probe: {err}"));
        }
    }

    let mut check = DoctorCheck::new("updates.status", "updates", status, summary).details(details);
    if let Some(remediation) = remediation {
        check = check.remediation(remediation);
    }
    check
}

fn push_cached_version_details(details: &mut Vec<String>, version_file: &Path) {
    details.push(format!("version cache: {}", version_file.display()));
    match std::fs::read_to_string(version_file) {
        Ok(contents) => match serde_json::from_str::<VersionInfo>(&contents) {
            Ok(info) => {
                details.push(format!("cached latest version: {}", info.latest_version));
                if let Some(last_checked_at) = info.last_checked_at {
                    details.push(format!("last checked at: {last_checked_at}"));
                }
                if let Some(dismissed_version) = info.dismissed_version {
                    details.push(format!("dismissed version: {dismissed_version}"));
                }
            }
            Err(err) => details.push(format!("version cache parse: {err}")),
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            details.push("version cache: missing".to_string());
        }
        Err(err) => details.push(format!("version cache read: {err}")),
    }
}

fn update_action_label(context: &InstallContext) -> &'static str {
    match context {
        InstallContext::Npm => "npm install -g @openai/codex",
        InstallContext::Bun => "bun install -g @openai/codex",
        InstallContext::Brew => "brew upgrade --cask codex",
        InstallContext::Standalone { .. } => "standalone installer",
        InstallContext::Other => "manual or unknown",
    }
}

fn fetch_latest_version(context: &InstallContext) -> Result<String, String> {
    match context {
        InstallContext::Brew => fetch_homebrew_cask_version(),
        InstallContext::Npm
        | InstallContext::Bun
        | InstallContext::Standalone { .. }
        | InstallContext::Other => fetch_latest_github_release_version(),
    }
}

fn fetch_latest_github_release_version() -> Result<String, String> {
    #[derive(Deserialize)]
    struct ReleaseInfo {
        tag_name: String,
    }

    let info = http_get_json::<ReleaseInfo>(GITHUB_LATEST_RELEASE_URL)?;
    info.tag_name
        .strip_prefix("rust-v")
        .map(str::to_string)
        .ok_or_else(|| format!("failed to parse latest tag {}", info.tag_name))
}

fn fetch_homebrew_cask_version() -> Result<String, String> {
    #[derive(Deserialize)]
    struct HomebrewCaskInfo {
        version: String,
    }

    http_get_json::<HomebrewCaskInfo>(HOMEBREW_CASK_API_URL).map(|info| info.version)
}

fn http_get_json<T>(url: &str) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    let body = run_command("curl", ["-fsSL", "--max-time", "5", url])?;
    serde_json::from_str::<T>(&body).map_err(|err| err.to_string())
}

fn is_newer(latest: &str, current: &str) -> Option<bool> {
    match (parse_version(latest), parse_version(current)) {
        (Some(latest), Some(current)) => Some(latest > current),
        (Some(_), None) | (None, Some(_)) | (None, None) => None,
    }
}

fn parse_version(value: &str) -> Option<(u64, u64, u64)> {
    let mut parts = value.trim().split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    let minor = parts.next()?.parse::<u64>().ok()?;
    let patch = parts.next()?.parse::<u64>().ok()?;
    Some((major, minor, patch))
}

#[derive(Deserialize)]
struct VersionInfo {
    latest_version: String,
    #[serde(default)]
    last_checked_at: Option<String>,
    #[serde(default)]
    dismissed_version: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_newer_compares_plain_semver() {
        assert_eq!(is_newer("1.2.4", "1.2.3"), Some(true));
        assert_eq!(is_newer("1.2.3", "1.2.4"), Some(false));
        assert_eq!(is_newer("1.2.3-beta.1", "1.2.2"), None);
    }

    #[test]
    fn update_action_labels_install_contexts() {
        assert_eq!(
            update_action_label(&InstallContext::Npm),
            "npm install -g @openai/codex"
        );
        assert_eq!(
            update_action_label(&InstallContext::Other),
            "manual or unknown"
        );
    }
}
