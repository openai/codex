use crate::config_types::WindowsConfig;
use crate::exec::SandboxType;
use crate::protocol::SandboxPolicy;
use crate::spawn::StdioPolicy;
use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use tokio::process::Child;

pub(crate) fn preferred_windows_sandbox(_config: &WindowsConfig) -> Option<SandboxType> {
    None
}

#[allow(dead_code)]
pub(crate) fn prefer_wsl(_config: &WindowsConfig) -> bool {
    false
}

#[allow(dead_code)]
pub(crate) fn is_wsl_available() -> bool {
    false
}

#[allow(clippy::unused_async)]
pub(crate) async fn spawn_command_under_windows_wsl(
    _command: Vec<String>,
    _command_cwd: PathBuf,
    _sandbox_policy: &SandboxPolicy,
    _sandbox_policy_cwd: &Path,
    _stdio_policy: StdioPolicy,
    _env: HashMap<String, String>,
    _windows_config: &WindowsConfig,
) -> io::Result<Child> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Windows WSL sandboxing is only available on Windows",
    ))
}

#[allow(dead_code)]
pub(crate) fn sandbox_overrides_for_wsl(
    _sandbox_policy: &SandboxPolicy,
) -> io::Result<Vec<String>> {
    Ok(Vec::new())
}

#[allow(dead_code)]
pub(crate) fn convert_path_to_wsl(_path: &Path) -> io::Result<String> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "WSL path conversion only runs on Windows",
    ))
}
