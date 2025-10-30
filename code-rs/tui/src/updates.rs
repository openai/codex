use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use std::sync::atomic::{AtomicU8, Ordering};
use std::fs;
use std::io::ErrorKind;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use code_core::config::resolve_code_path_for_read;
use code_core::config::Config;
use code_core::default_client::create_client;
use tokio::process::Command;
use tracing::{info, warn};

const FORCE_UPGRADE_UNSET: u8 = 0;
const FORCE_UPGRADE_FALSE: u8 = 1;
const FORCE_UPGRADE_TRUE: u8 = 2;

static FORCE_UPGRADE_PREVIEW: AtomicU8 = AtomicU8::new(FORCE_UPGRADE_UNSET);

fn force_upgrade_preview_enabled() -> bool {
    match FORCE_UPGRADE_PREVIEW.load(Ordering::Relaxed) {
        FORCE_UPGRADE_TRUE => true,
        FORCE_UPGRADE_FALSE => false,
        _ => {
            let computed = std::env::var("SHOW_UPGRADE")
                .map(|value| {
                    let normalized = value.trim().to_ascii_lowercase();
                    matches!(normalized.as_str(), "1" | "true" | "yes" | "on")
                })
                .unwrap_or(false);

            FORCE_UPGRADE_PREVIEW.store(
                if computed {
                    FORCE_UPGRADE_TRUE
                } else {
                    FORCE_UPGRADE_FALSE
                },
                Ordering::Relaxed,
            );
            computed
        }
    }
}

pub fn upgrade_ui_enabled() -> bool {
    !cfg!(debug_assertions) || force_upgrade_preview_enabled()
}

pub fn auto_upgrade_runtime_enabled() -> bool {
    !cfg!(debug_assertions)
}

pub fn get_upgrade_version(config: &Config) -> Option<String> {
    let version_file = version_filepath(config);
    let read_path = resolve_code_path_for_read(&config.code_home, Path::new(VERSION_FILENAME));
    let info = read_version_info(&read_path).ok();
    let originator = config.responses_originator_header.clone();

    // Always refresh the cached latest version in the background so TUI startup
    // isn’t blocked by a network call. The UI reads the previously cached
    // value (if any) for this run; the next run shows the banner if needed.
    tokio::spawn(async move {
        check_for_update(&version_file, &originator)
            .await
            .inspect_err(|e| tracing::error!("Failed to update version: {e}"))
    });

    info.and_then(|info| {
        let current_version = code_version::version();
        if is_newer(&info.latest_version, current_version).unwrap_or(false) {
            Some(info.latest_version)
        } else {
            None
        }
    })
}

#[derive(Debug, Clone)]
pub struct UpdateCheckInfo {
    pub latest_version: Option<String>,
}

