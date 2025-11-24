//! A bounded single-producer single-consumer pipe.
//!
//! This crate provides a ring buffer that can be asynchronously read from and written to. It is
//! created via the [`pipe`] function, which returns a pair of [`Reader`] and [`Writer`] handles.
//! They implement the [`AsyncRead`] and [`AsyncWrite`] traits, respectively.
//!
//! The handles are single-producer/single-consumer; to clarify, they cannot be cloned and need `&mut`
//! access to read or write to them. If multiple-producer/multiple-consumer handles are needed,
//! consider wrapping them in an `Arc<Mutex<...>>` or similar.
//!
//! When the sender is dropped, remaining bytes in the pipe can still be read. After that, attempts
//! to read will result in `Ok(0)`, i.e. they will always 'successfully' read 0 bytes.
//!
//! When the receiver is dropped, the pipe is closed and no more bytes and be written into it.
//! Further writes will result in `Ok(0)`, i.e. they will always 'successfully' write 0 bytes.
//!
//! # Version 0.2.0 Notes
//!
//! Previously, this crate contained other synchronization primitives, such as bounded channels, locks,
//! and event listeners. These have been split out into their own crates:
//!
//! - [`async-channel`](https://docs.rs/async-channel)
//! - [`async-dup`](https://docs.rs/async-dup)
//! - [`async-lock`](https://docs.rs/async-lock)
//! - [`async-mutex`](https://docs.rs/async-mutex)
//! - [`event-listener`](https://docs.rs/event-listener)
//!
//! # Examples
//!
//! ## Asynchronous Tasks
//!
//! Communicate between asynchronous tasks, potentially on other threads.
//!
//! ```
//! use async_channel::unbounded;
//! use async_executor::Executor;
//! use easy_parallel::Parallel;
//! use futures_lite::{future, prelude::*};
//! use std::time::Duration;
//!
//! # if cfg!(miri) { return; }
//!
//! // Create a pair of handles.
//! let (mut reader, mut writer) = piper::pipe(1024);
//!
//! // Create the executor.
//! let ex = Executor::new();
//! let (signal, shutdown) = unbounded::<()>();
//!
//! // Spawn a detached task for random data to the pipe.
//! let writer = ex.spawn(async move {
//!     for _ in 0..1_000 {
//!         // Generate 8 random numnbers.
//!         let random = fastrand::u64(..).to_le_bytes();
//!
//!         // Write them to the pipe.
//!         writer.write_all(&random).await.unwrap();
//!
//!         // Wait a bit.
//!         async_io::Timer::after(Duration::from_millis(5)).await;
//!     }
//!
//!     // Drop the writer to close the pipe.
//!     drop(writer);
//! });
//!
//! // Detach the task so that it runs in the background.
//! writer.detach();
//!
//! // Spawn a task for reading from the pipe.
//! let reader = ex.spawn(async move {
//!     let mut buf = vec![];
//!
//!     // Read all bytes from the pipe.
//!     reader.read_to_end(&mut buf).await.unwrap();
//!
//!     println!("Random data: {:#?}", buf);
//! });
//!
//! Parallel::new()
//!     // Run four executor threads.
//!     .each(0..4, |_| future::block_on(ex.run(shutdown.recv())))
//!     // Run the main future on the current thread.
//!     .finish(|| future::block_on(async {
//!         // Wait for the reader to finish.
//!         reader.await;
//!
//!         // Signal the executor threads to shut down.
//!         drop(signal);
//!     }));
//! ```
//!
//! ## Blocking I/O
//!
//! File I/O is blocking; therefore, in `async` code, you must run it on another thread. This example
//! spawns another thread for reading a file and writing it to a pipe.
//!
//! ```no_run
//! use futures_lite::{future, prelude::*};
//! use std::fs::File;
//! use std::io::prelude::*;
//! use std::thread;
//!
//! // Create a pair of handles.
//! let (mut r, mut w) = piper::pipe(1024);
//!
//! // Spawn a thread for reading a file.
//! thread::spawn(move || {
//!     let mut file = File::open("Cargo.toml").unwrap();
//!
//!     // Read the file into a buffer.
//!     let mut buf = [0u8; 16384];
//!     future::block_on(async move {
//!         loop {
//!             // Read a chunk of bytes from the file.
//!             // Blocking is okay here, since this is a separate thread.
//!             let n = file.read(&mut buf).unwrap();
//!             if n == 0 {
//!                 break;
//!             }
//!
//!             // Write the chunk to the pipe.
//!             w.write_all(&buf[..n]).await.unwrap();
//!         }
//!
//!         // Close the pipe.
//!         drop(w);
//!     });
//! });
//!
//! # future::block_on(async move {
//! // Read bytes from the pipe.
//! let mut buf = vec![];
//! r.read_to_end(&mut buf).await.unwrap();
//!
//! println!("Read {} bytes", buf.len());
//! # });
//! ```
//!
//! However, the lower-level [`poll_fill`] and [`poll_drain`] methods take `impl Read` and `impl Write`
//! arguments, respectively. This allows you to skip the buffer entirely and read/write directly from
//! the file into the pipe. This approach should be preferred when possible, as it avoids an extra
//! copy.
//!
//! ```no_run
//! # use futures_lite::future;
//! # use std::fs::File;
//! # let mut file: File = unimplemented!();
//! # let mut w: piper::Writer = unimplemented!();
//! // In the `future::block_on` call above...
//! # future::block_on(async move {
//! loop {
//!     let n = future::poll_fn(|cx| w.poll_fill(cx, &mut file)).await.unwrap();
//!     if n == 0 {
//!         break;
//!     }
//! }
//! # });
//! ```
//!
//! The [`blocking`] crate is preferred in this use case, since it uses more efficient strategies for
//! thread management and pipes.
//!
//! [`poll_fill`]: struct.Writer.html#method.poll_fill
//! [`poll_drain`]: struct.Reader.html#method.poll_drain
//! [`blocking`]: https://docs.rs/blocking

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(missing_docs)]
#![doc(
    html_favicon_url = "https://raw.githubusercontent.com/smol-rs/smol/master/assets/images/logo_fullsize_transparent.png"
)]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/smol-rs/smol/master/assets/images/logo_fullsize_transparent.png"
)]

