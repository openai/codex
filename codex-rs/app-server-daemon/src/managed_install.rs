use std::path::Path;
use std::path::PathBuf;

pub(crate) fn managed_codex_bin(codex_home: &Path) -> PathBuf {
    codex_home
        .join("packages")
        .join("standalone")
        .join("current")
        .join(managed_codex_file_name())
}

fn managed_codex_file_name() -> &'static str {
    if cfg!(windows) { "codex.exe" } else { "codex" }
}
