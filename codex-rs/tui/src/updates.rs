#![cfg(any(not(debug_assertions), test))]

use chrono::Duration;
use chrono::Utc;
use std::path::Path;
use std::path::PathBuf;

use codex_core::config::Config;

pub fn get_upgrade_version(config: &Config) -> Option<String> {
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
            check_for_update(&version_file)
                .await
                .inspect_err(|e| tracing::error!("Failed to update version: {e}"))
        });
    }

    info.and_then(|info| {
        let current_version = env!("CARGO_PKG_VERSION");
        if is_newer(&info.latest_version, current_version).unwrap_or(false) {
            Some(info.latest_version)
        } else {
            None
        }
    })
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct VersionInfo {
    latest_version: String,
    // ISO-8601 timestamp (RFC3339)
    last_checked_at: chrono::DateTime<chrono::Utc>,
}

const VERSION_FILENAME: &str = "version.jsonl";
const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/openai/codex/releases/latest";

fn version_filepath(config: &Config) -> PathBuf {
    let mut path = config.codex_home.clone();
    path.push(VERSION_FILENAME);
    path
}

fn read_version_info(version_file: &Path) -> anyhow::Result<VersionInfo> {
    let contents = std::fs::read_to_string(version_file)?;
    Ok(serde_json::from_str(&contents)?)
}

async fn check_for_update(version_file: &Path) -> anyhow::Result<()> {
    #[derive(serde::Deserialize, Debug, Clone)]
    struct ReleaseInfo {
        tag_name: String,
    }

    let resp = reqwest::Client::new()
        .get(LATEST_RELEASE_URL)
        .header(
            "User-Agent",
            format!(
                "codex/{} (+https://github.com/openai/codex)",
                env!("CARGO_PKG_VERSION")
            ),
        )
        .send()
        .await?
        .error_for_status()?
        .json::<ReleaseInfo>()
        .await?;

    let latest_tag_name = resp.tag_name;

    let info = VersionInfo {
        latest_version: latest_tag_name
            .strip_prefix("rust-v")
            .ok_or_else(|| {
                anyhow::anyhow!("Failed to parse latest tag name '{}'", latest_tag_name)
            })?
            .into(),
        last_checked_at: chrono::Utc::now(),
    };

    let json_line = format!("{}\n", serde_json::to_string(&info)?);
    if let Some(parent) = version_file.parent() {
        tokio::fs::create_dir_all(parent).await.ok();
    }
    tokio::fs::write(version_file, json_line).await.ok();
    Ok(())
}

fn is_newer(latest: &str, current: &str) -> Option<bool> {
    match (parse_version(latest), parse_version(current)) {
        (Some(l), Some(c)) => Some(l > c),
        _ => None,
    }
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
}
