//! Tokio support for [Windows named pipes].
//!
//! [Windows named pipes]: https://docs.microsoft.com/en-us/windows/win32/ipc/named-pipes

use std::ffi::c_void;
use std::ffi::OsStr;
use std::io::{self, Read, Write};
use std::pin::Pin;
use std::ptr;
use std::ptr::null_mut;
use std::task::{Context, Poll};

use crate::io::{AsyncRead, AsyncWrite, Interest, PollEvented, ReadBuf, Ready};
use crate::os::windows::io::{AsHandle, AsRawHandle, BorrowedHandle, FromRawHandle, RawHandle};

cfg_io_util! {
    use bytes::BufMut;
}

// Hide imports which are not used when generating documentation.
#[cfg(windows)]
mod doc {
    pub(super) use crate::os::windows::ffi::OsStrExt;
    pub(super) mod windows_sys {
        pub(crate) use windows_sys::{
            Win32::Foundation::*, Win32::Storage::FileSystem::*, Win32::System::Pipes::*,
            Win32::System::SystemServices::*,
        };
    }
    pub(super) use mio::windows as mio_windows;
}

// NB: none of these shows up in public API, so don't document them.
#[cfg(not(windows))]
mod doc {
    pub(super) mod mio_windows {
        pub type NamedPipe = crate::doc::NotDefinedHere;
    }
}

use self::doc::*;

/// A [Windows named pipe] server.
///
/// Accepting client connections involves creating a server with
/// [`ServerOptions::create`] and waiting for clients to connect using
/// [`NamedPipeServer::connect`].
///
/// To avoid having clients sporadically fail with
/// [`std::io::ErrorKind::NotFound`] when they connect to a server, we must
/// ensure that at least one server instance is available at all times. This
/// means that the typical listen loop for a server is a bit involved, because
/// we have to ensure that we never drop a server accidentally while a client
/// might connect.
///
/// So a correctly implemented server looks like this:
///
/// ```no_run
/// use std::io;
/// use tokio::net::windows::named_pipe::ServerOptions;
///
/// const PIPE_NAME: &str = r"\\.\pipe\named-pipe-idiomatic-server";
///
/// # #[tokio::main] async fn main() -> std::io::Result<()> {
/// // The first server needs to be constructed early so that clients can
/// // be correctly connected. Otherwise calling .wait will cause the client to
/// // error.
/// //
/// // Here we also make use of `first_pipe_instance`, which will ensure that
/// // there are no other servers up and running already.
/// let mut server = ServerOptions::new()
///     .first_pipe_instance(true)
///     .create(PIPE_NAME)?;
///
/// // Spawn the server loop.
/// let server = tokio::spawn(async move {
///     loop {
///         // Wait for a client to connect.
///         server.connect().await?;
///         let connected_client = server;
///
///         // Construct the next server to be connected before sending the one
///         // we already have of onto a task. This ensures that the server
///         // isn't closed (after it's done in the task) before a new one is
///         // available. Otherwise the client might error with
///         // `io::ErrorKind::NotFound`.
///         server = ServerOptions::new().create(PIPE_NAME)?;
///
///         let client = tokio::spawn(async move {
///             /* use the connected client */
/// #           Ok::<_, std::io::Error>(())
///         });
/// #       if true { break } // needed for type inference to work
///     }
///
///     Ok::<_, io::Error>(())
/// });
///
/// /* do something else not server related here */
/// # Ok(()) }
/// ```
///
/// [Windows named pipe]: https://docs.microsoft.com/en-us/windows/win32/ipc/named-pipes
#[derive(Debug)]
pub struct NamedPipeServer {
    io: PollEvented<mio_windows::NamedPipe>,
}

impl NamedPipeServer {
    /// Constructs a new named pipe server from the specified raw handle.
    ///
    /// This function will consume ownership of the handle given, passing
    /// responsibility for closing the handle to the returned object.
    ///
    /// This function is also unsafe as the primitives currently returned have
    /// the contract that they are the sole owner of the file descriptor they
    /// are wrapping. Usage of this function could accidentally allow violating
    /// this contract which can cause memory unsafety in code that relies on it
    /// being true.
    ///
    /// # Errors
    ///
    /// This errors if called outside of a [Tokio Runtime], or in a runtime that
    /// has not [enabled I/O], or if any OS-specific I/O errors occur.
    ///
    /// [Tokio Runtime]: crate::runtime::Runtime
    /// [enabled I/O]: crate::runtime::Builder::enable_io
    pub unsafe fn from_raw_handle(handle: RawHandle) -> io::Result<Self> {
        let named_pipe = mio_windows::NamedPipe::from_raw_handle(handle);

        Ok(Self {
            io: PollEvented::new(named_pipe)?,
        })
    }

    /// Retrieves information about the named pipe the server is associated
    /// with.
    ///
    /// ```no_run
    /// use tokio::net::windows::named_pipe::{PipeEnd, PipeMode, ServerOptions};
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\tokio-named-pipe-server-info";
    ///
    /// # #[tokio::main] async fn main() -> std::io::Result<()> {
    /// let server = ServerOptions::new()
    ///     .pipe_mode(PipeMode::Message)
    ///     .max_instances(5)
    ///     .create(PIPE_NAME)?;
    ///
    /// let server_info = server.info()?;
    ///
    /// assert_eq!(server_info.end, PipeEnd::Server);
    /// assert_eq!(server_info.mode, PipeMode::Message);
    /// assert_eq!(server_info.max_instances, 5);
    /// # Ok(()) }
    /// ```
    pub fn info(&self) -> io::Result<PipeInfo> {
        // Safety: we're ensuring the lifetime of the named pipe.
        unsafe { named_pipe_info(self.io.as_raw_handle()) }
    }

