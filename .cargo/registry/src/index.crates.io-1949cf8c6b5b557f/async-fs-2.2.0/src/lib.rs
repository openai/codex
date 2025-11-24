//! Async filesystem primitives.
//!
//! This crate is an async version of [`std::fs`].
//!
//! # Implementation
//!
//! This crate uses [`blocking`] to offload blocking I/O onto a thread pool.
//!
//! [`blocking`]: https://docs.rs/blocking
//!
//! # Examples
//!
//! Create a new file and write some bytes to it:
//!
//! ```no_run
//! use async_fs::File;
//! use futures_lite::io::AsyncWriteExt;
//!
//! # futures_lite::future::block_on(async {
//! let mut file = File::create("a.txt").await?;
//! file.write_all(b"Hello, world!").await?;
//! file.flush().await?;
//! # std::io::Result::Ok(()) });
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs, missing_debug_implementations, rust_2018_idioms)]
#![doc(
    html_favicon_url = "https://raw.githubusercontent.com/smol-rs/smol/master/assets/images/logo_fullsize_transparent.png"
)]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/smol-rs/smol/master/assets/images/logo_fullsize_transparent.png"
)]

use std::ffi::OsString;
use std::fmt;
use std::future::Future;
use std::io::{self, SeekFrom};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

#[cfg(unix)]
use std::os::unix::fs::{DirEntryExt as _, OpenOptionsExt as _};

#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt as _;

use async_lock::Mutex;
use blocking::{unblock, Unblock};
use futures_lite::future::FutureExt;
use futures_lite::io::{AsyncRead, AsyncSeek, AsyncWrite, AsyncWriteExt};
use futures_lite::ready;
use futures_lite::stream::Stream;

#[doc(no_inline)]
pub use std::fs::{FileType, Metadata, Permissions};

/// Returns the canonical form of a path.
///
/// The returned path is in absolute form with all intermediate components normalized and symbolic
/// links resolved.
///
/// # Errors
///
/// An error will be returned in the following situations:
///
/// * `path` does not point to an existing file or directory.
/// * A non-final component in `path` is not a directory.
/// * Some other I/O error occurred.
///
/// # Examples
///
/// ```no_run
/// # futures_lite::future::block_on(async {
/// let path = async_fs::canonicalize(".").await?;
/// # std::io::Result::Ok(()) });
/// ```
pub async fn canonicalize<P: AsRef<Path>>(path: P) -> io::Result<PathBuf> {
    let path = path.as_ref().to_owned();
    unblock(move || std::fs::canonicalize(path)).await
}

/// Copies a file to a new location.
///
/// On success, the total number of bytes copied is returned and equals the length of the `dst`
/// file after this operation.
///
/// The old contents of `dst` will be overwritten. If `src` and `dst` both point to the same
/// file, then the file will likely get truncated as a result of this operation.
///
/// If you're working with open [`File`]s and want to copy contents through those types, use
/// [`futures_lite::io::copy()`] instead.
///
/// # Errors
///
/// An error will be returned in the following situations:
///
/// * `src` does not point to an existing file.
/// * The current process lacks permissions to read `src` or write `dst`.
/// * Some other I/O error occurred.
///
/// # Examples
///
/// ```no_run
/// # futures_lite::future::block_on(async {
/// let num_bytes = async_fs::copy("a.txt", "b.txt").await?;
/// # std::io::Result::Ok(()) });
/// ```
pub async fn copy<P: AsRef<Path>, Q: AsRef<Path>>(src: P, dst: Q) -> io::Result<u64> {
    let src = src.as_ref().to_owned();
    let dst = dst.as_ref().to_owned();
    unblock(move || std::fs::copy(&src, &dst)).await
}

/// Creates a new, empty directory at the provided path
///
/// # Platform-specific behavior
///
/// This function currently corresponds to the `mkdir` function on Unix
/// and the `CreateDirectory` function on Windows.
/// Note that, this [may change in the future][changes].
///
/// [changes]: io#platform-specific-behavior
///
/// **NOTE**: If a parent of the given path doesn't exist, this function will
/// return an error. To create a directory and all its missing parents at the
/// same time, use the [`create_dir_all`] function.
///
/// # Errors
///
/// This function will return an error in the following situations, but is not
/// limited to just these cases:
///
/// * User lacks permissions to create directory at `path`.
/// * A parent of the given path doesn't exist. (To create a directory and all
///   its missing parents at the same time, use the [`create_dir_all`]
///   function.)
/// * `path` already exists.
///
/// # Examples
///
/// ```no_run
/// use std::fs;
///
/// fn main() -> std::io::Result<()> {
///     fs::create_dir("/some/dir")?;
///     Ok(())
/// }
/// ```
pub async fn create_dir<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let path = path.as_ref().to_owned();
    unblock(move || std::fs::create_dir(path)).await
}

/// Recursively create a directory and all of its parent components if they
/// are missing.
///
/// # Platform-specific behavior
///
/// This function currently corresponds to the `mkdir` function on Unix
/// and the `CreateDirectory` function on Windows.
/// Note that, this [may change in the future][changes].
///
/// [changes]: io#platform-specific-behavior
///
/// # Errors
///
/// This function will return an error in the following situations, but is not
/// limited to just these cases:
///
/// * If any directory in the path specified by `path`
///   does not already exist and it could not be created otherwise. The specific
///   error conditions for when a directory is being created (after it is
///   determined to not exist) are outlined by [`fs::create_dir`].
///
/// Notable exception is made for situations where any of the directories
/// specified in the `path` could not be created as it was being created concurrently.
/// Such cases are considered to be successful. That is, calling `create_dir_all`
/// concurrently from multiple threads or processes is guaranteed not to fail
/// due to a race condition with itself.
///
/// [`fs::create_dir`]: create_dir
///
/// # Examples
///
/// ```no_run
/// use std::fs;
///
/// fn main() -> std::io::Result<()> {
///     fs::create_dir_all("/some/dir")?;
///     Ok(())
/// }
/// ```
pub async fn create_dir_all<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let path = path.as_ref().to_owned();
    unblock(move || std::fs::create_dir_all(path)).await
}

/// Creates a hard link on the filesystem.
///
/// The `dst` path will be a link pointing to the `src` path. Note that operating systems often
/// require these two paths to be located on the same filesystem.
///
/// # Errors
///
/// An error will be returned in the following situations:
///
/// * `src` does not point to an existing file.
/// * Some other I/O error occurred.
///
/// # Examples
///
/// ```no_run
/// # futures_lite::future::block_on(async {
/// async_fs::hard_link("a.txt", "b.txt").await?;
/// # std::io::Result::Ok(()) });
/// ```
pub async fn hard_link<P: AsRef<Path>, Q: AsRef<Path>>(src: P, dst: Q) -> io::Result<()> {
    let src = src.as_ref().to_owned();
    let dst = dst.as_ref().to_owned();
    unblock(move || std::fs::hard_link(&src, &dst)).await
}

