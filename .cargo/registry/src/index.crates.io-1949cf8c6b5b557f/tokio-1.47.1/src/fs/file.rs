//! Types for working with [`File`].
//!
//! [`File`]: File

use crate::fs::{asyncify, OpenOptions};
use crate::io::blocking::{Buf, DEFAULT_MAX_BUF_SIZE};
use crate::io::{AsyncRead, AsyncSeek, AsyncWrite, ReadBuf};
use crate::sync::Mutex;

use std::cmp;
use std::fmt;
use std::fs::{Metadata, Permissions};
use std::future::Future;
use std::io::{self, Seek, SeekFrom};
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{ready, Context, Poll};

#[cfg(test)]
use super::mocks::JoinHandle;
#[cfg(test)]
use super::mocks::MockFile as StdFile;
#[cfg(test)]
use super::mocks::{spawn_blocking, spawn_mandatory_blocking};
#[cfg(not(test))]
use crate::blocking::JoinHandle;
#[cfg(not(test))]
use crate::blocking::{spawn_blocking, spawn_mandatory_blocking};
#[cfg(not(test))]
use std::fs::File as StdFile;

/// A reference to an open file on the filesystem.
///
/// This is a specialized version of [`std::fs::File`] for usage from the
/// Tokio runtime.
///
/// An instance of a `File` can be read and/or written depending on what options
/// it was opened with. Files also implement [`AsyncSeek`] to alter the logical
/// cursor that the file contains internally.
///
/// A file will not be closed immediately when it goes out of scope if there
/// are any IO operations that have not yet completed. To ensure that a file is
/// closed immediately when it is dropped, you should call [`flush`] before
/// dropping it. Note that this does not ensure that the file has been fully
/// written to disk; the operating system might keep the changes around in an
/// in-memory buffer. See the [`sync_all`] method for telling the OS to write
/// the data to disk.
///
/// Reading and writing to a `File` is usually done using the convenience
/// methods found on the [`AsyncReadExt`] and [`AsyncWriteExt`] traits.
///
/// [`AsyncSeek`]: trait@crate::io::AsyncSeek
/// [`flush`]: fn@crate::io::AsyncWriteExt::flush
/// [`sync_all`]: fn@crate::fs::File::sync_all
/// [`AsyncReadExt`]: trait@crate::io::AsyncReadExt
/// [`AsyncWriteExt`]: trait@crate::io::AsyncWriteExt
///
/// # Examples
///
/// Create a new file and asynchronously write bytes to it:
///
/// ```no_run
/// use tokio::fs::File;
/// use tokio::io::AsyncWriteExt; // for write_all()
///
/// # async fn dox() -> std::io::Result<()> {
/// let mut file = File::create("foo.txt").await?;
/// file.write_all(b"hello, world!").await?;
/// # Ok(())
/// # }
/// ```
///
/// Read the contents of a file into a buffer:
///
/// ```no_run
/// use tokio::fs::File;
/// use tokio::io::AsyncReadExt; // for read_to_end()
///
/// # async fn dox() -> std::io::Result<()> {
/// let mut file = File::open("foo.txt").await?;
///
/// let mut contents = vec![];
/// file.read_to_end(&mut contents).await?;
///
/// println!("len = {}", contents.len());
/// # Ok(())
/// # }
/// ```
pub struct File {
    std: Arc<StdFile>,
    inner: Mutex<Inner>,
    max_buf_size: usize,
}

struct Inner {
    state: State,

    /// Errors from writes/flushes are returned in write/flush calls. If a write
    /// error is observed while performing a read, it is saved until the next
    /// write / flush call.
    last_write_err: Option<io::ErrorKind>,

    pos: u64,
}

#[derive(Debug)]
enum State {
    Idle(Option<Buf>),
    Busy(JoinHandle<(Operation, Buf)>),
}

#[derive(Debug)]
enum Operation {
    Read(io::Result<usize>),
    Write(io::Result<()>),
    Seek(io::Result<u64>),
}

