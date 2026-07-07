use std::path::Path;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::OnceLock;
use std::sync::PoisonError;

use tempfile::tempdir;

/// Serializes tests that mutate process-wide CODEX_HOME.
///
/// Keep OAuth tests on this one guard instead of defining per-module helpers; otherwise
/// concurrently running test modules can point File/Secrets storage at different homes.
pub(super) struct TempCodexHome {
    _guard: MutexGuard<'static, ()>,
    _dir: tempfile::TempDir,
}

impl TempCodexHome {
    pub(super) fn new() -> Self {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let guard = LOCK
            .get_or_init(Mutex::default)
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let dir = tempdir().expect("create CODEX_HOME temp dir");
        unsafe {
            std::env::set_var("CODEX_HOME", dir.path());
        }
        Self {
            _guard: guard,
            _dir: dir,
        }
    }

    pub(super) fn path(&self) -> &Path {
        self._dir.path()
    }
}

impl Drop for TempCodexHome {
    fn drop(&mut self) {
        unsafe {
            std::env::remove_var("CODEX_HOME");
        }
    }
}