/// Reads metadata for a path.
///
/// This function will traverse symbolic links to read metadata for the target file or directory.
/// If you want to read metadata without following symbolic links, use [`symlink_metadata()`]
/// instead.
///
/// # Errors
///
/// An error will be returned in the following situations:
///
/// * `path` does not point to an existing file or directory.
/// * The current process lacks permissions to read metadata for the path.
/// * Some other I/O error occurred.
///
/// # Examples
///
/// ```no_run
/// # futures_lite::future::block_on(async {
/// let perm = async_fs::metadata("a.txt").await?.permissions();
/// # std::io::Result::Ok(()) });
/// ```
pub async fn metadata<P: AsRef<Path>>(path: P) -> io::Result<Metadata> {
    let path = path.as_ref().to_owned();
    unblock(move || std::fs::metadata(path)).await
}

/// Reads the entire contents of a file as raw bytes.
///
/// This is a convenience function for reading entire files. It pre-allocates a buffer based on the
/// file size when available, so it is typically faster than manually opening a file and reading
/// from it.
///
/// If you want to read the contents as a string, use [`read_to_string()`] instead.
///
/// # Errors
///
/// An error will be returned in the following situations:
///
/// * `path` does not point to an existing file.
/// * The current process lacks permissions to read the file.
/// * Some other I/O error occurred.
///
/// # Examples
///
/// ```no_run
/// # futures_lite::future::block_on(async {
/// let contents = async_fs::read("a.txt").await?;
/// # std::io::Result::Ok(()) });
/// ```
pub async fn read<P: AsRef<Path>>(path: P) -> io::Result<Vec<u8>> {
    let path = path.as_ref().to_owned();
    unblock(move || std::fs::read(path)).await
}

/// Returns a stream of entries in a directory.
///
/// The stream yields items of type [`io::Result`]`<`[`DirEntry`]`>`. Note that I/O errors can
/// occur while reading from the stream.
///
/// # Errors
///
/// An error will be returned in the following situations:
///
/// * `path` does not point to an existing directory.
/// * The current process lacks permissions to read the contents of the directory.
/// * Some other I/O error occurred.
///
/// # Examples
///
/// ```no_run
/// # futures_lite::future::block_on(async {
/// use futures_lite::stream::StreamExt;
///
/// let mut entries = async_fs::read_dir(".").await?;
///
/// while let Some(entry) = entries.try_next().await? {
///     println!("{}", entry.file_name().to_string_lossy());
/// }
/// # std::io::Result::Ok(()) });
/// ```
pub async fn read_dir<P: AsRef<Path>>(path: P) -> io::Result<ReadDir> {
    let path = path.as_ref().to_owned();
    unblock(move || std::fs::read_dir(path).map(|inner| ReadDir(State::Idle(Some(inner))))).await
}

/// A stream of entries in a directory.
///
/// This stream is returned by [`read_dir()`] and yields items of type
/// [`io::Result`]`<`[`DirEntry`]`>`. Each [`DirEntry`] can then retrieve information like entry's
/// path or metadata.
pub struct ReadDir(State);

/// The state of an asynchronous `ReadDir`.
///
/// The `ReadDir` can be either idle or busy performing an asynchronous operation.
#[allow(clippy::large_enum_variant)] // TODO: Windows-specific
enum State {
    Idle(Option<std::fs::ReadDir>),
    Busy(blocking::Task<(std::fs::ReadDir, Option<io::Result<std::fs::DirEntry>>)>),
}

impl fmt::Debug for ReadDir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReadDir").finish()
    }
}

impl Stream for ReadDir {
    type Item = io::Result<DirEntry>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match &mut self.0 {
                State::Idle(opt) => {
                    let mut inner = opt.take().unwrap();

                    // Start the operation asynchronously.
                    self.0 = State::Busy(unblock(move || {
                        let next = inner.next();
                        (inner, next)
                    }));
                }
                // Poll the asynchronous operation the file is currently blocked on.
                State::Busy(task) => {
                    let (inner, opt) = ready!(task.poll(cx));
                    self.0 = State::Idle(Some(inner));
                    return Poll::Ready(opt.map(|res| res.map(|inner| DirEntry(Arc::new(inner)))));
                }
            }
        }
    }
}

/// An entry in a directory.
///
/// A stream of entries in a directory is returned by [`read_dir()`].
///
/// For Unix-specific options, import the [`DirEntryExt`][`std::os::unix::fs::DirEntryExt`] trait.
pub struct DirEntry(Arc<std::fs::DirEntry>);

impl DirEntry {
    /// Returns the full path to this entry.
    ///
    /// The full path is created by joining the original path passed to [`read_dir()`] with the
    /// name of this entry.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use futures_lite::stream::StreamExt;
    ///
    /// # futures_lite::future::block_on(async {
    /// let mut dir = async_fs::read_dir(".").await?;
    ///
    /// while let Some(entry) = dir.try_next().await? {
    ///     println!("{:?}", entry.path());
    /// }
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn path(&self) -> PathBuf {
        self.0.path()
    }

    /// Reads the metadata for this entry.
    ///
    /// This function will traverse symbolic links to read the metadata.
    ///
    /// If you want to read metadata without following symbolic links, use [`symlink_metadata()`]
    /// instead.
    ///
    /// # Errors
    ///
    /// An error will be returned in the following situations:
    ///
    /// * This entry does not point to an existing file or directory anymore.
    /// * The current process lacks permissions to read the metadata.
    /// * Some other I/O error occurred.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use futures_lite::stream::StreamExt;
    ///
    /// # futures_lite::future::block_on(async {
    /// let mut dir = async_fs::read_dir(".").await?;
    ///
    /// while let Some(entry) = dir.try_next().await? {
    ///     println!("{:?}", entry.metadata().await?);
    /// }
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn metadata(&self) -> io::Result<Metadata> {
        let inner = self.0.clone();
        unblock(move || inner.metadata()).await
    }

    /// Reads the file type for this entry.
    ///
    /// This function will not traverse symbolic links if this entry points at one.
    ///
    /// If you want to read metadata with following symbolic links, use [`metadata()`] instead.
    ///
    /// # Errors
    ///
    /// An error will be returned in the following situations:
    ///
    /// * This entry does not point to an existing file or directory anymore.
    /// * The current process lacks permissions to read this entry's metadata.
    /// * Some other I/O error occurred.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use futures_lite::stream::StreamExt;
    ///
    /// # futures_lite::future::block_on(async {
    /// let mut dir = async_fs::read_dir(".").await?;
    ///
    /// while let Some(entry) = dir.try_next().await? {
    ///     println!("{:?}", entry.file_type().await?);
    /// }
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn file_type(&self) -> io::Result<FileType> {
        let inner = self.0.clone();
        unblock(move || inner.file_type()).await
    }

