use crate::fs::asyncify;

use std::io;
use std::path::Path;

/// Creates a new directory symlink on the filesystem.
///
/// The `link` path will be a directory symbolic link pointing to the `original`
/// path.
///
/// This is an async version of [`std::os::windows::fs::symlink_dir`][std]
///
/// [std]: https://doc.rust-lang.org/std/os/windows/fs/fn.symlink_dir.html
pub async fn symlink_dir(original: impl AsRef<Path>, link: impl AsRef<Path>) -> io::Result<()> {
    let original = original.as_ref().to_owned();
    let link = link.as_ref().to_owned();

    asyncify(move || std::os::windows::fs::symlink_dir(original, link)).await
}
