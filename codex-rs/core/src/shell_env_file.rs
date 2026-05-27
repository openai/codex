use std::collections::HashMap;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use codex_protocol::ThreadId;
use codex_utils_absolute_path::AbsolutePathBuf;
use tempfile::TempPath;

pub(crate) const CODEX_ENV_FILE_ENV_VAR: &str = "CODEX_ENV_FILE";

#[cfg(not(windows))]
const SHELL_ENV_DIR: &str = "shell_env";

/// Session-owned script that hooks can populate with exported shell state.
///
/// Local shell tool commands source this file before running so lifecycle hook
/// setup remains scoped to the active session rather than persistent config.
pub(crate) struct ShellEnvFile {
    path: TempPath,
}

impl ShellEnvFile {
    pub(crate) fn for_session(
        codex_home: &AbsolutePathBuf,
        thread_id: ThreadId,
    ) -> Result<Option<Self>> {
        #[cfg(windows)]
        {
            let _ = (codex_home, thread_id);
            // TODO: Support a Windows shell environment persistence contract,
            // likely with PowerShell- and cmd-compatible formats.
            Ok(None)
        }
        #[cfg(not(windows))]
        {
            Self::new(codex_home, thread_id).map(Some)
        }
    }

    #[cfg(not(windows))]
    fn new(codex_home: &AbsolutePathBuf, thread_id: ThreadId) -> Result<Self> {
        let dir = codex_home.join(SHELL_ENV_DIR);
        std::fs::create_dir_all(dir.as_path())
            .with_context(|| format!("failed to create shell env dir {}", dir.display()))?;
        let file = tempfile::Builder::new()
            .prefix(&format!("{thread_id}."))
            .suffix(".sh")
            .tempfile_in(dir.as_path())
            .with_context(|| format!("failed to create shell env file in {}", dir.display()))?;
        Ok(Self {
            path: file.into_temp_path(),
        })
    }

    pub(crate) fn path(&self) -> &Path {
        self.path.as_ref()
    }

    pub(crate) fn insert_into_env(&self, env: &mut HashMap<String, String>) {
        env.insert(
            CODEX_ENV_FILE_ENV_VAR.to_string(),
            self.path().to_string_lossy().to_string(),
        );
    }
}

#[cfg(test)]
#[path = "shell_env_file_tests.rs"]
mod tests;