    /// Returns the bare name of this entry without the leading path.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use futures_lite::stream::StreamExt;
    ///
    /// # futures_lite::future::block_on(async {
    /// let mut dir = async_fs::read_dir(".").await?;
    ///
    /// while let Some(entry) = dir.try_next().await? {
    ///     println!("{}", entry.file_name().to_string_lossy());
    /// }
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn file_name(&self) -> OsString {
        self.0.file_name()
    }
}

impl fmt::Debug for DirEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("DirEntry").field(&self.path()).finish()
    }
}

impl Clone for DirEntry {
    fn clone(&self) -> Self {
        DirEntry(self.0.clone())
    }
}

#[cfg(unix)]
impl unix::DirEntryExt for DirEntry {
    fn ino(&self) -> u64 {
        self.0.ino()
    }
}

/// Reads a symbolic link and returns the path it points to.
///
/// # Errors
///
/// An error will be returned in the following situations:
///
/// * `path` does not point to an existing link.
/// * Some other I/O error occurred.
///
/// # Examples
///
/// ```no_run
/// # futures_lite::future::block_on(async {
/// let path = async_fs::read_link("a.txt").await?;
/// # std::io::Result::Ok(()) });
/// ```
pub async fn read_link<P: AsRef<Path>>(path: P) -> io::Result<PathBuf> {
    let path = path.as_ref().to_owned();
    unblock(move || std::fs::read_link(path)).await
}

/// Reads the entire contents of a file as a string.
///
/// This is a convenience function for reading entire files. It pre-allocates a string based on the
/// file size when available, so it is typically faster than manually opening a file and reading
/// from it.
///
/// If you want to read the contents as raw bytes, use [`read()`] instead.
///
/// # Errors
///
/// An error will be returned in the following situations:
///
/// * `path` does not point to an existing file.
/// * The current process lacks permissions to read the file.
/// * The contents of the file cannot be read as a UTF-8 string.
/// * Some other I/O error occurred.
///
/// # Examples
///
/// ```no_run
/// # futures_lite::future::block_on(async {
/// let contents = async_fs::read_to_string("a.txt").await?;
/// # std::io::Result::Ok(()) });
/// ```
pub async fn read_to_string<P: AsRef<Path>>(path: P) -> io::Result<String> {
    let path = path.as_ref().to_owned();
    unblock(move || std::fs::read_to_string(path)).await
}

/// Removes an empty directory.
///
/// Note that this function can only delete an empty directory. If you want to delete a directory
/// and all of its contents, use [`remove_dir_all()`] instead.
///
/// # Errors
///
/// An error will be returned in the following situations:
///
/// * `path` is not an existing and empty directory.
/// * The current process lacks permissions to remove the directory.
/// * Some other I/O error occurred.
///
/// # Examples
///
/// ```no_run
/// # futures_lite::future::block_on(async {
/// async_fs::remove_dir("./some/directory").await?;
/// # std::io::Result::Ok(()) });
/// ```
pub async fn remove_dir<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let path = path.as_ref().to_owned();
    unblock(move || std::fs::remove_dir(path)).await
}

/// Removes a directory and all of its contents.
///
/// # Errors
///
/// An error will be returned in the following situations:
///
/// * `path` is not an existing directory.
/// * The current process lacks permissions to remove the directory.
/// * Some other I/O error occurred.
///
/// # Examples
///
/// ```no_run
/// # futures_lite::future::block_on(async {
/// async_fs::remove_dir_all("./some/directory").await?;
/// # std::io::Result::Ok(()) });
/// ```
pub async fn remove_dir_all<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let path = path.as_ref().to_owned();
    unblock(move || std::fs::remove_dir_all(path)).await
}

/// Removes a file.
///
/// # Errors
///
/// An error will be returned in the following situations:
///
/// * `path` does not point to an existing file.
/// * The current process lacks permissions to remove the file.
/// * Some other I/O error occurred.
///
/// # Examples
///
/// ```no_run
/// # futures_lite::future::block_on(async {
/// async_fs::remove_file("a.txt").await?;
/// # std::io::Result::Ok(()) });
/// ```
pub async fn remove_file<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let path = path.as_ref().to_owned();
    unblock(move || std::fs::remove_file(path)).await
}

/// Renames a file or directory to a new location.
///
/// If a file or directory already exists at the target location, it will be overwritten by this
/// operation.
///
/// # Errors
///
/// An error will be returned in the following situations:
///
/// * `src` does not point to an existing file or directory.
/// * `src` and `dst` are on different filesystems.
/// * The current process lacks permissions to do the rename operation.
/// * Some other I/O error occurred.
///
/// # Examples
///
/// ```no_run
/// # futures_lite::future::block_on(async {
/// async_fs::rename("a.txt", "b.txt").await?;
/// # std::io::Result::Ok(()) });
/// ```
pub async fn rename<P: AsRef<Path>, Q: AsRef<Path>>(src: P, dst: Q) -> io::Result<()> {
    let src = src.as_ref().to_owned();
    let dst = dst.as_ref().to_owned();
    unblock(move || std::fs::rename(&src, &dst)).await
}

/// Changes the permissions of a file or directory.
///
/// # Errors
///
/// An error will be returned in the following situations:
///
/// * `path` does not point to an existing file or directory.
/// * The current process lacks permissions to change attributes on the file or directory.
/// * Some other I/O error occurred.
///
/// # Examples
///
/// ```no_run
/// # futures_lite::future::block_on(async {
/// let mut perm = async_fs::metadata("a.txt").await?.permissions();
/// perm.set_readonly(true);
/// async_fs::set_permissions("a.txt", perm).await?;
/// # std::io::Result::Ok(()) });
/// ```
pub async fn set_permissions<P: AsRef<Path>>(path: P, perm: Permissions) -> io::Result<()> {
    let path = path.as_ref().to_owned();
    unblock(move || std::fs::set_permissions(path, perm)).await
}

/// Reads metadata for a path without following symbolic links.
///
/// If you want to follow symbolic links before reading metadata of the target file or directory,
/// use [`metadata()`] instead.
///
/// # Errors
///
/// An error will be returned in the following situations:
///
/// * `path` does not point to an existing file or directory.
/// * The current process lacks permissions to read metadata for the path.
/// * Some other I/O error occurred.
///
/// # Examples
///
/// ```no_run
/// # futures_lite::future::block_on(async {
/// let perm = async_fs::symlink_metadata("a.txt").await?.permissions();
/// # std::io::Result::Ok(()) });
/// ```
pub async fn symlink_metadata<P: AsRef<Path>>(path: P) -> io::Result<Metadata> {
    let path = path.as_ref().to_owned();
    unblock(move || std::fs::symlink_metadata(path)).await
}

