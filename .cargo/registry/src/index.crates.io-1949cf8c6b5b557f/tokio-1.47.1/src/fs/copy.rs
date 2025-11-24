use crate::fs::asyncify;
use std::path::Path;

/// Copies the contents of one file to another. This function will also copy the permission bits
/// of the original file to the destination file.
/// This function will overwrite the contents of to.
///
/// This is the async equivalent of [`std::fs::copy`].
///
/// # Examples
///
/// ```no_run
/// use tokio::fs;
///
/// # async fn dox() -> std::io::Result<()> {
/// fs::copy("foo.txt", "bar.txt").await?;
/// # Ok(())
/// # }
/// ```
pub async fn copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<u64, std::io::Error> {
    let from = from.as_ref().to_owned();
    let to = to.as_ref().to_owned();
    asyncify(|| std::fs::copy(from, to)).await
}
