//! Shared containment dependency inventory entries.

use std::io;
use std::path::PathBuf;

pub(super) type Dependency = (&'static str, io::Result<PathBuf>);

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