/// Writes a slice of bytes as the new contents of a file.
///
/// This function will create a file if it does not exist, and will entirely replace its contents
/// if it does.
///
/// # Errors
///
/// An error will be returned in the following situations:
///
/// * The file's parent directory does not exist.
/// * The current process lacks permissions to write to the file.
/// * Some other I/O error occurred.
///
/// # Examples
///
/// ```no_run
/// # futures_lite::future::block_on(async {
/// async_fs::write("a.txt", b"Hello world!").await?;
/// # std::io::Result::Ok(()) });
/// ```
pub async fn write<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> io::Result<()> {
    let path = path.as_ref().to_owned();
    let contents = contents.as_ref().to_owned();
    unblock(move || std::fs::write(&path, contents)).await
}

/// A builder for creating directories with configurable options.
///
/// For Unix-specific options, import the [`DirBuilderExt`][`std::os::unix::fs::DirBuilderExt`]
/// trait.
#[derive(Debug, Default)]
pub struct DirBuilder {
    /// Set to `true` if non-existent parent directories should be created.
    recursive: bool,

    /// Unix mode for newly created directories.
    #[cfg(unix)]
    mode: Option<u32>,
}

impl DirBuilder {
    /// Creates a blank set of options.
    ///
    /// The [`recursive()`][`DirBuilder::recursive()`] option is initially set to `false`.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_fs::DirBuilder;
    ///
    /// let builder = DirBuilder::new();
    /// ```
    pub fn new() -> DirBuilder {
        #[cfg(not(unix))]
        let builder = DirBuilder { recursive: false };

        #[cfg(unix)]
        let builder = DirBuilder {
            recursive: false,
            mode: None,
        };

        builder
    }

    /// Sets the option for recursive mode.
    ///
    /// When set to `true`, this option means all parent directories should be created recursively
    /// if they don't exist. Parents are created with the same permissions as the final directory.
    ///
    /// This option is initially set to `false`.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_fs::DirBuilder;
    ///
    /// let mut builder = DirBuilder::new();
    /// builder.recursive(true);
    /// ```
    pub fn recursive(&mut self, recursive: bool) -> &mut Self {
        self.recursive = recursive;
        self
    }

    /// Creates a directory with the configured options.
    ///
    /// It is considered an error if the directory already exists unless recursive mode is enabled.
    ///
    /// # Errors
    ///
    /// An error will be returned in the following situations:
    ///
    /// * `path` already points to an existing file or directory.
    /// * The current process lacks permissions to create the directory or its missing parents.
    /// * Some other I/O error occurred.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_fs::DirBuilder;
    ///
    /// # futures_lite::future::block_on(async {
    /// DirBuilder::new()
    ///     .recursive(true)
    ///     .create("./some/directory")
    ///     .await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn create<P: AsRef<Path>>(&self, path: P) -> impl Future<Output = io::Result<()>> {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(self.recursive);

        #[cfg(unix)]
        {
            if let Some(mode) = self.mode {
                std::os::unix::fs::DirBuilderExt::mode(&mut builder, mode);
            }
        }

        let path = path.as_ref().to_owned();
        unblock(move || builder.create(path))
    }
}

#[cfg(unix)]
impl unix::DirBuilderExt for DirBuilder {
    fn mode(&mut self, mode: u32) -> &mut Self {
        self.mode = Some(mode);
        self
    }
}

/// An open file on the filesystem.
///
/// Depending on what options the file was opened with, this type can be used for reading and/or
/// writing.
///
/// Files are automatically closed when they get dropped and any errors detected on closing are
/// ignored. Use the [`sync_all()`][`File::sync_all()`] method before dropping a file if such
/// errors need to be handled.
///
/// **NOTE:** If writing to a file, make sure to call
/// [`flush()`][`futures_lite::io::AsyncWriteExt::flush()`], [`sync_data()`][`File::sync_data()`],
/// or [`sync_all()`][`File::sync_all()`] before dropping the file or else some written data
/// might get lost!
///
/// # Examples
///
/// Create a new file and write some bytes to it:
///
/// ```no_run
/// use async_fs::File;
/// use futures_lite::io::AsyncWriteExt;
///
/// # futures_lite::future::block_on(async {
/// let mut file = File::create("a.txt").await?;
///
/// file.write_all(b"Hello, world!").await?;
/// file.flush().await?;
/// # std::io::Result::Ok(()) });
/// ```
///
/// Read the contents of a file into a vector of bytes:
///
/// ```no_run
/// use async_fs::File;
/// use futures_lite::io::AsyncReadExt;
///
/// # futures_lite::future::block_on(async {
/// let mut file = File::open("a.txt").await?;
///
/// let mut contents = Vec::new();
/// file.read_to_end(&mut contents).await?;
/// # std::io::Result::Ok(()) });
/// ```
pub struct File {
    /// Always accessible reference to the file.
    file: Arc<std::fs::File>,

    /// Performs blocking I/O operations on a thread pool.
    unblock: Mutex<Unblock<ArcFile>>,

    /// Logical file cursor, tracked when reading from the file.
    ///
    /// This will be set to an error if the file is not seekable.
    read_pos: Option<io::Result<u64>>,

    /// Set to `true` if the file needs flushing.
    is_dirty: bool,
}

impl File {
    /// Creates an async file from a blocking file.
    fn new(inner: std::fs::File, is_dirty: bool) -> File {
        let file = Arc::new(inner);
        let unblock = Mutex::new(Unblock::new(ArcFile(file.clone())));
        let read_pos = None;
        File {
            file,
            unblock,
            read_pos,
            is_dirty,
        }
    }

