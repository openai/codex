use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
use std::io;
use std::path::Path;
use std::sync::Arc;

use codex_utils_absolute_path::AbsolutePathBuf;

use crate::ExecutorFileSystem;
use crate::FileMetadata;
use crate::ReadDirectoryEntry;
use crate::LOCAL_ENVIRONMENT_ID;
use crate::LOCAL_FS;

/// Binds an absolute path to the executor filesystem and environment that owns it.
#[derive(Clone)]
pub struct EnvironmentPathRef {
    environment_id: String,
    file_system: Arc<dyn ExecutorFileSystem>,
    path: AbsolutePathBuf,
}

impl EnvironmentPathRef {
    pub fn new(
        environment_id: String,
        file_system: Arc<dyn ExecutorFileSystem>,
        path: AbsolutePathBuf,
    ) -> Self {
        Self {
            environment_id,
            file_system,
            path,
        }
    }

    pub fn local(path: AbsolutePathBuf) -> Self {
        Self::new(
            LOCAL_ENVIRONMENT_ID.to_string(),
            Arc::clone(&LOCAL_FS),
            path,
        )
    }

    pub fn path(&self) -> &AbsolutePathBuf {
        &self.path
    }

    pub fn environment_id(&self) -> &str {
        &self.environment_id
    }

    pub fn file_system(&self) -> Arc<dyn ExecutorFileSystem> {
        Arc::clone(&self.file_system)
    }

    pub async fn read_to_string(&self) -> io::Result<String> {
        self.file_system
            .read_file_text(&self.path, /*sandbox*/ None)
            .await
    }

    pub async fn metadata(&self) -> io::Result<FileMetadata> {
        self.file_system
            .get_metadata(&self.path, /*sandbox*/ None)
            .await
    }

    pub async fn read_directory(&self) -> io::Result<Vec<ReadDirectoryEntry>> {
        self.file_system
            .read_directory(&self.path, /*sandbox*/ None)
            .await
    }

    pub fn join_relative(&self, relative: &Path) -> Option<Self> {
        relative
            .is_relative()
            .then(|| self.with_path(self.path.join(relative)))
    }

    pub fn parent_dir(&self) -> Option<Self> {
        self.path.parent().map(|path| self.with_path(path))
    }

    pub fn with_path(&self, path: AbsolutePathBuf) -> Self {
        Self::new(
            self.environment_id.clone(),
            Arc::clone(&self.file_system),
            path,
        )
    }
}

impl PartialEq for EnvironmentPathRef {
    fn eq(&self, other: &Self) -> bool {
        self.environment_id == other.environment_id
            && Arc::ptr_eq(&self.file_system, &other.file_system)
            && self.path == other.path
    }
}

impl Eq for EnvironmentPathRef {}

impl Hash for EnvironmentPathRef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.environment_id.hash(state);
        (Arc::as_ptr(&self.file_system) as *const () as usize).hash(state);
        self.path.hash(state);
    }
}

impl fmt::Debug for EnvironmentPathRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EnvironmentPathRef")
            .field("environment_id", &self.environment_id)
            .field("path", &self.path)
            .finish()
    }
}
