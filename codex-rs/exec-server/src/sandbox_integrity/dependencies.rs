//! Shared inventory entries and filesystem facts, independent of any checker.

use codex_utils_absolute_path::AbsolutePathBuf;
use std::fs;
use std::io;
use std::path::PathBuf;

pub(super) type Dependency = (&'static str, io::Result<PathBuf>);

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
pub(super) fn codex_and_launcher_dependencies(
    request: &codex_sandboxing::SandboxExecRequest,
) -> Vec<Dependency> {
    let mut dependencies = vec![codex_executable()];
    if let Some(exe) = request.command.first() {
        dependencies.push(("sandbox_launcher", Ok(PathBuf::from(exe))));
    }
    dependencies
}