    /// Opens a file in read-only mode.
    ///
    /// See the [`OpenOptions::open()`] function for more options.
    ///
    /// # Errors
    ///
    /// An error will be returned in the following situations:
    ///
    /// * `path` does not point to an existing file.
    /// * The current process lacks permissions to read the file.
    /// * Some other I/O error occurred.
    ///
    /// For more details, see the list of errors documented by [`OpenOptions::open()`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_fs::File;
    ///
    /// # futures_lite::future::block_on(async {
    /// let file = File::open("a.txt").await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn open<P: AsRef<Path>>(path: P) -> io::Result<File> {
        let path = path.as_ref().to_owned();
        let file = unblock(move || std::fs::File::open(path)).await?;
        Ok(File::new(file, false))
    }

    /// Opens a file in write-only mode.
    ///
    /// This method will create a file if it does not exist, and will truncate it if it does.
    ///
    /// See the [`OpenOptions::open`] function for more options.
    ///
    /// # Errors
    ///
    /// An error will be returned in the following situations:
    ///
    /// * The file's parent directory does not exist.
    /// * The current process lacks permissions to write to the file.
    /// * Some other I/O error occurred.
    ///
    /// For more details, see the list of errors documented by [`OpenOptions::open()`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_fs::File;
    ///
    /// # futures_lite::future::block_on(async {
    /// let file = File::create("a.txt").await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn create<P: AsRef<Path>>(path: P) -> io::Result<File> {
        let path = path.as_ref().to_owned();
        let file = unblock(move || std::fs::File::create(path)).await?;
        Ok(File::new(file, false))
    }

    /// Synchronizes OS-internal buffered contents and metadata to disk.
    ///
    /// This function will ensure that all in-memory data reaches the filesystem.
    ///
    /// This can be used to handle errors that would otherwise only be caught by closing the file.
    /// When a file is dropped, errors in synchronizing this in-memory data are ignored.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_fs::File;
    /// use futures_lite::io::AsyncWriteExt;
    ///
    /// # futures_lite::future::block_on(async {
    /// let mut file = File::create("a.txt").await?;
    ///
    /// file.write_all(b"Hello, world!").await?;
    /// file.sync_all().await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn sync_all(&self) -> io::Result<()> {
        let mut inner = self.unblock.lock().await;
        inner.flush().await?;
        let file = self.file.clone();
        unblock(move || file.sync_all()).await
    }

    /// Synchronizes OS-internal buffered contents to disk.
    ///
    /// This is similar to [`sync_all()`][`File::sync_data()`], except that file metadata may not
    /// be synchronized.
    ///
    /// This is intended for use cases that must synchronize the contents of the file, but don't
    /// need the file metadata synchronized to disk.
    ///
    /// Note that some platforms may simply implement this in terms of
    /// [`sync_all()`][`File::sync_data()`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_fs::File;
    /// use futures_lite::io::AsyncWriteExt;
    ///
    /// # futures_lite::future::block_on(async {
    /// let mut file = File::create("a.txt").await?;
    ///
    /// file.write_all(b"Hello, world!").await?;
    /// file.sync_data().await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn sync_data(&self) -> io::Result<()> {
        let mut inner = self.unblock.lock().await;
        inner.flush().await?;
        let file = self.file.clone();
        unblock(move || file.sync_data()).await
    }

    /// Truncates or extends the file.
    ///
    /// If `size` is less than the current file size, then the file will be truncated. If it is
    /// greater than the current file size, then the file will be extended to `size` and have all
    /// intermediate data filled with zeros.
    ///
    /// The file's cursor stays at the same position, even if the cursor ends up being past the end
    /// of the file after this operation.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_fs::File;
    ///
    /// # futures_lite::future::block_on(async {
    /// let mut file = File::create("a.txt").await?;
    /// file.set_len(10).await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn set_len(&self, size: u64) -> io::Result<()> {
        let mut inner = self.unblock.lock().await;
        inner.flush().await?;
        let file = self.file.clone();
        unblock(move || file.set_len(size)).await
    }

    /// Reads the file's metadata.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_fs::File;
    ///
    /// # futures_lite::future::block_on(async {
    /// let file = File::open("a.txt").await?;
    /// let metadata = file.metadata().await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn metadata(&self) -> io::Result<Metadata> {
        let file = self.file.clone();
        unblock(move || file.metadata()).await
    }

    /// Changes the permissions on the file.
    ///
    /// # Errors
    ///
    /// An error will be returned in the following situations:
    ///
    /// * The current process lacks permissions to change attributes on the file.
    /// * Some other I/O error occurred.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_fs::File;
    ///
    /// # futures_lite::future::block_on(async {
    /// let file = File::create("a.txt").await?;
    ///
    /// let mut perms = file.metadata().await?.permissions();
    /// perms.set_readonly(true);
    /// file.set_permissions(perms).await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn set_permissions(&self, perm: Permissions) -> io::Result<()> {
        let file = self.file.clone();
        unblock(move || file.set_permissions(perm)).await
    }

    /// Repositions the cursor after reading.
    ///
    /// When reading from a file, actual file reads run asynchronously in the background, which
    /// means the real file cursor is usually ahead of the logical cursor, and the data between
    /// them is buffered in memory. This kind of buffering is an important optimization.
    ///
    /// After reading ends, if we decide to perform a write or a seek operation, the real file
    /// cursor must first be repositioned back to the correct logical position.
    fn poll_reposition(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if let Some(Ok(read_pos)) = self.read_pos {
            ready!(Pin::new(self.unblock.get_mut()).poll_seek(cx, SeekFrom::Start(read_pos)))?;
        }
        self.read_pos = None;
        Poll::Ready(Ok(()))
    }
}

impl fmt::Debug for File {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.file.fmt(f)
    }
}

impl From<std::fs::File> for File {
    fn from(inner: std::fs::File) -> File {
        File::new(inner, true)
    }
}

#[cfg(unix)]
impl std::os::unix::io::AsRawFd for File {
    fn as_raw_fd(&self) -> std::os::unix::io::RawFd {
        self.file.as_raw_fd()
    }
}

#[cfg(windows)]
impl std::os::windows::io::AsRawHandle for File {
    fn as_raw_handle(&self) -> std::os::windows::io::RawHandle {
        self.file.as_raw_handle()
    }
}

#[cfg(unix)]
impl From<std::os::unix::io::OwnedFd> for File {
    fn from(fd: std::os::unix::io::OwnedFd) -> Self {
        File::from(std::fs::File::from(fd))
    }
}

#[cfg(windows)]
impl From<std::os::windows::io::OwnedHandle> for File {
    fn from(fd: std::os::windows::io::OwnedHandle) -> Self {
        File::from(std::fs::File::from(fd))
    }
}

#[cfg(unix)]
impl std::os::unix::io::AsFd for File {
    fn as_fd(&self) -> std::os::unix::io::BorrowedFd<'_> {
        self.file.as_fd()
    }
}

#[cfg(windows)]
impl std::os::windows::io::AsHandle for File {
    fn as_handle(&self) -> std::os::windows::io::BorrowedHandle<'_> {
        self.file.as_handle()
    }
}

impl AsyncRead for File {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        // Before reading begins, remember the current cursor position.
        if self.read_pos.is_none() {
            // Initialize the logical cursor to the current position in the file.
            self.read_pos = Some(ready!(self.as_mut().poll_seek(cx, SeekFrom::Current(0))));
        }

