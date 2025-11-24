use crate::fs::asyncify;

use std::io;
use std::path::{Path, PathBuf};

/// Returns the canonical, absolute form of a path with all intermediate
/// components normalized and symbolic links resolved.
///
/// This is an async version of [`std::fs::canonicalize`].
///
/// # Platform-specific behavior
///
/// This function currently corresponds to the `realpath` function on Unix
/// and the `CreateFile` and `GetFinalPathNameByHandle` functions on Windows.
/// Note that, this [may change in the future][changes].
///
/// On Windows, this converts the path to use [extended length path][path]
/// syntax, which allows your program to use longer path names, but means you
/// can only join backslash-delimited paths to it, and it may be incompatible
/// with other applications (if passed to the application on the command-line,
/// or written to a file another application may read).
///
/// [changes]: https://doc.rust-lang.org/std/io/index.html#platform-specific-behavior
/// [path]: https://msdn.microsoft.com/en-us/library/windows/desktop/aa365247(v=vs.85).aspx#maxpath
///
/// # Errors
///
/// This function will return an error in the following situations, but is not
/// limited to just these cases:
///
/// * `path` does not exist.
/// * A non-final component in path is not a directory.
///
/// # Examples
///
/// ```no_run
/// use tokio::fs;
/// use std::io;
///
/// #[tokio::main]
/// async fn main() -> io::Result<()> {
///     let path = fs::canonicalize("../a/../foo.txt").await?;
///     Ok(())
/// }
/// ```
pub async fn canonicalize(path: impl AsRef<Path>) -> io::Result<PathBuf> {
    let path = path.as_ref().to_owned();
    asyncify(move || std::fs::canonicalize(path)).await
}
