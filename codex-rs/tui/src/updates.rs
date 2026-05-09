#![cfg(any(not(debug_assertions), test))]
#![cfg_attr(test, allow(dead_code))]

use crate::legacy_core::config::Config;
use crate::npm_registry;
use crate::npm_registry::NpmPackageInfo;
use crate::update_action;
use crate::update_action::UpdateAction;
use crate::update_versions::extract_version_from_latest_tag;
use crate::update_versions::is_newer;
use crate::update_versions::is_source_build_version;
use chrono::DateTime;
use chrono::Duration;
use chrono::Utc;
use codex_login::default_client::create_client;
use serde::Deserialize;
use serde::Serialize;
use std::path::Path;
use std::path::PathBuf;

use crate::version::CODEX_CLI_VERSION;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpgradeNotice {
    Available(String),
    RemediationNeeded(String),
}

pub fn get_upgrade_version(config: &Config) -> Option<String> {
    if !config.check_for_update_on_startup || is_source_build_version(CODEX_CLI_VERSION) {
        return None;
    }

    let action = update_action::get_update_action();
    let version_file = version_filepath(config);
    let info = read_version_info(&version_file).ok();

    if match &info {
        None => true,
        Some(info) => info.last_checked_at < Utc::now() - Duration::hours(20),
    } {
        // Refresh the cached latest version in the background so TUI startup
        // isn’t blocked by a network call. The UI reads the previously cached
        // value (if any) for this run; the next run shows the banner if needed.
        tokio::spawn(async move {
            check_for_update(&version_file, action)
                .await
                .inspect_err(|e| tracing::error!("Failed to update version: {e}"))
        });
    }

    info.and_then(latest_upgrade_version)
}