impl File {
    /// Attempts to open a file in read-only mode.
    ///
    /// See [`OpenOptions`] for more details.
    ///
    /// # Errors
    ///
    /// This function will return an error if called from outside of the Tokio
    /// runtime or if path does not already exist. Other errors may also be
    /// returned according to `OpenOptions::open`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::fs::File;
    /// use tokio::io::AsyncReadExt;
    ///
    /// # async fn dox() -> std::io::Result<()> {
    /// let mut file = File::open("foo.txt").await?;
    ///
    /// let mut contents = vec![];
    /// file.read_to_end(&mut contents).await?;
    ///
    /// println!("len = {}", contents.len());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// The [`read_to_end`] method is defined on the [`AsyncReadExt`] trait.
    ///
    /// [`read_to_end`]: fn@crate::io::AsyncReadExt::read_to_end
    /// [`AsyncReadExt`]: trait@crate::io::AsyncReadExt
    pub async fn open(path: impl AsRef<Path>) -> io::Result<File> {
        let path = path.as_ref().to_owned();
        let std = asyncify(|| StdFile::open(path)).await?;

        Ok(File::from_std(std))
    }

    /// Opens a file in write-only mode.
    ///
    /// This function will create a file if it does not exist, and will truncate
    /// it if it does.
    ///
    /// See [`OpenOptions`] for more details.
    ///
    /// # Errors
    ///
    /// Results in an error if called from outside of the Tokio runtime or if
    /// the underlying [`create`] call results in an error.
    ///
    /// [`create`]: std::fs::File::create
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::fs::File;
    /// use tokio::io::AsyncWriteExt;
    ///
    /// # async fn dox() -> std::io::Result<()> {
    /// let mut file = File::create("foo.txt").await?;
    /// file.write_all(b"hello, world!").await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// The [`write_all`] method is defined on the [`AsyncWriteExt`] trait.
    ///
    /// [`write_all`]: fn@crate::io::AsyncWriteExt::write_all
    /// [`AsyncWriteExt`]: trait@crate::io::AsyncWriteExt
    pub async fn create(path: impl AsRef<Path>) -> io::Result<File> {
        let path = path.as_ref().to_owned();
        let std_file = asyncify(move || StdFile::create(path)).await?;
        Ok(File::from_std(std_file))
    }

