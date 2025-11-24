use crate::fs::asyncify;

use std::io;
use std::path::Path;

/// Recursively creates a directory and all of its parent components if they
/// are missing.
///
/// This is an async version of [`std::fs::create_dir_all`].
///
/// # Platform-specific behavior
///
/// This function currently corresponds to the `mkdir` function on Unix
/// and the `CreateDirectory` function on Windows.
/// Note that, this [may change in the future][changes].
///
/// [changes]: https://doc.rust-lang.org/std/io/index.html#platform-specific-behavior
///
/// # Errors
///
/// This function will return an error in the following situations, but is not
/// limited to just these cases:
///
/// * If any directory in the path specified by `path` does not already exist
///   and it could not be created otherwise. The specific error conditions for
///   when a directory is being created (after it is determined to not exist) are
///   outlined by [`fs::create_dir`].
///
/// Notable exception is made for situations where any of the directories
/// specified in the `path` could not be created as it was being created concurrently.
/// Such cases are considered to be successful. That is, calling `create_dir_all`
/// concurrently from multiple threads or processes is guaranteed not to fail
/// due to a race condition with itself.
///
/// [`fs::create_dir`]: std::fs::create_dir
///
/// # Examples
///
/// ```no_run
/// use tokio::fs;
///
/// #[tokio::main]
/// async fn main() -> std::io::Result<()> {
///     fs::create_dir_all("/some/dir").await?;
///     Ok(())
/// }
/// ```
pub async fn create_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
    let path = path.as_ref().to_owned();
    asyncify(move || std::fs::create_dir_all(path)).await
}
