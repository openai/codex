use std::io;
use std::path::Path;
use std::path::PathBuf;

use codex_utils_absolute_path::AbsolutePathBuf;

pub(super) const CONFIG_PATH_KEY: &str = "codex.config-source.path";

#[cfg(unix)]
pub(super) fn git_var_path_from_bytes(path: &[u8]) -> io::Result<PathBuf> {
    use std::os::unix::ffi::OsStringExt;

    Ok(PathBuf::from(std::ffi::OsString::from_vec(path.to_vec())))
}

#[cfg(not(unix))]
pub(super) fn git_var_path_from_bytes(path: &[u8]) -> io::Result<PathBuf> {
    Ok(PathBuf::from(std::str::from_utf8(path).map_err(|_| {
        invalid_config_source("non-UTF-8 Git config source path")
    })?))
}

pub(super) fn resolve_literal_path(path: impl AsRef<Path>, base: &Path) -> PathBuf {
    let path = path.as_ref();
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

pub(super) fn normalize_absolute_path(path: impl AsRef<Path>) -> io::Result<PathBuf> {
    Ok(AbsolutePathBuf::from_absolute_path(path)?.into_path_buf())
}

#[cfg(windows)]
pub(super) fn reject_raw_ambiguous_windows_config_path(path: &str) -> io::Result<()> {
    if windows_config_path_is_ambiguous(path) {
        return Err(invalid_config_source("ambiguous Windows Git config path"));
    }
    Ok(())
}

#[cfg(not(windows))]
pub(super) fn reject_raw_ambiguous_windows_config_path(_path: &str) -> io::Result<()> {
    Ok(())
}

#[cfg(any(windows, test))]
pub(super) fn windows_config_path_is_ambiguous(path: &str) -> bool {
    crate::path_authority::windows_path_is_ambiguous(path)
}

pub(super) fn invalid_config_source(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
