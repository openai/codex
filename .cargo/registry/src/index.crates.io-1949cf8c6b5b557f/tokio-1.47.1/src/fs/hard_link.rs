use crate::fs::asyncify;

use std::io;
use std::path::Path;

/// Creates a new hard link on the filesystem.
///
/// This is an async version of [`std::fs::hard_link`].
///
/// The `link` path will be a link pointing to the `original` path. Note that systems
/// often require these two paths to both be located on the same filesystem.
///
/// # Platform-specific behavior
///
/// This function currently corresponds to the `link` function on Unix
/// and the `CreateHardLink` function on Windows.
/// Note that, this [may change in the future][changes].
///
/// [changes]: https://doc.rust-lang.org/std/io/index.html#platform-specific-behavior
///
/// # Errors
///
/// This function will return an error in the following situations, but is not
/// limited to just these cases:
///
/// * The `original` path is not a file or doesn't exist.
///
/// # Examples
///
/// ```no_run
/// use tokio::fs;
///
/// #[tokio::main]
/// async fn main() -> std::io::Result<()> {
///     fs::hard_link("a.txt", "b.txt").await?; // Hard link a.txt to b.txt
///     Ok(())
/// }
/// ```
pub async fn hard_link(original: impl AsRef<Path>, link: impl AsRef<Path>) -> io::Result<()> {
    let original = original.as_ref().to_owned();
    let link = link.as_ref().to_owned();

    asyncify(move || std::fs::hard_link(original, link)).await
}
