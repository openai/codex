use crate::fs::asyncify;

use std::collections::VecDeque;
use std::ffi::OsString;
use std::fs::{FileType, Metadata};
use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{ready, Context, Poll};

#[cfg(test)]
use super::mocks::spawn_blocking;
#[cfg(test)]
use super::mocks::JoinHandle;
#[cfg(not(test))]
use crate::blocking::spawn_blocking;
#[cfg(not(test))]
use crate::blocking::JoinHandle;

const CHUNK_SIZE: usize = 32;

/// Returns a stream over the entries within a directory.
///
/// This is an async version of [`std::fs::read_dir`].
///
/// This operation is implemented by running the equivalent blocking
/// operation on a separate thread pool using [`spawn_blocking`].
///
/// [`spawn_blocking`]: crate::task::spawn_blocking
pub async fn read_dir(path: impl AsRef<Path>) -> io::Result<ReadDir> {
    let path = path.as_ref().to_owned();
    asyncify(|| -> io::Result<ReadDir> {
        let mut std = std::fs::read_dir(path)?;
        let mut buf = VecDeque::with_capacity(CHUNK_SIZE);
        let remain = ReadDir::next_chunk(&mut buf, &mut std);

        Ok(ReadDir(State::Idle(Some((buf, std, remain)))))
    })
    .await
}

/// Reads the entries in a directory.
///
/// This struct is returned from the [`read_dir`] function of this module and
/// will yield instances of [`DirEntry`]. Through a [`DirEntry`] information
/// like the entry's path and possibly other metadata can be learned.
///
/// A `ReadDir` can be turned into a `Stream` with [`ReadDirStream`].
///
/// [`ReadDirStream`]: https://docs.rs/tokio-stream/0.1/tokio_stream/wrappers/struct.ReadDirStream.html
///
/// # Errors
///
/// This stream will return an [`Err`] if there's some sort of intermittent
/// IO error during iteration.
///
/// [`read_dir`]: read_dir
/// [`DirEntry`]: DirEntry
/// [`Err`]: std::result::Result::Err
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct ReadDir(State);

#[derive(Debug)]
enum State {
    Idle(Option<(VecDeque<io::Result<DirEntry>>, std::fs::ReadDir, bool)>),
    Pending(JoinHandle<(VecDeque<io::Result<DirEntry>>, std::fs::ReadDir, bool)>),
}

impl ReadDir {
    /// Returns the next entry in the directory stream.
    ///
    /// # Cancel safety
    ///
    /// This method is cancellation safe.
    pub async fn next_entry(&mut self) -> io::Result<Option<DirEntry>> {
        use std::future::poll_fn;
        poll_fn(|cx| self.poll_next_entry(cx)).await
    }

    /// Polls for the next directory entry in the stream.
    ///
    /// This method returns:
    ///
    ///  * `Poll::Pending` if the next directory entry is not yet available.
    ///  * `Poll::Ready(Ok(Some(entry)))` if the next directory entry is available.
    ///  * `Poll::Ready(Ok(None))` if there are no more directory entries in this
    ///    stream.
    ///  * `Poll::Ready(Err(err))` if an IO error occurred while reading the next
    ///    directory entry.
    ///
    /// When the method returns `Poll::Pending`, the `Waker` in the provided
    /// `Context` is scheduled to receive a wakeup when the next directory entry
    /// becomes available on the underlying IO resource.
    ///
    /// Note that on multiple calls to `poll_next_entry`, only the `Waker` from
    /// the `Context` passed to the most recent call is scheduled to receive a
    /// wakeup.
    pub fn poll_next_entry(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<Option<DirEntry>>> {
        loop {
            match self.0 {
                State::Idle(ref mut data) => {
                    let (buf, _, ref remain) = data.as_mut().unwrap();

                    if let Some(ent) = buf.pop_front() {
                        return Poll::Ready(ent.map(Some));
                    } else if !remain {
                        return Poll::Ready(Ok(None));
                    }

                    let (mut buf, mut std, _) = data.take().unwrap();

                    self.0 = State::Pending(spawn_blocking(move || {
                        let remain = ReadDir::next_chunk(&mut buf, &mut std);
                        (buf, std, remain)
                    }));
                }
                State::Pending(ref mut rx) => {
                    self.0 = State::Idle(Some(ready!(Pin::new(rx).poll(cx))?));
                }
            }
        }
    }

    fn next_chunk(buf: &mut VecDeque<io::Result<DirEntry>>, std: &mut std::fs::ReadDir) -> bool {
        for _ in 0..CHUNK_SIZE {
            let ret = match std.next() {
                Some(ret) => ret,
                None => return false,
            };

            let success = ret.is_ok();

            buf.push_back(ret.map(|std| DirEntry {
                #[cfg(not(any(
                    target_os = "solaris",
                    target_os = "illumos",
                    target_os = "haiku",
                    target_os = "vxworks",
                    target_os = "aix",
                    target_os = "nto",
                    target_os = "vita",
                )))]
                file_type: std.file_type().ok(),
                std: Arc::new(std),
            }));

            if !success {
                break;
            }
        }

        true
    }
}

