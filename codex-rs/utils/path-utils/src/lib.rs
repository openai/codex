//! Path normalization, symlink resolution, and atomic writes shared across Codex crates.

pub(crate) mod env;
pub use env::is_wsl;

use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashSet;
#[cfg(unix)]
use std::ffi::CString;
#[cfg(unix)]
use std::fs::File;
use std::io;
#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::os::fd::FromRawFd;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use tempfile::NamedTempFile;

pub fn normalize_for_path_comparison(path: impl AsRef<Path>) -> std::io::Result<PathBuf> {
    let canonical = path.as_ref().canonicalize()?;
    Ok(normalize_for_wsl(canonical))
}

/// Compare paths after applying Codex's filesystem normalization.
///
/// If either path cannot be normalized, this falls back to direct path equality.
pub fn paths_match_after_normalization(left: impl AsRef<Path>, right: impl AsRef<Path>) -> bool {
    if let (Ok(left), Ok(right)) = (
        normalize_for_path_comparison(left.as_ref()),
        normalize_for_path_comparison(right.as_ref()),
    ) {
        return left == right;
    }
    left.as_ref() == right.as_ref()
}

pub fn normalize_for_native_workdir(path: impl AsRef<Path>) -> PathBuf {
    normalize_for_native_workdir_with_flag(path.as_ref().to_path_buf(), cfg!(windows))
}

pub struct SymlinkWritePaths {
    pub read_path: Option<PathBuf>,
    pub write_path: PathBuf,
}

/// Resolve the final filesystem target for `path` while retaining a safe write path.
///
/// This follows symlink chains (including relative symlink targets) until it reaches a
/// non-symlink path. If the chain cycles or any metadata/link resolution fails, it
/// returns `read_path: None` and uses the original absolute path as `write_path`.
/// There is no fixed max-resolution count; cycles are detected via a visited set.
pub fn resolve_symlink_write_paths(path: &Path) -> io::Result<SymlinkWritePaths> {
    let root = AbsolutePathBuf::from_absolute_path(path)
        .map(AbsolutePathBuf::into_path_buf)
        .unwrap_or_else(|_| path.to_path_buf());
    let mut current = root.clone();
    let mut visited = HashSet::new();

    // Follow symlink chains while guarding against cycles.
    loop {
        let meta = match std::fs::symlink_metadata(&current) {
            Ok(meta) => meta,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                return Ok(SymlinkWritePaths {
                    read_path: Some(current.clone()),
                    write_path: current,
                });
            }
            Err(_) => {
                return Ok(SymlinkWritePaths {
                    read_path: None,
                    write_path: root,
                });
            }
        };

        if !meta.file_type().is_symlink() {
            return Ok(SymlinkWritePaths {
                read_path: Some(current.clone()),
                write_path: current,
            });
        }

        // If we've already seen this path, the chain cycles.
        if !visited.insert(current.clone()) {
            return Ok(SymlinkWritePaths {
                read_path: None,
                write_path: root,
            });
        }

        let target = match std::fs::read_link(&current) {
            Ok(target) => target,
            Err(_) => {
                return Ok(SymlinkWritePaths {
                    read_path: None,
                    write_path: root,
                });
            }
        };

        let next = if target.is_absolute() {
            AbsolutePathBuf::from_absolute_path(&target)
        } else if let Some(parent) = current.parent() {
            Ok(AbsolutePathBuf::resolve_path_against_base(&target, parent))
        } else {
            return Ok(SymlinkWritePaths {
                read_path: None,
                write_path: root,
            });
        };

        let next = match next {
            Ok(path) => path.into_path_buf(),
            Err(_) => {
                return Ok(SymlinkWritePaths {
                    read_path: None,
                    write_path: root,
                });
            }
        };

        current = next;
    }
}

pub fn write_atomically(write_path: &Path, contents: &str) -> io::Result<()> {
    let parent = write_path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("path {} has no parent directory", write_path.display()),
        )
    })?;
    std::fs::create_dir_all(parent)?;
    let tmp = NamedTempFile::new_in(parent)?;
    std::fs::write(tmp.path(), contents)?;
    tmp.persist(write_path)?;
    Ok(())
}