pub fn get_upgrade_version_for_history(config: &Config) -> Option<String> {
    let latest = get_upgrade_version(config)?;
    let version_file = version_filepath(config);
    if let Ok(info) = read_version_info(&version_file)
        && should_show_prompt_update_remediation(&info, &latest)
    {
        return None;
    }
    Some(latest)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct VersionInfo {
    latest_version: String,
    // ISO-8601 timestamp (RFC3339)
    last_checked_at: DateTime<Utc>,
    #[serde(default)]
    dismissed_version: Option<String>,
    #[serde(default)]
    successful_prompt_update: Option<SuccessfulPromptUpdate>,
    #[serde(default)]
    suppressed_version: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct SuccessfulPromptUpdate {
    from_version: String,
    target_version: String,
}

const VERSION_FILENAME: &str = "version.json";
// We use the latest version from the cask if installation is via homebrew - homebrew does not immediately pick up the latest release and can lag behind.
const HOMEBREW_CASK_API_URL: &str = "https://formulae.brew.sh/api/cask/codex.json";
const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/openai/codex/releases/latest";

#[derive(Deserialize, Debug, Clone)]
struct ReleaseInfo {
    tag_name: String,
}

#[derive(Deserialize, Debug, Clone)]
struct HomebrewCaskInfo {
    version: String,
}

pub(crate) fn version_filepath(config: &Config) -> PathBuf {
    config.codex_home.join(VERSION_FILENAME).into_path_buf()
}

fn read_version_info(version_file: &Path) -> anyhow::Result<VersionInfo> {
    let contents = std::fs::read_to_string(version_file)?;
    Ok(serde_json::from_str(&contents)?)
}

async fn check_for_update(version_file: &Path, action: Option<UpdateAction>) -> anyhow::Result<()> {
    let latest_version = match action {
        Some(UpdateAction::BrewUpgrade) => {
            let HomebrewCaskInfo { version } = create_client()
                .get(HOMEBREW_CASK_API_URL)
                .send()
                .await?
                .error_for_status()?
                .json::<HomebrewCaskInfo>()
                .await?;
            version
        }
        Some(UpdateAction::NpmGlobalLatest) | Some(UpdateAction::BunGlobalLatest) => {
            let latest_version = fetch_latest_github_release_version().await?;
            let package_info = create_client()
                .get(npm_registry::PACKAGE_URL)
                .send()
                .await?
                .error_for_status()?
                .json::<NpmPackageInfo>()
                .await?;
            npm_registry::ensure_version_ready(&package_info, &latest_version)?;
            latest_version
        }
        Some(UpdateAction::StandaloneUnix) | Some(UpdateAction::StandaloneWindows) | None => {
            fetch_latest_github_release_version().await?
        }
    };

    // Preserve local prompt state across version refreshes.
    let prev_info = read_version_info(version_file).ok();
    let info = VersionInfo {
        latest_version,
        last_checked_at: Utc::now(),
        dismissed_version: prev_info.as_ref().and_then(|p| p.dismissed_version.clone()),
        successful_prompt_update: prev_info
            .as_ref()
            .and_then(|p| p.successful_prompt_update.clone()),
        suppressed_version: prev_info.and_then(|p| p.suppressed_version),
    };

    let json_line = format!("{}\n", serde_json::to_string(&info)?);
    if let Some(parent) = version_file.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(version_file, json_line).await?;
    Ok(())
}

async fn fetch_latest_github_release_version() -> anyhow::Result<String> {
    let ReleaseInfo {
        tag_name: latest_tag_name,
    } = create_client()
        .get(LATEST_RELEASE_URL)
        .send()
        .await?
        .error_for_status()?
        .json::<ReleaseInfo>()
        .await?;
    extract_version_from_latest_tag(&latest_tag_name)
}

/// Returns the upgrade notice to show in a popup, if one should be shown.
/// This respects the user's dismissal choice for the current latest version.
pub fn get_upgrade_notice_for_popup(config: &Config) -> Option<UpgradeNotice> {
    if !config.check_for_update_on_startup || is_source_build_version(CODEX_CLI_VERSION) {
        return None;
    }

    let version_file = version_filepath(config);
    let latest = get_upgrade_version(config)?;
    let Ok(info) = read_version_info(&version_file) else {
        return Some(UpgradeNotice::Available(latest));
    };

    if info.dismissed_version.as_deref() == Some(latest.as_str()) {
        return None;
    }
    if should_show_prompt_update_remediation(&info, &latest) {
        Some(UpgradeNotice::RemediationNeeded(latest))
    } else {
        Some(UpgradeNotice::Available(latest))
    }
}

/// Persist a dismissal for the current latest version so we don't show
/// the update popup again for this version.
pub async fn dismiss_version(config: &Config, version: &str) -> anyhow::Result<()> {
    let version_file = version_filepath(config);
    let mut info = match read_version_info(&version_file) {
        Ok(info) => info,
        Err(_) => return Ok(()),
    };
    info.dismissed_version = Some(version.to_string());
    let json_line = format!("{}\n", serde_json::to_string(&info)?);
    if let Some(parent) = version_file.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(version_file, json_line).await?;
    Ok(())
}

/// Persist a successful prompt-triggered update attempt so the next launch can
/// detect whether the running executable actually changed.
pub fn record_successful_prompt_update_attempt(
    version_file: &Path,
    target_version: &str,
) -> anyhow::Result<()> {
    let mut info = read_version_info(version_file)?;
    info.successful_prompt_update = Some(SuccessfulPromptUpdate {
        from_version: CODEX_CLI_VERSION.to_string(),
        target_version: target_version.to_string(),
    });
    write_version_info_sync(version_file, &info)
}

/// Suppress future notices for the current latest version after we explain a
/// likely no-op update once.
pub async fn suppress_version_after_remediation(
    config: &Config,
    version: &str,
) -> anyhow::Result<()> {
    let version_file = version_filepath(config);
    let mut info = match read_version_info(&version_file) {
        Ok(info) => info,
        Err(_) => return Ok(()),
    };
    info.suppressed_version = Some(version.to_string());
    let json_line = format!("{}\n", serde_json::to_string(&info)?);
    if let Some(parent) = version_file.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(version_file, json_line).await?;
    Ok(())
}

fn should_show_prompt_update_remediation(info: &VersionInfo, latest: &str) -> bool {
    matches!(
        info.successful_prompt_update.as_ref(),
        Some(SuccessfulPromptUpdate {
            from_version,
            target_version,
        }) if from_version == CODEX_CLI_VERSION && target_version == latest
    )
}

fn latest_upgrade_version(info: VersionInfo) -> Option<String> {
    if info.suppressed_version.as_deref() == Some(info.latest_version.as_str()) {
        return None;
    }
    if is_newer(&info.latest_version, CODEX_CLI_VERSION).unwrap_or(/*default*/ false) {
        Some(info.latest_version)
    } else {
        None
    }
}

fn write_version_info_sync(version_file: &Path, info: &VersionInfo) -> anyhow::Result<()> {
    let json_line = format!("{}\n", serde_json::to_string(info)?);
    if let Some(parent) = version_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(version_file, json_line)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version_info(latest_version: &str) -> VersionInfo {
        VersionInfo {
            latest_version: latest_version.to_string(),
            last_checked_at: Utc::now(),
            dismissed_version: None,
            successful_prompt_update: None,
            suppressed_version: None,
        }
    }

    #[test]
    fn successful_prompt_update_for_current_binary_triggers_remediation() {
        let mut info = version_info("9.9.9");
        info.successful_prompt_update = Some(SuccessfulPromptUpdate {
            from_version: CODEX_CLI_VERSION.to_string(),
            target_version: "9.9.9".to_string(),
        });

        assert!(should_show_prompt_update_remediation(&info, "9.9.9"));
        assert!(!should_show_prompt_update_remediation(&info, "9.9.10"));
    }

    #[test]
    fn remediation_suppression_hides_the_same_latest_version() {
        let mut info = version_info("9.9.9");
        info.suppressed_version = Some("9.9.9".to_string());

        assert_eq!(latest_upgrade_version(info), None);
    }

    #[test]
    fn newer_latest_version_ignores_stale_remediation_suppression() {
        let mut info = version_info("9.9.10");
        info.suppressed_version = Some("9.9.9".to_string());

        assert_eq!(latest_upgrade_version(info), Some("9.9.10".to_string()));
    }

    #[test]
    fn successful_prompt_update_attempt_is_persisted() -> anyhow::Result<()> {
        let tempdir = tempfile::tempdir()?;
        let version_file = tempdir.path().join(VERSION_FILENAME);
        write_version_info_sync(&version_file, &version_info("9.9.9"))?;

        record_successful_prompt_update_attempt(&version_file, "9.9.9")?;

        let info = read_version_info(&version_file)?;
        assert_eq!(
            info.successful_prompt_update,
            Some(SuccessfulPromptUpdate {
                from_version: CODEX_CLI_VERSION.to_string(),
                target_version: "9.9.9".to_string(),
            })
        );
        Ok(())
    }
}