extern crate alloc;

use core::convert::Infallible;
use core::mem;
use core::slice;
use core::task::{Context, Poll};

use alloc::vec::Vec;

use sync::atomic::{self, AtomicBool, AtomicUsize, Ordering};
use sync::Arc;

#[cfg(feature = "std")]
use std::{
    io::{self, Read, Write},
    pin::Pin,
};

use atomic_waker::AtomicWaker;

#[cfg(feature = "std")]
use futures_io::{AsyncRead, AsyncWrite};

macro_rules! ready {
    ($e:expr) => {{
        match $e {
            Poll::Ready(t) => t,
            Poll::Pending => return Poll::Pending,
        }
    }};
}

/// Creates a bounded single-producer single-consumer pipe.
///
/// A pipe is a ring buffer of `cap` bytes that can be asynchronously read from and written to.
///
/// See the [crate-level documentation](index.html) for more details.
///
/// # Panics
///
/// This function panics if `cap` is 0 or if `cap * 2` overflows a `usize`.
#[allow(clippy::incompatible_msrv)] // false positive: https://github.com/rust-lang/rust-clippy/issues/12280
pub fn pipe(cap: usize) -> (Reader, Writer) {
    assert!(cap > 0, "capacity must be positive");
    assert!(cap.checked_mul(2).is_some(), "capacity is too large");

    // Allocate the ring buffer.
    let mut v = Vec::with_capacity(cap);
    let buffer = v.as_mut_ptr();
    mem::forget(v);

    let inner = Arc::new(Pipe {
        head: AtomicUsize::new(0),
        tail: AtomicUsize::new(0),
        reader: AtomicWaker::new(),
        writer: AtomicWaker::new(),
        closed: AtomicBool::new(false),
        buffer,
        cap,
    });

    // Use a random number generator to randomize fair yielding behavior.
    let mut rng = rng();

    let r = Reader {
        inner: inner.clone(),
        head: 0,
        tail: 0,
        rng: rng.fork(),
    };

    let w = Writer {
        inner,
        head: 0,
        tail: 0,
        zeroed_until: 0,
        rng,
    };

    (r, w)
}

/// The reading side of a pipe.
///
/// This type is created by the [`pipe`] function. See its documentation for more details.
pub struct Reader {
    /// The inner ring buffer.
    inner: Arc<Pipe>,