/// Creates or truncates a file for writing without following symlinks in any path component.
///
/// Relative paths are resolved from the process's current directory. The returned file uses mode
/// `0o666`, subject to the process umask.
#[cfg(unix)]
pub fn open_file_for_write_no_follow(path: &Path) -> io::Result<File> {
    let components = path
        .components()
        .filter_map(|component| match component {
            Component::RootDir | Component::CurDir => None,
            Component::ParentDir => Some(Ok(std::ffi::OsStr::new(".."))),
            Component::Normal(component) => Some(Ok(component)),
            Component::Prefix(_) => Some(Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unsupported path prefix in {}", path.display()),
            ))),
        })
        .collect::<io::Result<Vec<_>>>()?;
    let Some((file_name, parent_components)) = components.split_last() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("path {} does not name a file", path.display()),
        ));
    };

    let mut current_dir = if path.is_absolute() {
        File::open("/")?
    } else {
        File::open(".")?
    };
    for component in parent_components {
        current_dir = openat(
            &current_dir,
            component,
            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW,
            /*mode*/ 0,
        )?;
    }
    openat(
        &current_dir,
        file_name,
        libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        /*mode*/ 0o666,
    )
}

#[cfg(unix)]
fn openat(
    directory: &File,
    path: &std::ffi::OsStr,
    flags: libc::c_int,
    mode: libc::mode_t,
) -> io::Result<File> {
    let path = CString::new(path.as_bytes())
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    // SAFETY: `path` is NUL-terminated, `directory` owns a valid fd, and the
    // returned fd is transferred into a `File` exactly once.
    let fd = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            path.as_ptr(),
            flags,
            libc::c_uint::from(mode),
        )
    };
    if fd == -1 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `openat` returned a new owned file descriptor.
    Ok(unsafe { File::from_raw_fd(fd) })
}

fn normalize_for_wsl(path: PathBuf) -> PathBuf {
    normalize_for_wsl_with_flag(path, env::is_wsl())
}

fn normalize_for_native_workdir_with_flag(path: PathBuf, is_windows: bool) -> PathBuf {
    if is_windows {
        dunce::simplified(&path).to_path_buf()
    } else {
        path
    }
}

fn normalize_for_wsl_with_flag(path: PathBuf, is_wsl: bool) -> PathBuf {
    if !is_wsl {
        return path;
    }

    if !is_wsl_case_insensitive_path(&path) {
        return path;
    }

    lower_ascii_path(path)
}

fn is_wsl_case_insensitive_path(path: &Path) -> bool {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::ffi::OsStrExt;
        use std::path::Component;

        let mut components = path.components();
        let Some(Component::RootDir) = components.next() else {
            return false;
        };
        let Some(Component::Normal(mnt)) = components.next() else {
            return false;
        };
        if !ascii_eq_ignore_case(mnt.as_bytes(), b"mnt") {
            return false;
        }
        let Some(Component::Normal(drive)) = components.next() else {
            return false;
        };
        let drive_bytes = drive.as_bytes();
        drive_bytes.len() == 1 && drive_bytes[0].is_ascii_alphabetic()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = path;
        false
    }
}

#[cfg(target_os = "linux")]
fn ascii_eq_ignore_case(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(lhs, rhs)| lhs.to_ascii_lowercase() == *rhs)
}

#[cfg(target_os = "linux")]
fn lower_ascii_path(path: PathBuf) -> PathBuf {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::ffi::OsStringExt;

    // WSL mounts Windows drives under /mnt/<drive>, which are case-insensitive.
    let bytes = path.as_os_str().as_bytes();
    let mut lowered = Vec::with_capacity(bytes.len());
    for byte in bytes {
        lowered.push(byte.to_ascii_lowercase());
    }
    PathBuf::from(OsString::from_vec(lowered))
}

#[cfg(not(target_os = "linux"))]
fn lower_ascii_path(path: PathBuf) -> PathBuf {
    path
}

#[cfg(test)]
#[path = "path_utils_tests.rs"]
mod tests;
