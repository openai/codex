use std::collections::HashMap;
use std::path::PathBuf;

use codex_protocol::ThreadId;
use codex_utils_absolute_path::AbsolutePathBuf;

const HOOK_ENV_DIR: &str = "env";

#[derive(Debug)]
pub(crate) struct HookEnvFile {
    path: AbsolutePathBuf,
}

impl HookEnvFile {
    pub(crate) fn new(codex_home: &AbsolutePathBuf, thread_id: ThreadId) -> Self {
        if let Some(path) = env_path(codex_hooks::CODEX_ENV_FILE_ENV_VAR)
            .or_else(|| env_path(codex_hooks::CLAUDE_ENV_FILE_ENV_VAR))
        {
            return Self { path };
        }

        Self {
            path: codex_home
                .join("tmp")
                .join(HOOK_ENV_DIR)
                .join(format!("{thread_id}-{}.sh", uuid::Uuid::new_v4())),
        }
    }

    pub(crate) fn path(&self) -> &AbsolutePathBuf {
        &self.path
    }

    pub(crate) fn add_to_env(&self, env: &mut HashMap<String, String>) {
        add_env_file_vars(env, &self.path);
    }

    pub(crate) fn ensure_parent_dir(&self) {
        let Some(parent) = self.path.as_path().parent() else {
            return;
        };
        if let Err(err) = std::fs::create_dir_all(parent) {
            tracing::warn!(
                path = %parent.display(),
                "failed to create hook env file directory: {err}"
            );
        }
    }
}

pub(crate) fn add_env_file_vars(env: &mut HashMap<String, String>, path: &AbsolutePathBuf) {
    let path = path.display().to_string();
    env.insert(
        codex_hooks::CODEX_ENV_FILE_ENV_VAR.to_string(),
        path.clone(),
    );
    env.insert(codex_hooks::CLAUDE_ENV_FILE_ENV_VAR.to_string(), path);
}

fn env_path(name: &str) -> Option<AbsolutePathBuf> {
    let value = std::env::var_os(name)?;
    if value.as_os_str().is_empty() {
        return None;
    }

    let path = PathBuf::from(value);
    let path = if path.is_absolute() {
        path
    } else {
        std::env::current_dir().ok()?.join(path)
    };
    AbsolutePathBuf::from_absolute_path(path).ok()
}
