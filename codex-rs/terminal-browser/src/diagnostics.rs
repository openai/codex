use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;

use crate::network::BrowserNetworkPolicy;
use crate::runtime::BrowserRuntime;
use crate::sandbox::BrowserLaunchContext;
use crate::sandbox::prepare_browser_launch;

const VERSION_TIMEOUT: Duration = Duration::from_secs(3);
const SUPPORTED_VERSION: &str = "0.0.3-codex.1";
const MAX_VERSION_OUTPUT_CHARS: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
/// Result of validating the external Carbonyl installation used by the experiment.
pub struct BrowserDoctorReport {
    /// Whether the executable passed all launch compatibility checks.
    pub healthy: bool,
    /// Human-readable local diagnostic suitable for the `/browser doctor` surface.
    pub summary: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BrowserInstallation {
    pub(crate) path: PathBuf,
    pub(crate) version: String,
}

pub(crate) async fn inspect_installation(
    binary: &Path,
    launch_context: &BrowserLaunchContext,
) -> Result<BrowserInstallation> {
    let path = std::fs::canonicalize(binary).context("canonicalize Carbonyl executable")?;
    ensure_executable(&path)?;
    let runtime = BrowserRuntime::create(/*persistent_profile*/ None)?;
    let network_policy = BrowserNetworkPolicy::Disabled;
    let launch = prepare_browser_launch(
        &path,
        vec!["--version".to_string()],
        &runtime.root,
        &runtime.profile,
        runtime.environment(&network_policy),
        &network_policy,
        launch_context,
    )?;
    let mut command = tokio::process::Command::new(&launch.program);
    #[cfg(unix)]
    if let Some(arg0) = launch.arg0.as_deref() {
        command.arg0(arg0);
    }
    command
        .args(&launch.args)
        .current_dir(launch.cwd.as_path())
        .env_clear()
        .envs(&launch.env)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(/*kill_on_drop*/ true);
    let output = tokio::time::timeout(VERSION_TIMEOUT, command.output())
        .await
        .context("Carbonyl version probe timed out")?
        .context("run Carbonyl version probe")?;
    anyhow::ensure!(
        output.status.success(),
        "Carbonyl version probe exited with {}",
        output.status
    );
    let version = first_nonempty_line(&output.stdout)
        .or_else(|| first_nonempty_line(&output.stderr))
        .context("Carbonyl version probe returned no version")?;
    anyhow::ensure!(
        has_supported_version(&version),
        "unsupported Carbonyl version `{version}`; this experiment currently requires {SUPPORTED_VERSION}"
    );
    Ok(BrowserInstallation { path, version })
}

pub(crate) fn unavailable_report(reason: &str) -> BrowserDoctorReport {
    BrowserDoctorReport {
        healthy: false,
        summary: format!("Carbonyl unavailable: {reason}"),
    }
}

pub(crate) fn installation_report(result: &Result<BrowserInstallation>) -> BrowserDoctorReport {
    match result {
        Ok(installation) => BrowserDoctorReport {
            healthy: true,
            summary: format!(
                "Carbonyl is ready: {} ({}, {})",
                installation.path.display(),
                installation.version,
                std::env::consts::ARCH
            ),
        },
        Err(error) => BrowserDoctorReport {
            healthy: false,
            summary: format!("Carbonyl validation failed: {error}"),
        },
    }
}

fn first_nonempty_line(bytes: &[u8]) -> Option<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.chars().take(MAX_VERSION_OUTPUT_CHARS).collect())
}

fn has_supported_version(version_line: &str) -> bool {
    version_line.split_ascii_whitespace().last() == Some(SUPPORTED_VERSION)
}

#[cfg(unix)]
fn ensure_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mode = std::fs::metadata(path)
        .context("read Carbonyl executable metadata")?
        .permissions()
        .mode();
    anyhow::ensure!(mode & 0o111 != 0, "Carbonyl path is not executable");
    Ok(())
}

#[cfg(not(unix))]
fn ensure_executable(path: &Path) -> Result<()> {
    anyhow::ensure!(path.is_file(), "Carbonyl path is not an executable file");
    Ok(())
}

#[cfg(test)]
#[path = "diagnostics_tests.rs"]
mod tests;
