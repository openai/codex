//! Atomic credential-file replacement without broadening existing file permissions.
//! Symlinks retain their target; hard links and read-only files fail rather than split identity.

#[cfg(not(windows))]
use std::fs::File;
use std::io;
use std::path::Path;
use std::path::PathBuf;

pub(super) fn resolve_auth_file(mut path: PathBuf) -> io::Result<PathBuf> {
    for _ in 0..40 {
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                let target = std::fs::read_link(&path)?;
                path = if target.is_absolute() {
                    target
                } else {
                    path.parent()
                        .ok_or_else(|| io::Error::other("credential link has no parent"))?
                        .join(target)
                };
            }
            Ok(_) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(path),
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "too many credential symlinks",
    ))
}

pub(super) fn check_replaceable(path: &Path) -> io::Result<()> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if metadata.permissions().readonly() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "credential file is read-only",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() > 1 && metadata.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "atomic credential replacement does not support hard-linked files",
            ));
        }
    }
    // Check the destination's write permission, not only its parent directory's.
    std::fs::OpenOptions::new().write(true).open(path)?;
    Ok(())
}

#[cfg(not(windows))]
pub(super) fn replacement(path: &Path) -> io::Result<tempfile::NamedTempFile<File>> {
    tempfile::Builder::new().prefix(".auth-").tempfile_in(
        path.parent()
            .ok_or_else(|| io::Error::other("credential file has no parent"))?,
    )
}

#[cfg(windows)]
#[path = "storage_file_windows.rs"]
mod windows;
#[cfg(windows)]
pub(super) use windows::replacement;
