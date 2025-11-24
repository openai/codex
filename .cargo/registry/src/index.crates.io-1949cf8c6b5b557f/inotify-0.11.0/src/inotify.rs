use std::{
    io,
    os::unix::io::{
        AsFd,
        AsRawFd,
        BorrowedFd,
        FromRawFd,
        IntoRawFd,
        OwnedFd,
        RawFd,
    },
    path::Path,
    sync::{
        atomic::AtomicBool,
        Arc,
    }
};

use inotify_sys as ffi;
use libc::{
    F_GETFL,
    F_SETFL,
    O_NONBLOCK,
    fcntl,
};

use crate::events::Events;
use crate::fd_guard::FdGuard;
use crate::util::read_into_buffer;
use crate::watches::{
    WatchDescriptor,
    WatchMask,
    Watches,
};


#[cfg(feature = "stream")]
use crate::stream::EventStream;


/// Idiomatic Rust wrapper around Linux's inotify API
///
/// `Inotify` is a wrapper around an inotify instance. It generally tries to
/// adhere to the underlying inotify API closely, while making access to it
/// safe and convenient.
///
/// Please refer to the [top-level documentation] for further details and a
/// usage example.
///
/// [top-level documentation]: crate
#[derive(Debug)]
pub struct Inotify {
    fd: Arc<FdGuard>,
}

impl Inotify {
    /// Creates an [`Inotify`] instance
    ///
    /// Initializes an inotify instance by calling [`inotify_init1`].
    ///
    /// This method passes both flags accepted by [`inotify_init1`], not giving
    /// the user any choice in the matter, as not passing the flags would be
    /// inappropriate in the context of this wrapper:
    ///
    /// - [`IN_CLOEXEC`] prevents leaking file descriptors to other processes.
    /// - [`IN_NONBLOCK`] controls the blocking behavior of the inotify API,
    ///   which is entirely managed by this wrapper.
    ///
    /// # Errors
    ///
    /// Directly returns the error from the call to [`inotify_init1`], without
    /// adding any error conditions of its own.
    ///
    /// # Examples
    ///
    /// ```
    /// use inotify::Inotify;
    ///
    /// let inotify = Inotify::init()
    ///     .expect("Failed to initialize an inotify instance");
    /// ```
    ///
    /// [`inotify_init1`]: inotify_sys::inotify_init1
    /// [`IN_CLOEXEC`]: inotify_sys::IN_CLOEXEC
    /// [`IN_NONBLOCK`]: inotify_sys::IN_NONBLOCK
    pub fn init() -> io::Result<Inotify> {
        let fd = unsafe {
            // Initialize inotify and pass both `IN_CLOEXEC` and `IN_NONBLOCK`.
            //
            // `IN_NONBLOCK` is needed, because `Inotify` manages blocking
            // behavior for the API consumer, and the way we do that is to make
            // everything non-blocking by default and later override that as
            // required.
            //
            // Passing `IN_CLOEXEC` prevents leaking file descriptors to
            // processes executed by this process and seems to be a best
            // practice. I don't grasp this issue completely and failed to find
            // any authoritative sources on the topic. There's some discussion in
            // the open(2) and fcntl(2) man pages, but I didn't find that
            // helpful in understanding the issue of leaked file descriptors.
            // For what it's worth, there's a Rust issue about this:
            // https://github.com/rust-lang/rust/issues/12148
            ffi::inotify_init1(ffi::IN_CLOEXEC | ffi::IN_NONBLOCK)
        };

        if fd == -1 {
            return Err(io::Error::last_os_error());
        }

        Ok(Inotify {
            fd: Arc::new(FdGuard {
                fd,
                close_on_drop: AtomicBool::new(true),
            }),
        })
    }

    /// Gets an interface that allows adding and removing watches.
    /// See [`Watches::add`] and [`Watches::remove`].
    pub fn watches(&self) -> Watches {
        Watches::new(self.fd.clone())
    }

    /// Deprecated: use `Inotify.watches().add()` instead
    #[deprecated = "use `Inotify.watches().add()` instead"]
    pub fn add_watch<P>(&mut self, path: P, mask: WatchMask)
        -> io::Result<WatchDescriptor>
        where P: AsRef<Path>
    {
        self.watches().add(path, mask)
    }