feature! {
    #![unix]

    use std::os::unix::fs::DirEntryExt;

    impl DirEntry {
        /// Returns the underlying `d_ino` field in the contained `dirent`
        /// structure.
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::fs;
        ///
        /// # #[tokio::main]
        /// # async fn main() -> std::io::Result<()> {
        /// let mut entries = fs::read_dir(".").await?;
        /// while let Some(entry) = entries.next_entry().await? {
        ///     // Here, `entry` is a `DirEntry`.
        ///     println!("{:?}: {}", entry.file_name(), entry.ino());
        /// }
        /// # Ok(())
        /// # }
        /// ```
        pub fn ino(&self) -> u64 {
            self.as_inner().ino()
        }
    }
}

/// Entries returned by the [`ReadDir`] stream.
///
/// [`ReadDir`]: struct@ReadDir
///
/// This is a specialized version of [`std::fs::DirEntry`] for usage from the
/// Tokio runtime.
///
/// An instance of `DirEntry` represents an entry inside of a directory on the
/// filesystem. Each entry can be inspected via methods to learn about the full
/// path or possibly other metadata through per-platform extension traits.
#[derive(Debug)]
pub struct DirEntry {
    #[cfg(not(any(
        target_os = "solaris",
        target_os = "illumos",
        target_os = "haiku",
        target_os = "vxworks",
        target_os = "aix",
        target_os = "nto",
        target_os = "vita",
    )))]
    file_type: Option<FileType>,
    std: Arc<std::fs::DirEntry>,
}

impl DirEntry {
    /// Returns the full path to the file that this entry represents.
    ///
    /// The full path is created by joining the original path to `read_dir`
    /// with the filename of this entry.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::fs;
    ///
    /// # async fn dox() -> std::io::Result<()> {
    /// let mut entries = fs::read_dir(".").await?;
    ///
    /// while let Some(entry) = entries.next_entry().await? {
    ///     println!("{:?}", entry.path());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// This prints output like:
    ///
    /// ```text
    /// "./whatever.txt"
    /// "./foo.html"
    /// "./hello_world.rs"
    /// ```
    ///
    /// The exact text, of course, depends on what files you have in `.`.
    pub fn path(&self) -> PathBuf {
        self.std.path()
    }

    /// Returns the bare file name of this directory entry without any other
    /// leading path component.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::fs;
    ///
    /// # async fn dox() -> std::io::Result<()> {
    /// let mut entries = fs::read_dir(".").await?;
    ///
    /// while let Some(entry) = entries.next_entry().await? {
    ///     println!("{:?}", entry.file_name());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn file_name(&self) -> OsString {
        self.std.file_name()
    }

    /// Returns the metadata for the file that this entry points at.
    ///
    /// This function will not traverse symlinks if this entry points at a
    /// symlink.
    ///
    /// # Platform-specific behavior
    ///
    /// On Windows this function is cheap to call (no extra system calls
    /// needed), but on Unix platforms this function is the equivalent of
    /// calling `symlink_metadata` on the path.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::fs;
    ///
    /// # async fn dox() -> std::io::Result<()> {
    /// let mut entries = fs::read_dir(".").await?;
    ///
    /// while let Some(entry) = entries.next_entry().await? {
    ///     if let Ok(metadata) = entry.metadata().await {
    ///         // Now let's show our entry's permissions!
    ///         println!("{:?}: {:?}", entry.path(), metadata.permissions());
    ///     } else {
    ///         println!("Couldn't get file type for {:?}", entry.path());
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn metadata(&self) -> io::Result<Metadata> {
        let std = self.std.clone();
        asyncify(move || std.metadata()).await
    }

    /// Returns the file type for the file that this entry points at.
    ///
    /// This function will not traverse symlinks if this entry points at a
    /// symlink.
    ///
    /// # Platform-specific behavior
    ///
    /// On Windows and most Unix platforms this function is free (no extra
    /// system calls needed), but some Unix platforms may require the equivalent
    /// call to `symlink_metadata` to learn about the target file type.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::fs;
    ///
    /// # async fn dox() -> std::io::Result<()> {
    /// let mut entries = fs::read_dir(".").await?;
    ///
    /// while let Some(entry) = entries.next_entry().await? {
    ///     if let Ok(file_type) = entry.file_type().await {
    ///         // Now let's show our entry's file type!
    ///         println!("{:?}: {:?}", entry.path(), file_type);
    ///     } else {
    ///         println!("Couldn't get file type for {:?}", entry.path());
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn file_type(&self) -> io::Result<FileType> {
        #[cfg(not(any(
            target_os = "solaris",
            target_os = "illumos",
            target_os = "haiku",
            target_os = "vxworks",
            target_os = "aix",
            target_os = "nto",
            target_os = "vita",
        )))]
        if let Some(file_type) = self.file_type {
            return Ok(file_type);
        }

        let std = self.std.clone();
        asyncify(move || std.file_type()).await
    }

    /// Returns a reference to the underlying `std::fs::DirEntry`.
    #[cfg(unix)]
    pub(super) fn as_inner(&self) -> &std::fs::DirEntry {
        &self.std
    }
}