    /// The head index, moved by the reader, in the range `0..2*cap`.
    ///
    /// This index always matches `inner.head`.
    head: usize,

    /// The tail index, moved by the writer, in the range `0..2*cap`.
    ///
    /// This index is a snapshot of `index.tail` that might become stale at any point.
    tail: usize,

    /// Random number generator.
    rng: fastrand::Rng,
}

/// The writing side of a pipe.
///
/// This type is created by the [`pipe`] function. See its documentation for more details.
pub struct Writer {
    /// The inner ring buffer.
    inner: Arc<Pipe>,

    /// The head index, moved by the reader, in the range `0..2*cap`.
    ///
    /// This index is a snapshot of `index.head` that might become stale at any point.
    head: usize,

    /// The tail index, moved by the writer, in the range `0..2*cap`.
    ///
    /// This index always matches `inner.tail`.
    tail: usize,

    /// How many bytes at the beginning of the buffer have been zeroed.
    ///
    /// The pipe allocates an uninitialized buffer, and we must be careful about passing
    /// uninitialized data to user code. Zeroing the buffer right after allocation would be too
    /// expensive, so we zero it in smaller chunks as the writer makes progress.
    zeroed_until: usize,

    /// Random number generator.
    rng: fastrand::Rng,
}

/// The inner ring buffer.
///
/// Head and tail indices are in the range `0..2*cap`, even though they really map onto the
/// `0..cap` range. The distance between head and tail indices is never more than `cap`.
///
/// The reason why indices are not in the range `0..cap` is because we need to distinguish between
/// the pipe being empty and being full. If head and tail were in `0..cap`, then `head == tail`
/// could mean the pipe is either empty or full, but we don't know which!
struct Pipe {
    /// The head index, moved by the reader, in the range `0..2*cap`.
    head: AtomicUsize,

    /// The tail index, moved by the writer, in the range `0..2*cap`.
    tail: AtomicUsize,

    /// A waker representing the blocked reader.
    reader: AtomicWaker,

    /// A waker representing the blocked writer.
    writer: AtomicWaker,

    /// Set to `true` if the reader or writer was dropped.
    closed: AtomicBool,

    /// The byte buffer.
    buffer: *mut u8,

    /// The buffer capacity.
    cap: usize,
}

unsafe impl Sync for Pipe {}
unsafe impl Send for Pipe {}

impl Drop for Pipe {
    fn drop(&mut self) {
        // Deallocate the byte buffer.
        unsafe {
            Vec::from_raw_parts(self.buffer, 0, self.cap);
        }
    }
}

impl Drop for Reader {
    fn drop(&mut self) {
        // Dropping closes the pipe and then wakes the writer.
        self.inner.closed.store(true, Ordering::SeqCst);
        self.inner.writer.wake();
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        // Dropping closes the pipe and then wakes the reader.
        self.inner.closed.store(true, Ordering::SeqCst);
        self.inner.reader.wake();
    }
}

impl Pipe {
    /// Get the length of the data in the pipe.
    fn len(&self) -> usize {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);

        if head <= tail {
            tail - head
        } else {
            (2 * self.cap) - (head - tail)
        }
    }
}

