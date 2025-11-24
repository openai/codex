use crate::fs::asyncify;

use std::fs::Metadata;
use std::io;
use std::path::Path;

/// Given a path, queries the file system to get information about a file,
/// directory, etc.
///
/// This is an async version of [`std::fs::metadata`].
///
/// This function will traverse symbolic links to query information about the
/// destination file.
///
/// # Platform-specific behavior
///
/// This function currently corresponds to the `stat` function on Unix and the
/// `GetFileAttributesEx` function on Windows.  Note that, this [may change in
/// the future][changes].
///
/// [changes]: https://doc.rust-lang.org/std/io/index.html#platform-specific-behavior
///
/// # Errors
///
/// This function will return an error in the following situations, but is not
/// limited to just these cases:
///
/// * The user lacks permissions to perform `metadata` call on `path`.
/// * `path` does not exist.
///
/// # Examples
///
/// ```rust,no_run
/// use tokio::fs;
///
/// #[tokio::main]
/// async fn main() -> std::io::Result<()> {
///     let attr = fs::metadata("/some/file/path.txt").await?;
///     // inspect attr ...
///     Ok(())
/// }
/// ```
pub async fn metadata(path: impl AsRef<Path>) -> io::Result<Metadata> {
    let path = path.as_ref().to_owned();
    asyncify(|| std::fs::metadata(path)).await
}