    /// Deprecated: use `Inotify.watches().remove()` instead
    #[deprecated = "use `Inotify.watches().remove()` instead"]
    pub fn rm_watch(&mut self, wd: WatchDescriptor) -> io::Result<()> {
        self.watches().remove(wd)
    }

    /// Waits until events are available, then returns them
    ///
    /// Blocks the current thread until at least one event is available. If this
    /// is not desirable, please consider [`Inotify::read_events`].
    ///
    /// This method calls [`Inotify::read_events`] internally and behaves
    /// essentially the same, apart from the blocking behavior. Please refer to
    /// the documentation of [`Inotify::read_events`] for more information.
    pub fn read_events_blocking<'a>(&mut self, buffer: &'a mut [u8])
        -> io::Result<Events<'a>>
    {
        unsafe {
            let res = fcntl(**self.fd, F_GETFL);
            if res == -1 {
                return Err(io::Error::last_os_error());
            }
            if fcntl(**self.fd, F_SETFL, res & !O_NONBLOCK) == -1 {
                return Err(io::Error::last_os_error());
            }
        };
        let result = self.read_events(buffer);
        unsafe {
            let res = fcntl(**self.fd, F_GETFL);
            if res == -1 {
                return Err(io::Error::last_os_error());
            }
            if fcntl(**self.fd, F_SETFL, res | O_NONBLOCK) == -1 {
                return Err(io::Error::last_os_error());
            }
        };

        result
    }

    /// Returns one buffer's worth of available events
    ///
    /// Reads as many events as possible into `buffer`, and returns an iterator
    /// over them. If no events are available, an iterator is still returned. If
    /// you need a method that will block until at least one event is available,
    /// please consider [`read_events_blocking`].
    ///
    /// Please note that inotify will merge identical successive unread events 
    /// into a single event. This means this method can not be used to count the 
    /// number of file system events.
    ///
    /// The `buffer` argument, as the name indicates, is used as a buffer for
    /// the inotify events. Its contents may be overwritten.
    ///
    /// # Errors
    ///
    /// This function directly returns all errors from the call to [`read`].
    /// In addition, [`ErrorKind::UnexpectedEof`] is returned, if the call to
    /// [`read`] returns `0`, signaling end-of-file.
    ///
    /// If `buffer` is too small, this will result in an error with
    /// [`ErrorKind::InvalidInput`]. On very old Linux kernels,
    /// [`ErrorKind::UnexpectedEof`] will be returned instead.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use inotify::Inotify;
    /// use std::io::ErrorKind;
    ///
    /// let mut inotify = Inotify::init()
    ///     .expect("Failed to initialize an inotify instance");
    ///
    /// let mut buffer = [0; 1024];
    /// let events = loop {
    ///     match inotify.read_events(&mut buffer) {
    ///         Ok(events) => break events,
    ///         Err(error) if error.kind() == ErrorKind::WouldBlock => continue,
    ///         _ => panic!("Error while reading events"),
    ///     }
    /// };
    ///
    /// for event in events {
    ///     // Handle event
    /// }
    /// ```
    ///
    /// [`read_events_blocking`]: Self::read_events_blocking
    /// [`read`]: libc::read
    /// [`ErrorKind::UnexpectedEof`]: std::io::ErrorKind::UnexpectedEof
    /// [`ErrorKind::InvalidInput`]: std::io::ErrorKind::InvalidInput
    pub fn read_events<'a>(&mut self, buffer: &'a mut [u8])
        -> io::Result<Events<'a>>
    {
        let num_bytes = read_into_buffer(**self.fd, buffer);

        let num_bytes = match num_bytes {
            0 => {
                return Err(
                    io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "`read` return `0`, signaling end-of-file"
                    )
                );
            }
            -1 => {
                let error = io::Error::last_os_error();
                return Err(error);
            },
            _ if num_bytes < 0 => {
                panic!("{} {} {} {} {} {}",
                    "Unexpected return value from `read`. Received a negative",
                    "value that was not `-1`. According to the `read` man page",
                    "this shouldn't happen, as either `-1` is returned on",
                    "error, `0` on end-of-file, or a positive value for the",
                    "number of bytes read. Returned value:",
                    num_bytes,
                );
            }
            _ => {
                // The value returned by `read` should be `isize`. Let's quickly
                // verify this with the following assignment, so we can be sure
                // our cast below is valid.
                let num_bytes: isize = num_bytes;

                // The type returned by `read` is `isize`, and we've ruled out
                // all negative values with the match arms above. This means we
                // can safely cast to `usize`.
                debug_assert!(num_bytes > 0);
                num_bytes as usize
            }
        };

        Ok(Events::new(Arc::downgrade(&self.fd), buffer, num_bytes))
    }

    /// Deprecated: use `into_event_stream()` instead, which enforces a single `Stream` and predictable reads.
    /// Using this method to create multiple `EventStream` instances from one `Inotify` is unsupported,
    /// as they will contend over one event source and each produce unpredictable stream contents.
    #[deprecated = "use `into_event_stream()` instead, which enforces a single Stream and predictable reads"]
    #[cfg(feature = "stream")]
    pub fn event_stream<T>(&mut self, buffer: T)
        -> io::Result<EventStream<T>>
    where
        T: AsMut<[u8]> + AsRef<[u8]>,
    {
        EventStream::new(self.fd.clone(), buffer)
    }

    /// Create a stream which collects events. Consumes the `Inotify` instance.
    ///
    /// Returns a `Stream` over all events that are available. This stream is an
    /// infinite source of events.
    ///
    /// An internal buffer which can hold the largest possible event is used.
    #[cfg(feature = "stream")]
    pub fn into_event_stream<T>(self, buffer: T)
        -> io::Result<EventStream<T>>
    where
        T: AsMut<[u8]> + AsRef<[u8]>,
    {
        EventStream::new(self.fd, buffer)
    }

    /// Creates an `Inotify` instance using the file descriptor which was originally
    /// initialized in `Inotify::init`. This is intended to be used to transform an
    /// `EventStream` back into an `Inotify`. Do not attempt to clone `Inotify` with this.
    #[cfg(feature = "stream")]
    pub(crate) fn from_file_descriptor(fd: Arc<FdGuard>) -> Self
    {
        Inotify {
            fd,
        }
    }

    /// Closes the inotify instance
    ///
    /// Closes the file descriptor referring to the inotify instance. The user
    /// usually doesn't have to call this function, as the underlying inotify
    /// instance is closed automatically, when [`Inotify`] is dropped.
    ///
    /// # Errors
    ///
    /// Directly returns the error from the call to [`close`], without adding any
    /// error conditions of its own.
    ///
    /// # Examples
    ///
    /// ```
    /// use inotify::Inotify;
    ///
    /// let mut inotify = Inotify::init()
    ///     .expect("Failed to initialize an inotify instance");
    ///
    /// inotify.close()
    ///     .expect("Failed to close inotify instance");
    /// ```
    ///
    /// [`close`]: libc::close
    pub fn close(self) -> io::Result<()> {
        // `self` will be dropped when this method returns. If this is the only
        // owner of `fd`, the `Arc` will also be dropped. The `Drop`
        // implementation for `FdGuard` will attempt to close the file descriptor
        // again, unless this flag here is cleared.
        self.fd.should_not_close();

        match unsafe { ffi::close(**self.fd) } {
            0 => Ok(()),
            _ => Err(io::Error::last_os_error()),
        }
    }
}

impl AsRawFd for Inotify {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}

impl FromRawFd for Inotify {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Inotify {
            fd: Arc::new(FdGuard::from_raw_fd(fd))
        }
    }
}

impl IntoRawFd for Inotify {
    #[inline]
    fn into_raw_fd(self) -> RawFd {
        self.fd.should_not_close();
        self.fd.fd
    }
}

impl AsFd for Inotify {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

impl From<Inotify> for OwnedFd {
    fn from(fd: Inotify) -> OwnedFd {
        unsafe { OwnedFd::from_raw_fd(fd.into_raw_fd()) }
    }
}

impl From<OwnedFd> for Inotify {
    fn from(fd: OwnedFd) -> Inotify {
        unsafe { Inotify::from_raw_fd(fd.into_raw_fd()) }
    }
}
