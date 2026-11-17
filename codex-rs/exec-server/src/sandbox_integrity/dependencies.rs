//! Shared inventory entries and filesystem facts, independent of any checker.

use codex_sandboxing::SandboxIntegrityRequest;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
#[cfg(not(target_os = "macos"))]
use std::collections::HashMap;
#[cfg(not(target_os = "macos"))]
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::PathBuf;

pub(super) type Dependency = (&'static str, io::Result<PathBuf>);

/// Inputs retained by the blocking worker. Seatbelt checks fixed paths and uses only the policy cwd.
pub(super) struct IntegrityCommand {
    pub(super) sandbox_policy_cwd: PathUri,
    #[cfg(not(target_os = "macos"))]
    pub(super) cwd: PathUri,
    #[cfg(not(target_os = "macos"))]
    pub(super) launcher: Option<OsString>,
    #[cfg(not(target_os = "macos"))]
    pub(super) env: HashMap<String, String>,
}

impl From<SandboxIntegrityRequest<'_>> for IntegrityCommand {
    fn from(request: SandboxIntegrityRequest<'_>) -> Self {
        Self {
            sandbox_policy_cwd: request.sandbox_policy_cwd.clone(),
            #[cfg(not(target_os = "macos"))]
            cwd: request.cwd.clone(),
            #[cfg(not(target_os = "macos"))]
            launcher: request.launcher.map(OsString::from),
            #[cfg(not(target_os = "macos"))]
            env: request.env.clone(),
        }
    }
}

pub(super) struct DependencyTarget {
    pub path: AbsolutePathBuf,
    pub metadata: fs::Metadata,
    pub resolved_path: Option<AbsolutePathBuf>,
}

impl DependencyTarget {
    pub(super) fn inspect(path: io::Result<PathBuf>) -> io::Result<Self> {
        let path = AbsolutePathBuf::from_absolute_path(path?)?;
        Ok(Self {
            metadata: fs::metadata(&path)?,
            resolved_path: path.canonicalize().ok(),
            path,
        })
    }
}

pub(super) fn codex_executable() -> Dependency {
    ("codex", std::env::current_exe())
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(super) fn codex_and_launcher_dependencies(request: &IntegrityCommand) -> Vec<Dependency> {
    let mut dependencies = vec![codex_executable()];
    if let Some(exe) = request.launcher.as_deref() {
        dependencies.push(("sandbox_launcher", Ok(PathBuf::from(exe))));
    }
    dependencies
}