    /// Enables a named pipe server process to wait for a client process to
    /// connect to an instance of a named pipe. A client process connects by
    /// creating a named pipe with the same name.
    ///
    /// This corresponds to the [`ConnectNamedPipe`] system call.
    ///
    /// # Cancel safety
    ///
    /// This method is cancellation safe in the sense that if it is used as the
    /// event in a [`select!`](crate::select) statement and some other branch
    /// completes first, then no connection events have been lost.
    ///
    /// [`ConnectNamedPipe`]: https://docs.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-connectnamedpipe
    ///
    /// # Example
    ///
    /// ```no_run
    /// use tokio::net::windows::named_pipe::ServerOptions;
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\mynamedpipe";
    ///
    /// # #[tokio::main] async fn main() -> std::io::Result<()> {
    /// let pipe = ServerOptions::new().create(PIPE_NAME)?;
    ///
    /// // Wait for a client to connect.
    /// pipe.connect().await?;
    ///
    /// // Use the connected client...
    /// # Ok(()) }
    /// ```
    pub async fn connect(&self) -> io::Result<()> {
        match self.io.connect() {
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                self.io
                    .registration()
                    .async_io(Interest::WRITABLE, || self.io.connect())
                    .await
            }
            x => x,
        }
    }

    /// Disconnects the server end of a named pipe instance from a client
    /// process.
    ///
    /// ```
    /// use tokio::io::AsyncWriteExt;
    /// use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};
    /// use windows_sys::Win32::Foundation::ERROR_PIPE_NOT_CONNECTED;
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\tokio-named-pipe-disconnect";
    ///
    /// # #[tokio::main] async fn main() -> std::io::Result<()> {
    /// let server = ServerOptions::new()
    ///     .create(PIPE_NAME)?;
    ///
    /// let mut client = ClientOptions::new()
    ///     .open(PIPE_NAME)?;
    ///
    /// // Wait for a client to become connected.
    /// server.connect().await?;
    ///
    /// // Forcibly disconnect the client.
    /// server.disconnect()?;
    ///
    /// // Write fails with an OS-specific error after client has been
    /// // disconnected.
    /// let e = client.write(b"ping").await.unwrap_err();
    /// assert_eq!(e.raw_os_error(), Some(ERROR_PIPE_NOT_CONNECTED as i32));
    /// # Ok(()) }
    /// ```
    pub fn disconnect(&self) -> io::Result<()> {
        self.io.disconnect()
    }

    /// Waits for any of the requested ready states.
    ///
    /// This function is usually paired with `try_read()` or `try_write()`. It
    /// can be used to concurrently read / write to the same pipe on a single
    /// task without splitting the pipe.
    ///
    /// The function may complete without the pipe being ready. This is a
    /// false-positive and attempting an operation will return with
    /// `io::ErrorKind::WouldBlock`. The function can also return with an empty
    /// [`Ready`] set, so you should always check the returned value and possibly
    /// wait again if the requested states are not set.
    ///
    /// # Examples
    ///
    /// Concurrently read and write to the pipe on the same task without
    /// splitting.
    ///
    /// ```no_run
    /// use tokio::io::Interest;
    /// use tokio::net::windows::named_pipe;
    /// use std::error::Error;
    /// use std::io;
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\tokio-named-pipe-server-ready";
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn Error>> {
    ///     let server = named_pipe::ServerOptions::new()
    ///         .create(PIPE_NAME)?;
    ///
    ///     loop {
    ///         let ready = server.ready(Interest::READABLE | Interest::WRITABLE).await?;
    ///
    ///         if ready.is_readable() {
    ///             let mut data = vec![0; 1024];
    ///             // Try to read data, this may still fail with `WouldBlock`
    ///             // if the readiness event is a false positive.
    ///             match server.try_read(&mut data) {
    ///                 Ok(n) => {
    ///                     println!("read {} bytes", n);
    ///                 }
    ///                 Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                     continue;
    ///                 }
    ///                 Err(e) => {
    ///                     return Err(e.into());
    ///                 }
    ///             }
    ///         }
    ///
    ///         if ready.is_writable() {
    ///             // Try to write data, this may still fail with `WouldBlock`
    ///             // if the readiness event is a false positive.
    ///             match server.try_write(b"hello world") {
    ///                 Ok(n) => {
    ///                     println!("write {} bytes", n);
    ///                 }
    ///                 Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                     continue;
    ///                 }
    ///                 Err(e) => {
    ///                     return Err(e.into());
    ///                 }
    ///             }
    ///         }
    ///     }
    /// }
    /// ```
    pub async fn ready(&self, interest: Interest) -> io::Result<Ready> {
        let event = self.io.registration().readiness(interest).await?;
        Ok(event.ready)
    }

    /// Waits for the pipe to become readable.
    ///
    /// This function is equivalent to `ready(Interest::READABLE)` and is usually
    /// paired with `try_read()`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::windows::named_pipe;
    /// use std::error::Error;
    /// use std::io;
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\tokio-named-pipe-server-readable";
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn Error>> {
    ///     let server = named_pipe::ServerOptions::new()
    ///         .create(PIPE_NAME)?;
    ///
    ///     let mut msg = vec![0; 1024];
    ///
    ///     loop {
    ///         // Wait for the pipe to be readable
    ///         server.readable().await?;
    ///
    ///         // Try to read data, this may still fail with `WouldBlock`
    ///         // if the readiness event is a false positive.
    ///         match server.try_read(&mut msg) {
    ///             Ok(n) => {
    ///                 msg.truncate(n);
    ///                 break;
    ///             }
    ///             Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                 continue;
    ///             }
    ///             Err(e) => {
    ///                 return Err(e.into());
    ///             }
    ///         }
    ///     }
    ///
    ///     println!("GOT = {:?}", msg);
    ///     Ok(())
    /// }
    /// ```
    pub async fn readable(&self) -> io::Result<()> {
        self.ready(Interest::READABLE).await?;
        Ok(())
    }

    /// Polls for read readiness.
    ///
    /// If the pipe is not currently ready for reading, this method will
    /// store a clone of the `Waker` from the provided `Context`. When the pipe
    /// becomes ready for reading, `Waker::wake` will be called on the waker.
    ///
    /// Note that on multiple calls to `poll_read_ready` or `poll_read`, only
    /// the `Waker` from the `Context` passed to the most recent call is
    /// scheduled to receive a wakeup. (However, `poll_write_ready` retains a
    /// second, independent waker.)
    ///
    /// This function is intended for cases where creating and pinning a future
    /// via [`readable`] is not feasible. Where possible, using [`readable`] is
    /// preferred, as this supports polling from multiple tasks at once.
    ///
    /// # Return value
    ///
    /// The function returns:
    ///
    /// * `Poll::Pending` if the pipe is not ready for reading.
    /// * `Poll::Ready(Ok(()))` if the pipe is ready for reading.
    /// * `Poll::Ready(Err(e))` if an error is encountered.
    ///
    /// # Errors
    ///
    /// This function may encounter any standard I/O error except `WouldBlock`.
    ///
    /// [`readable`]: method@Self::readable
    pub fn poll_read_ready(&self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.io.registration().poll_read_ready(cx).map_ok(|_| ())
    }

    /// Tries to read data from the pipe into the provided buffer, returning how
    /// many bytes were read.
    ///
    /// Receives any pending data from the pipe but does not wait for new data
    /// to arrive. On success, returns the number of bytes read. Because
    /// `try_read()` is non-blocking, the buffer does not have to be stored by
    /// the async task and can exist entirely on the stack.
    ///
    /// Usually, [`readable()`] or [`ready()`] is used with this function.
    ///
    /// [`readable()`]: NamedPipeServer::readable()
    /// [`ready()`]: NamedPipeServer::ready()
    ///
    /// # Return
    ///
    /// If data is successfully read, `Ok(n)` is returned, where `n` is the
    /// number of bytes read. If `n` is `0`, then it can indicate one of two scenarios:
    ///
    /// 1. The pipe's read half is closed and will no longer yield data.
    /// 2. The specified buffer was 0 bytes in length.
    ///
    /// If the pipe is not ready to read data,
    /// `Err(io::ErrorKind::WouldBlock)` is returned.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::windows::named_pipe;
    /// use std::error::Error;
    /// use std::io;
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\tokio-named-pipe-server-try-read";
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn Error>> {
    ///     let server = named_pipe::ServerOptions::new()
    ///         .create(PIPE_NAME)?;
    ///
    ///     loop {
    ///         // Wait for the pipe to be readable
    ///         server.readable().await?;
    ///
    ///         // Creating the buffer **after** the `await` prevents it from
    ///         // being stored in the async task.
    ///         let mut buf = [0; 4096];
    ///
    ///         // Try to read data, this may still fail with `WouldBlock`
    ///         // if the readiness event is a false positive.
    ///         match server.try_read(&mut buf) {
    ///             Ok(0) => break,
    ///             Ok(n) => {
    ///                 println!("read {} bytes", n);
    ///             }
    ///             Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                 continue;
    ///             }
    ///             Err(e) => {
    ///                 return Err(e.into());
    ///             }
    ///         }
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn try_read(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.io
            .registration()
            .try_io(Interest::READABLE, || (&*self.io).read(buf))
    }

    /// Tries to read data from the pipe into the provided buffers, returning
    /// how many bytes were read.
    ///
    /// Data is copied to fill each buffer in order, with the final buffer
    /// written to possibly being only partially filled. This method behaves
    /// equivalently to a single call to [`try_read()`] with concatenated
    /// buffers.
    ///
    /// Receives any pending data from the pipe but does not wait for new data
    /// to arrive. On success, returns the number of bytes read. Because
    /// `try_read_vectored()` is non-blocking, the buffer does not have to be
    /// stored by the async task and can exist entirely on the stack.
    ///
    /// Usually, [`readable()`] or [`ready()`] is used with this function.
    ///
    /// [`try_read()`]: NamedPipeServer::try_read()
    /// [`readable()`]: NamedPipeServer::readable()
    /// [`ready()`]: NamedPipeServer::ready()
    ///
    /// # Return
    ///
    /// If data is successfully read, `Ok(n)` is returned, where `n` is the
    /// number of bytes read. `Ok(0)` indicates the pipe's read half is closed
    /// and will no longer yield data. If the pipe is not ready to read data
    /// `Err(io::ErrorKind::WouldBlock)` is returned.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::windows::named_pipe;
    /// use std::error::Error;
    /// use std::io::{self, IoSliceMut};
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\tokio-named-pipe-server-try-read-vectored";
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn Error>> {
    ///     let server = named_pipe::ServerOptions::new()
    ///         .create(PIPE_NAME)?;
    ///
    ///     loop {
    ///         // Wait for the pipe to be readable
    ///         server.readable().await?;
    ///
    ///         // Creating the buffer **after** the `await` prevents it from
    ///         // being stored in the async task.
    ///         let mut buf_a = [0; 512];
    ///         let mut buf_b = [0; 1024];
    ///         let mut bufs = [
    ///             IoSliceMut::new(&mut buf_a),
    ///             IoSliceMut::new(&mut buf_b),
    ///         ];
    ///
    ///         // Try to read data, this may still fail with `WouldBlock`
    ///         // if the readiness event is a false positive.
    ///         match server.try_read_vectored(&mut bufs) {
    ///             Ok(0) => break,
    ///             Ok(n) => {
    ///                 println!("read {} bytes", n);
    ///             }
    ///             Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                 continue;
    ///             }
    ///             Err(e) => {
    ///                 return Err(e.into());
    ///             }
    ///         }
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn try_read_vectored(&self, bufs: &mut [io::IoSliceMut<'_>]) -> io::Result<usize> {
        self.io
            .registration()
            .try_io(Interest::READABLE, || (&*self.io).read_vectored(bufs))
    }

    cfg_io_util! {
        /// Tries to read data from the stream into the provided buffer, advancing the
        /// buffer's internal cursor, returning how many bytes were read.
        ///
        /// Receives any pending data from the pipe but does not wait for new data
        /// to arrive. On success, returns the number of bytes read. Because
        /// `try_read_buf()` is non-blocking, the buffer does not have to be stored by
        /// the async task and can exist entirely on the stack.
        ///
        /// Usually, [`readable()`] or [`ready()`] is used with this function.
        ///
        /// [`readable()`]: NamedPipeServer::readable()
        /// [`ready()`]: NamedPipeServer::ready()
        ///
        /// # Return
        ///
        /// If data is successfully read, `Ok(n)` is returned, where `n` is the
        /// number of bytes read. `Ok(0)` indicates the stream's read half is closed
        /// and will no longer yield data. If the stream is not ready to read data
        /// `Err(io::ErrorKind::WouldBlock)` is returned.
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use tokio::net::windows::named_pipe;
        /// use std::error::Error;
        /// use std::io;
        ///
        /// const PIPE_NAME: &str = r"\\.\pipe\tokio-named-pipe-client-readable";
        ///
        /// #[tokio::main]
        /// async fn main() -> Result<(), Box<dyn Error>> {
        ///     let server = named_pipe::ServerOptions::new().create(PIPE_NAME)?;
        ///
        ///     loop {
        ///         // Wait for the pipe to be readable
        ///         server.readable().await?;
        ///
        ///         let mut buf = Vec::with_capacity(4096);
        ///
        ///         // Try to read data, this may still fail with `WouldBlock`
        ///         // if the readiness event is a false positive.
        ///         match server.try_read_buf(&mut buf) {
        ///             Ok(0) => break,
        ///             Ok(n) => {
        ///                 println!("read {} bytes", n);
        ///             }
        ///             Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
        ///                 continue;
        ///             }
        ///             Err(e) => {
        ///                 return Err(e.into());
        ///             }
        ///         }
        ///     }
        ///
        ///     Ok(())
        /// }
        /// ```
        pub fn try_read_buf<B: BufMut>(&self, buf: &mut B) -> io::Result<usize> {
            self.io.registration().try_io(Interest::READABLE, || {
                use std::io::Read;

                let dst = buf.chunk_mut();
                let dst =
                    unsafe { &mut *(dst as *mut _ as *mut [std::mem::MaybeUninit<u8>] as *mut [u8]) };

                // Safety: We trust `NamedPipeServer::read` to have filled up `n` bytes in the
                // buffer.
                let n = (&*self.io).read(dst)?;

                unsafe {
                    buf.advance_mut(n);
                }

                Ok(n)
            })
        }
    }

    /// Waits for the pipe to become writable.
    ///
    /// This function is equivalent to `ready(Interest::WRITABLE)` and is usually
    /// paired with `try_write()`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::windows::named_pipe;
    /// use std::error::Error;
    /// use std::io;
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\tokio-named-pipe-server-writable";
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn Error>> {
    ///     let server = named_pipe::ServerOptions::new()
    ///         .create(PIPE_NAME)?;
    ///
    ///     loop {
    ///         // Wait for the pipe to be writable
    ///         server.writable().await?;
    ///
    ///         // Try to write data, this may still fail with `WouldBlock`
    ///         // if the readiness event is a false positive.
    ///         match server.try_write(b"hello world") {
    ///             Ok(n) => {
    ///                 break;
    ///             }
    ///             Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                 continue;
    ///             }
    ///             Err(e) => {
    ///                 return Err(e.into());
    ///             }
    ///         }
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn writable(&self) -> io::Result<()> {
        self.ready(Interest::WRITABLE).await?;
        Ok(())
    }

    /// Polls for write readiness.
    ///
    /// If the pipe is not currently ready for writing, this method will
    /// store a clone of the `Waker` from the provided `Context`. When the pipe
    /// becomes ready for writing, `Waker::wake` will be called on the waker.
    ///
    /// Note that on multiple calls to `poll_write_ready` or `poll_write`, only
    /// the `Waker` from the `Context` passed to the most recent call is
    /// scheduled to receive a wakeup. (However, `poll_read_ready` retains a
    /// second, independent waker.)
    ///
    /// This function is intended for cases where creating and pinning a future
    /// via [`writable`] is not feasible. Where possible, using [`writable`] is
    /// preferred, as this supports polling from multiple tasks at once.
    ///
    /// # Return value
    ///
    /// The function returns:
    ///
    /// * `Poll::Pending` if the pipe is not ready for writing.
    /// * `Poll::Ready(Ok(()))` if the pipe is ready for writing.
    /// * `Poll::Ready(Err(e))` if an error is encountered.
    ///
    /// # Errors
    ///
    /// This function may encounter any standard I/O error except `WouldBlock`.
    ///
    /// [`writable`]: method@Self::writable
    pub fn poll_write_ready(&self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.io.registration().poll_write_ready(cx).map_ok(|_| ())
    }

    /// Tries to write a buffer to the pipe, returning how many bytes were
    /// written.
    ///
    /// The function will attempt to write the entire contents of `buf`, but
    /// only part of the buffer may be written.
    ///
    /// This function is usually paired with `writable()`.
    ///
    /// # Return
    ///
    /// If data is successfully written, `Ok(n)` is returned, where `n` is the
    /// number of bytes written. If the pipe is not ready to write data,
    /// `Err(io::ErrorKind::WouldBlock)` is returned.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::windows::named_pipe;
    /// use std::error::Error;
    /// use std::io;
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\tokio-named-pipe-server-try-write";
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn Error>> {
    ///     let server = named_pipe::ServerOptions::new()
    ///         .create(PIPE_NAME)?;
    ///
    ///     loop {
    ///         // Wait for the pipe to be writable
    ///         server.writable().await?;
    ///
    ///         // Try to write data, this may still fail with `WouldBlock`
    ///         // if the readiness event is a false positive.
    ///         match server.try_write(b"hello world") {
    ///             Ok(n) => {
    ///                 break;
    ///             }
    ///             Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                 continue;
    ///             }
    ///             Err(e) => {
    ///                 return Err(e.into());
    ///             }
    ///         }
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn try_write(&self, buf: &[u8]) -> io::Result<usize> {
        self.io
            .registration()
            .try_io(Interest::WRITABLE, || (&*self.io).write(buf))
    }

    /// Tries to write several buffers to the pipe, returning how many bytes
    /// were written.
    ///
    /// Data is written from each buffer in order, with the final buffer read
    /// from possible being only partially consumed. This method behaves
    /// equivalently to a single call to [`try_write()`] with concatenated
    /// buffers.
    ///
    /// This function is usually paired with `writable()`.
    ///
    /// [`try_write()`]: NamedPipeServer::try_write()
    ///
    /// # Return
    ///
    /// If data is successfully written, `Ok(n)` is returned, where `n` is the
    /// number of bytes written. If the pipe is not ready to write data,
    /// `Err(io::ErrorKind::WouldBlock)` is returned.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::windows::named_pipe;
    /// use std::error::Error;
    /// use std::io;
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\tokio-named-pipe-server-try-write-vectored";
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn Error>> {
    ///     let server = named_pipe::ServerOptions::new()
    ///         .create(PIPE_NAME)?;
    ///
    ///     let bufs = [io::IoSlice::new(b"hello "), io::IoSlice::new(b"world")];
    ///
    ///     loop {
    ///         // Wait for the pipe to be writable
    ///         server.writable().await?;
    ///
    ///         // Try to write data, this may still fail with `WouldBlock`
    ///         // if the readiness event is a false positive.
    ///         match server.try_write_vectored(&bufs) {
    ///             Ok(n) => {
    ///                 break;
    ///             }
    ///             Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                 continue;
    ///             }
    ///             Err(e) => {
    ///                 return Err(e.into());
    ///             }
    ///         }
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn try_write_vectored(&self, buf: &[io::IoSlice<'_>]) -> io::Result<usize> {
        self.io
            .registration()
            .try_io(Interest::WRITABLE, || (&*self.io).write_vectored(buf))
    }

    /// Tries to read or write from the pipe using a user-provided IO operation.
    ///
    /// If the pipe is ready, the provided closure is called. The closure
    /// should attempt to perform IO operation from the pipe by manually
    /// calling the appropriate syscall. If the operation fails because the
    /// pipe is not actually ready, then the closure should return a
    /// `WouldBlock` error and the readiness flag is cleared. The return value
    /// of the closure is then returned by `try_io`.
    ///
    /// If the pipe is not ready, then the closure is not called
    /// and a `WouldBlock` error is returned.
    ///
    /// The closure should only return a `WouldBlock` error if it has performed
    /// an IO operation on the pipe that failed due to the pipe not being
    /// ready. Returning a `WouldBlock` error in any other situation will
    /// incorrectly clear the readiness flag, which can cause the pipe to
    /// behave incorrectly.
    ///
    /// The closure should not perform the IO operation using any of the
    /// methods defined on the Tokio `NamedPipeServer` type, as this will mess with
    /// the readiness flag and can cause the pipe to behave incorrectly.
    ///
    /// This method is not intended to be used with combined interests.
    /// The closure should perform only one type of IO operation, so it should not
    /// require more than one ready state. This method may panic or sleep forever
    /// if it is called with a combined interest.
    ///
    /// Usually, [`readable()`], [`writable()`] or [`ready()`] is used with this function.
    ///
    /// [`readable()`]: NamedPipeServer::readable()
    /// [`writable()`]: NamedPipeServer::writable()
    /// [`ready()`]: NamedPipeServer::ready()
    pub fn try_io<R>(
        &self,
        interest: Interest,
        f: impl FnOnce() -> io::Result<R>,
    ) -> io::Result<R> {
        self.io.registration().try_io(interest, f)
    }

    /// Reads or writes from the pipe using a user-provided IO operation.
    ///
    /// The readiness of the pipe is awaited and when the pipe is ready,
    /// the provided closure is called. The closure should attempt to perform
    /// IO operation on the pipe by manually calling the appropriate syscall.
    /// If the operation fails because the pipe is not actually ready,
    /// then the closure should return a `WouldBlock` error. In such case the
    /// readiness flag is cleared and the pipe readiness is awaited again.
    /// This loop is repeated until the closure returns an `Ok` or an error
    /// other than `WouldBlock`.
    ///
    /// The closure should only return a `WouldBlock` error if it has performed
    /// an IO operation on the pipe that failed due to the pipe not being
    /// ready. Returning a `WouldBlock` error in any other situation will
    /// incorrectly clear the readiness flag, which can cause the pipe to
    /// behave incorrectly.
    ///
    /// The closure should not perform the IO operation using any of the methods
    /// defined on the Tokio `NamedPipeServer` type, as this will mess with the
    /// readiness flag and can cause the pipe to behave incorrectly.
    ///
    /// This method is not intended to be used with combined interests.
    /// The closure should perform only one type of IO operation, so it should not
    /// require more than one ready state. This method may panic or sleep forever
    /// if it is called with a combined interest.
    pub async fn async_io<R>(
        &self,
        interest: Interest,
        f: impl FnMut() -> io::Result<R>,
    ) -> io::Result<R> {
        self.io.registration().async_io(interest, f).await
    }
}

impl AsyncRead for NamedPipeServer {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        unsafe { self.io.poll_read(cx, buf) }
    }
}

impl AsyncWrite for NamedPipeServer {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.io.poll_write(cx, buf)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        self.io.poll_write_vectored(cx, bufs)
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_flush(cx)
    }
}

impl AsRawHandle for NamedPipeServer {
    fn as_raw_handle(&self) -> RawHandle {
        self.io.as_raw_handle()
    }
}

impl AsHandle for NamedPipeServer {
    fn as_handle(&self) -> BorrowedHandle<'_> {
        unsafe { BorrowedHandle::borrow_raw(self.as_raw_handle()) }
    }
}

/// A [Windows named pipe] client.
///
/// Constructed using [`ClientOptions::open`].
///
/// Connecting a client correctly involves a few steps. When connecting through
/// [`ClientOptions::open`], it might error indicating one of two things:
///
/// * [`std::io::ErrorKind::NotFound`] - There is no server available.
/// * [`ERROR_PIPE_BUSY`] - There is a server available, but it is busy. Sleep
///   for a while and try again.
///
/// So a correctly implemented client looks like this:
///
/// ```no_run
/// use std::time::Duration;
/// use tokio::net::windows::named_pipe::ClientOptions;
/// use tokio::time;
/// use windows_sys::Win32::Foundation::ERROR_PIPE_BUSY;
///
/// const PIPE_NAME: &str = r"\\.\pipe\named-pipe-idiomatic-client";
///
/// # #[tokio::main] async fn main() -> std::io::Result<()> {
/// let client = loop {
///     match ClientOptions::new().open(PIPE_NAME) {
///         Ok(client) => break client,
///         Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) => (),
///         Err(e) => return Err(e),
///     }
///
///     time::sleep(Duration::from_millis(50)).await;
/// };
///
/// /* use the connected client */
/// # Ok(()) }
/// ```
///
/// [`ERROR_PIPE_BUSY`]: https://docs.rs/windows-sys/latest/windows_sys/Win32/Foundation/constant.ERROR_PIPE_BUSY.html
/// [Windows named pipe]: https://docs.microsoft.com/en-us/windows/win32/ipc/named-pipes
#[derive(Debug)]
pub struct NamedPipeClient {
    io: PollEvented<mio_windows::NamedPipe>,
}

impl NamedPipeClient {
    /// Constructs a new named pipe client from the specified raw handle.
    ///
    /// This function will consume ownership of the handle given, passing
    /// responsibility for closing the handle to the returned object.
    ///
    /// This function is also unsafe as the primitives currently returned have
    /// the contract that they are the sole owner of the file descriptor they
    /// are wrapping. Usage of this function could accidentally allow violating
    /// this contract which can cause memory unsafety in code that relies on it
    /// being true.
    ///
    /// # Errors
    ///
    /// This errors if called outside of a [Tokio Runtime], or in a runtime that
    /// has not [enabled I/O], or if any OS-specific I/O errors occur.
    ///
    /// [Tokio Runtime]: crate::runtime::Runtime
    /// [enabled I/O]: crate::runtime::Builder::enable_io
    pub unsafe fn from_raw_handle(handle: RawHandle) -> io::Result<Self> {
        let named_pipe = mio_windows::NamedPipe::from_raw_handle(handle);

        Ok(Self {
            io: PollEvented::new(named_pipe)?,
        })
    }

    /// Retrieves information about the named pipe the client is associated
    /// with.
    ///
    /// ```no_run
    /// use tokio::net::windows::named_pipe::{ClientOptions, PipeEnd, PipeMode};
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\tokio-named-pipe-client-info";
    ///
    /// # #[tokio::main] async fn main() -> std::io::Result<()> {
    /// let client = ClientOptions::new()
    ///     .open(PIPE_NAME)?;
    ///
    /// let client_info = client.info()?;
    ///
    /// assert_eq!(client_info.end, PipeEnd::Client);
    /// assert_eq!(client_info.mode, PipeMode::Message);
    /// assert_eq!(client_info.max_instances, 5);
    /// # Ok(()) }
    /// ```
    pub fn info(&self) -> io::Result<PipeInfo> {
        // Safety: we're ensuring the lifetime of the named pipe.
        unsafe { named_pipe_info(self.io.as_raw_handle()) }
    }

    /// Waits for any of the requested ready states.
    ///
    /// This function is usually paired with `try_read()` or `try_write()`. It
    /// can be used to concurrently read / write to the same pipe on a single
    /// task without splitting the pipe.
    ///
    /// The function may complete without the pipe being ready. This is a
    /// false-positive and attempting an operation will return with
    /// `io::ErrorKind::WouldBlock`. The function can also return with an empty
    /// [`Ready`] set, so you should always check the returned value and possibly
    /// wait again if the requested states are not set.
    ///
    /// # Examples
    ///
    /// Concurrently read and write to the pipe on the same task without
    /// splitting.
    ///
    /// ```no_run
    /// use tokio::io::Interest;
    /// use tokio::net::windows::named_pipe;
    /// use std::error::Error;
    /// use std::io;
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\tokio-named-pipe-client-ready";
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn Error>> {
    ///     let client = named_pipe::ClientOptions::new().open(PIPE_NAME)?;
    ///
    ///     loop {
    ///         let ready = client.ready(Interest::READABLE | Interest::WRITABLE).await?;
    ///
    ///         if ready.is_readable() {
    ///             let mut data = vec![0; 1024];
    ///             // Try to read data, this may still fail with `WouldBlock`
    ///             // if the readiness event is a false positive.
    ///             match client.try_read(&mut data) {
    ///                 Ok(n) => {
    ///                     println!("read {} bytes", n);
    ///                 }
    ///                 Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                     continue;
    ///                 }
    ///                 Err(e) => {
    ///                     return Err(e.into());
    ///                 }
    ///             }
    ///         }
    ///
    ///         if ready.is_writable() {
    ///             // Try to write data, this may still fail with `WouldBlock`
    ///             // if the readiness event is a false positive.
    ///             match client.try_write(b"hello world") {
    ///                 Ok(n) => {
    ///                     println!("write {} bytes", n);
    ///                 }
    ///                 Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                     continue;
    ///                 }
    ///                 Err(e) => {
    ///                     return Err(e.into());
    ///                 }
    ///             }
    ///         }
    ///     }
    /// }
    /// ```
    pub async fn ready(&self, interest: Interest) -> io::Result<Ready> {
        let event = self.io.registration().readiness(interest).await?;
        Ok(event.ready)
    }

    /// Waits for the pipe to become readable.
    ///
    /// This function is equivalent to `ready(Interest::READABLE)` and is usually
    /// paired with `try_read()`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::windows::named_pipe;
    /// use std::error::Error;
    /// use std::io;
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\tokio-named-pipe-client-readable";
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn Error>> {
    ///     let client = named_pipe::ClientOptions::new().open(PIPE_NAME)?;
    ///
    ///     let mut msg = vec![0; 1024];
    ///
    ///     loop {
    ///         // Wait for the pipe to be readable
    ///         client.readable().await?;
    ///
    ///         // Try to read data, this may still fail with `WouldBlock`
    ///         // if the readiness event is a false positive.
    ///         match client.try_read(&mut msg) {
    ///             Ok(n) => {
    ///                 msg.truncate(n);
    ///                 break;
    ///             }
    ///             Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                 continue;
    ///             }
    ///             Err(e) => {
    ///                 return Err(e.into());
    ///             }
    ///         }
    ///     }
    ///
    ///     println!("GOT = {:?}", msg);
    ///     Ok(())
    /// }
    /// ```
    pub async fn readable(&self) -> io::Result<()> {
        self.ready(Interest::READABLE).await?;
        Ok(())
    }

    /// Polls for read readiness.
    ///
    /// If the pipe is not currently ready for reading, this method will
    /// store a clone of the `Waker` from the provided `Context`. When the pipe
    /// becomes ready for reading, `Waker::wake` will be called on the waker.
    ///
    /// Note that on multiple calls to `poll_read_ready` or `poll_read`, only
    /// the `Waker` from the `Context` passed to the most recent call is
    /// scheduled to receive a wakeup. (However, `poll_write_ready` retains a
    /// second, independent waker.)
    ///
    /// This function is intended for cases where creating and pinning a future
    /// via [`readable`] is not feasible. Where possible, using [`readable`] is
    /// preferred, as this supports polling from multiple tasks at once.
    ///
    /// # Return value
    ///
    /// The function returns:
    ///
    /// * `Poll::Pending` if the pipe is not ready for reading.
    /// * `Poll::Ready(Ok(()))` if the pipe is ready for reading.
    /// * `Poll::Ready(Err(e))` if an error is encountered.
    ///
    /// # Errors
    ///
    /// This function may encounter any standard I/O error except `WouldBlock`.
    ///
    /// [`readable`]: method@Self::readable
    pub fn poll_read_ready(&self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.io.registration().poll_read_ready(cx).map_ok(|_| ())
    }

    /// Tries to read data from the pipe into the provided buffer, returning how
    /// many bytes were read.
    ///
    /// Receives any pending data from the pipe but does not wait for new data
    /// to arrive. On success, returns the number of bytes read. Because
    /// `try_read()` is non-blocking, the buffer does not have to be stored by
    /// the async task and can exist entirely on the stack.
    ///
    /// Usually, [`readable()`] or [`ready()`] is used with this function.
    ///
    /// [`readable()`]: NamedPipeClient::readable()
    /// [`ready()`]: NamedPipeClient::ready()
    ///
    /// # Return
    ///
    /// If data is successfully read, `Ok(n)` is returned, where `n` is the
    /// number of bytes read. If `n` is `0`, then it can indicate one of two scenarios:
    ///
    /// 1. The pipe's read half is closed and will no longer yield data.
    /// 2. The specified buffer was 0 bytes in length.
    ///
    /// If the pipe is not ready to read data,
    /// `Err(io::ErrorKind::WouldBlock)` is returned.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::windows::named_pipe;
    /// use std::error::Error;
    /// use std::io;
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\tokio-named-pipe-client-try-read";
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn Error>> {
    ///     let client = named_pipe::ClientOptions::new().open(PIPE_NAME)?;
    ///
    ///     loop {
    ///         // Wait for the pipe to be readable
    ///         client.readable().await?;
    ///
    ///         // Creating the buffer **after** the `await` prevents it from
    ///         // being stored in the async task.
    ///         let mut buf = [0; 4096];
    ///
    ///         // Try to read data, this may still fail with `WouldBlock`
    ///         // if the readiness event is a false positive.
    ///         match client.try_read(&mut buf) {
    ///             Ok(0) => break,
    ///             Ok(n) => {
    ///                 println!("read {} bytes", n);
    ///             }
    ///             Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                 continue;
    ///             }
    ///             Err(e) => {
    ///                 return Err(e.into());
    ///             }
    ///         }
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn try_read(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.io
            .registration()
            .try_io(Interest::READABLE, || (&*self.io).read(buf))
    }

    /// Tries to read data from the pipe into the provided buffers, returning
    /// how many bytes were read.
    ///
    /// Data is copied to fill each buffer in order, with the final buffer
    /// written to possibly being only partially filled. This method behaves
    /// equivalently to a single call to [`try_read()`] with concatenated
    /// buffers.
    ///
    /// Receives any pending data from the pipe but does not wait for new data
    /// to arrive. On success, returns the number of bytes read. Because
    /// `try_read_vectored()` is non-blocking, the buffer does not have to be
    /// stored by the async task and can exist entirely on the stack.
    ///
    /// Usually, [`readable()`] or [`ready()`] is used with this function.
    ///
    /// [`try_read()`]: NamedPipeClient::try_read()
    /// [`readable()`]: NamedPipeClient::readable()
    /// [`ready()`]: NamedPipeClient::ready()
    ///
    /// # Return
    ///
    /// If data is successfully read, `Ok(n)` is returned, where `n` is the
    /// number of bytes read. `Ok(0)` indicates the pipe's read half is closed
    /// and will no longer yield data. If the pipe is not ready to read data
    /// `Err(io::ErrorKind::WouldBlock)` is returned.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::windows::named_pipe;
    /// use std::error::Error;
    /// use std::io::{self, IoSliceMut};
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\tokio-named-pipe-client-try-read-vectored";
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn Error>> {
    ///     let client = named_pipe::ClientOptions::new().open(PIPE_NAME)?;
    ///
    ///     loop {
    ///         // Wait for the pipe to be readable
    ///         client.readable().await?;
    ///
    ///         // Creating the buffer **after** the `await` prevents it from
    ///         // being stored in the async task.
    ///         let mut buf_a = [0; 512];
    ///         let mut buf_b = [0; 1024];
    ///         let mut bufs = [
    ///             IoSliceMut::new(&mut buf_a),
    ///             IoSliceMut::new(&mut buf_b),
    ///         ];
    ///
    ///         // Try to read data, this may still fail with `WouldBlock`
    ///         // if the readiness event is a false positive.
    ///         match client.try_read_vectored(&mut bufs) {
    ///             Ok(0) => break,
    ///             Ok(n) => {
    ///                 println!("read {} bytes", n);
    ///             }
    ///             Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                 continue;
    ///             }
    ///             Err(e) => {
    ///                 return Err(e.into());
    ///             }
    ///         }
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn try_read_vectored(&self, bufs: &mut [io::IoSliceMut<'_>]) -> io::Result<usize> {
        self.io
            .registration()
            .try_io(Interest::READABLE, || (&*self.io).read_vectored(bufs))
    }

    cfg_io_util! {
        /// Tries to read data from the stream into the provided buffer, advancing the
        /// buffer's internal cursor, returning how many bytes were read.
        ///
        /// Receives any pending data from the pipe but does not wait for new data
        /// to arrive. On success, returns the number of bytes read. Because
        /// `try_read_buf()` is non-blocking, the buffer does not have to be stored by
        /// the async task and can exist entirely on the stack.
        ///
        /// Usually, [`readable()`] or [`ready()`] is used with this function.
        ///
        /// [`readable()`]: NamedPipeClient::readable()
        /// [`ready()`]: NamedPipeClient::ready()
        ///
        /// # Return
        ///
        /// If data is successfully read, `Ok(n)` is returned, where `n` is the
        /// number of bytes read. `Ok(0)` indicates the stream's read half is closed
        /// and will no longer yield data. If the stream is not ready to read data
        /// `Err(io::ErrorKind::WouldBlock)` is returned.
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use tokio::net::windows::named_pipe;
        /// use std::error::Error;
        /// use std::io;
        ///
        /// const PIPE_NAME: &str = r"\\.\pipe\tokio-named-pipe-client-readable";
        ///
        /// #[tokio::main]
        /// async fn main() -> Result<(), Box<dyn Error>> {
        ///     let client = named_pipe::ClientOptions::new().open(PIPE_NAME)?;
        ///
        ///     loop {
        ///         // Wait for the pipe to be readable
        ///         client.readable().await?;
        ///
        ///         let mut buf = Vec::with_capacity(4096);
        ///
        ///         // Try to read data, this may still fail with `WouldBlock`
        ///         // if the readiness event is a false positive.
        ///         match client.try_read_buf(&mut buf) {
        ///             Ok(0) => break,
        ///             Ok(n) => {
        ///                 println!("read {} bytes", n);
        ///             }
        ///             Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
        ///                 continue;
        ///             }
        ///             Err(e) => {
        ///                 return Err(e.into());
        ///             }
        ///         }
        ///     }
        ///
        ///     Ok(())
        /// }
        /// ```
        pub fn try_read_buf<B: BufMut>(&self, buf: &mut B) -> io::Result<usize> {
            self.io.registration().try_io(Interest::READABLE, || {
                use std::io::Read;

                let dst = buf.chunk_mut();
                let dst =
                    unsafe { &mut *(dst as *mut _ as *mut [std::mem::MaybeUninit<u8>] as *mut [u8]) };

                // Safety: We trust `NamedPipeClient::read` to have filled up `n` bytes in the
                // buffer.
                let n = (&*self.io).read(dst)?;

                unsafe {
                    buf.advance_mut(n);
                }

                Ok(n)
            })
        }
    }

    /// Waits for the pipe to become writable.
    ///
    /// This function is equivalent to `ready(Interest::WRITABLE)` and is usually
    /// paired with `try_write()`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::windows::named_pipe;
    /// use std::error::Error;
    /// use std::io;
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\tokio-named-pipe-client-writable";
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn Error>> {
    ///     let client = named_pipe::ClientOptions::new().open(PIPE_NAME)?;
    ///
    ///     loop {
    ///         // Wait for the pipe to be writable
    ///         client.writable().await?;
    ///
    ///         // Try to write data, this may still fail with `WouldBlock`
    ///         // if the readiness event is a false positive.
    ///         match client.try_write(b"hello world") {
    ///             Ok(n) => {
    ///                 break;
    ///             }
    ///             Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                 continue;
    ///             }
    ///             Err(e) => {
    ///                 return Err(e.into());
    ///             }
    ///         }
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn writable(&self) -> io::Result<()> {
        self.ready(Interest::WRITABLE).await?;
        Ok(())
    }

    /// Polls for write readiness.
    ///
    /// If the pipe is not currently ready for writing, this method will
    /// store a clone of the `Waker` from the provided `Context`. When the pipe
    /// becomes ready for writing, `Waker::wake` will be called on the waker.
    ///
    /// Note that on multiple calls to `poll_write_ready` or `poll_write`, only
    /// the `Waker` from the `Context` passed to the most recent call is
    /// scheduled to receive a wakeup. (However, `poll_read_ready` retains a
    /// second, independent waker.)
    ///
    /// This function is intended for cases where creating and pinning a future
    /// via [`writable`] is not feasible. Where possible, using [`writable`] is
    /// preferred, as this supports polling from multiple tasks at once.
    ///
    /// # Return value
    ///
    /// The function returns:
    ///
    /// * `Poll::Pending` if the pipe is not ready for writing.
    /// * `Poll::Ready(Ok(()))` if the pipe is ready for writing.
    /// * `Poll::Ready(Err(e))` if an error is encountered.
    ///
    /// # Errors
    ///
    /// This function may encounter any standard I/O error except `WouldBlock`.
    ///
    /// [`writable`]: method@Self::writable
    pub fn poll_write_ready(&self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.io.registration().poll_write_ready(cx).map_ok(|_| ())
    }

    /// Tries to write a buffer to the pipe, returning how many bytes were
    /// written.
    ///
    /// The function will attempt to write the entire contents of `buf`, but
    /// only part of the buffer may be written.
    ///
    /// This function is usually paired with `writable()`.
    ///
    /// # Return
    ///
    /// If data is successfully written, `Ok(n)` is returned, where `n` is the
    /// number of bytes written. If the pipe is not ready to write data,
    /// `Err(io::ErrorKind::WouldBlock)` is returned.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::windows::named_pipe;
    /// use std::error::Error;
    /// use std::io;
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\tokio-named-pipe-client-try-write";
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn Error>> {
    ///     let client = named_pipe::ClientOptions::new().open(PIPE_NAME)?;
    ///
    ///     loop {
    ///         // Wait for the pipe to be writable
    ///         client.writable().await?;
    ///
    ///         // Try to write data, this may still fail with `WouldBlock`
    ///         // if the readiness event is a false positive.
    ///         match client.try_write(b"hello world") {
    ///             Ok(n) => {
    ///                 break;
    ///             }
    ///             Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                 continue;
    ///             }
    ///             Err(e) => {
    ///                 return Err(e.into());
    ///             }
    ///         }
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn try_write(&self, buf: &[u8]) -> io::Result<usize> {
        self.io
            .registration()
            .try_io(Interest::WRITABLE, || (&*self.io).write(buf))
    }

    /// Tries to write several buffers to the pipe, returning how many bytes
    /// were written.
    ///
    /// Data is written from each buffer in order, with the final buffer read
    /// from possible being only partially consumed. This method behaves
    /// equivalently to a single call to [`try_write()`] with concatenated
    /// buffers.
    ///
    /// This function is usually paired with `writable()`.
    ///
    /// [`try_write()`]: NamedPipeClient::try_write()
    ///
    /// # Return
    ///
    /// If data is successfully written, `Ok(n)` is returned, where `n` is the
    /// number of bytes written. If the pipe is not ready to write data,
    /// `Err(io::ErrorKind::WouldBlock)` is returned.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::net::windows::named_pipe;
    /// use std::error::Error;
    /// use std::io;
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\tokio-named-pipe-client-try-write-vectored";
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn Error>> {
    ///     let client = named_pipe::ClientOptions::new().open(PIPE_NAME)?;
    ///
    ///     let bufs = [io::IoSlice::new(b"hello "), io::IoSlice::new(b"world")];
    ///
    ///     loop {
    ///         // Wait for the pipe to be writable
    ///         client.writable().await?;
    ///
    ///         // Try to write data, this may still fail with `WouldBlock`
    ///         // if the readiness event is a false positive.
    ///         match client.try_write_vectored(&bufs) {
    ///             Ok(n) => {
    ///                 break;
    ///             }
    ///             Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///                 continue;
    ///             }
    ///             Err(e) => {
    ///                 return Err(e.into());
    ///             }
    ///         }
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn try_write_vectored(&self, buf: &[io::IoSlice<'_>]) -> io::Result<usize> {
        self.io
            .registration()
            .try_io(Interest::WRITABLE, || (&*self.io).write_vectored(buf))
    }

    /// Tries to read or write from the pipe using a user-provided IO operation.
    ///
    /// If the pipe is ready, the provided closure is called. The closure
    /// should attempt to perform IO operation from the pipe by manually
    /// calling the appropriate syscall. If the operation fails because the
    /// pipe is not actually ready, then the closure should return a
    /// `WouldBlock` error and the readiness flag is cleared. The return value
    /// of the closure is then returned by `try_io`.
    ///
    /// If the pipe is not ready, then the closure is not called
    /// and a `WouldBlock` error is returned.
    ///
    /// The closure should only return a `WouldBlock` error if it has performed
    /// an IO operation on the pipe that failed due to the pipe not being
    /// ready. Returning a `WouldBlock` error in any other situation will
    /// incorrectly clear the readiness flag, which can cause the pipe to
    /// behave incorrectly.
    ///
    /// The closure should not perform the IO operation using any of the methods
    /// defined on the Tokio `NamedPipeClient` type, as this will mess with the
    /// readiness flag and can cause the pipe to behave incorrectly.
    ///
    /// This method is not intended to be used with combined interests.
    /// The closure should perform only one type of IO operation, so it should not
    /// require more than one ready state. This method may panic or sleep forever
    /// if it is called with a combined interest.
    ///
    /// Usually, [`readable()`], [`writable()`] or [`ready()`] is used with this function.
    ///
    /// [`readable()`]: NamedPipeClient::readable()
    /// [`writable()`]: NamedPipeClient::writable()
    /// [`ready()`]: NamedPipeClient::ready()
    pub fn try_io<R>(
        &self,
        interest: Interest,
        f: impl FnOnce() -> io::Result<R>,
    ) -> io::Result<R> {
        self.io.registration().try_io(interest, f)
    }

    /// Reads or writes from the pipe using a user-provided IO operation.
    ///
    /// The readiness of the pipe is awaited and when the pipe is ready,
    /// the provided closure is called. The closure should attempt to perform
    /// IO operation on the pipe by manually calling the appropriate syscall.
    /// If the operation fails because the pipe is not actually ready,
    /// then the closure should return a `WouldBlock` error. In such case the
    /// readiness flag is cleared and the pipe readiness is awaited again.
    /// This loop is repeated until the closure returns an `Ok` or an error
    /// other than `WouldBlock`.
    ///
    /// The closure should only return a `WouldBlock` error if it has performed
    /// an IO operation on the pipe that failed due to the pipe not being
    /// ready. Returning a `WouldBlock` error in any other situation will
    /// incorrectly clear the readiness flag, which can cause the pipe to
    /// behave incorrectly.
    ///
    /// The closure should not perform the IO operation using any of the methods
    /// defined on the Tokio `NamedPipeClient` type, as this will mess with the
    /// readiness flag and can cause the pipe to behave incorrectly.
    ///
    /// This method is not intended to be used with combined interests.
    /// The closure should perform only one type of IO operation, so it should not
    /// require more than one ready state. This method may panic or sleep forever
    /// if it is called with a combined interest.
    pub async fn async_io<R>(
        &self,
        interest: Interest,
        f: impl FnMut() -> io::Result<R>,
    ) -> io::Result<R> {
        self.io.registration().async_io(interest, f).await
    }
}

impl AsyncRead for NamedPipeClient {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        unsafe { self.io.poll_read(cx, buf) }
    }
}

impl AsyncWrite for NamedPipeClient {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.io.poll_write(cx, buf)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        self.io.poll_write_vectored(cx, bufs)
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_flush(cx)
    }
}

impl AsRawHandle for NamedPipeClient {
    fn as_raw_handle(&self) -> RawHandle {
        self.io.as_raw_handle()
    }
}

impl AsHandle for NamedPipeClient {
    fn as_handle(&self) -> BorrowedHandle<'_> {
        unsafe { BorrowedHandle::borrow_raw(self.as_raw_handle()) }
    }
}

/// A builder structure for construct a named pipe with named pipe-specific
/// options. This is required to use for named pipe servers who wants to modify
/// pipe-related options.
///
/// See [`ServerOptions::create`].
#[derive(Debug, Clone)]
pub struct ServerOptions {
    // dwOpenMode
    access_inbound: bool,
    access_outbound: bool,
    first_pipe_instance: bool,
    write_dac: bool,
    write_owner: bool,
    access_system_security: bool,
    // dwPipeMode
    pipe_mode: PipeMode,
    reject_remote_clients: bool,
    // other options
    max_instances: u32,
    out_buffer_size: u32,
    in_buffer_size: u32,
    default_timeout: u32,
}

impl ServerOptions {
    /// Creates a new named pipe builder with the default settings.
    ///
    /// ```
    /// use tokio::net::windows::named_pipe::ServerOptions;
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\tokio-named-pipe-new";
    ///
    /// # #[tokio::main] async fn main() -> std::io::Result<()> {
    /// let server = ServerOptions::new().create(PIPE_NAME)?;
    /// # Ok(()) }
    /// ```
    pub fn new() -> ServerOptions {
        ServerOptions {
            access_inbound: true,
            access_outbound: true,
            first_pipe_instance: false,
            write_dac: false,
            write_owner: false,
            access_system_security: false,
            pipe_mode: PipeMode::Byte,
            reject_remote_clients: true,
            max_instances: windows_sys::PIPE_UNLIMITED_INSTANCES,
            out_buffer_size: 65536,
            in_buffer_size: 65536,
            default_timeout: 0,
        }
    }

    /// The pipe mode.
    ///
    /// The default pipe mode is [`PipeMode::Byte`]. See [`PipeMode`] for
    /// documentation of what each mode means.
    ///
    /// This corresponds to specifying `PIPE_TYPE_` and `PIPE_READMODE_` in  [`dwPipeMode`].
    ///
    /// [`dwPipeMode`]: https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createnamedpipea
    pub fn pipe_mode(&mut self, pipe_mode: PipeMode) -> &mut Self {
        self.pipe_mode = pipe_mode;
        self
    }

    /// The flow of data in the pipe goes from client to server only.
    ///
    /// This corresponds to setting [`PIPE_ACCESS_INBOUND`].
    ///
    /// [`PIPE_ACCESS_INBOUND`]: https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createnamedpipea#pipe_access_inbound
    ///
    /// # Errors
    ///
    /// Server side prevents connecting by denying inbound access, client errors
    /// with [`std::io::ErrorKind::PermissionDenied`] when attempting to create
    /// the connection.
    ///
    /// ```
    /// use std::io;
    /// use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\tokio-named-pipe-access-inbound-err1";
    ///
    /// # #[tokio::main] async fn main() -> io::Result<()> {
    /// let _server = ServerOptions::new()
    ///     .access_inbound(false)
    ///     .create(PIPE_NAME)?;
    ///
    /// let e = ClientOptions::new()
    ///     .open(PIPE_NAME)
    ///     .unwrap_err();
    ///
    /// assert_eq!(e.kind(), io::ErrorKind::PermissionDenied);
    /// # Ok(()) }
    /// ```
    ///
    /// Disabling writing allows a client to connect, but errors with
    /// [`std::io::ErrorKind::PermissionDenied`] if a write is attempted.
    ///
    /// ```
    /// use std::io;
    /// use tokio::io::AsyncWriteExt;
    /// use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\tokio-named-pipe-access-inbound-err2";
    ///
    /// # #[tokio::main] async fn main() -> io::Result<()> {
    /// let server = ServerOptions::new()
    ///     .access_inbound(false)
    ///     .create(PIPE_NAME)?;
    ///
    /// let mut client = ClientOptions::new()
    ///     .write(false)
    ///     .open(PIPE_NAME)?;
    ///
    /// server.connect().await?;
    ///
    /// let e = client.write(b"ping").await.unwrap_err();
    /// assert_eq!(e.kind(), io::ErrorKind::PermissionDenied);
    /// # Ok(()) }
    /// ```
    ///
    /// # Examples
    ///
    /// A unidirectional named pipe that only supports server-to-client
    /// communication.
    ///
    /// ```
    /// use std::io;
    /// use tokio::io::{AsyncReadExt, AsyncWriteExt};
    /// use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\tokio-named-pipe-access-inbound";
    ///
    /// # #[tokio::main] async fn main() -> io::Result<()> {
    /// let mut server = ServerOptions::new()
    ///     .access_inbound(false)
    ///     .create(PIPE_NAME)?;
    ///
    /// let mut client = ClientOptions::new()
    ///     .write(false)
    ///     .open(PIPE_NAME)?;
    ///
    /// server.connect().await?;
    ///
    /// let write = server.write_all(b"ping");
    ///
    /// let mut buf = [0u8; 4];
    /// let read = client.read_exact(&mut buf);
    ///
    /// let ((), read) = tokio::try_join!(write, read)?;
    ///
    /// assert_eq!(read, 4);
    /// assert_eq!(&buf[..], b"ping");
    /// # Ok(()) }
    /// ```
    pub fn access_inbound(&mut self, allowed: bool) -> &mut Self {
        self.access_inbound = allowed;
        self
    }

    /// The flow of data in the pipe goes from server to client only.
    ///
    /// This corresponds to setting [`PIPE_ACCESS_OUTBOUND`].
    ///
    /// [`PIPE_ACCESS_OUTBOUND`]: https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createnamedpipea#pipe_access_outbound
    ///
    /// # Errors
    ///
    /// Server side prevents connecting by denying outbound access, client
    /// errors with [`std::io::ErrorKind::PermissionDenied`] when attempting to
    /// create the connection.
    ///
    /// ```
    /// use std::io;
    /// use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\tokio-named-pipe-access-outbound-err1";
    ///
    /// # #[tokio::main] async fn main() -> io::Result<()> {
    /// let server = ServerOptions::new()
    ///     .access_outbound(false)
    ///     .create(PIPE_NAME)?;
    ///
    /// let e = ClientOptions::new()
    ///     .open(PIPE_NAME)
    ///     .unwrap_err();
    ///
    /// assert_eq!(e.kind(), io::ErrorKind::PermissionDenied);
    /// # Ok(()) }
    /// ```
    ///
    /// Disabling reading allows a client to connect, but attempting to read
    /// will error with [`std::io::ErrorKind::PermissionDenied`].
    ///
    /// ```
    /// use std::io;
    /// use tokio::io::AsyncReadExt;
    /// use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\tokio-named-pipe-access-outbound-err2";
    ///
    /// # #[tokio::main] async fn main() -> io::Result<()> {
    /// let server = ServerOptions::new()
    ///     .access_outbound(false)
    ///     .create(PIPE_NAME)?;
    ///
    /// let mut client = ClientOptions::new()
    ///     .read(false)
    ///     .open(PIPE_NAME)?;
    ///
    /// server.connect().await?;
    ///
    /// let mut buf = [0u8; 4];
    /// let e = client.read(&mut buf).await.unwrap_err();
    /// assert_eq!(e.kind(), io::ErrorKind::PermissionDenied);
    /// # Ok(()) }
    /// ```
    ///
    /// # Examples
    ///
    /// A unidirectional named pipe that only supports client-to-server
    /// communication.
    ///
    /// ```
    /// use tokio::io::{AsyncReadExt, AsyncWriteExt};
    /// use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\tokio-named-pipe-access-outbound";
    ///
    /// # #[tokio::main] async fn main() -> std::io::Result<()> {
    /// let mut server = ServerOptions::new()
    ///     .access_outbound(false)
    ///     .create(PIPE_NAME)?;
    ///
    /// let mut client = ClientOptions::new()
    ///     .read(false)
    ///     .open(PIPE_NAME)?;
    ///
    /// server.connect().await?;
    ///
    /// let write = client.write_all(b"ping");
    ///
    /// let mut buf = [0u8; 4];
    /// let read = server.read_exact(&mut buf);
    ///
    /// let ((), read) = tokio::try_join!(write, read)?;
    ///
    /// println!("done reading and writing");
    ///
    /// assert_eq!(read, 4);
    /// assert_eq!(&buf[..], b"ping");
    /// # Ok(()) }
    /// ```
    pub fn access_outbound(&mut self, allowed: bool) -> &mut Self {
        self.access_outbound = allowed;
        self
    }

    /// If you attempt to create multiple instances of a pipe with this flag
    /// set, creation of the first server instance succeeds, but creation of any
    /// subsequent instances will fail with
    /// [`std::io::ErrorKind::PermissionDenied`].
    ///
    /// This option is intended to be used with servers that want to ensure that
    /// they are the only process listening for clients on a given named pipe.
    /// This is accomplished by enabling it for the first server instance
    /// created in a process.
    ///
    /// This corresponds to setting [`FILE_FLAG_FIRST_PIPE_INSTANCE`].
    ///
    /// # Errors
    ///
    /// If this option is set and more than one instance of the server for a
    /// given named pipe exists, calling [`create`] will fail with
    /// [`std::io::ErrorKind::PermissionDenied`].
    ///
    /// ```
    /// use std::io;
    /// use tokio::net::windows::named_pipe::ServerOptions;
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\tokio-named-pipe-first-instance-error";
    ///
    /// # #[tokio::main] async fn main() -> io::Result<()> {
    /// let server1 = ServerOptions::new()
    ///     .first_pipe_instance(true)
    ///     .create(PIPE_NAME)?;
    ///
    /// // Second server errs, since it's not the first instance.
    /// let e = ServerOptions::new()
    ///     .first_pipe_instance(true)
    ///     .create(PIPE_NAME)
    ///     .unwrap_err();
    ///
    /// assert_eq!(e.kind(), io::ErrorKind::PermissionDenied);
    /// # Ok(()) }
    /// ```
    ///
    /// # Examples
    ///
    /// ```
    /// use std::io;
    /// use tokio::net::windows::named_pipe::ServerOptions;
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\tokio-named-pipe-first-instance";
    ///
    /// # #[tokio::main] async fn main() -> io::Result<()> {
    /// let mut builder = ServerOptions::new();
    /// builder.first_pipe_instance(true);
    ///
    /// let server = builder.create(PIPE_NAME)?;
    /// let e = builder.create(PIPE_NAME).unwrap_err();
    /// assert_eq!(e.kind(), io::ErrorKind::PermissionDenied);
    /// drop(server);
    ///
    /// // OK: since, we've closed the other instance.
    /// let _server2 = builder.create(PIPE_NAME)?;
    /// # Ok(()) }
    /// ```
    ///
    /// [`create`]: ServerOptions::create
    /// [`FILE_FLAG_FIRST_PIPE_INSTANCE`]: https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createnamedpipea#pipe_first_pipe_instance
    pub fn first_pipe_instance(&mut self, first: bool) -> &mut Self {
        self.first_pipe_instance = first;
        self
    }

    /// Requests permission to modify the pipe's discretionary access control list.
    ///
    /// This corresponds to setting [`WRITE_DAC`] in dwOpenMode.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::{io, os::windows::prelude::AsRawHandle, ptr};
    ///
    /// use tokio::net::windows::named_pipe::ServerOptions;
    /// use windows_sys::{
    ///     Win32::Foundation::ERROR_SUCCESS,
    ///     Win32::Security::DACL_SECURITY_INFORMATION,
    ///     Win32::Security::Authorization::{SetSecurityInfo, SE_KERNEL_OBJECT},
    /// };
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\write_dac_pipe";
    ///
    /// # #[tokio::main] async fn main() -> io::Result<()> {
    /// let mut pipe_template = ServerOptions::new();
    /// pipe_template.write_dac(true);
    /// let pipe = pipe_template.create(PIPE_NAME)?;
    ///
    /// unsafe {
    ///     assert_eq!(
    ///         ERROR_SUCCESS,
    ///         SetSecurityInfo(
    ///             pipe.as_raw_handle() as _,
    ///             SE_KERNEL_OBJECT,
    ///             DACL_SECURITY_INFORMATION,
    ///             ptr::null_mut(),
    ///             ptr::null_mut(),
    ///             ptr::null_mut(),
    ///             ptr::null_mut(),
    ///         )
    ///     );
    /// }
    ///
    /// # Ok(()) }
    /// ```
    ///
    /// ```
    /// use std::{io, os::windows::prelude::AsRawHandle, ptr};
    ///
    /// use tokio::net::windows::named_pipe::ServerOptions;
    /// use windows_sys::{
    ///     Win32::Foundation::ERROR_ACCESS_DENIED,
    ///     Win32::Security::DACL_SECURITY_INFORMATION,
    ///     Win32::Security::Authorization::{SetSecurityInfo, SE_KERNEL_OBJECT},
    /// };
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\write_dac_pipe_fail";
    ///
    /// # #[tokio::main] async fn main() -> io::Result<()> {
    /// let mut pipe_template = ServerOptions::new();
    /// pipe_template.write_dac(false);
    /// let pipe = pipe_template.create(PIPE_NAME)?;
    ///
    /// unsafe {
    ///     assert_eq!(
    ///         ERROR_ACCESS_DENIED,
    ///         SetSecurityInfo(
    ///             pipe.as_raw_handle() as _,
    ///             SE_KERNEL_OBJECT,
    ///             DACL_SECURITY_INFORMATION,
    ///             ptr::null_mut(),
    ///             ptr::null_mut(),
    ///             ptr::null_mut(),
    ///             ptr::null_mut(),
    ///         )
    ///     );
    /// }
    ///
    /// # Ok(()) }
    /// ```
    ///
    /// [`WRITE_DAC`]: https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createnamedpipea
    pub fn write_dac(&mut self, requested: bool) -> &mut Self {
        self.write_dac = requested;
        self
    }

    /// Requests permission to modify the pipe's owner.
    ///
    /// This corresponds to setting [`WRITE_OWNER`] in dwOpenMode.
    ///
    /// [`WRITE_OWNER`]: https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createnamedpipea
    pub fn write_owner(&mut self, requested: bool) -> &mut Self {
        self.write_owner = requested;
        self
    }

    /// Requests permission to modify the pipe's system access control list.
    ///
    /// This corresponds to setting [`ACCESS_SYSTEM_SECURITY`] in dwOpenMode.
    ///
    /// [`ACCESS_SYSTEM_SECURITY`]: https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createnamedpipea
    pub fn access_system_security(&mut self, requested: bool) -> &mut Self {
        self.access_system_security = requested;
        self
    }

    /// Indicates whether this server can accept remote clients or not. Remote
    /// clients are disabled by default.
    ///
    /// This corresponds to setting [`PIPE_REJECT_REMOTE_CLIENTS`].
    ///
    /// [`PIPE_REJECT_REMOTE_CLIENTS`]: https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createnamedpipea#pipe_reject_remote_clients
    pub fn reject_remote_clients(&mut self, reject: bool) -> &mut Self {
        self.reject_remote_clients = reject;
        self
    }

    /// The maximum number of instances that can be created for this pipe. The
    /// first instance of the pipe can specify this value; the same number must
    /// be specified for other instances of the pipe. Acceptable values are in
    /// the range 1 through 254. The default value is unlimited.
    ///
    /// This corresponds to specifying [`nMaxInstances`].
    ///
    /// [`nMaxInstances`]: https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createnamedpipea
    ///
    /// # Errors
    ///
    /// The same numbers of `max_instances` have to be used by all servers. Any
    /// additional servers trying to be built which uses a mismatching value
    /// might error.
    ///
    /// ```
    /// use std::io;
    /// use tokio::net::windows::named_pipe::{ServerOptions, ClientOptions};
    /// use windows_sys::Win32::Foundation::ERROR_PIPE_BUSY;
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\tokio-named-pipe-max-instances";
    ///
    /// # #[tokio::main] async fn main() -> io::Result<()> {
    /// let mut server = ServerOptions::new();
    /// server.max_instances(2);
    ///
    /// let s1 = server.create(PIPE_NAME)?;
    /// let c1 = ClientOptions::new().open(PIPE_NAME);
    ///
    /// let s2 = server.create(PIPE_NAME)?;
    /// let c2 = ClientOptions::new().open(PIPE_NAME);
    ///
    /// // Too many servers!
    /// let e = server.create(PIPE_NAME).unwrap_err();
    /// assert_eq!(e.raw_os_error(), Some(ERROR_PIPE_BUSY as i32));
    ///
    /// // Still too many servers even if we specify a higher value!
    /// let e = server.max_instances(100).create(PIPE_NAME).unwrap_err();
    /// assert_eq!(e.raw_os_error(), Some(ERROR_PIPE_BUSY as i32));
    /// # Ok(()) }
    /// ```
    ///
    /// # Panics
    ///
    /// This function will panic if more than 254 instances are specified. If
    /// you do not wish to set an instance limit, leave it unspecified.
    ///
    /// ```should_panic
    /// use tokio::net::windows::named_pipe::ServerOptions;
    ///
    /// # #[tokio::main] async fn main() -> std::io::Result<()> {
    /// let builder = ServerOptions::new().max_instances(255);
    /// # Ok(()) }
    /// ```
    #[track_caller]
    pub fn max_instances(&mut self, instances: usize) -> &mut Self {
        assert!(instances < 255, "cannot specify more than 254 instances");
        self.max_instances = instances as u32;
        self
    }

    /// The number of bytes to reserve for the output buffer.
    ///
    /// This corresponds to specifying [`nOutBufferSize`].
    ///
    /// [`nOutBufferSize`]: https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createnamedpipea
    pub fn out_buffer_size(&mut self, buffer: u32) -> &mut Self {
        self.out_buffer_size = buffer;
        self
    }

    /// The number of bytes to reserve for the input buffer.
    ///
    /// This corresponds to specifying [`nInBufferSize`].
    ///
    /// [`nInBufferSize`]: https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createnamedpipea
    pub fn in_buffer_size(&mut self, buffer: u32) -> &mut Self {
        self.in_buffer_size = buffer;
        self
    }

    /// Creates the named pipe identified by `addr` for use as a server.
    ///
    /// This uses the [`CreateNamedPipe`] function.
    ///
    /// [`CreateNamedPipe`]: https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createnamedpipea
    ///
    /// # Errors
    ///
    /// This errors if called outside of a [Tokio Runtime], or in a runtime that
    /// has not [enabled I/O], or if any OS-specific I/O errors occur.
    ///
    /// [Tokio Runtime]: crate::runtime::Runtime
    /// [enabled I/O]: crate::runtime::Builder::enable_io
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::net::windows::named_pipe::ServerOptions;
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\tokio-named-pipe-create";
    ///
    /// # #[tokio::main] async fn main() -> std::io::Result<()> {
    /// let server = ServerOptions::new().create(PIPE_NAME)?;
    /// # Ok(()) }
    /// ```
    pub fn create(&self, addr: impl AsRef<OsStr>) -> io::Result<NamedPipeServer> {
        // Safety: We're calling create_with_security_attributes_raw w/ a null
        // pointer which disables it.
        unsafe { self.create_with_security_attributes_raw(addr, ptr::null_mut()) }
    }

    /// Creates the named pipe identified by `addr` for use as a server.
    ///
    /// This is the same as [`create`] except that it supports providing the raw
    /// pointer to a structure of [`SECURITY_ATTRIBUTES`] which will be passed
    /// as the `lpSecurityAttributes` argument to [`CreateFile`].
    ///
    /// # Errors
    ///
    /// This errors if called outside of a [Tokio Runtime], or in a runtime that
    /// has not [enabled I/O], or if any OS-specific I/O errors occur.
    ///
    /// [Tokio Runtime]: crate::runtime::Runtime
    /// [enabled I/O]: crate::runtime::Builder::enable_io
    ///
    /// # Safety
    ///
    /// The `attrs` argument must either be null or point at a valid instance of
    /// the [`SECURITY_ATTRIBUTES`] structure. If the argument is null, the
    /// behavior is identical to calling the [`create`] method.
    ///
    /// [`create`]: ServerOptions::create
    /// [`CreateFile`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew
    /// [`SECURITY_ATTRIBUTES`]: https://docs.rs/windows-sys/latest/windows_sys/Win32/Security/struct.SECURITY_ATTRIBUTES.html
    pub unsafe fn create_with_security_attributes_raw(
        &self,
        addr: impl AsRef<OsStr>,
        attrs: *mut c_void,
    ) -> io::Result<NamedPipeServer> {
        let addr = encode_addr(addr);

        let pipe_mode = {
            let mut mode = if matches!(self.pipe_mode, PipeMode::Message) {
                windows_sys::PIPE_TYPE_MESSAGE | windows_sys::PIPE_READMODE_MESSAGE
            } else {
                windows_sys::PIPE_TYPE_BYTE | windows_sys::PIPE_READMODE_BYTE
            };
            if self.reject_remote_clients {
                mode |= windows_sys::PIPE_REJECT_REMOTE_CLIENTS;
            } else {
                mode |= windows_sys::PIPE_ACCEPT_REMOTE_CLIENTS;
            }
            mode
        };
        let open_mode = {
            let mut mode = windows_sys::FILE_FLAG_OVERLAPPED;
            if self.access_inbound {
                mode |= windows_sys::PIPE_ACCESS_INBOUND;
            }
            if self.access_outbound {
                mode |= windows_sys::PIPE_ACCESS_OUTBOUND;
            }
            if self.first_pipe_instance {
                mode |= windows_sys::FILE_FLAG_FIRST_PIPE_INSTANCE;
            }
            if self.write_dac {
                mode |= windows_sys::WRITE_DAC;
            }
            if self.write_owner {
                mode |= windows_sys::WRITE_OWNER;
            }
            if self.access_system_security {
                mode |= windows_sys::ACCESS_SYSTEM_SECURITY;
            }
            mode
        };

        let h = windows_sys::CreateNamedPipeW(
            addr.as_ptr(),
            open_mode,
            pipe_mode,
            self.max_instances,
            self.out_buffer_size,
            self.in_buffer_size,
            self.default_timeout,
            attrs as *mut _,
        );

        if h == windows_sys::INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }

        NamedPipeServer::from_raw_handle(h as _)
    }
}

/// A builder suitable for building and interacting with named pipes from the
/// client side.
///
/// See [`ClientOptions::open`].
#[derive(Debug, Clone)]
pub struct ClientOptions {
    generic_read: bool,
    generic_write: bool,
    security_qos_flags: u32,
    pipe_mode: PipeMode,
}

impl ClientOptions {
    /// Creates a new named pipe builder with the default settings.
    ///
    /// ```
    /// use tokio::net::windows::named_pipe::{ServerOptions, ClientOptions};
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\tokio-named-pipe-client-new";
    ///
    /// # #[tokio::main] async fn main() -> std::io::Result<()> {
    /// // Server must be created in order for the client creation to succeed.
    /// let server = ServerOptions::new().create(PIPE_NAME)?;
    /// let client = ClientOptions::new().open(PIPE_NAME)?;
    /// # Ok(()) }
    /// ```
    pub fn new() -> Self {
        Self {
            generic_read: true,
            generic_write: true,
            security_qos_flags: windows_sys::SECURITY_IDENTIFICATION
                | windows_sys::SECURITY_SQOS_PRESENT,
            pipe_mode: PipeMode::Byte,
        }
    }

    /// If the client supports reading data. This is enabled by default.
    ///
    /// This corresponds to setting [`GENERIC_READ`] in the call to [`CreateFile`].
    ///
    /// [`GENERIC_READ`]: https://docs.microsoft.com/en-us/windows/win32/secauthz/generic-access-rights
    /// [`CreateFile`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew
    pub fn read(&mut self, allowed: bool) -> &mut Self {
        self.generic_read = allowed;
        self
    }

    /// If the created pipe supports writing data. This is enabled by default.
    ///
    /// This corresponds to setting [`GENERIC_WRITE`] in the call to [`CreateFile`].
    ///
    /// [`GENERIC_WRITE`]: https://docs.microsoft.com/en-us/windows/win32/secauthz/generic-access-rights
    /// [`CreateFile`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew
    pub fn write(&mut self, allowed: bool) -> &mut Self {
        self.generic_write = allowed;
        self
    }

    /// Sets qos flags which are combined with other flags and attributes in the
    /// call to [`CreateFile`].
    ///
    /// By default `security_qos_flags` is set to [`SECURITY_IDENTIFICATION`],
    /// calling this function would override that value completely with the
    /// argument specified.
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
    /// [`CreateFile`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilea
    /// [`SECURITY_IDENTIFICATION`]: https://docs.rs/windows-sys/latest/windows_sys/Win32/Storage/FileSystem/constant.SECURITY_IDENTIFICATION.html
    /// [Impersonation Levels]: https://docs.microsoft.com/en-us/windows/win32/api/winnt/ne-winnt-security_impersonation_level
    pub fn security_qos_flags(&mut self, flags: u32) -> &mut Self {
        // See: https://github.com/rust-lang/rust/pull/58216
        self.security_qos_flags = flags | windows_sys::SECURITY_SQOS_PRESENT;
        self
    }

    /// The pipe mode.
    ///
    /// The default pipe mode is [`PipeMode::Byte`]. See [`PipeMode`] for
    /// documentation of what each mode means.
    pub fn pipe_mode(&mut self, pipe_mode: PipeMode) -> &mut Self {
        self.pipe_mode = pipe_mode;
        self
    }

    /// Opens the named pipe identified by `addr`.
    ///
    /// This opens the client using [`CreateFile`] with the
    /// `dwCreationDisposition` option set to `OPEN_EXISTING`.
    ///
    /// [`CreateFile`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilea
    ///
    /// # Errors
    ///
    /// This errors if called outside of a [Tokio Runtime], or in a runtime that
    /// has not [enabled I/O], or if any OS-specific I/O errors occur.
    ///
    /// There are a few errors you need to take into account when creating a
    /// named pipe on the client side:
    ///
    /// * [`std::io::ErrorKind::NotFound`] - This indicates that the named pipe
    ///   does not exist. Presumably the server is not up.
    /// * [`ERROR_PIPE_BUSY`] - This error is raised when the named pipe exists,
    ///   but the server is not currently waiting for a connection. Please see the
    ///   examples for how to check for this error.
    ///
    /// [`ERROR_PIPE_BUSY`]: https://docs.rs/windows-sys/latest/windows_sys/Win32/Foundation/constant.ERROR_PIPE_BUSY.html
    /// [enabled I/O]: crate::runtime::Builder::enable_io
    /// [Tokio Runtime]: crate::runtime::Runtime
    ///
    /// A connect loop that waits until a pipe becomes available looks like
    /// this:
    ///
    /// ```no_run
    /// use std::time::Duration;
    /// use tokio::net::windows::named_pipe::ClientOptions;
    /// use tokio::time;
    /// use windows_sys::Win32::Foundation::ERROR_PIPE_BUSY;
    ///
    /// const PIPE_NAME: &str = r"\\.\pipe\mynamedpipe";
    ///
    /// # #[tokio::main] async fn main() -> std::io::Result<()> {
    /// let client = loop {
    ///     match ClientOptions::new().open(PIPE_NAME) {
    ///         Ok(client) => break client,
    ///         Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) => (),
    ///         Err(e) => return Err(e),
    ///     }
    ///
    ///     time::sleep(Duration::from_millis(50)).await;
    /// };
    ///
    /// // use the connected client.
    /// # Ok(()) }
    /// ```
    pub fn open(&self, addr: impl AsRef<OsStr>) -> io::Result<NamedPipeClient> {
        // Safety: We're calling open_with_security_attributes_raw w/ a null
        // pointer which disables it.
        unsafe { self.open_with_security_attributes_raw(addr, ptr::null_mut()) }
    }

    /// Opens the named pipe identified by `addr`.
    ///
    /// This is the same as [`open`] except that it supports providing the raw
    /// pointer to a structure of [`SECURITY_ATTRIBUTES`] which will be passed
    /// as the `lpSecurityAttributes` argument to [`CreateFile`].
    ///
    /// # Safety
    ///
    /// The `attrs` argument must either be null or point at a valid instance of
    /// the [`SECURITY_ATTRIBUTES`] structure. If the argument is null, the
    /// behavior is identical to calling the [`open`] method.
    ///
    /// [`open`]: ClientOptions::open
    /// [`CreateFile`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew
    /// [`SECURITY_ATTRIBUTES`]: https://docs.rs/windows-sys/latest/windows_sys/Win32/Security/struct.SECURITY_ATTRIBUTES.html
    pub unsafe fn open_with_security_attributes_raw(
        &self,
        addr: impl AsRef<OsStr>,
        attrs: *mut c_void,
    ) -> io::Result<NamedPipeClient> {
        let addr = encode_addr(addr);

        let desired_access = {
            let mut access = 0;
            if self.generic_read {
                access |= windows_sys::GENERIC_READ;
            }
            if self.generic_write {
                access |= windows_sys::GENERIC_WRITE;
            }
            access
        };

        // NB: We could use a platform specialized `OpenOptions` here, but since
        // we have access to windows_sys it ultimately doesn't hurt to use
        // `CreateFile` explicitly since it allows the use of our already
        // well-structured wide `addr` to pass into CreateFileW.
        let h = windows_sys::CreateFileW(
            addr.as_ptr(),
            desired_access,
            0,
            attrs as *mut _,
            windows_sys::OPEN_EXISTING,
            self.get_flags(),
            null_mut(),
        );

        if h == windows_sys::INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }

        if matches!(self.pipe_mode, PipeMode::Message) {
            let mode = windows_sys::PIPE_READMODE_MESSAGE;
            let result =
                windows_sys::SetNamedPipeHandleState(h, &mode, ptr::null_mut(), ptr::null_mut());

            if result == 0 {
                return Err(io::Error::last_os_error());
            }
        }

        NamedPipeClient::from_raw_handle(h as _)
    }

    fn get_flags(&self) -> u32 {
        self.security_qos_flags | windows_sys::FILE_FLAG_OVERLAPPED
    }
}

/// The pipe mode of a named pipe.
///
/// Set through [`ServerOptions::pipe_mode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PipeMode {
    /// Data is written to the pipe as a stream of bytes. The pipe does not
    /// distinguish bytes written during different write operations.
    ///
    /// Corresponds to [`PIPE_TYPE_BYTE`].
    ///
    /// [`PIPE_TYPE_BYTE`]: https://docs.rs/windows-sys/latest/windows_sys/Win32/System/Pipes/constant.PIPE_TYPE_BYTE.html
    Byte,
    /// Data is written to the pipe as a stream of messages. The pipe treats the
    /// bytes written during each write operation as a message unit. Any reading
    /// on a named pipe returns [`ERROR_MORE_DATA`] when a message is not read
    /// completely.
    ///
    /// Corresponds to [`PIPE_TYPE_MESSAGE`].
    ///
    /// [`ERROR_MORE_DATA`]: https://docs.rs/windows-sys/latest/windows_sys/Win32/Foundation/constant.ERROR_MORE_DATA.html
    /// [`PIPE_TYPE_MESSAGE`]: https://docs.rs/windows-sys/latest/windows_sys/Win32/System/Pipes/constant.PIPE_TYPE_MESSAGE.html
    Message,
}

/// Indicates the end of a named pipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PipeEnd {
    /// The named pipe refers to the client end of a named pipe instance.
    ///
    /// Corresponds to [`PIPE_CLIENT_END`].
    ///
    /// [`PIPE_CLIENT_END`]: https://docs.rs/windows-sys/latest/windows_sys/Win32/System/Pipes/constant.PIPE_CLIENT_END.html
    Client,
    /// The named pipe refers to the server end of a named pipe instance.
    ///
    /// Corresponds to [`PIPE_SERVER_END`].
    ///
    /// [`PIPE_SERVER_END`]: https://docs.rs/windows-sys/latest/windows_sys/Win32/System/Pipes/constant.PIPE_SERVER_END.html
    Server,
}

/// Information about a named pipe.
///
/// Constructed through [`NamedPipeServer::info`] or [`NamedPipeClient::info`].
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct PipeInfo {
    /// Indicates the mode of a named pipe.
    pub mode: PipeMode,
    /// Indicates the end of a named pipe.
    pub end: PipeEnd,
    /// The maximum number of instances that can be created for this pipe.
    pub max_instances: u32,
    /// The number of bytes to reserve for the output buffer.
    pub out_buffer_size: u32,
    /// The number of bytes to reserve for the input buffer.
    pub in_buffer_size: u32,
}

/// Encodes an address so that it is a null-terminated wide string.
fn encode_addr(addr: impl AsRef<OsStr>) -> Box<[u16]> {
    let len = addr.as_ref().encode_wide().count();
    let mut vec = Vec::with_capacity(len + 1);
    vec.extend(addr.as_ref().encode_wide());
    vec.push(0);
    vec.into_boxed_slice()
}

/// Internal function to get the info out of a raw named pipe.
unsafe fn named_pipe_info(handle: RawHandle) -> io::Result<PipeInfo> {
    let mut flags = 0;
    let mut out_buffer_size = 0;
    let mut in_buffer_size = 0;
    let mut max_instances = 0;

    let result = windows_sys::GetNamedPipeInfo(
        handle as _,
        &mut flags,
        &mut out_buffer_size,
        &mut in_buffer_size,
        &mut max_instances,
    );

    if result == 0 {
        return Err(io::Error::last_os_error());
    }

    let mut end = PipeEnd::Client;
    let mut mode = PipeMode::Byte;

    if flags & windows_sys::PIPE_SERVER_END != 0 {
        end = PipeEnd::Server;
    }

    if flags & windows_sys::PIPE_TYPE_MESSAGE != 0 {
        mode = PipeMode::Message;
    }

    Ok(PipeInfo {
        end,
        mode,
        out_buffer_size,
        in_buffer_size,
        max_instances,
    })
}
