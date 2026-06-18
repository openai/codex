use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
#[error(
    "failed to initialize sqlite local db at {}: {detail}",
    database_path.display()
)]
pub struct LocalStateDbStartupError {
    database_path: PathBuf,
    detail: String,
}

impl LocalStateDbStartupError {
    pub fn new(database_path: PathBuf, detail: String) -> Self {
        Self {
            database_path,
            detail,
        }
    }

    pub fn database_path(&self) -> &Path {
        self.database_path.as_path()
    }

    pub fn state_db_path(&self) -> &Path {
        self.database_path()
    }

    pub fn detail(&self) -> &str {
        self.detail.as_str()
    }
}
