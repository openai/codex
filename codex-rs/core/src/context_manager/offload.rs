use std::path::Path;
use std::path::PathBuf;
use tracing::warn;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub(crate) struct ContextOffloader {
    root: PathBuf,
}

impl ContextOffloader {
    pub(crate) fn new(codex_home: &Path) -> Self {
        Self {
            root: codex_home.join("context"),
        }
    }

    pub(crate) fn write_user_message(&self, text: &str) -> Option<PathBuf> {
        self.write_text("usermsgs", "user-message", text)
    }

    fn write_text(&self, dir_name: &str, file_prefix: &str, text: &str) -> Option<PathBuf> {
        let dir = self.root.join(dir_name);
        if let Err(err) = std::fs::create_dir_all(&dir) {
            warn!(error = %err, "failed to create offload directory");
            return None;
        }

        let id = Uuid::now_v7();
        let path = dir.join(format!("{file_prefix}-{id}.txt"));
        if let Err(err) = std::fs::write(&path, text.as_bytes()) {
            warn!(error = %err, "failed to write offloaded content");
            return None;
        }

        Some(path)
    }
}