impl Reader {
    /// Gets the total length of the data in the pipe.
    ///
    /// This method returns the number of bytes that have been written into the pipe but haven't been
    /// read yet.
    ///
    /// # Examples
    ///
    /// ```
    /// let (mut reader, mut writer) = piper::pipe(10);
    /// let _ = writer.try_fill(&[0u8; 5]);
    /// assert_eq!(reader.len(), 5);
    /// ```
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Tell whether or not the pipe is empty.
    ///
    /// This method returns `true` if the pipe is empty, and `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// let (mut reader, mut writer) = piper::pipe(10);
    /// assert!(reader.is_empty());
    /// let _ = writer.try_fill(&[0u8; 5]);
    /// assert!(!reader.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.inner.len() == 0
    }

    /// Gets the total capacity of the pipe.
    ///
    /// This method returns the number of bytes that the pipe can hold at a time.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// let (reader, _) = piper::pipe(10);
    /// assert_eq!(reader.capacity(), 10);
    /// # });
    /// ```
    pub fn capacity(&self) -> usize {
        self.inner.cap
    }

    /// Tell whether or not the pipe is full.
    ///
    /// The pipe is full if the number of bytes written into it is equal to its capacity. At this point,
    /// writes will block until some data is read from the pipe.
    ///
    /// This method returns `true` if the pipe is full, and `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// let (mut reader, mut writer) = piper::pipe(10);
    /// assert!(!reader.is_full());
    /// let _ = writer.try_fill(&[0u8; 10]);
    /// assert!(reader.is_full());
    /// let _ = reader.try_drain(&mut [0u8; 5]);
    /// assert!(!reader.is_full());
    /// ```
    pub fn is_full(&self) -> bool {
        self.inner.len() == self.inner.cap
    }

    /// Tell whether or not the pipe is closed.
    ///
    /// The pipe is closed if either the reader or the writer has been dropped. At this point, attempting
    /// to write into the pipe will return `Poll::Ready(Ok(0))` and attempting to read from the pipe after
    /// any previously written bytes are read will return `Poll::Ready(Ok(0))`.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// let (mut reader, mut writer) = piper::pipe(10);
    /// assert!(!reader.is_closed());
    /// drop(writer);
    /// assert!(reader.is_closed());
    /// # });
    /// ```
    pub fn is_closed(&self) -> bool {
        self.inner.closed.load(Ordering::SeqCst)
    }

    /// Reads bytes from this reader and writes into blocking `dest`.
    ///
    /// This method reads directly from the pipe's internal buffer into `dest`. This avoids an extra copy,
    /// but it may block the thread if `dest` blocks.
    ///
    /// If the pipe is empty, this method returns `Poll::Pending`. If the pipe is closed, this method
    /// returns `Poll::Ready(Ok(0))`. Errors in `dest` are bubbled up through `Poll::Ready(Err(e))`.
    /// Otherwise, this method returns `Poll::Ready(Ok(n))` where `n` is the number of bytes written.
    ///
    /// This method is only available when the `std` feature is enabled. For `no_std` environments,
    /// consider using [`poll_drain_bytes`] instead.
    ///
    /// [`poll_drain_bytes`]: #method.poll_drain_bytes
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::{future, prelude::*};
    /// # future::block_on(async {
    ///
    /// let (mut r, mut w) = piper::pipe(1024);
    ///
    /// // Write some data to the pipe.
    /// w.write_all(b"hello world").await.unwrap();
    ///
    /// // Try reading from the pipe.
    /// let mut buf = [0; 1024];
    /// let n = future::poll_fn(|cx| r.poll_drain(cx, &mut buf[..])).await.unwrap();
    ///
    /// // The data was written to the buffer.
    /// assert_eq!(&buf[..n], b"hello world");
    /// # });
    /// ```
    #[cfg(feature = "std")]
    pub fn poll_drain(
        &mut self,
        cx: &mut Context<'_>,
        dest: impl Write,
    ) -> Poll<io::Result<usize>> {
        self.drain_inner(Some(cx), dest)
    }

    /// Reads bytes from this reader.
    ///
    /// Rather than taking a `Write` trait object, this method takes a slice of bytes to write into.
    /// Because of this, it is infallible and can be used in `no_std` environments.
    ///
    /// The same conditions that apply to [`poll_drain`] apply to this method.
    ///
    /// [`poll_drain`]: #method.poll_drain
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::{future, prelude::*};
    /// # future::block_on(async {
    /// let (mut r, mut w) = piper::pipe(1024);
    ///
    /// // Write some data to the pipe.
    /// w.write_all(b"hello world").await.unwrap();
    ///
    /// // Try reading from the pipe.
    /// let mut buf = [0; 1024];
    /// let n = future::poll_fn(|cx| r.poll_drain_bytes(cx, &mut buf[..])).await;
    ///
    /// // The data was written to the buffer.
    /// assert_eq!(&buf[..n], b"hello world");
    /// # });
    /// ```
    pub fn poll_drain_bytes(&mut self, cx: &mut Context<'_>, dest: &mut [u8]) -> Poll<usize> {
        match self.drain_inner(Some(cx), WriteBytes(dest)) {
            Poll::Ready(Ok(n)) => Poll::Ready(n),
            Poll::Ready(Err(e)) => match e {},
            Poll::Pending => Poll::Pending,
        }
    }

    /// Tries to read bytes from this reader.
    ///
    /// Returns the total number of bytes that were read from this reader.
    ///
    /// # Examples
    ///
    /// ```
    /// let (mut r, mut w) = piper::pipe(1024);
    ///
    /// // `try_drain()` returns 0 off the bat.
    /// let mut buf = [0; 10];
    /// assert_eq!(r.try_drain(&mut buf), 0);
    ///
    /// // After a write it returns the data.
    /// w.try_fill(&[0, 1, 2, 3, 4]);
    /// assert_eq!(r.try_drain(&mut buf), 5);
    /// assert_eq!(&buf[..5], &[0, 1, 2, 3, 4]);
    /// ```
    pub fn try_drain(&mut self, dest: &mut [u8]) -> usize {
        match self.drain_inner(None, WriteBytes(dest)) {
            Poll::Ready(Ok(n)) => n,
            Poll::Ready(Err(e)) => match e {},
            Poll::Pending => 0,
        }
    }

    /// Reads bytes from this reader and writes into blocking `dest`.
    #[inline]
    fn drain_inner<W: WriteLike>(
        &mut self,
        mut cx: Option<&mut Context<'_>>,
        mut dest: W,
    ) -> Poll<Result<usize, W::Error>> {
        let cap = self.inner.cap;

        // Calculates the distance between two indices.
        let distance = |a: usize, b: usize| {
            if a <= b {
                b - a
            } else {
                2 * cap - (a - b)
            }
        };

        // If the pipe appears to be empty...
        if distance(self.head, self.tail) == 0 {
            // Reload the tail in case it's become stale.
            self.tail = self.inner.tail.load(Ordering::Acquire);

            // If the pipe is now really empty...
            if distance(self.head, self.tail) == 0 {
                // Register the waker.
                if let Some(cx) = cx.as_mut() {
                    self.inner.reader.register(cx.waker());
                }
                atomic::fence(Ordering::SeqCst);

                // Reload the tail after registering the waker.
                self.tail = self.inner.tail.load(Ordering::Acquire);

                // If the pipe is still empty...
                if distance(self.head, self.tail) == 0 {
                    // Check whether the pipe is closed or just empty.
                    if self.inner.closed.load(Ordering::Relaxed) {
                        return Poll::Ready(Ok(0));
                    } else {
                        return Poll::Pending;
                    }
                }
            }
        }

        // The pipe is not empty so remove the waker.
        self.inner.reader.take();

        // Yield with some small probability - this improves fairness.
        if let Some(cx) = cx {
            ready!(maybe_yield(&mut self.rng, cx));
        }

        // Given an index in `0..2*cap`, returns the real index in `0..cap`.
        let real_index = |i: usize| {
            if i < cap {
                i
            } else {
                i - cap
            }
        };

        // Number of bytes read so far.
        let mut count = 0;

        loop {
            // Calculate how many bytes to read in this iteration.
            let n = (128 * 1024) // Not too many bytes in one go - better to wake the writer soon!
                .min(distance(self.head, self.tail)) // No more than bytes in the pipe.
                .min(cap - real_index(self.head)); // Don't go past the buffer boundary.

            // Create a slice of data in the pipe buffer.
            let pipe_slice =
                unsafe { slice::from_raw_parts(self.inner.buffer.add(real_index(self.head)), n) };

            // Copy bytes from the pipe buffer into `dest`.
            let n = dest.write(pipe_slice)?;
            count += n;

            // If pipe is empty or `dest` is full, return.
            if n == 0 {
                return Poll::Ready(Ok(count));
            }

            // Move the head forward.
            if self.head + n < 2 * cap {
                self.head += n;
            } else {
                self.head = 0;
            }

            // Store the current head index.
            self.inner.head.store(self.head, Ordering::Release);

            // Wake the writer because the pipe is not full.
            self.inner.writer.wake();
        }
    }
}