        let n = ready!(Pin::new(self.unblock.get_mut()).poll_read(cx, buf))?;

        // Update the logical cursor if the file is seekable.
        if let Some(Ok(pos)) = self.read_pos.as_mut() {
            *pos += n as u64;
        }

        Poll::Ready(Ok(n))
    }
}

impl AsyncWrite for File {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        ready!(self.poll_reposition(cx))?;
        self.is_dirty = true;
        Pin::new(self.unblock.get_mut()).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if self.is_dirty {
            ready!(Pin::new(self.unblock.get_mut()).poll_flush(cx))?;
            self.is_dirty = false;
        }
        Poll::Ready(Ok(()))
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(self.unblock.get_mut()).poll_close(cx)
    }
}

impl AsyncSeek for File {
    fn poll_seek(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        pos: SeekFrom,
    ) -> Poll<io::Result<u64>> {
        ready!(self.poll_reposition(cx))?;
        Pin::new(self.unblock.get_mut()).poll_seek(cx, pos)
    }
}

/// A wrapper around `Arc<std::fs::File>` that implements `Read`, `Write`, and `Seek`.
struct ArcFile(Arc<std::fs::File>);

impl io::Read for ArcFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (&*self.0).read(buf)
    }
}

impl io::Write for ArcFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&*self.0).write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        (&*self.0).flush()
    }
}

impl io::Seek for ArcFile {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        (&*self.0).seek(pos)
    }
}

/// A builder for opening files with configurable options.
///
/// Files can be opened in [`read`][`OpenOptions::read()`] and/or
/// [`write`][`OpenOptions::write()`] mode.
///
/// The [`append`][`OpenOptions::append()`] option opens files in a special writing mode that
/// moves the file cursor to the end of file before every write operation.
///
/// It is also possible to [`truncate`][`OpenOptions::truncate()`] the file right after opening,
/// to [`create`][`OpenOptions::create()`] a file if it doesn't exist yet, or to always create a
/// new file with [`create_new`][`OpenOptions::create_new()`].
///
/// # Examples
///
/// Open a file for reading:
///
/// ```no_run
/// use async_fs::OpenOptions;
///
/// # futures_lite::future::block_on(async {
/// let file = OpenOptions::new()
///     .read(true)
///     .open("a.txt")
///     .await?;
/// # std::io::Result::Ok(()) });
/// ```
///
/// Open a file for both reading and writing, and create it if it doesn't exist yet:
///
/// ```no_run
/// use async_fs::OpenOptions;
///
/// # futures_lite::future::block_on(async {
/// let file = OpenOptions::new()
///     .read(true)
///     .write(true)
///     .create(true)
///     .open("a.txt")
///     .await?;
/// # std::io::Result::Ok(()) });
/// ```
#[derive(Clone, Debug)]
pub struct OpenOptions(std::fs::OpenOptions);

impl OpenOptions {
    /// Creates a blank set of options.
    ///
    /// All options are initially set to `false`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_fs::OpenOptions;
    ///
    /// # futures_lite::future::block_on(async {
    /// let file = OpenOptions::new()
    ///     .read(true)
    ///     .open("a.txt")
    ///     .await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn new() -> OpenOptions {
        OpenOptions(std::fs::OpenOptions::new())
    }

    /// Configures the option for read mode.
    ///
    /// When set to `true`, this option means the file will be readable after opening.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_fs::OpenOptions;
    ///
    /// # futures_lite::future::block_on(async {
    /// let file = OpenOptions::new()
    ///     .read(true)
    ///     .open("a.txt")
    ///     .await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn read(&mut self, read: bool) -> &mut OpenOptions {
        self.0.read(read);
        self
    }

    /// Configures the option for write mode.
    ///
    /// When set to `true`, this option means the file will be writable after opening.
    ///
    /// If the file already exists, write calls on it will overwrite the previous contents without
    /// truncating it.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_fs::OpenOptions;
    ///
    /// # futures_lite::future::block_on(async {
    /// let file = OpenOptions::new()
    ///     .write(true)
    ///     .open("a.txt")
    ///     .await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn write(&mut self, write: bool) -> &mut OpenOptions {
        self.0.write(write);
        self
    }

    /// Configures the option for append mode.
    ///
    /// When set to `true`, this option means the file will be writable after opening and the file
    /// cursor will be moved to the end of file before every write operation.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_fs::OpenOptions;
    ///
    /// # futures_lite::future::block_on(async {
    /// let file = OpenOptions::new()
    ///     .append(true)
    ///     .open("a.txt")
    ///     .await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn append(&mut self, append: bool) -> &mut OpenOptions {
        self.0.append(append);
        self
    }

    /// Configures the option for truncating the previous file.
    ///
    /// When set to `true`, the file will be truncated to the length of 0 bytes.
    ///
    /// The file must be opened in [`write`][`OpenOptions::write()`] or
    /// [`append`][`OpenOptions::append()`] mode for truncation to work.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_fs::OpenOptions;
    ///
    /// # futures_lite::future::block_on(async {
    /// let file = OpenOptions::new()
    ///     .write(true)
    ///     .truncate(true)
    ///     .open("a.txt")
    ///     .await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn truncate(&mut self, truncate: bool) -> &mut OpenOptions {
        self.0.truncate(truncate);
        self
    }

    /// Configures the option for creating a new file if it doesn't exist.
    ///
    /// When set to `true`, this option means a new file will be created if it doesn't exist.
    ///
    /// The file must be opened in [`write`][`OpenOptions::write()`] or
    /// [`append`][`OpenOptions::append()`] mode for file creation to work.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_fs::OpenOptions;
    ///
    /// # futures_lite::future::block_on(async {
    /// let file = OpenOptions::new()
    ///     .write(true)
    ///     .create(true)
    ///     .open("a.txt")
    ///     .await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn create(&mut self, create: bool) -> &mut OpenOptions {
        self.0.create(create);
        self
    }

    /// Configures the option for creating a new file or failing if it already exists.
    ///
    /// When set to `true`, this option means a new file will be created, or the open operation
    /// will fail if the file already exists.
    ///
    /// The file must be opened in [`write`][`OpenOptions::write()`] or
    /// [`append`][`OpenOptions::append()`] mode for file creation to work.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_fs::OpenOptions;
    ///
    /// # futures_lite::future::block_on(async {
    /// let file = OpenOptions::new()
    ///     .write(true)
    ///     .create_new(true)
    ///     .open("a.txt")
    ///     .await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn create_new(&mut self, create_new: bool) -> &mut OpenOptions {
        self.0.create_new(create_new);
        self
    }

    /// Opens a file with the configured options.
    ///
    /// # Errors
    ///
    /// An error will be returned in the following situations:
    ///
    /// * The file does not exist and neither [`create`] nor [`create_new`] were set.
    /// * The file's parent directory does not exist.
    /// * The current process lacks permissions to open the file in the configured mode.
    /// * The file already exists and [`create_new`] was set.
    /// * Invalid combination of options was used, like [`truncate`] was set but [`write`] wasn't,
    ///   or none of [`read`], [`write`], and [`append`] modes was set.
    /// * An OS-level occurred, like too many files are open or the file name is too long.
    /// * Some other I/O error occurred.
    ///
    /// [`read`]: `OpenOptions::read()`
    /// [`write`]: `OpenOptions::write()`
    /// [`append`]: `OpenOptions::append()`
    /// [`truncate`]: `OpenOptions::truncate()`
    /// [`create`]: `OpenOptions::create()`
    /// [`create_new`]: `OpenOptions::create_new()`
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_fs::OpenOptions;
    ///
    /// # futures_lite::future::block_on(async {
    /// let file = OpenOptions::new()
    ///     .read(true)
    ///     .open("a.txt")
    ///     .await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn open<P: AsRef<Path>>(&self, path: P) -> impl Future<Output = io::Result<File>> {
        let path = path.as_ref().to_owned();
        let options = self.0.clone();
        async move {
            let file = unblock(move || options.open(path)).await?;
            Ok(File::new(file, false))
        }
    }
}

