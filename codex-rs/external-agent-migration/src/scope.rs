use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

/// The filesystem boundary within which migration detection or import runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum MigrationScope {
    Home,
    Repository { root: PathBuf },
}

impl MigrationScope {
    pub(super) fn home() -> Self {
        Self::Home
    }

    pub(super) fn from_cwd(cwd: Option<&Path>) -> io::Result<Option<Self>> {
        let Some(cwd) = cwd.filter(|cwd| !cwd.as_os_str().is_empty()) else {
            return Ok(Some(Self::Home));
        };

        let mut current = if cwd.is_absolute() {
            cwd.to_path_buf()
        } else {
            std::env::current_dir()?.join(cwd)
        };

        if !current.exists() {
            return Ok(None);
        }

        if current.is_file() {
            let Some(parent) = current.parent() else {
                return Ok(None);
            };
            current = parent.to_path_buf();
        }

        let fallback = current.clone();
        loop {
            let git_path = current.join(".git");
            if git_path.is_dir() || git_path.is_file() {
                return Self::repository(current);
            }
            if !current.pop() {
                break;
            }
        }

        Self::repository(fallback)
    }

    fn repository(root: PathBuf) -> io::Result<Option<Self>> {
        for relative_path in [
            ".codex",
            ".codex/config.toml",
            ".codex/agents",
            ".codex/hooks",
            ".agents",
            ".agents/skills",
        ] {
            if is_redirected_destination(&root.join(relative_path))? {
                return Ok(None);
            }
        }

        Ok(Some(Self::Repository { root }))
    }

    pub(super) fn repo_root(&self) -> Option<&Path> {
        match self {
            Self::Home => None,
            Self::Repository { root } => Some(root),
        }
    }

    pub(super) fn cwd(&self) -> Option<PathBuf> {
        self.repo_root().map(Path::to_path_buf)
    }

    pub(super) fn is_home(&self) -> bool {
        matches!(self, Self::Home)
    }
}

pub(super) fn is_redirected_destination(path: &Path) -> io::Result<bool> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };

    let is_redirected = metadata.file_type().is_symlink();
    #[cfg(windows)]
    let is_redirected = is_redirected || metadata.file_attributes() & 0x0400 != 0;

    Ok(is_redirected)
}

#[cfg(test)]
#[path = "scope_tests.rs"]
mod tests;
