use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use crate::backend::MemoriesBackendError;

pub(super) async fn read_sorted_dir_paths(
    dir_path: &Path,
) -> Result<Vec<PathBuf>, MemoriesBackendError> {
    let mut dir = match tokio::fs::read_dir(dir_path).await {
        Ok(dir) => dir,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err.into()),
    };
    let mut paths = Vec::new();
    while let Some(entry) = dir.next_entry().await? {
        paths.push(entry.path());
    }
    paths.sort();
    Ok(paths)
}

pub(super) fn reject_symlink(
    path: &str,
    metadata: &std::fs::Metadata,
) -> Result<(), MemoriesBackendError> {
    if metadata.file_type().is_symlink() {
        return Err(MemoriesBackendError::invalid_path(
            path,
            "must not be a symlink",
        ));
    }
    Ok(())
}

pub(super) fn is_hidden_component(component: Component<'_>) -> bool {
    matches!(
        component,
        Component::Normal(name) if name.to_string_lossy().starts_with('.')
    )
}

pub(super) fn is_hidden_path(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| name.to_string_lossy().starts_with('.'))
}

pub(super) fn display_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .filter(|component| !component.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}
