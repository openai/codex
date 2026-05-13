use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use codex_utils_absolute_path::AbsolutePathBuf;

use crate::backend::ListMemoriesRequest;
use crate::backend::ListMemoriesResponse;
use crate::backend::MemoriesBackend;
use crate::backend::MemoriesBackendError;
use crate::backend::ReadMemoryRequest;
use crate::backend::ReadMemoryResponse;
use crate::backend::SearchMemoriesRequest;
use crate::backend::SearchMemoriesResponse;

mod list;
mod path;
mod read;
mod search;

#[derive(Debug, Clone)]
pub(crate) struct LocalMemoriesBackend {
    root: PathBuf,
}

impl LocalMemoriesBackend {
    pub(crate) fn from_codex_home(codex_home: &AbsolutePathBuf) -> Self {
        Self::from_memory_root(codex_home.join("memories").to_path_buf())
    }

    pub(crate) fn from_memory_root(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    async fn resolve_scoped_path(
        &self,
        relative_path: Option<&str>,
    ) -> Result<PathBuf, MemoriesBackendError> {
        let Some(relative_path) = relative_path else {
            return Ok(self.root.clone());
        };
        let relative = Path::new(relative_path);
        if relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            return Err(MemoriesBackendError::invalid_path(
                relative_path,
                "must stay within the memories root",
            ));
        }
        if relative.components().any(path::is_hidden_component) {
            return Err(MemoriesBackendError::NotFound {
                path: relative_path.to_string(),
            });
        }

        let components = relative.components().collect::<Vec<_>>();
        let mut scoped_path = self.root.clone();
        for (idx, component) in components.iter().enumerate() {
            scoped_path.push(component.as_os_str());

            let Some(metadata) = Self::metadata_or_none(&scoped_path).await? else {
                for remaining_component in components.iter().skip(idx + 1) {
                    scoped_path.push(remaining_component.as_os_str());
                }
                return Ok(scoped_path);
            };

            path::reject_symlink(
                &path::display_relative_path(&self.root, &scoped_path),
                &metadata,
            )?;
            if idx + 1 < components.len() && !metadata.is_dir() {
                return Err(MemoriesBackendError::invalid_path(
                    relative_path,
                    "traverses through a non-directory path component",
                ));
            }
        }

        Ok(scoped_path)
    }

    async fn metadata_or_none(
        path: &Path,
    ) -> Result<Option<std::fs::Metadata>, MemoriesBackendError> {
        match tokio::fs::symlink_metadata(path).await {
            Ok(metadata) => Ok(Some(metadata)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err.into()),
        }
    }
}

impl MemoriesBackend for LocalMemoriesBackend {
    async fn list(
        &self,
        request: ListMemoriesRequest,
    ) -> Result<ListMemoriesResponse, MemoriesBackendError> {
        list::list(self, request).await
    }

    async fn read(
        &self,
        request: ReadMemoryRequest,
    ) -> Result<ReadMemoryResponse, MemoriesBackendError> {
        read::read(self, request).await
    }

    async fn search(
        &self,
        request: SearchMemoriesRequest,
    ) -> Result<SearchMemoriesResponse, MemoriesBackendError> {
        search::search(self, request).await
    }
}
