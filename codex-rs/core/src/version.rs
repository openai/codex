use std::path::Path;

use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

pub const VERSION_FILENAME: &str = "version.json";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VersionInfo {
    pub latest_version: String,
    // ISO-8601 timestamp (RFC3339)
    pub last_checked_at: DateTime<Utc>,
    #[serde(default)]
    pub dismissed_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    major: u64,
    minor: u64,
    patch: u64,
    pre: Option<Vec<PrereleaseIdent>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PrereleaseIdent {
    Numeric(u64),
    Alpha(String),
}

impl Version {
    pub fn parse(input: &str) -> Option<Self> {
        let mut input = input.trim();
        if let Some(stripped) = input.strip_prefix("rust-v") {
            input = stripped;
        }
        if let Some(stripped) = input.strip_prefix('v') {
            input = stripped;
        }
        let input = input.splitn(2, '+').next().unwrap_or(input);
        let mut parts = input.splitn(2, '-');
        let core = parts.next()?;
        let pre = parts.next();
        let mut nums = core.split('.');
        let major = nums.next()?.parse::<u64>().ok()?;
        let minor = nums.next()?.parse::<u64>().ok()?;
        let patch = nums.next()?.parse::<u64>().ok()?;
        if nums.next().is_some() {
            return None;
        }
        let pre = match pre {
            None => None,
            Some(value) if value.is_empty() => None,
            Some(value) => {
                let mut idents = Vec::new();
                for ident in value.split('.') {
                    if ident.is_empty() {
                        return None;
                    }
                    let parsed = if ident.chars().all(|c| c.is_ascii_digit()) {
                        ident.parse::<u64>().ok().map(PrereleaseIdent::Numeric)
                    } else {
                        Some(PrereleaseIdent::Alpha(ident.to_string()))
                    };
                    idents.push(parsed?);
                }
                Some(idents)
            }
        };
        Some(Self {
            major,
            minor,
            patch,
            pre,
        })
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.major.cmp(&other.major) {
            std::cmp::Ordering::Equal => {}
            ordering => return ordering,
        }
        match self.minor.cmp(&other.minor) {
            std::cmp::Ordering::Equal => {}
            ordering => return ordering,
        }
        match self.patch.cmp(&other.patch) {
            std::cmp::Ordering::Equal => {}
            ordering => return ordering,
        }
        match (&self.pre, &other.pre) {
            (None, None) => std::cmp::Ordering::Equal,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (Some(_), None) => std::cmp::Ordering::Less,
            (Some(left), Some(right)) => compare_prerelease_idents(left, right),
        }
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

pub fn is_newer(latest: &str, current: &str) -> Option<bool> {
    let latest = Version::parse(latest)?;
    if latest.pre.is_some() {
        return Some(false);
    }
    let current = Version::parse(current)?;
    let current = Version {
        pre: None,
        ..current
    };
    Some(latest > current)
}

pub fn is_up_to_date(latest: &str, current: &str) -> Option<bool> {
    let latest = Version::parse(latest)?;
    if latest.pre.is_some() {
        return Some(true);
    }
    let current = Version::parse(current)?;
    let current = Version {
        pre: None,
        ..current
    };
    Some(current >= latest)
}

pub fn read_version_info(version_file: &Path) -> anyhow::Result<VersionInfo> {
    let contents = std::fs::read_to_string(version_file)?;
    Ok(serde_json::from_str(&contents)?)
}

pub fn read_latest_version(version_file: &Path) -> Option<String> {
    read_version_info(version_file)
        .ok()
        .map(|info| info.latest_version)
}

pub fn extract_version_from_cask(cask_contents: &str) -> anyhow::Result<String> {
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

pub fn extract_version_from_latest_tag(latest_tag_name: &str) -> anyhow::Result<String> {
    latest_tag_name
        .strip_prefix("rust-v")
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse latest tag name '{latest_tag_name}'"))
}

fn compare_prerelease_idents(
    left: &[PrereleaseIdent],
    right: &[PrereleaseIdent],
) -> std::cmp::Ordering {
    for (l, r) in left.iter().zip(right.iter()) {
        let ordering = match (l, r) {
            (PrereleaseIdent::Numeric(a), PrereleaseIdent::Numeric(b)) => a.cmp(b),
            (PrereleaseIdent::Alpha(a), PrereleaseIdent::Alpha(b)) => a.cmp(b),
            (PrereleaseIdent::Numeric(_), PrereleaseIdent::Alpha(_)) => std::cmp::Ordering::Less,
            (PrereleaseIdent::Alpha(_), PrereleaseIdent::Numeric(_)) => std::cmp::Ordering::Greater,
        };
        if ordering != std::cmp::Ordering::Equal {
            return ordering;
        }
    }
    left.len().cmp(&right.len())
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn prerelease_current_is_ignored() {
        assert_eq!(is_newer("1.2.3", "1.2.3-alpha.1"), Some(false));
        assert_eq!(is_up_to_date("1.2.3", "1.2.3-alpha.1"), Some(true));
    }

    #[test]
    fn prerelease_latest_is_ignored() {
        assert_eq!(is_newer("1.2.4-alpha.1", "1.2.3"), Some(false));
        assert_eq!(is_up_to_date("1.2.4-alpha.1", "1.2.3"), Some(true));
    }

    #[test]
    fn prerelease_latest_is_not_considered_newer() {
        assert_eq!(is_newer("0.11.0-beta.1", "0.11.0"), Some(false));
        assert_eq!(is_newer("1.0.0-rc.1", "1.0.0"), Some(false));
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
        assert_eq!(Version::parse(" 1.2.3 \n").is_some(), true);
        assert_eq!(is_newer(" 1.2.3 ", "1.2.2"), Some(true));
    }

    #[test]
    fn parses_version_from_cask_contents() {
        let cask = r#"
            cask "codex" do
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
}