#[cfg(feature = "std")]
impl AsyncRead for Reader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        self.poll_drain_bytes(cx, buf).map(Ok)
    }
}

impl Writer {
    /// Gets the total length of the data in the pipe.
    ///
    /// This method returns the number of bytes that have been written into the pipe but haven't been
    /// read yet.
    ///
    /// # Examples
    ///
    /// ```
    /// let (_reader, mut writer) = piper::pipe(10);
    /// let _ = writer.try_fill(&[0u8; 5]);
    /// assert_eq!(writer.len(), 5);
    /// ```
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Tell whether or not the pipe is empty.
    ///
    /// This method returns `true` if the pipe is empty, and `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// let (_reader, mut writer) = piper::pipe(10);
    /// assert!(writer.is_empty());
    /// let _ = writer.try_fill(&[0u8; 5]);
    /// assert!(!writer.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.inner.len() == 0
    }

    /// Gets the total capacity of the pipe.
    ///
    /// This method returns the number of bytes that the pipe can hold at a time.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// let (_, writer) = piper::pipe(10);
    /// assert_eq!(writer.capacity(), 10);
    /// # });
    /// ```
    pub fn capacity(&self) -> usize {
        self.inner.cap
    }

    /// Tell whether or not the pipe is full.
    ///
    /// The pipe is full if the number of bytes written into it is equal to its capacity. At this point,
    /// writes will block until some data is read from the pipe.
    ///
    /// This method returns `true` if the pipe is full, and `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// let (mut reader, mut writer) = piper::pipe(10);
    /// assert!(!writer.is_full());
    /// let _ = writer.try_fill(&[0u8; 10]);
    /// assert!(writer.is_full());
    /// let _ = reader.try_drain(&mut [0u8; 5]);
    /// assert!(!writer.is_full());
    /// ```
    pub fn is_full(&self) -> bool {
        self.inner.len() == self.inner.cap
    }

    /// Tell whether or not the pipe is closed.
    ///
    /// The pipe is closed if either the reader or the writer has been dropped. At this point, attempting
    /// to write into the pipe will return `Poll::Ready(Ok(0))` and attempting to read from the pipe after
    /// any previously written bytes are read will return `Poll::Ready(Ok(0))`.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// let (reader, writer) = piper::pipe(10);
    /// assert!(!writer.is_closed());
    /// drop(reader);
    /// assert!(writer.is_closed());
    /// # });
    /// ```
    pub fn is_closed(&self) -> bool {
        self.inner.closed.load(Ordering::SeqCst)
    }

    /// Reads bytes from blocking `src` and writes into this writer.
    ///
    /// This method writes directly from `src` into the pipe's internal buffer. This avoids an extra copy,
    /// but it may block the thread if `src` blocks.
    ///
    /// If the pipe is full, this method returns `Poll::Pending`. If the pipe is closed, this method
    /// returns `Poll::Ready(Ok(0))`. Errors in `src` are bubbled up through `Poll::Ready(Err(e))`.
    /// Otherwise, this method returns `Poll::Ready(Ok(n))` where `n` is the number of bytes read.
    ///
    /// This method is only available when the `std` feature is enabled. For `no_std` environments,
    /// consider using [`poll_fill_bytes`] instead.
    ///
    /// [`poll_fill_bytes`]: #method.poll_fill_bytes
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::{future, prelude::*};
    /// # future::block_on(async {
    ///
    /// // Create a pipe.
    /// let (mut reader, mut writer) = piper::pipe(1024);
    ///
    /// // Fill the pipe with some bytes.
    /// let data = b"hello world";
    /// let n = future::poll_fn(|cx| writer.poll_fill(cx, &data[..])).await.unwrap();
    /// assert_eq!(n, data.len());
    ///
    /// // Read the bytes back.
    /// let mut buf = [0; 1024];
    /// reader.read_exact(&mut buf[..data.len()]).await.unwrap();
    /// assert_eq!(&buf[..data.len()], data);
    /// # });
    /// ```
    #[cfg(feature = "std")]
    pub fn poll_fill(&mut self, cx: &mut Context<'_>, src: impl Read) -> Poll<io::Result<usize>> {
        self.fill_inner(Some(cx), src)
    }

    /// Writes bytes into this writer.
    ///
    /// Rather than taking a `Read` trait object, this method takes a slice of bytes to read from.
    /// Because of this, it is infallible and can be used in `no_std` environments.
    ///
    /// The same conditions that apply to [`poll_fill`] apply to this method.
    ///
    /// [`poll_fill`]: #method.poll_fill
    ///
    /// # Examples
    ///
    /// ```
    /// use futures_lite::{future, prelude::*};
    /// # future::block_on(async {
    ///
    /// // Create a pipe.
    /// let (mut reader, mut writer) = piper::pipe(1024);
    ///
    /// // Fill the pipe with some bytes.
    /// let data = b"hello world";
    /// let n = future::poll_fn(|cx| writer.poll_fill_bytes(cx, &data[..])).await;
    /// assert_eq!(n, data.len());
    ///
    /// // Read the bytes back.
    /// let mut buf = [0; 1024];
    /// reader.read_exact(&mut buf[..data.len()]).await.unwrap();
    /// assert_eq!(&buf[..data.len()], data);
    /// # });
    /// ```
    pub fn poll_fill_bytes(&mut self, cx: &mut Context<'_>, bytes: &[u8]) -> Poll<usize> {
        match self.fill_inner(Some(cx), ReadBytes(bytes)) {
            Poll::Ready(Ok(n)) => Poll::Ready(n),
            Poll::Ready(Err(e)) => match e {},
            Poll::Pending => Poll::Pending,
        }
    }

    /// Tries to write bytes to this writer.
    ///
    /// Returns the total number of bytes that were read from this reader.
    ///
    /// # Examples
    ///
    /// ```
    /// let (mut r, mut w) = piper::pipe(1024);
    ///
    /// let mut buf = [0; 10];
    /// assert_eq!(w.try_fill(&[0, 1, 2, 3, 4]), 5);
    /// assert_eq!(r.try_drain(&mut buf), 5);
    /// assert_eq!(&buf[..5], &[0, 1, 2, 3, 4]);
    /// ```
    pub fn try_fill(&mut self, dest: &[u8]) -> usize {
        match self.fill_inner(None, ReadBytes(dest)) {
            Poll::Ready(Ok(n)) => n,
            Poll::Ready(Err(e)) => match e {},
            Poll::Pending => 0,
        }
    }

    /// Reads bytes from blocking `src` and writes into this writer.
    #[inline]
    fn fill_inner<R: ReadLike>(
        &mut self,
        mut cx: Option<&mut Context<'_>>,
        mut src: R,
    ) -> Poll<Result<usize, R::Error>> {
        // Just a quick check if the pipe is closed, which is why a relaxed load is okay.
        if self.inner.closed.load(Ordering::Relaxed) {
            return Poll::Ready(Ok(0));
        }

        // Calculates the distance between two indices.
        let cap = self.inner.cap;
        let distance = |a: usize, b: usize| {
            if a <= b {
                b - a
            } else {
                2 * cap - (a - b)
            }
        };

        // If the pipe appears to be full...
        if distance(self.head, self.tail) == cap {
            // Reload the head in case it's become stale.
            self.head = self.inner.head.load(Ordering::Acquire);

            // If the pipe is now really empty...
            if distance(self.head, self.tail) == cap {
                // Register the waker.
                if let Some(cx) = cx.as_mut() {
                    self.inner.writer.register(cx.waker());
                }
                atomic::fence(Ordering::SeqCst);

                // Reload the head after registering the waker.
                self.head = self.inner.head.load(Ordering::Acquire);

                // If the pipe is still full...
                if distance(self.head, self.tail) == cap {
                    // Check whether the pipe is closed or just full.
                    if self.inner.closed.load(Ordering::Relaxed) {
                        return Poll::Ready(Ok(0));
                    } else {
                        return Poll::Pending;
                    }
                }
            }
        }

        // The pipe is not full so remove the waker.
        self.inner.writer.take();

        // Yield with some small probability - this improves fairness.
        if let Some(cx) = cx {
            ready!(maybe_yield(&mut self.rng, cx));
        }

        // Given an index in `0..2*cap`, returns the real index in `0..cap`.
        let real_index = |i: usize| {
            if i < cap {
                i
            } else {
                i - cap
            }
        };

        // Number of bytes written so far.
        let mut count = 0;

        loop {
            // Calculate how many bytes to write in this iteration.
            let n = (128 * 1024) // Not too many bytes in one go - better to wake the reader soon!
                .min(self.zeroed_until * 2 + 4096) // Don't zero too many bytes when starting.
                .min(cap - distance(self.head, self.tail)) // No more than space in the pipe.
                .min(cap - real_index(self.tail)); // Don't go past the buffer boundary.

            // Create a slice of available space in the pipe buffer.
            let pipe_slice_mut = unsafe {
                let from = real_index(self.tail);
                let to = from + n;

                // Make sure all bytes in the slice are initialized.
                if self.zeroed_until < to {
                    self.inner
                        .buffer
                        .add(self.zeroed_until)
                        .write_bytes(0u8, to - self.zeroed_until);
                    self.zeroed_until = to;
                }

                slice::from_raw_parts_mut(self.inner.buffer.add(from), n)
            };

            // Copy bytes from `src` into the piper buffer.
            let n = src.read(pipe_slice_mut)?;
            count += n;

            // If the pipe is full or closed, or `src` is empty, return.
            if n == 0 || self.inner.closed.load(Ordering::Relaxed) {
                return Poll::Ready(Ok(count));
            }

            // Move the tail forward.
            if self.tail + n < 2 * cap {
                self.tail += n;
            } else {
                self.tail = 0;
            }

            // Store the current tail index.
            self.inner.tail.store(self.tail, Ordering::Release);

            // Wake the reader because the pipe is not empty.
            self.inner.reader.wake();
        }
    }
}