impl Default for OpenOptions {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(unix)]
impl unix::OpenOptionsExt for OpenOptions {
    fn mode(&mut self, mode: u32) -> &mut Self {
        self.0.mode(mode);
        self
    }

    fn custom_flags(&mut self, flags: i32) -> &mut Self {
        self.0.custom_flags(flags);
        self
    }
}

#[cfg(windows)]
impl windows::OpenOptionsExt for OpenOptions {
    fn access_mode(&mut self, access: u32) -> &mut Self {
        self.0.access_mode(access);
        self
    }

    fn share_mode(&mut self, val: u32) -> &mut Self {
        self.0.share_mode(val);
        self
    }

    fn custom_flags(&mut self, flags: u32) -> &mut Self {
        self.0.custom_flags(flags);
        self
    }

    fn attributes(&mut self, val: u32) -> &mut Self {
        self.0.attributes(val);
        self
    }

    fn security_qos_flags(&mut self, flags: u32) -> &mut Self {
        self.0.security_qos_flags(flags);
        self
    }
}

mod __private {
    #[doc(hidden)]
    pub trait Sealed {}

    impl Sealed for super::OpenOptions {}
    impl Sealed for super::File {}
    impl Sealed for super::DirBuilder {}
    impl Sealed for super::DirEntry {}
}

/// Unix-specific extensions.
#[cfg(unix)]
pub mod unix {
    use super::__private::Sealed;
    use super::*;

    #[doc(no_inline)]
    pub use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};

    /// Creates a new symbolic link on the filesystem.
    ///
    /// The `dst` path will be a symbolic link pointing to the `src` path.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # futures_lite::future::block_on(async {
    /// async_fs::unix::symlink("a.txt", "b.txt").await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn symlink<P: AsRef<Path>, Q: AsRef<Path>>(src: P, dst: Q) -> io::Result<()> {
        let src = src.as_ref().to_owned();
        let dst = dst.as_ref().to_owned();
        unblock(move || std::os::unix::fs::symlink(&src, &dst)).await
    }

    /// Unix-specific extensions to [`DirBuilder`].
    pub trait DirBuilderExt: Sealed {
        /// Sets the mode to create new directories with.
        ///
        /// This option defaults to `0o777`.
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use async_fs::{DirBuilder, unix::DirBuilderExt};
        ///
        /// let mut builder = DirBuilder::new();
        /// builder.mode(0o755);
        /// ```
        fn mode(&mut self, mode: u32) -> &mut Self;
    }

    /// Unix-specific extension methods for [`DirEntry`].
    pub trait DirEntryExt: Sealed {
        /// Returns the underlying `d_ino` field in the contained `dirent` structure.
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use async_fs::unix::DirEntryExt;
        /// use futures_lite::stream::StreamExt;
        ///
        /// # futures_lite::future::block_on(async {
        /// let mut entries = async_fs::read_dir(".").await?;
        ///
        /// while let Some(entry) = entries.try_next().await? {
        ///     println!("{:?}: {}", entry.file_name(), entry.ino());
        /// }
        /// # std::io::Result::Ok(()) });
        /// ```
        fn ino(&self) -> u64;
    }

    /// Unix-specific extensions to [`OpenOptions`].
    pub trait OpenOptionsExt: Sealed {
        /// Sets the mode bits that a new file will be created with.
        ///
        /// If a new file is created as part of an [`OpenOptions::open()`] call then this
        /// specified `mode` will be used as the permission bits for the new file.
        ///
        /// If no `mode` is set, the default of `0o666` will be used.
        /// The operating system masks out bits with the system's `umask`, to produce
        /// the final permissions.
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use async_fs::{OpenOptions, unix::OpenOptionsExt};
        ///
        /// # futures_lite::future::block_on(async {
        /// let mut options = OpenOptions::new();
        /// // Read/write permissions for owner and read permissions for others.
        /// options.mode(0o644);
        /// let file = options.open("foo.txt").await?;
        /// # std::io::Result::Ok(()) });
        /// ```
        fn mode(&mut self, mode: u32) -> &mut Self;

        /// Passes custom flags to the `flags` argument of `open`.
        ///
        /// The bits that define the access mode are masked out with `O_ACCMODE`, to
        /// ensure they do not interfere with the access mode set by Rust's options.
        ///
        /// Custom flags can only set flags, not remove flags set by Rust's options.
        /// This options overwrites any previously set custom flags.
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use async_fs::{OpenOptions, unix::OpenOptionsExt};
        ///
        /// # futures_lite::future::block_on(async {
        /// let mut options = OpenOptions::new();
        /// options.write(true);
        /// options.custom_flags(libc::O_NOFOLLOW);
        /// let file = options.open("foo.txt").await?;
        /// # std::io::Result::Ok(()) });
        /// ```
        fn custom_flags(&mut self, flags: i32) -> &mut Self;
    }
}

/// Windows-specific extensions.
#[cfg(windows)]
pub mod windows {
    use super::__private::Sealed;
    use super::*;

    #[doc(no_inline)]
    pub use std::os::windows::fs::MetadataExt;

