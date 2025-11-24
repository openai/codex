//! inotify support for working with inotify objects.
//!
//! # Examples
//!
//! ```
//! use rustix::fs::inotify;
//! use rustix::io;
//! use std::mem::MaybeUninit;
//!
//! # fn test() -> io::Result<()> {
//! // Create an inotify object. In this example, we use `NONBLOCK` so that the
//! // reader fails with `WOULDBLOCK` when no events are ready. Otherwise it
//! // will block until at least one event is ready.
//! let inotify = inotify::init(inotify::CreateFlags::NONBLOCK)?;
//!
//! // Add a directory to watch.
//! inotify::add_watch(
//!     &inotify,
//!     "/path/to/some/directory/to/watch",
//!     inotify::WatchFlags::ALL_EVENTS,
//! )?;
//!
//! // Generate some events in the watched directory…
//!
//! // Loop over pending events.
//! let mut buf = [MaybeUninit::uninit(); 512];
//! let mut iter = inotify::Reader::new(inotify, &mut buf);
//! loop {
//!     let entry = match iter.next() {
//!         // Stop iterating if there are no more events for now.
//!         Err(io::Errno::WOULDBLOCK) => break,
//!         Err(e) => return Err(e),
//!         Ok(entry) => entry,
//!     };
//!
//!     // Use `entry`…
//! }
//!
//! # Ok(())
//! # }

#![allow(unused_qualifications)]

use super::inotify;
pub use crate::backend::fs::inotify::{CreateFlags, ReadFlags, WatchFlags};
use crate::backend::fs::syscalls;
use crate::fd::{AsFd, OwnedFd};
use crate::ffi::CStr;
use crate::io;
use crate::io::{read_uninit, Errno};
use core::mem::{align_of, size_of, MaybeUninit};
use linux_raw_sys::general::inotify_event;

#[deprecated(note = "Use `inotify::add_watch`.")]
#[doc(hidden)]
pub use add_watch as inotify_add_watch;
#[deprecated(note = "Use `inotify::init`.")]
#[doc(hidden)]
pub use init as inotify_init;
#[deprecated(note = "Use `inotify::remove_watch`.")]
#[doc(hidden)]
pub use remove_watch as inotify_remove_watch;

/// `inotify_init1(flags)`—Creates a new inotify object.
///
/// Use the [`CreateFlags::CLOEXEC`] flag to prevent the resulting file
/// descriptor from being implicitly passed across `exec` boundaries.
#[doc(alias = "inotify_init1")]
#[inline]
pub fn init(flags: inotify::CreateFlags) -> io::Result<OwnedFd> {
    syscalls::inotify_init1(flags)
}

/// `inotify_add_watch(self, path, flags)`—Adds a watch to inotify.
///
/// This registers or updates a watch for the filesystem path `path` and
/// returns a watch descriptor corresponding to this watch.
///
/// Note: Due to the existence of hardlinks, providing two different paths to
/// this method may result in it returning the same watch descriptor. An
/// application should keep track of this externally to avoid logic errors.
#[doc(alias = "inotify_add_watch")]
#[inline]
pub fn add_watch<P: crate::path::Arg>(
    inot: impl AsFd,
    path: P,
    flags: inotify::WatchFlags,
) -> io::Result<i32> {
    path.into_with_c_str(|path| syscalls::inotify_add_watch(inot.as_fd(), path, flags))
}

/// `inotify_rm_watch(self, wd)`—Removes a watch from this inotify.
///
/// The watch descriptor provided should have previously been returned by
/// [`inotify::add_watch`] and not previously have been removed.
#[doc(alias = "inotify_rm_watch")]
#[inline]
pub fn remove_watch(inot: impl AsFd, wd: i32) -> io::Result<()> {
    syscalls::inotify_rm_watch(inot.as_fd(), wd)
}