#[cfg(feature = "std")]
impl AsyncWrite for Writer {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.poll_fill_bytes(cx, buf).map(Ok)
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        // Nothing to flush.
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        // Set the closed flag.
        self.inner.closed.store(true, Ordering::Release);

        // Wake up any tasks that may be waiting on the pipe.
        self.inner.reader.wake();
        self.inner.writer.wake();

        // The pipe is now closed.
        Poll::Ready(Ok(()))
    }
}

/// A trait for reading bytes into a pipe.
trait ReadLike {
    /// The error type.
    type Error;

    /// Reads bytes into the given buffer.
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error>;
}

#[cfg(feature = "std")]
impl<R: Read> ReadLike for R {
    type Error = io::Error;

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        Read::read(self, buf)
    }
}

/// Implements `no_std` reading around a byte slice.
struct ReadBytes<'a>(&'a [u8]);

impl ReadLike for ReadBytes<'_> {
    type Error = Infallible;

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let n = self.0.len().min(buf.len());
        buf[..n].copy_from_slice(&self.0[..n]);
        self.0 = &self.0[n..];
        Ok(n)
    }
}

/// A trait for writing bytes from a pipe.
trait WriteLike {
    /// The error type.
    type Error;

    /// Writes bytes from the given buffer.
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error>;
}

