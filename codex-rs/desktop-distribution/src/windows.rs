use std::path::PathBuf;
use std::process::Command;

use crate::DesktopDistributionError;
use crate::InstalledDesktop;

const STORE_PUBLISHER_ID: &str = "2p2nqsd0c76g0";

/// Queries the signed stable MSIX identity and uses the package location as the app root.
pub(crate) fn discover() -> Result<Option<InstalledDesktop>, DesktopDistributionError> {
    let script = format!(
        r#"
$location = Get-AppxPackage -Name 'OpenAI.Codex' -ErrorAction SilentlyContinue |
  Where-Object {{ $_.PublisherId -eq '{STORE_PUBLISHER_ID}' }} |
  Select-Object -First 1 -ExpandProperty InstallLocation
if ($location) {{ $location }}
"#
    );
    let output = Command::new("powershell.exe")
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-Command")
        .arg(script)
        .output()
        .map_err(|error| DesktopDistributionError::Discovery(error.to_string()))?;
    if !output.status.success() {
        return Ok(None);
    }
    let install_location = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if install_location.is_empty() {
        return Ok(None);
    }
    let app_root = PathBuf::from(install_location);
    let resources_root = app_root.join("app/resources");
    InstalledDesktop::from_paths(app_root, resources_root).map(Some)
}