    /// Opens a file in read-write mode.
    ///
    /// This function will create a file if it does not exist, or return an error
    /// if it does. This way, if the call succeeds, the file returned is guaranteed
    /// to be new.
    ///
    /// This option is useful because it is atomic. Otherwise between checking
    /// whether a file exists and creating a new one, the file may have been
    /// created by another process (a TOCTOU race condition / attack).
    ///
    /// This can also be written using `File::options().read(true).write(true).create_new(true).open(...)`.
    ///
    /// See [`OpenOptions`] for more details.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::fs::File;
    /// use tokio::io::AsyncWriteExt;
    ///
    /// # async fn dox() -> std::io::Result<()> {
    /// let mut file = File::create_new("foo.txt").await?;
    /// file.write_all(b"hello, world!").await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// The [`write_all`] method is defined on the [`AsyncWriteExt`] trait.
    ///
    /// [`write_all`]: fn@crate::io::AsyncWriteExt::write_all
    /// [`AsyncWriteExt`]: trait@crate::io::AsyncWriteExt
    pub async fn create_new<P: AsRef<Path>>(path: P) -> std::io::Result<File> {
        Self::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)
            .await
    }

    /// Returns a new [`OpenOptions`] object.
    ///
    /// This function returns a new `OpenOptions` object that you can use to
    /// open or create a file with specific options if `open()` or `create()`
    /// are not appropriate.
    ///
    /// It is equivalent to `OpenOptions::new()`, but allows you to write more
    /// readable code. Instead of
    /// `OpenOptions::new().append(true).open("example.log")`,
    /// you can write `File::options().append(true).open("example.log")`. This
    /// also avoids the need to import `OpenOptions`.
    ///
    /// See the [`OpenOptions::new`] function for more details.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::fs::File;
    /// use tokio::io::AsyncWriteExt;
    ///
    /// # async fn dox() -> std::io::Result<()> {
    /// let mut f = File::options().append(true).open("example.log").await?;
    /// f.write_all(b"new line\n").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn options() -> OpenOptions {
        OpenOptions::new()
    }

    /// Converts a [`std::fs::File`] to a [`tokio::fs::File`](File).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // This line could block. It is not recommended to do this on the Tokio
    /// // runtime.
    /// let std_file = std::fs::File::open("foo.txt").unwrap();
    /// let file = tokio::fs::File::from_std(std_file);
    /// ```
    pub fn from_std(std: StdFile) -> File {
        File {
            std: Arc::new(std),
            inner: Mutex::new(Inner {
                state: State::Idle(Some(Buf::with_capacity(0))),
                last_write_err: None,
                pos: 0,
            }),
            max_buf_size: DEFAULT_MAX_BUF_SIZE,
        }
    }

    /// Attempts to sync all OS-internal metadata to disk.
    ///
    /// This function will attempt to ensure that all in-core data reaches the
    /// filesystem before returning.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::fs::File;
    /// use tokio::io::AsyncWriteExt;
    ///
    /// # async fn dox() -> std::io::Result<()> {
    /// let mut file = File::create("foo.txt").await?;
    /// file.write_all(b"hello, world!").await?;
    /// file.sync_all().await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// The [`write_all`] method is defined on the [`AsyncWriteExt`] trait.
    ///
    /// [`write_all`]: fn@crate::io::AsyncWriteExt::write_all
    /// [`AsyncWriteExt`]: trait@crate::io::AsyncWriteExt
    pub async fn sync_all(&self) -> io::Result<()> {
        let mut inner = self.inner.lock().await;
        inner.complete_inflight().await;

        let std = self.std.clone();
        asyncify(move || std.sync_all()).await
    }

    /// This function is similar to `sync_all`, except that it may not
    /// synchronize file metadata to the filesystem.
    ///
    /// This is intended for use cases that must synchronize content, but don't
    /// need the metadata on disk. The goal of this method is to reduce disk
    /// operations.
    ///
    /// Note that some platforms may simply implement this in terms of `sync_all`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::fs::File;
    /// use tokio::io::AsyncWriteExt;
    ///
    /// # async fn dox() -> std::io::Result<()> {
    /// let mut file = File::create("foo.txt").await?;
    /// file.write_all(b"hello, world!").await?;
    /// file.sync_data().await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// The [`write_all`] method is defined on the [`AsyncWriteExt`] trait.
    ///
    /// [`write_all`]: fn@crate::io::AsyncWriteExt::write_all
    /// [`AsyncWriteExt`]: trait@crate::io::AsyncWriteExt
    pub async fn sync_data(&self) -> io::Result<()> {
        let mut inner = self.inner.lock().await;
        inner.complete_inflight().await;

        let std = self.std.clone();
        asyncify(move || std.sync_data()).await
    }

    /// Truncates or extends the underlying file, updating the size of this file to become size.
    ///
    /// If the size is less than the current file's size, then the file will be
    /// shrunk. If it is greater than the current file's size, then the file
    /// will be extended to size and have all of the intermediate data filled in
    /// with 0s.
    ///
    /// # Errors
    ///
    /// This function will return an error if the file is not opened for
    /// writing.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::fs::File;
    /// use tokio::io::AsyncWriteExt;
    ///
    /// # async fn dox() -> std::io::Result<()> {
    /// let mut file = File::create("foo.txt").await?;
    /// file.write_all(b"hello, world!").await?;
    /// file.set_len(10).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// The [`write_all`] method is defined on the [`AsyncWriteExt`] trait.
    ///
    /// [`write_all`]: fn@crate::io::AsyncWriteExt::write_all
    /// [`AsyncWriteExt`]: trait@crate::io::AsyncWriteExt
    pub async fn set_len(&self, size: u64) -> io::Result<()> {
        let mut inner = self.inner.lock().await;
        inner.complete_inflight().await;

        let mut buf = match inner.state {
            State::Idle(ref mut buf_cell) => buf_cell.take().unwrap(),
            _ => unreachable!(),
        };

        let seek = if !buf.is_empty() {
            Some(SeekFrom::Current(buf.discard_read()))
        } else {
            None
        };

        let std = self.std.clone();

        inner.state = State::Busy(spawn_blocking(move || {
            let res = if let Some(seek) = seek {
                (&*std).seek(seek).and_then(|_| std.set_len(size))
            } else {
                std.set_len(size)
            }
            .map(|()| 0); // the value is discarded later

            // Return the result as a seek
            (Operation::Seek(res), buf)
        }));

        let (op, buf) = match inner.state {
            State::Idle(_) => unreachable!(),
            State::Busy(ref mut rx) => rx.await?,
        };

        inner.state = State::Idle(Some(buf));

        match op {
            Operation::Seek(res) => res.map(|pos| {
                inner.pos = pos;
            }),
            _ => unreachable!(),
        }
    }

    /// Queries metadata about the underlying file.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::fs::File;
    ///
    /// # async fn dox() -> std::io::Result<()> {
    /// let file = File::open("foo.txt").await?;
    /// let metadata = file.metadata().await?;
    ///
    /// println!("{:?}", metadata);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn metadata(&self) -> io::Result<Metadata> {
        let std = self.std.clone();
        asyncify(move || std.metadata()).await
    }

    /// Creates a new `File` instance that shares the same underlying file handle
    /// as the existing `File` instance. Reads, writes, and seeks will affect both
    /// File instances simultaneously.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::fs::File;
    ///
    /// # async fn dox() -> std::io::Result<()> {
    /// let file = File::open("foo.txt").await?;
    /// let file_clone = file.try_clone().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn try_clone(&self) -> io::Result<File> {
        self.inner.lock().await.complete_inflight().await;
        let std = self.std.clone();
        let std_file = asyncify(move || std.try_clone()).await?;
        Ok(File::from_std(std_file))
    }

    /// Destructures `File` into a [`std::fs::File`]. This function is
    /// async to allow any in-flight operations to complete.
    ///
    /// Use `File::try_into_std` to attempt conversion immediately.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::fs::File;
    ///
    /// # async fn dox() -> std::io::Result<()> {
    /// let tokio_file = File::open("foo.txt").await?;
    /// let std_file = tokio_file.into_std().await;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn into_std(mut self) -> StdFile {
        self.inner.get_mut().complete_inflight().await;
        Arc::try_unwrap(self.std).expect("Arc::try_unwrap failed")
    }

    /// Tries to immediately destructure `File` into a [`std::fs::File`].
    ///
    /// # Errors
    ///
    /// This function will return an error containing the file if some
    /// operation is in-flight.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::fs::File;
    ///
    /// # async fn dox() -> std::io::Result<()> {
    /// let tokio_file = File::open("foo.txt").await?;
    /// let std_file = tokio_file.try_into_std().unwrap();
    /// # Ok(())
    /// # }
    /// ```
    pub fn try_into_std(mut self) -> Result<StdFile, Self> {
        match Arc::try_unwrap(self.std) {
            Ok(file) => Ok(file),
            Err(std_file_arc) => {
                self.std = std_file_arc;
                Err(self)
            }
        }
    }

    /// Changes the permissions on the underlying file.
    ///
    /// # Platform-specific behavior
    ///
    /// This function currently corresponds to the `fchmod` function on Unix and
    /// the `SetFileInformationByHandle` function on Windows. Note that, this
    /// [may change in the future][changes].
    ///
    /// [changes]: https://doc.rust-lang.org/std/io/index.html#platform-specific-behavior
    ///
    /// # Errors
    ///
    /// This function will return an error if the user lacks permission change
    /// attributes on the underlying file. It may also return an error in other
    /// os-specific unspecified cases.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::fs::File;
    ///
    /// # async fn dox() -> std::io::Result<()> {
    /// let file = File::open("foo.txt").await?;
    /// let mut perms = file.metadata().await?.permissions();
    /// perms.set_readonly(true);
    /// file.set_permissions(perms).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn set_permissions(&self, perm: Permissions) -> io::Result<()> {
        let std = self.std.clone();
        asyncify(move || std.set_permissions(perm)).await
    }

    /// Set the maximum buffer size for the underlying [`AsyncRead`] / [`AsyncWrite`] operation.
    ///
    /// Although Tokio uses a sensible default value for this buffer size, this function would be
    /// useful for changing that default depending on the situation.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::fs::File;
    /// use tokio::io::AsyncWriteExt;
    ///
    /// # async fn dox() -> std::io::Result<()> {
    /// let mut file = File::open("foo.txt").await?;
    ///
    /// // Set maximum buffer size to 8 MiB
    /// file.set_max_buf_size(8 * 1024 * 1024);
    ///
    /// let mut buf = vec![1; 1024 * 1024 * 1024];
    ///
    /// // Write the 1 GiB buffer in chunks up to 8 MiB each.
    /// file.write_all(&mut buf).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn set_max_buf_size(&mut self, max_buf_size: usize) {
        self.max_buf_size = max_buf_size;
    }
}