#[cfg(feature = "std")]
impl<W: Write> WriteLike for W {
    type Error = io::Error;

    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        Write::write(self, buf)
    }
}

/// Implements `no_std` writing around a byte slice.
struct WriteBytes<'a>(&'a mut [u8]);

impl WriteLike for WriteBytes<'_> {
    type Error = Infallible;

    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        let n = self.0.len().min(buf.len());
        self.0[..n].copy_from_slice(&buf[..n]);

        // mem::take() is not available on 1.36
        #[allow(clippy::mem_replace_with_default)]
        {
            let slice = mem::replace(&mut self.0, &mut []);
            self.0 = &mut slice[n..];
        }

        Ok(n)
    }
}

/// Yield with some small probability.
fn maybe_yield(rng: &mut fastrand::Rng, cx: &mut Context<'_>) -> Poll<()> {
    if rng.usize(..100) == 0 {
        cx.waker().wake_by_ref();
        Poll::Pending
    } else {
        Poll::Ready(())
    }
}

/// Get a random number generator.
#[cfg(feature = "std")]
#[inline]
fn rng() -> fastrand::Rng {
    fastrand::Rng::new()
}

/// Get a random number generator.
///
/// This uses a fixed seed due to the lack of a good RNG in `no_std` environments.
#[cfg(not(feature = "std"))]
#[inline]
fn rng() -> fastrand::Rng {
    // Chosen by fair roll of the dice.
    fastrand::Rng::with_seed(0x7e9b496634c97ec6)
}

/// ```
/// use piper::{Reader, Writer};
/// fn _send_sync<T: Send + Sync>() {}
/// _send_sync::<Reader>();
/// _send_sync::<Writer>();
/// ```
fn _assert_send_sync() {}

mod sync {
    #[cfg(not(feature = "portable-atomic"))]
    pub use core::sync::atomic;

    #[cfg(not(feature = "portable-atomic"))]
    pub use alloc::sync::Arc;

    #[cfg(feature = "portable-atomic")]
    pub use portable_atomic_crate as atomic;

    #[cfg(feature = "portable-atomic")]
    pub use portable_atomic_util::Arc;
}
