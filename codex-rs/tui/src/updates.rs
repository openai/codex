#![cfg(any(not(debug_assertions), test))]
#![cfg_attr(test, allow(dead_code))]

use crate::update_action::UpdateAction;
use chrono::DateTime;
use chrono::Duration;
use chrono::Utc;
use codex_core::config::Config;
use codex_core::default_client::create_client;
use serde::Deserialize;
use serde::Serialize;
use std::path::Path;
use std::path::PathBuf;

use crate::version::CODEX_CLI_VERSION;

pub fn get_upgrade_version(config: &Config) -> Option<String> {
    if !config.check_for_update_on_startup {
        return None;
    }

    let version_file = version_filepath(config);
    let update_target = current_update_target();
    let info = read_version_info_for_source(&version_file, update_target.source_key)
        .ok()
        .flatten();

    if match &info {
        None => true,
        Some(info) => info.last_checked_at < Utc::now() - Duration::hours(20),
    } {
        // Refresh the cached latest version in the background so TUI startup
        // isn’t blocked by a network call. The UI reads the previously cached
        // value (if any) for this run; the next run shows the banner if needed.
        tokio::spawn(async move {
            check_for_update(&version_file)
                .await
                .inspect_err(|e| tracing::error!("Failed to update version: {e}"))
        });
    }

    info.and_then(|info| {
        if is_newer(&info.latest_version, CODEX_CLI_VERSION).unwrap_or(false) {
            Some(info.latest_version)
        } else {
            None
        }
    })
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct VersionInfo {
    latest_version: String,
    // ISO-8601 timestamp (RFC3339)
    last_checked_at: DateTime<Utc>,
    #[serde(default)]
    dismissed_version: Option<String>,
    #[serde(default)]
    source_key: Option<String>,
}

const VERSION_FILENAME: &str = "version.json";
const NPM_LATEST_URL: &str = "https://registry.npmjs.org/@ixe1%2Fcodexel/latest";
const NPM_SOURCE_KEY: &str = "npm:@ixe1/codexel";
// We use the latest version from the cask if installation is via homebrew - homebrew does not immediately pick up the latest release and can lag behind.
const HOMEBREW_CASK_URL: &str =
    "https://raw.githubusercontent.com/Homebrew/homebrew-cask/HEAD/Casks/c/codexel.rb";
const HOMEBREW_SOURCE_KEY: &str = "brew:codexel";
const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/Ixe1/codexel/releases/latest";
const GITHUB_SOURCE_KEY: &str = "github:Ixe1/codexel";

#[derive(Deserialize, Debug, Clone)]
struct ReleaseInfo {
    tag_name: String,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
struct NpmLatestInfo {
    version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UpdateSource {
    Npm,
    Homebrew,
    Github,
}

#[derive(Debug, Clone, Copy)]
struct UpdateTarget {
    source: UpdateSource,
    source_key: &'static str,
}

fn version_filepath(config: &Config) -> PathBuf {
    config.codex_home.join(VERSION_FILENAME)
}

fn read_version_info(version_file: &Path) -> anyhow::Result<VersionInfo> {
    let contents = std::fs::read_to_string(version_file)?;
    Ok(serde_json::from_str(&contents)?)
}

fn read_version_info_for_source(
    version_file: &Path,
    source_key: &str,
) -> anyhow::Result<Option<VersionInfo>> {
    let info = read_version_info(version_file)?;
    Ok(filter_version_info_by_source(info, source_key))
}

fn filter_version_info_by_source(info: VersionInfo, source_key: &str) -> Option<VersionInfo> {
    if info.source_key.as_deref() == Some(source_key) {
        Some(info)
    } else {
        None
    }
}

fn resolve_update_target(action: Option<UpdateAction>) -> UpdateTarget {
    match action {
        Some(UpdateAction::BrewUpgrade) => UpdateTarget {
            source: UpdateSource::Homebrew,
            source_key: HOMEBREW_SOURCE_KEY,
        },
        Some(UpdateAction::NpmUpgrade | UpdateAction::BunUpgrade) => UpdateTarget {
            source: UpdateSource::Npm,
            source_key: NPM_SOURCE_KEY,
        },
        None => UpdateTarget {
            source: UpdateSource::Github,
            source_key: GITHUB_SOURCE_KEY,
        },
    }
}

#[cfg(not(debug_assertions))]
fn current_update_target() -> UpdateTarget {
    resolve_update_target(crate::update_action::get_update_action())
}

#[cfg(test)]
fn current_update_target() -> UpdateTarget {
    resolve_update_target(None)
}

async fn check_for_update(version_file: &Path) -> anyhow::Result<()> {
    let update_target = current_update_target();
    let latest_version = match update_target.source {
        UpdateSource::Homebrew => {
            let cask_contents = create_client()
                .get(HOMEBREW_CASK_URL)
                .send()
                .await?
                .error_for_status()?
                .text()
                .await?;
            extract_version_from_cask(&cask_contents)?
        }
        UpdateSource::Npm => {
            let NpmLatestInfo { version } = create_client()
                .get(NPM_LATEST_URL)
                .send()
                .await?
                .error_for_status()?
                .json::<NpmLatestInfo>()
                .await?;
            version
        }
        UpdateSource::Github => {
            let ReleaseInfo {
                tag_name: latest_tag_name,
            } = create_client()
                .get(LATEST_RELEASE_URL)
                .send()
                .await?
                .error_for_status()?
                .json::<ReleaseInfo>()
                .await?;
            extract_version_from_latest_tag(&latest_tag_name)?
        }
    };

    // Preserve any previously dismissed version if present.
    let prev_info = read_version_info_for_source(version_file, update_target.source_key)
        .ok()
        .flatten();
    let info = VersionInfo {
        latest_version,
        last_checked_at: Utc::now(),
        dismissed_version: prev_info.and_then(|p| p.dismissed_version),
        source_key: Some(update_target.source_key.to_string()),
    };

    let json_line = format!("{}\n", serde_json::to_string(&info)?);
    if let Some(parent) = version_file.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(version_file, json_line).await?;
    Ok(())
}

fn is_newer(latest: &str, current: &str) -> Option<bool> {
    match (parse_version(latest), parse_version(current)) {
        (Some(l), Some(c)) => Some(l > c),
        _ => None,
    }
}

fn extract_version_from_cask(cask_contents: &str) -> anyhow::Result<String> {
    cask_contents
        .lines()
        .find_map(|line| {
            let line = line.trim();
            line.strip_prefix("version \"")
                .and_then(|rest| rest.strip_suffix('"'))
                .map(ToString::to_string)
        })
        .ok_or_else(|| anyhow::anyhow!("Failed to find version in Homebrew cask file"))
}

fn extract_version_from_latest_tag(latest_tag_name: &str) -> anyhow::Result<String> {
    latest_tag_name
        .strip_prefix("rust-v")
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse latest tag name '{latest_tag_name}'"))
}

/// Returns the latest version to show in a popup, if it should be shown.
/// This respects the user's dismissal choice for the current latest version.
pub fn get_upgrade_version_for_popup(config: &Config) -> Option<String> {
    if !config.check_for_update_on_startup {
        return None;
    }

    let version_file = version_filepath(config);
    let latest = get_upgrade_version(config)?;
    // If the user dismissed this exact version previously, do not show the popup.
    let source_key = current_update_target().source_key;
    if let Ok(Some(info)) = read_version_info_for_source(&version_file, source_key)
        && info.dismissed_version.as_deref() == Some(latest.as_str())
    {
        return None;
    }
    Some(latest)
}

/// Persist a dismissal for the current latest version so we don't show
/// the update popup again for this version.
pub async fn dismiss_version(config: &Config, version: &str) -> anyhow::Result<()> {
    let version_file = version_filepath(config);
    let source_key = current_update_target().source_key;
    let Some(mut info) = read_version_info_for_source(&version_file, source_key)
        .ok()
        .flatten()
    else {
        return Ok(());
    };
    info.dismissed_version = Some(version.to_string());
    info.source_key = Some(source_key.to_string());
    let json_line = format!("{}\n", serde_json::to_string(&info)?);
    if let Some(parent) = version_file.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(version_file, json_line).await?;
    Ok(())
}

fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let mut iter = v.trim().split('.');
    let maj = iter.next()?.parse::<u64>().ok()?;
    let min = iter.next()?.parse::<u64>().ok()?;
    let pat = iter.next()?.parse::<u64>().ok()?;
    Some((maj, min, pat))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parses_version_from_cask_contents() {
        let cask = r#"
            cask "codexel" do
              version "0.55.0"
            end
        "#;
        assert_eq!(
            extract_version_from_cask(cask).expect("failed to parse version"),
            "0.55.0"
        );
    }

    #[test]
    fn extracts_version_from_latest_tag() {
        assert_eq!(
            extract_version_from_latest_tag("rust-v1.5.0").expect("failed to parse version"),
            "1.5.0"
        );
    }

    #[test]
    fn latest_tag_without_prefix_is_invalid() {
        assert!(extract_version_from_latest_tag("v1.5.0").is_err());
    }

    #[test]
    fn prerelease_version_is_not_considered_newer() {
        assert_eq!(is_newer("0.11.0-beta.1", "0.11.0"), None);
        assert_eq!(is_newer("1.0.0-rc.1", "1.0.0"), None);
    }

    #[test]
    fn plain_semver_comparisons_work() {
        assert_eq!(is_newer("0.11.1", "0.11.0"), Some(true));
        assert_eq!(is_newer("0.11.0", "0.11.1"), Some(false));
        assert_eq!(is_newer("1.0.0", "0.9.9"), Some(true));
        assert_eq!(is_newer("0.9.9", "1.0.0"), Some(false));
    }

    #[test]
    fn whitespace_is_ignored() {
        assert_eq!(parse_version(" 1.2.3 \n"), Some((1, 2, 3)));
        assert_eq!(is_newer(" 1.2.3 ", "1.2.2"), Some(true));
    }

    #[test]
    fn parses_npm_latest_version() {
        let payload = r#"{ "name": "@ixe1/codexel", "version": "0.42.1" }"#;
        let parsed = serde_json::from_str::<NpmLatestInfo>(payload)
            .expect("failed to parse npm latest payload");
        assert_eq!(
            parsed,
            NpmLatestInfo {
                version: "0.42.1".to_string(),
            }
        );
    }

    #[test]
    fn cache_mismatch_is_ignored() {
        let info = VersionInfo {
            latest_version: "9.9.9".to_string(),
            last_checked_at: Utc::now(),
            dismissed_version: None,
            source_key: Some(GITHUB_SOURCE_KEY.to_string()),
        };
        assert!(filter_version_info_by_source(info, NPM_SOURCE_KEY).is_none());
    }
}
