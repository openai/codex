use std::env;
use std::path::{Path, PathBuf};

#[cfg(doc)]
use crate::{tempdir_in, tempfile_in, Builder};

// Once rust 1.70 is wide-spread (Debian stable), we can use OnceLock from stdlib.
use once_cell::sync::OnceCell as OnceLock;

static DEFAULT_TEMPDIR: OnceLock<PathBuf> = OnceLock::new();

/// Override the default temporary directory (defaults to [`std::env::temp_dir`]). This function
/// changes the _global_ default temporary directory for the entire program and should not be called
/// except in exceptional cases where it's not configured correctly by the platform. Applications
/// should first check if the path returned by [`env::temp_dir`] is acceptable.
///
/// If you're writing a library and want to control where your temporary files are placed, you
/// should instead use the `_in` variants of the various temporary file/directory constructors
/// ([`tempdir_in`], [`tempfile_in`], the so-named functions on [`Builder`], etc.).
///
/// Only the first call to this function will succeed. All further calls will fail with `Err(path)`
/// where `path` is previously set default temporary directory override.
///
/// **NOTE:** This function does not check if the specified directory exists and/or is writable.
pub fn override_temp_dir(path: &Path) -> Result<(), PathBuf> {
    let mut we_set = false;
    let val = DEFAULT_TEMPDIR.get_or_init(|| {
        we_set = true;
        path.to_path_buf()
    });
    if we_set {
        Ok(())
    } else {
        Err(val.to_owned())
    }
}

/// Returns the default temporary directory, used for both temporary directories and files if no
/// directory is explicitly specified.
///
/// This function simply delegates to [`std::env::temp_dir`] unless the default temporary directory
/// has been override by a call to [`override_temp_dir`].
///
/// **NOTE:** This function does not check if the returned directory exists and/or is writable.
pub fn temp_dir() -> PathBuf {
    DEFAULT_TEMPDIR
        .get()
        .map(|p| p.to_owned())
        // Don't cache this in case the user uses std::env::set to change the temporary directory.
        .unwrap_or_else(env::temp_dir)
}
