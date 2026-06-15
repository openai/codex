use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use crate::DesktopDistributionError;
use crate::InstalledDesktop;

const BUNDLE_IDENTIFIER: &str = "com.openai.codex";
const TEAM_IDENTIFIER: &str = "2DC432GLL2";

/// Asks LaunchServices for the signed stable Codex app, independent of name or location.
pub(crate) fn discover() -> Result<Option<InstalledDesktop>, DesktopDistributionError> {
    let script = format!(r#"POSIX path of (path to application id "{BUNDLE_IDENTIFIER}")"#);
    let output = Command::new("/usr/bin/osascript")
        .args(["-e", &script])
        .output()
        .map_err(|error| DesktopDistributionError::Discovery(error.to_string()))?;
    if !output.status.success() {
        return Ok(None);
    }
    let app_root = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    validate_candidate(&app_root)
}

pub(crate) fn validate_candidate(
    app_root: &Path,
) -> Result<Option<InstalledDesktop>, DesktopDistributionError> {
    let resources_root = app_root.join("Contents/Resources");
    if app_root.extension().is_some_and(|value| value == "app")
        && resources_root.is_dir()
        && has_openai_signature(app_root)?
    {
        return InstalledDesktop::from_paths(app_root.to_path_buf(), resources_root).map(Some);
    }
    Ok(None)
}

fn has_openai_signature(app_root: &std::path::Path) -> Result<bool, DesktopDistributionError> {
    let requirement = format!(
        r#"=identifier "{BUNDLE_IDENTIFIER}" and anchor apple generic and certificate leaf[subject.OU] = "{TEAM_IDENTIFIER}""#
    );
    let verification = Command::new("/usr/bin/codesign")
        .args(["--verify", "--strict", "-R", &requirement])
        .arg(app_root)
        .status()
        .map_err(|error| DesktopDistributionError::Discovery(error.to_string()))?;
    Ok(verification.success())
}