pub async fn check_for_updates_now(config: &Config) -> anyhow::Result<UpdateCheckInfo> {
    let version_file = version_filepath(config);
    let originator = config.responses_originator_header.clone();
    let info = check_for_update(&version_file, &originator).await?;
    let current_version = code_version::version().to_string();
    let latest_version = if is_newer(&info.latest_version, &current_version).unwrap_or(false) {
        Some(info.latest_version)
    } else {
        None
    };

    Ok(UpdateCheckInfo {
        latest_version,
    })
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct VersionInfo {
    latest_version: String,
    // ISO-8601 timestamp (RFC3339)
    last_checked_at: DateTime<Utc>,
    #[serde(default)]
    release_repo: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
struct ReleaseInfo {
    tag_name: String,
}

const VERSION_FILENAME: &str = "version.json";
const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/just-every/code/releases/latest";
const CURRENT_RELEASE_REPO: &str = "just-every/code";
const LEGACY_RELEASE_REPO: &str = "openai/codex";
pub const CODE_RELEASE_URL: &str = "https://github.com/just-every/code/releases/latest";

const AUTO_UPGRADE_LOCK_FILE: &str = "auto-upgrade.lock";
const AUTO_UPGRADE_LOCK_TTL: Duration = Duration::from_secs(900); // 15 minutes

#[derive(Debug, Clone)]
pub enum UpgradeResolution {
    Command { command: Vec<String>, display: String },
    Manual { instructions: String },
}

fn version_filepath(config: &Config) -> PathBuf {
    config.code_home.join(VERSION_FILENAME)
}

pub fn resolve_upgrade_resolution() -> UpgradeResolution {
    if std::env::var_os("CODEX_MANAGED_BY_NPM").is_some() {
        return UpgradeResolution::Command {
            command: vec![
                "npm".to_string(),
                "install".to_string(),
                "-g".to_string(),
                "@just-every/code@latest".to_string(),
            ],
            display: "npm install -g @just-every/code@latest".to_string(),
        };
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(exe_path) = std::env::current_exe() {
            if exe_path.starts_with("/opt/homebrew") || exe_path.starts_with("/usr/local") {
                return UpgradeResolution::Command {
                    command: vec![
                        "brew".to_string(),
                        "upgrade".to_string(),
                        "code".to_string(),
                    ],
                    display: "brew upgrade code".to_string(),
                };
            }
        }
    }

    UpgradeResolution::Manual {
        instructions: format!(
            "Download the latest release from {CODE_RELEASE_URL} and replace the installed binary."
        ),
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AutoUpgradeOutcome {
    pub installed_version: Option<String>,
    pub user_notice: Option<String>,
}

pub async fn auto_upgrade_if_enabled(config: &Config) -> anyhow::Result<AutoUpgradeOutcome> {
    if !config.auto_upgrade_enabled {
        return Ok(AutoUpgradeOutcome::default());
    }

    let resolution = resolve_upgrade_resolution();
    let (command, command_display) = match resolution {
        UpgradeResolution::Command {
            command,
            display: command_display,
        } if !command.is_empty() => (command, command_display),
        _ => {
            info!("auto-upgrade enabled but no managed installer detected; skipping");
            return Ok(AutoUpgradeOutcome::default());
        }
    };

    let info = match check_for_updates_now(config).await {
        Ok(info) => info,
        Err(err) => {
            warn!("auto-upgrade: failed to check for updates: {err}");
            return Ok(AutoUpgradeOutcome::default());
        }
    };

    let Some(latest_version) = info.latest_version.clone() else {
        // Already up to date
        return Ok(AutoUpgradeOutcome::default());
    };

    let lock = match AutoUpgradeLock::acquire(&config.code_home) {
        Ok(Some(lock)) => lock,
        Ok(None) => {
            info!("auto-upgrade already in progress by another instance; skipping");
            return Ok(AutoUpgradeOutcome::default());
        }
        Err(err) => {
            warn!("auto-upgrade: unable to acquire lock: {err}");
            return Ok(AutoUpgradeOutcome::default());
        }
    };

    info!(
        command = command_display.as_str(),
        latest_version = latest_version.as_str(),
        "auto-upgrade: running managed installer"
    );
    let result = execute_upgrade_command(&command).await;
    drop(lock);

    let mut outcome = AutoUpgradeOutcome {
        installed_version: None,
        user_notice: None,
    };

    match result {
        Ok(primary) => {
            if primary.success {
                info!("auto-upgrade: successfully installed {latest_version}");
                outcome.installed_version = Some(latest_version);
                return Ok(outcome);
            }

            #[cfg(any(target_os = "macos", target_os = "linux"))]
            {
                if !starts_with_sudo(&command) {
                    info!("auto-upgrade: retrying with sudo -n");
                    let sudo_command = wrap_with_sudo(&command);
                    match execute_upgrade_command(&sudo_command).await {
                        Ok(fallback) if fallback.success => {
                            info!("auto-upgrade: sudo retry succeeded; installed {latest_version}");
                            outcome.installed_version = Some(latest_version);
                            return Ok(outcome);
                        }
                        Ok(fallback) => {
                            if sudo_requires_manual_intervention(&fallback.stderr, fallback.status)
                            {
                                outcome.user_notice = Some(format!(
                                "Automatic upgrade needs your attention. Run `/update` to finish with `{}`.",
                                    command_display
                                ));
                            }
                            warn!(
                                "auto-upgrade: sudo retry failed: status={:?} stderr={}",
                                fallback.status,
                                truncate_for_log(&fallback.stderr)
                            );
                            return Ok(outcome);
                        }
                        Err(err) => {
                            warn!("auto-upgrade: sudo retry error: {err}");
                            outcome.user_notice = Some(format!(
                                "Automatic upgrade could not escalate permissions. Run `/update` to finish with `{}`.",
                                command_display
                            ));
                            return Ok(outcome);
                        }
                    }
                }
            }

            #[cfg(not(any(target_os = "macos", target_os = "linux")))]
            {
                let _ = primary; // suppress unused warning on non-Unix targets
            }

            warn!(
                "auto-upgrade: upgrade command failed: status={:?} stderr={}",
                primary.status,
                truncate_for_log(&primary.stderr)
            );
            Ok(outcome)
        }
        Err(err) => {
            warn!("auto-upgrade: failed to launch upgrade command: {err}");
            Ok(outcome)
        }
    }
}

struct AutoUpgradeLock {
    path: PathBuf,
}

impl AutoUpgradeLock {
    fn acquire(code_home: &Path) -> anyhow::Result<Option<Self>> {
        let path = code_home.join(AUTO_UPGRADE_LOCK_FILE);
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                writeln!(file, "{timestamp}")?;
                Ok(Some(Self { path }))
            }
            Err(err) if err.kind() == ErrorKind::AlreadyExists => {
                if Self::is_stale(&path)? {
                    let _ = fs::remove_file(&path);
                    match fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&path)
                    {
                        Ok(mut file) => {
                            let timestamp = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                            writeln!(file, "{timestamp}")?;
                            Ok(Some(Self { path }))
                        }
                        Err(err) if err.kind() == ErrorKind::AlreadyExists => Ok(None),
                        Err(err) => Err(err.into()),
                    }
                } else {
                    Ok(None)
                }
            }
            Err(err) => Err(err.into()),
        }
    }

    fn is_stale(path: &Path) -> anyhow::Result<bool> {
        match fs::read_to_string(path) {
            Ok(contents) => {
                if let Ok(stored) = contents.trim().parse::<u64>() {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    Ok(now.saturating_sub(stored) > AUTO_UPGRADE_LOCK_TTL.as_secs())
                } else {
                    Ok(true)
                }
            }
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(true),
            Err(err) => {
                warn!("auto-upgrade: failed reading lock file: {err}");
                Ok(true)
            }
        }
    }
}

impl Drop for AutoUpgradeLock {
    fn drop(&mut self) {
        if let Err(err) = fs::remove_file(&self.path) {
            if err.kind() != ErrorKind::NotFound {
                warn!("auto-upgrade: failed to remove lock file {}: {err}", self.path.display());
            }
        }
    }
}

#[derive(Debug, Clone)]
struct CommandCapture {
    success: bool,
    status: Option<i32>,
    stderr: String,
}

async fn execute_upgrade_command(command: &[String]) -> anyhow::Result<CommandCapture> {
    if command.is_empty() {
        anyhow::bail!("upgrade command is empty");
    }

    let mut cmd = Command::new(&command[0]);
    if command.len() > 1 {
        cmd.args(&command[1..]);
    }

    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let output = cmd.output().await?;
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    Ok(CommandCapture {
        success: output.status.success(),
        status: output.status.code(),
        stderr,
    })
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn wrap_with_sudo(command: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(command.len() + 3);
    out.push("sudo".to_string());
    out.push("-n".to_string());
    out.push("--".to_string());
    out.extend(command.iter().cloned());
    out
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn starts_with_sudo(command: &[String]) -> bool {
    command
        .first()
        .map(|c| c.eq_ignore_ascii_case("sudo"))
        .unwrap_or(false)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn sudo_requires_manual_intervention(stderr: &str, status: Option<i32>) -> bool {
    let lowered = stderr.to_ascii_lowercase();
    let needs_password = lowered.contains("password is required")
        || lowered.contains("a password is required")
        || lowered.contains("no tty present and no askpass program specified")
        || lowered.contains("must be run from a terminal")
        || lowered.contains("may not run sudo")
        || lowered.contains("permission denied");
    needs_password && status == Some(1)
}

fn truncate_for_log(text: &str) -> String {
    const LIMIT: usize = 256;
    const ELLIPSIS_BYTES: usize = '…'.len_utf8();
    if text.len() <= LIMIT {
        return text.replace('\n', " ");
    }

    let slice_limit = LIMIT.saturating_sub(ELLIPSIS_BYTES);
    let safe_boundary = text
        .char_indices()
        .map(|(idx, _)| idx)
        .chain(std::iter::once(text.len()))
        .take_while(|idx| *idx <= slice_limit)
        .last()
        .unwrap_or(0);

    let safe_slice = text.get(..safe_boundary).unwrap_or("");
    let mut truncated = safe_slice.to_string();
    truncated.push('…');
    truncated.replace('\n', " ")
}


fn read_version_info(version_file: &Path) -> anyhow::Result<VersionInfo> {
    let contents = std::fs::read_to_string(version_file)?;
    let info: VersionInfo = serde_json::from_str(&contents)?;
    let repo = info
        .release_repo
        .as_deref()
        .unwrap_or(LEGACY_RELEASE_REPO);
    if repo != CURRENT_RELEASE_REPO {
        anyhow::bail!(
            "stale version info from {repo}; discarding cached update metadata"
        );
    }
    Ok(info)
}

async fn check_for_update(version_file: &Path, originator: &str) -> anyhow::Result<VersionInfo> {
    let ReleaseInfo {
        tag_name: latest_tag_name,
    } = create_client(originator)
        .get(LATEST_RELEASE_URL)
        .send()
        .await?
        .error_for_status()?
        .json::<ReleaseInfo>()
        .await?;

    // Support both tagging schemes:
    // - "rust-vX.Y.Z" (legacy Rust-release workflow)
    // - "vX.Y.Z" (general release workflow)
    let latest_version = if let Some(v) = latest_tag_name.strip_prefix("rust-v") {
        v.to_string()
    } else if let Some(v) = latest_tag_name.strip_prefix('v') {
        v.to_string()
    } else {
        // As a last resort, accept the raw tag if it looks like semver
        // so we can recover from unexpected tag formats.
        match parse_version(&latest_tag_name) {
            Some(_) => latest_tag_name.clone(),
            None => anyhow::bail!(
                "Failed to parse latest tag name '{}': expected 'rust-vX.Y.Z' or 'vX.Y.Z'",
                latest_tag_name
            ),
        }
    };

    let info = VersionInfo {
        latest_version,
        last_checked_at: Utc::now(),
        release_repo: Some(CURRENT_RELEASE_REPO.to_string()),
    };

    let json_line = format!("{}\n", serde_json::to_string(&info)?);
    if let Some(parent) = version_file.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(version_file, json_line).await?;
    Ok(info)
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
    use chrono::TimeZone;
    use tempfile::tempdir;

    #[test]
    fn read_version_info_rejects_legacy_repo_cache() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("version.json");
        let legacy = serde_json::json!({
            "latest_version": "0.50.0",
            "last_checked_at": Utc.timestamp_opt(1_696_000_000, 0).unwrap().to_rfc3339(),
        });
        std::fs::write(&path, format!("{}\n", legacy)).unwrap();

        let err = read_version_info(&path).expect_err("legacy cache should be rejected");
        assert!(err
            .to_string()
            .contains("stale version info"));
    }

    #[test]
    fn read_version_info_accepts_current_repo_cache() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("version.json");
        let info = serde_json::json!({
            "latest_version": "0.4.7",
            "last_checked_at": Utc::now().to_rfc3339(),
            "release_repo": CURRENT_RELEASE_REPO,
        });
        std::fs::write(&path, format!("{}\n", info)).unwrap();

        let parsed = read_version_info(&path).expect("current repo cache should load");
        assert_eq!(parsed.latest_version, "0.4.7");
        assert_eq!(parsed.release_repo.as_deref(), Some(CURRENT_RELEASE_REPO));
    }
}