    /// Creates a new directory symbolic link on the filesystem.
    ///
    /// The `dst` path will be a directory symbolic link pointing to the `src` path.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # futures_lite::future::block_on(async {
    /// async_fs::windows::symlink_dir("a", "b").await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn symlink_dir<P: AsRef<Path>, Q: AsRef<Path>>(src: P, dst: Q) -> io::Result<()> {
        let src = src.as_ref().to_owned();
        let dst = dst.as_ref().to_owned();
        unblock(move || std::os::windows::fs::symlink_dir(&src, &dst)).await
    }

    /// Creates a new file symbolic link on the filesystem.
    ///
    /// The `dst` path will be a file symbolic link pointing to the `src` path.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # futures_lite::future::block_on(async {
    /// async_fs::windows::symlink_file("a.txt", "b.txt").await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn symlink_file<P: AsRef<Path>, Q: AsRef<Path>>(src: P, dst: Q) -> io::Result<()> {
        let src = src.as_ref().to_owned();
        let dst = dst.as_ref().to_owned();
        unblock(move || std::os::windows::fs::symlink_file(&src, &dst)).await
    }

    /// Windows-specific extensions to [`OpenOptions`].
    pub trait OpenOptionsExt: Sealed {
        /// Overrides the `dwDesiredAccess` argument to the call to [`CreateFile`]
        /// with the specified value.
        ///
        /// This will override the `read`, `write`, and `append` flags on the
        /// [`OpenOptions`] structure. This method provides fine-grained control over
        /// the permissions to read, write and append data, attributes (like hidden
        /// and system), and extended attributes.
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use async_fs::{OpenOptions, windows::OpenOptionsExt};
        ///
        /// # futures_lite::future::block_on(async {
        /// // Open without read and write permission, for example if you only need
        /// // to call `stat` on the file
        /// let file = OpenOptions::new().access_mode(0).open("foo.txt").await?;
        /// # std::io::Result::Ok(()) });
        /// ```
        ///
        /// [`CreateFile`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilea
        fn access_mode(&mut self, access: u32) -> &mut Self;

        /// Overrides the `dwShareMode` argument to the call to [`CreateFile`] with
        /// the specified value.
        ///
        /// By default `share_mode` is set to
        /// `FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE`. This allows
        /// other processes to read, write, and delete/rename the same file
        /// while it is open. Removing any of the flags will prevent other
        /// processes from performing the corresponding operation until the file
        /// handle is closed.
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use async_fs::{OpenOptions, windows::OpenOptionsExt};
        ///
        /// # futures_lite::future::block_on(async {
        /// // Do not allow others to read or modify this file while we have it open
        /// // for writing.
        /// let file = OpenOptions::new()
        ///     .write(true)
        ///     .share_mode(0)
        ///     .open("foo.txt")
        ///     .await?;
        /// # std::io::Result::Ok(()) });
        /// ```
        ///
        /// [`CreateFile`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilea
        fn share_mode(&mut self, val: u32) -> &mut Self;

        /// Sets extra flags for the `dwFileFlags` argument to the call to
        /// [`CreateFile2`] to the specified value (or combines it with
        /// `attributes` and `security_qos_flags` to set the `dwFlagsAndAttributes`
        /// for [`CreateFile`]).
        ///
        /// Custom flags can only set flags, not remove flags set by Rust's options.
        /// This option overwrites any previously set custom flags.
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use async_fs::{OpenOptions, windows::OpenOptionsExt};
        ///
        /// # futures_lite::future::block_on(async {
        /// let file = OpenOptions::new()
        ///     .create(true)
        ///     .write(true)
        ///     .custom_flags(windows_sys::Win32::Storage::FileSystem::FILE_FLAG_DELETE_ON_CLOSE)
        ///     .open("foo.txt")
        ///     .await?;
        /// # std::io::Result::Ok(()) });
        /// ```
        ///
        /// [`CreateFile`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilea
        /// [`CreateFile2`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfile2
        fn custom_flags(&mut self, flags: u32) -> &mut Self;

        /// Sets the `dwFileAttributes` argument to the call to [`CreateFile2`] to
        /// the specified value (or combines it with `custom_flags` and
        /// `security_qos_flags` to set the `dwFlagsAndAttributes` for
        /// [`CreateFile`]).
        ///
        /// If a _new_ file is created because it does not yet exist and
        /// `.create(true)` or `.create_new(true)` are specified, the new file is
        /// given the attributes declared with `.attributes()`.
        ///
        /// If an _existing_ file is opened with `.create(true).truncate(true)`, its
        /// existing attributes are preserved and combined with the ones declared
        /// with `.attributes()`.
        ///
        /// In all other cases the attributes get ignored.
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use async_fs::{OpenOptions, windows::OpenOptionsExt};
        ///
        /// # futures_lite::future::block_on(async {
        /// let file = OpenOptions::new()
        ///     .write(true)
        ///     .create(true)
        ///     .attributes(windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_HIDDEN)
        ///     .open("foo.txt")
        ///     .await?;
        /// # std::io::Result::Ok(()) });
        /// ```
        ///
        /// [`CreateFile`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilea
        /// [`CreateFile2`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfile2
        fn attributes(&mut self, val: u32) -> &mut Self;

        /// Sets the `dwSecurityQosFlags` argument to the call to [`CreateFile2`] to
        /// the specified value (or combines it with `custom_flags` and `attributes`
        /// to set the `dwFlagsAndAttributes` for [`CreateFile`]).
        ///
        /// By default `security_qos_flags` is not set. It should be specified when
        /// opening a named pipe, to control to which degree a server process can
        /// act on behalf of a client process (security impersonation level).
        ///
        /// When `security_qos_flags` is not set, a malicious program can gain the
        /// elevated privileges of a privileged Rust process when it allows opening
        /// user-specified paths, by tricking it into opening a named pipe. So
        /// arguably `security_qos_flags` should also be set when opening arbitrary
        /// paths. However the bits can then conflict with other flags, specifically
        /// `FILE_FLAG_OPEN_NO_RECALL`.
        ///
        /// For information about possible values, see [Impersonation Levels] on the
        /// Windows Dev Center site. The `SECURITY_SQOS_PRESENT` flag is set
        /// automatically when using this method.
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use async_fs::{OpenOptions, windows::OpenOptionsExt};
        ///
        /// # futures_lite::future::block_on(async {
        /// let file = OpenOptions::new()
        ///     .write(true)
        ///     .create(true)
        ///     .security_qos_flags(windows_sys::Win32::Storage::FileSystem::SECURITY_IDENTIFICATION)
        ///     .open(r"\\.\pipe\MyPipe")
        ///     .await?;
        /// # std::io::Result::Ok(()) });
        /// ```
        ///
        /// [`CreateFile`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilea
        /// [`CreateFile2`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfile2
        /// [Impersonation Levels]: https://docs.microsoft.com/en-us/windows/win32/api/winnt/ne-winnt-security_impersonation_level
        fn security_qos_flags(&mut self, flags: u32) -> &mut Self;
    }
}