impl AsyncRead for File {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        dst: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        ready!(crate::trace::trace_leaf(cx));

        let me = self.get_mut();
        let inner = me.inner.get_mut();

        loop {
            match inner.state {
                State::Idle(ref mut buf_cell) => {
                    let mut buf = buf_cell.take().unwrap();

                    if !buf.is_empty() || dst.remaining() == 0 {
                        buf.copy_to(dst);
                        *buf_cell = Some(buf);
                        return Poll::Ready(Ok(()));
                    }

                    let std = me.std.clone();

                    let max_buf_size = cmp::min(dst.remaining(), me.max_buf_size);
                    inner.state = State::Busy(spawn_blocking(move || {
                        // SAFETY: the `Read` implementation of `std` does not
                        // read from the buffer it is borrowing and correctly
                        // reports the length of the data written into the buffer.
                        let res = unsafe { buf.read_from(&mut &*std, max_buf_size) };
                        (Operation::Read(res), buf)
                    }));
                }
                State::Busy(ref mut rx) => {
                    let (op, mut buf) = ready!(Pin::new(rx).poll(cx))?;

                    match op {
                        Operation::Read(Ok(_)) => {
                            buf.copy_to(dst);
                            inner.state = State::Idle(Some(buf));
                            return Poll::Ready(Ok(()));
                        }
                        Operation::Read(Err(e)) => {
                            assert!(buf.is_empty());

                            inner.state = State::Idle(Some(buf));
                            return Poll::Ready(Err(e));
                        }
                        Operation::Write(Ok(())) => {
                            assert!(buf.is_empty());
                            inner.state = State::Idle(Some(buf));
                            continue;
                        }
                        Operation::Write(Err(e)) => {
                            assert!(inner.last_write_err.is_none());
                            inner.last_write_err = Some(e.kind());
                            inner.state = State::Idle(Some(buf));
                        }
                        Operation::Seek(result) => {
                            assert!(buf.is_empty());
                            inner.state = State::Idle(Some(buf));
                            if let Ok(pos) = result {
                                inner.pos = pos;
                            }
                            continue;
                        }
                    }
                }
            }
        }
    }
}

