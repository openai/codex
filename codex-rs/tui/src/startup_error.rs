use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
#[error(
    "failed to initialize sqlite state db at {}: {detail}",
    state_db_path.display()
)]
pub struct LocalStateDbStartupError {
    state_db_path: PathBuf,
    detail: String,
}

impl LocalStateDbStartupError {
    pub fn new(state_db_path: PathBuf, detail: String) -> Self {
        Self {
            state_db_path,
            detail,
        }
    }

    pub fn state_db_path(&self) -> &Path {
        self.state_db_path.as_path()
    }

    pub fn detail(&self) -> &str {
        self.detail.as_str()
    }
}