/// An inotify event iterator implemented with the read syscall.
///
/// See the [`RawDir`] API for more details and usage examples as this API is
/// based on it.
///
/// [`RawDir`]: crate::fs::raw_dir::RawDir
pub struct Reader<'buf, Fd: AsFd> {
    fd: Fd,
    buf: &'buf mut [MaybeUninit<u8>],
    initialized: usize,
    offset: usize,
}

impl<'buf, Fd: AsFd> Reader<'buf, Fd> {
    /// Create a new iterator from the given file descriptor and buffer.
    pub fn new(fd: Fd, buf: &'buf mut [MaybeUninit<u8>]) -> Self {
        Self {
            fd,
            buf: {
                let offset = buf.as_ptr().align_offset(align_of::<inotify_event>());
                if offset < buf.len() {
                    &mut buf[offset..]
                } else {
                    &mut []
                }
            },
            initialized: 0,
            offset: 0,
        }
    }
}

/// An inotify event.
#[derive(Debug)]
pub struct InotifyEvent<'a> {
    wd: i32,
    events: ReadFlags,
    cookie: u32,
    file_name: Option<&'a CStr>,
}

impl<'a> InotifyEvent<'a> {
    /// Returns the watch for which this event occurs.
    #[inline]
    pub fn wd(&self) -> i32 {
        self.wd
    }

    /// Returns a description of the events.
    #[inline]
    #[doc(alias = "mask")]
    pub fn events(&self) -> ReadFlags {
        self.events
    }

    /// Returns the unique cookie associating related events.
    #[inline]
    pub fn cookie(&self) -> u32 {
        self.cookie
    }

    /// Returns the file name of this event, if any.
    #[inline]
    pub fn file_name(&self) -> Option<&CStr> {
        self.file_name
    }
}

impl<'buf, Fd: AsFd> Reader<'buf, Fd> {
    /// Read the next inotify event.
    ///
    /// This is similar to `[Iterator::next`] except that it doesn't return an
    /// `Option`, because the stream doesn't have an ending. It always returns
    /// events or errors.
    ///
    /// If there are no events in the buffer and none ready to be read:
    ///  - If the file descriptor was opened with
    ///    [`inotify::CreateFlags::NONBLOCK`], this will fail with
    ///    [`Errno::AGAIN`].
    ///  - Otherwise this will block until at least one event is ready or an
    ///    error occurs.
    #[allow(unsafe_code)]
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> io::Result<InotifyEvent<'_>> {
        if self.is_buffer_empty() {
            match read_uninit(self.fd.as_fd(), self.buf).map(|(init, _)| init.len()) {
                Ok(0) => return Err(Errno::INVAL),
                Ok(bytes_read) => {
                    self.initialized = bytes_read;
                    self.offset = 0;
                }
                Err(e) => return Err(e),
            }
        }

        let ptr = self.buf[self.offset..].as_ptr();

        // SAFETY:
        // - This data is initialized by the check above.
        //   - Assumption: the kernel will not give us partial structs.
        // - Assumption: the kernel uses proper alignment between structs.
        // - The starting pointer is aligned (performed in `Reader::new`).
        let event = unsafe { &*ptr.cast::<inotify_event>() };

        self.offset += size_of::<inotify_event>() + usize::try_from(event.len).unwrap();

        Ok(InotifyEvent {
            wd: event.wd,
            events: ReadFlags::from_bits_retain(event.mask),
            cookie: event.cookie,
            file_name: if event.len > 0 {
                // SAFETY: The kernel guarantees a NUL-terminated string.
                Some(unsafe { CStr::from_ptr(event.name.as_ptr().cast()) })
            } else {
                None
            },
        })
    }

    /// Returns true if the internal buffer is empty and will be refilled when
    /// calling [`next`]. This is useful to avoid further blocking reads.
    ///
    /// [`next`]: Self::next
    pub fn is_buffer_empty(&self) -> bool {
        self.offset >= self.initialized
    }
}