impl AsyncSeek for File {
    fn start_seek(self: Pin<&mut Self>, mut pos: SeekFrom) -> io::Result<()> {
        let me = self.get_mut();
        let inner = me.inner.get_mut();

        match inner.state {
            State::Busy(_) => Err(io::Error::new(
                io::ErrorKind::Other,
                "other file operation is pending, call poll_complete before start_seek",
            )),
            State::Idle(ref mut buf_cell) => {
                let mut buf = buf_cell.take().unwrap();

                // Factor in any unread data from the buf
                if !buf.is_empty() {
                    let n = buf.discard_read();

                    if let SeekFrom::Current(ref mut offset) = pos {
                        *offset += n;
                    }
                }

                let std = me.std.clone();

                inner.state = State::Busy(spawn_blocking(move || {
                    let res = (&*std).seek(pos);
                    (Operation::Seek(res), buf)
                }));
                Ok(())
            }
        }
    }

    fn poll_complete(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<u64>> {
        ready!(crate::trace::trace_leaf(cx));
        let inner = self.inner.get_mut();

        loop {
            match inner.state {
                State::Idle(_) => return Poll::Ready(Ok(inner.pos)),
                State::Busy(ref mut rx) => {
                    let (op, buf) = ready!(Pin::new(rx).poll(cx))?;
                    inner.state = State::Idle(Some(buf));

                    match op {
                        Operation::Read(_) => {}
                        Operation::Write(Err(e)) => {
                            assert!(inner.last_write_err.is_none());
                            inner.last_write_err = Some(e.kind());
                        }
                        Operation::Write(_) => {}
                        Operation::Seek(res) => {
                            if let Ok(pos) = res {
                                inner.pos = pos;
                            }
                            return Poll::Ready(res);
                        }
                    }
                }
            }
        }
    }
}

impl AsyncWrite for File {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        src: &[u8],
    ) -> Poll<io::Result<usize>> {
        ready!(crate::trace::trace_leaf(cx));
        let me = self.get_mut();
        let inner = me.inner.get_mut();

        if let Some(e) = inner.last_write_err.take() {
            return Poll::Ready(Err(e.into()));
        }

        loop {
            match inner.state {
                State::Idle(ref mut buf_cell) => {
                    let mut buf = buf_cell.take().unwrap();

                    let seek = if !buf.is_empty() {
                        Some(SeekFrom::Current(buf.discard_read()))
                    } else {
                        None
                    };

                    let n = buf.copy_from(src, me.max_buf_size);
                    let std = me.std.clone();

                    let blocking_task_join_handle = spawn_mandatory_blocking(move || {
                        let res = if let Some(seek) = seek {
                            (&*std).seek(seek).and_then(|_| buf.write_to(&mut &*std))
                        } else {
                            buf.write_to(&mut &*std)
                        };

                        (Operation::Write(res), buf)
                    })
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::Other, "background task failed")
                    })?;

                    inner.state = State::Busy(blocking_task_join_handle);

                    return Poll::Ready(Ok(n));
                }
                State::Busy(ref mut rx) => {
                    let (op, buf) = ready!(Pin::new(rx).poll(cx))?;
                    inner.state = State::Idle(Some(buf));

                    match op {
                        Operation::Read(_) => {
                            // We don't care about the result here. The fact
                            // that the cursor has advanced will be reflected in
                            // the next iteration of the loop
                            continue;
                        }
                        Operation::Write(res) => {
                            // If the previous write was successful, continue.
                            // Otherwise, error.
                            res?;
                            continue;
                        }
                        Operation::Seek(_) => {
                            // Ignore the seek
                            continue;
                        }
                    }
                }
            }
        }
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<Result<usize, io::Error>> {
        ready!(crate::trace::trace_leaf(cx));
        let me = self.get_mut();
        let inner = me.inner.get_mut();

        if let Some(e) = inner.last_write_err.take() {
            return Poll::Ready(Err(e.into()));
        }

        loop {
            match inner.state {
                State::Idle(ref mut buf_cell) => {
                    let mut buf = buf_cell.take().unwrap();

                    let seek = if !buf.is_empty() {
                        Some(SeekFrom::Current(buf.discard_read()))
                    } else {
                        None
                    };

                    let n = buf.copy_from_bufs(bufs, me.max_buf_size);
                    let std = me.std.clone();

                    let blocking_task_join_handle = spawn_mandatory_blocking(move || {
                        let res = if let Some(seek) = seek {
                            (&*std).seek(seek).and_then(|_| buf.write_to(&mut &*std))
                        } else {
                            buf.write_to(&mut &*std)
                        };

                        (Operation::Write(res), buf)
                    })
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::Other, "background task failed")
                    })?;

                    inner.state = State::Busy(blocking_task_join_handle);

                    return Poll::Ready(Ok(n));
                }
                State::Busy(ref mut rx) => {
                    let (op, buf) = ready!(Pin::new(rx).poll(cx))?;
                    inner.state = State::Idle(Some(buf));

                    match op {
                        Operation::Read(_) => {
                            // We don't care about the result here. The fact
                            // that the cursor has advanced will be reflected in
                            // the next iteration of the loop
                            continue;
                        }
                        Operation::Write(res) => {
                            // If the previous write was successful, continue.
                            // Otherwise, error.
                            res?;
                            continue;
                        }
                        Operation::Seek(_) => {
                            // Ignore the seek
                            continue;
                        }
                    }
                }
            }
        }
    }

    fn is_write_vectored(&self) -> bool {
        true
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        ready!(crate::trace::trace_leaf(cx));
        let inner = self.inner.get_mut();
        inner.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        ready!(crate::trace::trace_leaf(cx));
        self.poll_flush(cx)
    }
}

impl From<StdFile> for File {
    fn from(std: StdFile) -> Self {
        Self::from_std(std)
    }
}

impl fmt::Debug for File {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("tokio::fs::File")
            .field("std", &self.std)
            .finish()
    }
}

#[cfg(unix)]
impl std::os::unix::io::AsRawFd for File {
    fn as_raw_fd(&self) -> std::os::unix::io::RawFd {
        self.std.as_raw_fd()
    }
}

#[cfg(unix)]
impl std::os::unix::io::AsFd for File {
    fn as_fd(&self) -> std::os::unix::io::BorrowedFd<'_> {
        unsafe {
            std::os::unix::io::BorrowedFd::borrow_raw(std::os::unix::io::AsRawFd::as_raw_fd(self))
        }
    }
}

#[cfg(unix)]
impl std::os::unix::io::FromRawFd for File {
    unsafe fn from_raw_fd(fd: std::os::unix::io::RawFd) -> Self {
        StdFile::from_raw_fd(fd).into()
    }
}

cfg_windows! {
    use crate::os::windows::io::{AsRawHandle, FromRawHandle, RawHandle, AsHandle, BorrowedHandle};

    impl AsRawHandle for File {
        fn as_raw_handle(&self) -> RawHandle {
            self.std.as_raw_handle()
        }
    }

    impl AsHandle for File {
        fn as_handle(&self) -> BorrowedHandle<'_> {
            unsafe {
                BorrowedHandle::borrow_raw(
                    AsRawHandle::as_raw_handle(self),
                )
            }
        }
    }

    impl FromRawHandle for File {
        unsafe fn from_raw_handle(handle: RawHandle) -> Self {
            StdFile::from_raw_handle(handle).into()
        }
    }
}

impl Inner {
    async fn complete_inflight(&mut self) {
        use std::future::poll_fn;

        poll_fn(|cx| self.poll_complete_inflight(cx)).await;
    }

    fn poll_complete_inflight(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        ready!(crate::trace::trace_leaf(cx));
        match self.poll_flush(cx) {
            Poll::Ready(Err(e)) => {
                self.last_write_err = Some(e.kind());
                Poll::Ready(())
            }
            Poll::Ready(Ok(())) => Poll::Ready(()),
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_flush(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        if let Some(e) = self.last_write_err.take() {
            return Poll::Ready(Err(e.into()));
        }

        let (op, buf) = match self.state {
            State::Idle(_) => return Poll::Ready(Ok(())),
            State::Busy(ref mut rx) => ready!(Pin::new(rx).poll(cx))?,
        };

        // The buffer is not used here
        self.state = State::Idle(Some(buf));

        match op {
            Operation::Read(_) => Poll::Ready(Ok(())),
            Operation::Write(res) => Poll::Ready(res),
            Operation::Seek(_) => Poll::Ready(Ok(())),
        }
    }
}

#[cfg(test)]
mod tests;
