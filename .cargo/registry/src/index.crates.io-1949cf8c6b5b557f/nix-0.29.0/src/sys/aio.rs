// vim: tw=80
//! POSIX Asynchronous I/O
//!
//! The POSIX AIO interface is used for asynchronous I/O on files and disk-like
//! devices.  It supports [`read`](struct.AioRead.html#method.new),
//! [`write`](struct.AioWrite.html#method.new),
//! [`fsync`](struct.AioFsync.html#method.new),
//! [`readv`](struct.AioReadv.html#method.new), and
//! [`writev`](struct.AioWritev.html#method.new), operations, subject to
//! platform support.  Completion
//! notifications can optionally be delivered via
//! [signals](../signal/enum.SigevNotify.html#variant.SigevSignal), via the
//! [`aio_suspend`](fn.aio_suspend.html) function, or via polling.  Some
//! platforms support other completion
//! notifications, such as
//! [kevent](../signal/enum.SigevNotify.html#variant.SigevKevent).
//!
//! Multiple operations may be submitted in a batch with
//! [`lio_listio`](fn.lio_listio.html), though the standard does not guarantee
//! that they will be executed atomically.
//!
//! Outstanding operations may be cancelled with
//! [`cancel`](trait.Aio.html#method.cancel) or
//! [`aio_cancel_all`](fn.aio_cancel_all.html), though the operating system may
//! not support this for all filesystems and devices.
#[cfg(target_os = "freebsd")]
use std::io::{IoSlice, IoSliceMut};
use std::{
    convert::TryFrom,
    fmt::{self, Debug},
    marker::{PhantomData, PhantomPinned},
    mem,
    os::unix::io::{AsFd, AsRawFd, BorrowedFd},
    pin::Pin,
    ptr, thread,
};

use libc::off_t;
use pin_utils::unsafe_pinned;

use crate::{
    errno::Errno,
    sys::{signal::*, time::TimeSpec},
    Result,
};

libc_enum! {
    /// Mode for `AioCb::fsync`.  Controls whether only data or both data and
    /// metadata are synced.
    #[repr(i32)]
    #[non_exhaustive]
    pub enum AioFsyncMode {
        /// do it like `fsync`
        O_SYNC,
        /// on supported operating systems only, do it like `fdatasync`
        #[cfg(any(apple_targets,
                  target_os = "linux",
                  target_os = "freebsd",
                  netbsdlike))]
        O_DSYNC
    }
    impl TryFrom<i32>
}

libc_enum! {
    /// Mode for [`lio_listio`](fn.lio_listio.html)
    #[repr(i32)]
    pub enum LioMode {
        /// Requests that [`lio_listio`](fn.lio_listio.html) block until all
        /// requested operations have been completed
        LIO_WAIT,
        /// Requests that [`lio_listio`](fn.lio_listio.html) return immediately
        LIO_NOWAIT,
    }
}

/// Return values for [`AioCb::cancel`](struct.AioCb.html#method.cancel) and
/// [`aio_cancel_all`](fn.aio_cancel_all.html)
#[repr(i32)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AioCancelStat {
    /// All outstanding requests were canceled
    AioCanceled = libc::AIO_CANCELED,
    /// Some requests were not canceled.  Their status should be checked with
    /// `AioCb::error`
    AioNotCanceled = libc::AIO_NOTCANCELED,
    /// All of the requests have already finished
    AioAllDone = libc::AIO_ALLDONE,
}

/// Newtype that adds Send and Sync to libc::aiocb, which contains raw pointers
#[repr(transparent)]
struct LibcAiocb(libc::aiocb);

unsafe impl Send for LibcAiocb {}
unsafe impl Sync for LibcAiocb {}

/// Base class for all AIO operations.  Should only be used directly when
/// checking for completion.
// We could create some kind of AsPinnedMut trait, and implement it for all aio
// ops, allowing the crate's users to get pinned references to `AioCb`.  That
// could save some code for things like polling methods.  But IMHO it would
// provide polymorphism at the wrong level.  Instead, the best place for
// polymorphism is at the level of `Futures`.
#[repr(C)]
struct AioCb<'a> {
    aiocb: LibcAiocb,
    /// Could this `AioCb` potentially have any in-kernel state?
    // It would be really nice to perform the in-progress check entirely at
    // compile time.  But I can't figure out how, because:
    // * Future::poll takes a `Pin<&mut self>` rather than `self`, and
    // * Rust's lack of an equivalent of C++'s Guaranteed Copy Elision means
    //   that there's no way to write an AioCb constructor that neither boxes
    //   the object itself, nor moves it during return.
    in_progress: bool,
    _fd: PhantomData<BorrowedFd<'a>>,
}

impl<'a> AioCb<'a> {
    pin_utils::unsafe_unpinned!(aiocb: LibcAiocb);

    fn aio_return(mut self: Pin<&mut Self>) -> Result<usize> {
        self.in_progress = false;
        unsafe {
            let p: *mut libc::aiocb = &mut self.aiocb.0;
            Errno::result(libc::aio_return(p))
        }
        .map(|r| r as usize)
    }

    fn cancel(mut self: Pin<&mut Self>) -> Result<AioCancelStat> {
        let r = unsafe {
            libc::aio_cancel(self.aiocb.0.aio_fildes, &mut self.aiocb.0)
        };
        match r {
            libc::AIO_CANCELED => Ok(AioCancelStat::AioCanceled),
            libc::AIO_NOTCANCELED => Ok(AioCancelStat::AioNotCanceled),
            libc::AIO_ALLDONE => Ok(AioCancelStat::AioAllDone),
            -1 => Err(Errno::last()),
            _ => panic!("unknown aio_cancel return value"),
        }
    }

    fn common_init(
        fd: BorrowedFd<'a>,
        prio: i32,
        sigev_notify: SigevNotify,
    ) -> Self {
        // Use mem::zeroed instead of explicitly zeroing each field, because the
        // number and name of reserved fields is OS-dependent.  On some OSes,
        // some reserved fields are used the kernel for state, and must be
        // explicitly zeroed when allocated.
        let mut a = unsafe { mem::zeroed::<libc::aiocb>() };
        a.aio_fildes = fd.as_raw_fd();
        a.aio_reqprio = prio;
        a.aio_sigevent = SigEvent::new(sigev_notify).sigevent();
        AioCb {
            aiocb: LibcAiocb(a),
            in_progress: false,
            _fd: PhantomData,
        }
    }

    fn error(self: Pin<&mut Self>) -> Result<()> {
        let r = unsafe { libc::aio_error(&self.aiocb().0) };
        match r {
            0 => Ok(()),
            num if num > 0 => Err(Errno::from_raw(num)),
            -1 => Err(Errno::last()),
            num => panic!("unknown aio_error return value {num:?}"),
        }
    }

    fn in_progress(&self) -> bool {
        self.in_progress
    }

    fn set_in_progress(mut self: Pin<&mut Self>) {
        self.as_mut().in_progress = true;
    }

    /// Update the notification settings for an existing AIO operation that has
    /// not yet been submitted.
    // Takes a normal reference rather than a pinned one because this method is
    // normally called before the object needs to be pinned, that is, before
    // it's been submitted to the kernel.
    fn set_sigev_notify(&mut self, sigev_notify: SigevNotify) {
        assert!(
            !self.in_progress,
            "Can't change notification settings for an in-progress operation"
        );
        self.aiocb.0.aio_sigevent = SigEvent::new(sigev_notify).sigevent();
    }
}

impl<'a> Debug for AioCb<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("AioCb")
            .field("aiocb", &self.aiocb.0)
            .field("in_progress", &self.in_progress)
            .finish()
    }
}

impl<'a> Drop for AioCb<'a> {
    /// If the `AioCb` has no remaining state in the kernel, just drop it.
    /// Otherwise, dropping constitutes a resource leak, which is an error
    fn drop(&mut self) {
        assert!(
            thread::panicking() || !self.in_progress,
            "Dropped an in-progress AioCb"
        );
    }
}

/// Methods common to all AIO operations
pub trait Aio {
    /// The return type of [`Aio::aio_return`].
    type Output;

    /// Retrieve return status of an asynchronous operation.
    ///
    /// Should only be called once for each operation, after [`Aio::error`]
    /// indicates that it has completed.  The result is the same as for the
    /// synchronous `read(2)`, `write(2)`, of `fsync(2)` functions.
    ///
    /// # References
    ///
    /// [aio_return](https://pubs.opengroup.org/onlinepubs/9699919799/functions/aio_return.html)
    fn aio_return(self: Pin<&mut Self>) -> Result<Self::Output>;

    /// Cancels an outstanding AIO request.
    ///
    /// The operating system is not required to implement cancellation for all
    /// file and device types.  Even if it does, there is no guarantee that the
    /// operation has not already completed.  So the caller must check the
    /// result and handle operations that were not canceled or that have already
    /// completed.
    ///
    /// # Examples
    ///
    /// Cancel an outstanding aio operation.  Note that we must still call
    /// `aio_return` to free resources, even though we don't care about the
    /// result.
    ///
    /// ```
    /// # use nix::errno::Errno;
    /// # use nix::Error;
    /// # use nix::sys::aio::*;
    /// # use nix::sys::signal::SigevNotify;
    /// # use std::{thread, time};
    /// # use std::io::Write;
    /// # use std::os::unix::io::AsFd;
    /// # use tempfile::tempfile;
    /// let wbuf = b"CDEF";
    /// let mut f = tempfile().unwrap();
    /// let mut aiocb = Box::pin(AioWrite::new(f.as_fd(),
    ///     2,   //offset
    ///     &wbuf[..],
    ///     0,   //priority
    ///     SigevNotify::SigevNone));
    /// aiocb.as_mut().submit().unwrap();
    /// let cs = aiocb.as_mut().cancel().unwrap();
    /// if cs == AioCancelStat::AioNotCanceled {
    ///     while (aiocb.as_mut().error() == Err(Errno::EINPROGRESS)) {
    ///         thread::sleep(time::Duration::from_millis(10));
    ///     }
    /// }
    /// // Must call `aio_return`, but ignore the result
    /// let _ = aiocb.as_mut().aio_return();
    /// ```
    ///
    /// # References
    ///
    /// [aio_cancel](https://pubs.opengroup.org/onlinepubs/9699919799/functions/aio_cancel.html)
    fn cancel(self: Pin<&mut Self>) -> Result<AioCancelStat>;

    /// Retrieve error status of an asynchronous operation.
    ///
    /// If the request has not yet completed, returns `EINPROGRESS`.  Otherwise,
    /// returns `Ok` or any other error.
    ///
    /// # Examples
    ///
    /// Issue an aio operation and use `error` to poll for completion.  Polling
    /// is an alternative to `aio_suspend`, used by most of the other examples.
    ///
    /// ```
    /// # use nix::errno::Errno;
    /// # use nix::Error;
    /// # use nix::sys::aio::*;
    /// # use nix::sys::signal::SigevNotify;
    /// # use std::{thread, time};
    /// # use std::os::unix::io::AsFd;
    /// # use tempfile::tempfile;
    /// const WBUF: &[u8] = b"abcdef123456";
    /// let mut f = tempfile().unwrap();
    /// let mut aiocb = Box::pin(AioWrite::new(f.as_fd(),
    ///     2,   //offset
    ///     WBUF,
    ///     0,   //priority
    ///     SigevNotify::SigevNone));
    /// aiocb.as_mut().submit().unwrap();
    /// while (aiocb.as_mut().error() == Err(Errno::EINPROGRESS)) {
    ///     thread::sleep(time::Duration::from_millis(10));
    /// }
    /// assert_eq!(aiocb.as_mut().aio_return().unwrap(), WBUF.len());
    /// ```
    ///
    /// # References
    ///
    /// [aio_error](https://pubs.opengroup.org/onlinepubs/9699919799/functions/aio_error.html)
    fn error(self: Pin<&mut Self>) -> Result<()>;

    /// Returns the underlying file descriptor associated with the operation.
    fn fd(&self) -> BorrowedFd;

    /// Does this operation currently have any in-kernel state?
    ///
    /// Dropping an operation that does have in-kernel state constitutes a
    /// resource leak.
    ///
    /// # Examples
    ///
    /// ```
    /// # use nix::errno::Errno;
    /// # use nix::Error;
    /// # use nix::sys::aio::*;
    /// # use nix::sys::signal::SigevNotify::SigevNone;
    /// # use std::{thread, time};
    /// # use std::os::unix::io::AsFd;
    /// # use tempfile::tempfile;
    /// let f = tempfile().unwrap();
    /// let mut aiof = Box::pin(AioFsync::new(f.as_fd(), AioFsyncMode::O_SYNC,
    ///     0, SigevNone));
    /// assert!(!aiof.as_mut().in_progress());
    /// aiof.as_mut().submit().expect("aio_fsync failed early");
    /// assert!(aiof.as_mut().in_progress());
    /// while (aiof.as_mut().error() == Err(Errno::EINPROGRESS)) {
    ///     thread::sleep(time::Duration::from_millis(10));
    /// }
    /// aiof.as_mut().aio_return().expect("aio_fsync failed late");
    /// assert!(!aiof.as_mut().in_progress());
    /// ```
    fn in_progress(&self) -> bool;

    /// Returns the priority of the `AioCb`
    fn priority(&self) -> i32;

    /// Update the notification settings for an existing AIO operation that has
    /// not yet been submitted.
    fn set_sigev_notify(&mut self, sev: SigevNotify);

    /// Returns the `SigEvent` that will be used for notification.
    fn sigevent(&self) -> SigEvent;

    /// Actually start the I/O operation.
    ///
    /// After calling this method and until [`Aio::aio_return`] returns `Ok`,
    /// the structure may not be moved in memory.
    fn submit(self: Pin<&mut Self>) -> Result<()>;
}

macro_rules! aio_methods {
    () => {
        fn cancel(self: Pin<&mut Self>) -> Result<AioCancelStat> {
            self.aiocb().cancel()
        }

        fn error(self: Pin<&mut Self>) -> Result<()> {
            self.aiocb().error()
        }

        fn fd(&self) -> BorrowedFd<'a> {
            // safe because self's lifetime is the same as the original file
            // descriptor.
            unsafe { BorrowedFd::borrow_raw(self.aiocb.aiocb.0.aio_fildes) }
        }

        fn in_progress(&self) -> bool {
            self.aiocb.in_progress()
        }

        fn priority(&self) -> i32 {
            self.aiocb.aiocb.0.aio_reqprio
        }

        fn set_sigev_notify(&mut self, sev: SigevNotify) {
            self.aiocb.set_sigev_notify(sev)
        }

        fn sigevent(&self) -> SigEvent {
            SigEvent::from(&self.aiocb.aiocb.0.aio_sigevent)
        }
    };
    ($func:ident) => {
        aio_methods!();

        fn aio_return(self: Pin<&mut Self>) -> Result<<Self as Aio>::Output> {
            self.aiocb().aio_return()
        }

        fn submit(mut self: Pin<&mut Self>) -> Result<()> {
            let p: *mut libc::aiocb = &mut self.as_mut().aiocb().aiocb.0;
            Errno::result({ unsafe { libc::$func(p) } }).map(|_| {
                self.aiocb().set_in_progress();
            })
        }
    };
}

/// An asynchronous version of `fsync(2)`.
///
/// # References
///
/// [aio_fsync](https://pubs.opengroup.org/onlinepubs/9699919799/functions/aio_fsync.html)
/// # Examples
///
/// ```
/// # use nix::errno::Errno;
/// # use nix::Error;
/// # use nix::sys::aio::*;
/// # use nix::sys::signal::SigevNotify::SigevNone;
/// # use std::{thread, time};
/// # use std::os::unix::io::AsFd;
/// # use tempfile::tempfile;
/// let f = tempfile().unwrap();
/// let mut aiof = Box::pin(AioFsync::new(f.as_fd(), AioFsyncMode::O_SYNC,
///     0, SigevNone));
/// aiof.as_mut().submit().expect("aio_fsync failed early");
/// while (aiof.as_mut().error() == Err(Errno::EINPROGRESS)) {
///     thread::sleep(time::Duration::from_millis(10));
/// }
/// aiof.as_mut().aio_return().expect("aio_fsync failed late");
/// ```
#[derive(Debug)]
#[repr(transparent)]
pub struct AioFsync<'a> {
    aiocb: AioCb<'a>,
    _pin: PhantomPinned,
}

impl<'a> AioFsync<'a> {
    unsafe_pinned!(aiocb: AioCb<'a>);

    /// Returns the operation's fsync mode: data and metadata or data only?
    pub fn mode(&self) -> AioFsyncMode {
        AioFsyncMode::try_from(self.aiocb.aiocb.0.aio_lio_opcode).unwrap()
    }

    /// Create a new `AioFsync`.
    ///
    /// # Arguments
    ///
    /// * `fd`:           File descriptor to sync.
    /// * `mode`:         Whether to sync file metadata too, or just data.
    /// * `prio`:         If POSIX Prioritized IO is supported, then the
    ///                   operation will be prioritized at the process's
    ///                   priority level minus `prio`.
    /// * `sigev_notify`: Determines how you will be notified of event
    ///                   completion.
    pub fn new(
        fd: BorrowedFd<'a>,
        mode: AioFsyncMode,
        prio: i32,
        sigev_notify: SigevNotify,
    ) -> Self {
        let mut aiocb = AioCb::common_init(fd, prio, sigev_notify);
        // To save some memory, store mode in an unused field of the AioCb.
        // True it isn't very much memory, but downstream creates will likely
        // create an enum containing this and other AioCb variants and pack
        // those enums into data structures like Vec, so it adds up.
        aiocb.aiocb.0.aio_lio_opcode = mode as libc::c_int;
        AioFsync {
            aiocb,
            _pin: PhantomPinned,
        }
    }
}

impl<'a> Aio for AioFsync<'a> {
    type Output = ();

    aio_methods!();

    fn aio_return(self: Pin<&mut Self>) -> Result<()> {
        self.aiocb().aio_return().map(drop)
    }

    fn submit(mut self: Pin<&mut Self>) -> Result<()> {
        let aiocb = &mut self.as_mut().aiocb().aiocb.0;
        let mode = mem::replace(&mut aiocb.aio_lio_opcode, 0);
        let p: *mut libc::aiocb = aiocb;
        Errno::result(unsafe { libc::aio_fsync(mode, p) }).map(|_| {
            self.aiocb().set_in_progress();
        })
    }
}

// AioFsync does not need AsMut, since it can't be used with lio_listio

impl<'a> AsRef<libc::aiocb> for AioFsync<'a> {
    fn as_ref(&self) -> &libc::aiocb {
        &self.aiocb.aiocb.0
    }
}

/// Asynchronously reads from a file descriptor into a buffer
///
/// # References
///
/// [aio_read](https://pubs.opengroup.org/onlinepubs/9699919799/functions/aio_read.html)
///
/// # Examples
///
///
/// ```
/// # use nix::errno::Errno;
/// # use nix::Error;
/// # use nix::sys::aio::*;
/// # use nix::sys::signal::SigevNotify;
/// # use std::{thread, time};
/// # use std::io::Write;
/// # use std::os::unix::io::AsFd;
/// # use tempfile::tempfile;
/// const INITIAL: &[u8] = b"abcdef123456";
/// const LEN: usize = 4;
/// let mut rbuf = vec![0; LEN];
/// let mut f = tempfile().unwrap();
/// f.write_all(INITIAL).unwrap();
/// {
///     let mut aior = Box::pin(
///         AioRead::new(
///             f.as_fd(),
///             2,   //offset
///             &mut rbuf,
///             0,   //priority
///             SigevNotify::SigevNone
///         )
///     );
///     aior.as_mut().submit().unwrap();
///     while (aior.as_mut().error() == Err(Errno::EINPROGRESS)) {
///         thread::sleep(time::Duration::from_millis(10));
///     }
///     assert_eq!(aior.as_mut().aio_return().unwrap(), LEN);
/// }
/// assert_eq!(rbuf, b"cdef");
/// ```
#[derive(Debug)]
#[repr(transparent)]
pub struct AioRead<'a> {
    aiocb: AioCb<'a>,
    _data: PhantomData<&'a [u8]>,
    _pin: PhantomPinned,
}

impl<'a> AioRead<'a> {
    unsafe_pinned!(aiocb: AioCb<'a>);

    /// Returns the requested length of the aio operation in bytes
    ///
    /// This method returns the *requested* length of the operation.  To get the
    /// number of bytes actually read or written by a completed operation, use
    /// `aio_return` instead.
    pub fn nbytes(&self) -> usize {
        self.aiocb.aiocb.0.aio_nbytes
    }

    /// Create a new `AioRead`, placing the data in a mutable slice.
    ///
    /// # Arguments
    ///
    /// * `fd`:           File descriptor to read from
    /// * `offs`:         File offset
    /// * `buf`:          A memory buffer.  It must outlive the `AioRead`.
    /// * `prio`:         If POSIX Prioritized IO is supported, then the
    ///                   operation will be prioritized at the process's
    ///                   priority level minus `prio`
    /// * `sigev_notify`: Determines how you will be notified of event
    ///                   completion.
    pub fn new(
        fd: BorrowedFd<'a>,
        offs: off_t,
        buf: &'a mut [u8],
        prio: i32,
        sigev_notify: SigevNotify,
    ) -> Self {
        let mut aiocb = AioCb::common_init(fd, prio, sigev_notify);
        aiocb.aiocb.0.aio_nbytes = buf.len();
        aiocb.aiocb.0.aio_buf = buf.as_mut_ptr().cast();
        aiocb.aiocb.0.aio_lio_opcode = libc::LIO_READ;
        aiocb.aiocb.0.aio_offset = offs;
        AioRead {
            aiocb,
            _data: PhantomData,
            _pin: PhantomPinned,
        }
    }

    /// Returns the file offset of the operation.
    pub fn offset(&self) -> off_t {
        self.aiocb.aiocb.0.aio_offset
    }
}

impl<'a> Aio for AioRead<'a> {
    type Output = usize;

    aio_methods!(aio_read);
}

impl<'a> AsMut<libc::aiocb> for AioRead<'a> {
    fn as_mut(&mut self) -> &mut libc::aiocb {
        &mut self.aiocb.aiocb.0
    }
}

impl<'a> AsRef<libc::aiocb> for AioRead<'a> {
    fn as_ref(&self) -> &libc::aiocb {
        &self.aiocb.aiocb.0
    }
}

/// Asynchronously reads from a file descriptor into a scatter/gather list of buffers.
///
/// # References
///
/// [aio_readv](https://www.freebsd.org/cgi/man.cgi?query=aio_readv)
///
/// # Examples
///
///
#[cfg_attr(fbsd14, doc = " ```")]
#[cfg_attr(not(fbsd14), doc = " ```no_run")]
/// # use nix::errno::Errno;
/// # use nix::Error;
/// # use nix::sys::aio::*;
/// # use nix::sys::signal::SigevNotify;
/// # use std::{thread, time};
/// # use std::io::{IoSliceMut, Write};
/// # use std::os::unix::io::AsFd;
/// # use tempfile::tempfile;
/// const INITIAL: &[u8] = b"abcdef123456";
/// let mut rbuf0 = vec![0; 4];
/// let mut rbuf1 = vec![0; 2];
/// let expected_len = rbuf0.len() + rbuf1.len();
/// let mut rbufs = [IoSliceMut::new(&mut rbuf0), IoSliceMut::new(&mut rbuf1)];
/// let mut f = tempfile().unwrap();
/// f.write_all(INITIAL).unwrap();
/// {
///     let mut aior = Box::pin(
///         AioReadv::new(
///             f.as_fd(),
///             2,   //offset
///             &mut rbufs,
///             0,   //priority
///             SigevNotify::SigevNone
///         )
///     );
///     aior.as_mut().submit().unwrap();
///     while (aior.as_mut().error() == Err(Errno::EINPROGRESS)) {
///         thread::sleep(time::Duration::from_millis(10));
///     }
///     assert_eq!(aior.as_mut().aio_return().unwrap(), expected_len);
/// }
/// assert_eq!(rbuf0, b"cdef");
/// assert_eq!(rbuf1, b"12");
/// ```
#[cfg(target_os = "freebsd")]
#[derive(Debug)]
#[repr(transparent)]
pub struct AioReadv<'a> {
    aiocb: AioCb<'a>,
    _data: PhantomData<&'a [&'a [u8]]>,
    _pin: PhantomPinned,
}

#[cfg(target_os = "freebsd")]
impl<'a> AioReadv<'a> {
    unsafe_pinned!(aiocb: AioCb<'a>);

    /// Returns the number of buffers the operation will read into.
    pub fn iovlen(&self) -> usize {
        self.aiocb.aiocb.0.aio_nbytes
    }

    /// Create a new `AioReadv`, placing the data in a list of mutable slices.
    ///
    /// # Arguments
    ///
    /// * `fd`:           File descriptor to read from
    /// * `offs`:         File offset
    /// * `bufs`:         A scatter/gather list of memory buffers.  They must
    ///                   outlive the `AioReadv`.
    /// * `prio`:         If POSIX Prioritized IO is supported, then the
    ///                   operation will be prioritized at the process's
    ///                   priority level minus `prio`
    /// * `sigev_notify`: Determines how you will be notified of event
    ///                   completion.
    pub fn new(
        fd: BorrowedFd<'a>,
        offs: off_t,
        bufs: &mut [IoSliceMut<'a>],
        prio: i32,
        sigev_notify: SigevNotify,
    ) -> Self {
        let mut aiocb = AioCb::common_init(fd, prio, sigev_notify);
        // In vectored mode, aio_nbytes stores the length of the iovec array,
        // not the byte count.
        aiocb.aiocb.0.aio_nbytes = bufs.len();
        aiocb.aiocb.0.aio_buf = bufs.as_mut_ptr().cast();
        aiocb.aiocb.0.aio_lio_opcode = libc::LIO_READV;
        aiocb.aiocb.0.aio_offset = offs;
        AioReadv {
            aiocb,
            _data: PhantomData,
            _pin: PhantomPinned,
        }
    }

    /// Returns the file offset of the operation.
    pub fn offset(&self) -> off_t {
        self.aiocb.aiocb.0.aio_offset
    }
}

#[cfg(target_os = "freebsd")]
impl<'a> Aio for AioReadv<'a> {
    type Output = usize;

    aio_methods!(aio_readv);
}

#[cfg(target_os = "freebsd")]
impl<'a> AsMut<libc::aiocb> for AioReadv<'a> {
    fn as_mut(&mut self) -> &mut libc::aiocb {
        &mut self.aiocb.aiocb.0
    }
}

#[cfg(target_os = "freebsd")]
impl<'a> AsRef<libc::aiocb> for AioReadv<'a> {
    fn as_ref(&self) -> &libc::aiocb {
        &self.aiocb.aiocb.0
    }
}

/// Asynchronously writes from a buffer to a file descriptor
///
/// # References
///
/// [aio_write](https://pubs.opengroup.org/onlinepubs/9699919799/functions/aio_write.html)
///
/// # Examples
///
/// ```
/// # use nix::errno::Errno;
/// # use nix::Error;
/// # use nix::sys::aio::*;
/// # use nix::sys::signal::SigevNotify;
/// # use std::{thread, time};
/// # use std::os::unix::io::AsFd;
/// # use tempfile::tempfile;
/// const WBUF: &[u8] = b"abcdef123456";
/// let mut f = tempfile().unwrap();
/// let mut aiow = Box::pin(
///     AioWrite::new(
///         f.as_fd(),
///         2,   //offset
///         WBUF,
///         0,   //priority
///         SigevNotify::SigevNone
///     )
/// );
/// aiow.as_mut().submit().unwrap();
/// while (aiow.as_mut().error() == Err(Errno::EINPROGRESS)) {
///     thread::sleep(time::Duration::from_millis(10));
/// }
/// assert_eq!(aiow.as_mut().aio_return().unwrap(), WBUF.len());
/// ```
#[derive(Debug)]
#[repr(transparent)]
pub struct AioWrite<'a> {
    aiocb: AioCb<'a>,
    _data: PhantomData<&'a [u8]>,
    _pin: PhantomPinned,
}

impl<'a> AioWrite<'a> {
    unsafe_pinned!(aiocb: AioCb<'a>);

    /// Returns the requested length of the aio operation in bytes
    ///
    /// This method returns the *requested* length of the operation.  To get the
    /// number of bytes actually read or written by a completed operation, use
    /// `aio_return` instead.
    pub fn nbytes(&self) -> usize {
        self.aiocb.aiocb.0.aio_nbytes
    }

    /// Construct a new `AioWrite`.
    ///
    /// # Arguments
    ///
    /// * `fd`:           File descriptor to write to
    /// * `offs`:         File offset
    /// * `buf`:          A memory buffer.  It must outlive the `AioWrite`.
    /// * `prio`:         If POSIX Prioritized IO is supported, then the
    ///                   operation will be prioritized at the process's
    ///                   priority level minus `prio`
    /// * `sigev_notify`: Determines how you will be notified of event
    ///                   completion.
    pub fn new(
        fd: BorrowedFd<'a>,
        offs: off_t,
        buf: &'a [u8],
        prio: i32,
        sigev_notify: SigevNotify,
    ) -> Self {
        let mut aiocb = AioCb::common_init(fd, prio, sigev_notify);
        aiocb.aiocb.0.aio_nbytes = buf.len();
        // casting an immutable buffer to a mutable pointer looks unsafe,
        // but technically its only unsafe to dereference it, not to create
        // it.  Type Safety guarantees that we'll never pass aiocb to
        // aio_read or aio_readv.
        aiocb.aiocb.0.aio_buf = buf.as_ptr().cast_mut().cast();
        aiocb.aiocb.0.aio_lio_opcode = libc::LIO_WRITE;
        aiocb.aiocb.0.aio_offset = offs;
        AioWrite {
            aiocb,
            _data: PhantomData,
            _pin: PhantomPinned,
        }
    }

    /// Returns the file offset of the operation.
    pub fn offset(&self) -> off_t {
        self.aiocb.aiocb.0.aio_offset
    }
}

impl<'a> Aio for AioWrite<'a> {
    type Output = usize;

    aio_methods!(aio_write);
}

impl<'a> AsMut<libc::aiocb> for AioWrite<'a> {
    fn as_mut(&mut self) -> &mut libc::aiocb {
        &mut self.aiocb.aiocb.0
    }
}

impl<'a> AsRef<libc::aiocb> for AioWrite<'a> {
    fn as_ref(&self) -> &libc::aiocb {
        &self.aiocb.aiocb.0
    }
}

/// Asynchronously writes from a scatter/gather list of buffers to a file descriptor.
///
/// # References
///
/// [aio_writev](https://www.freebsd.org/cgi/man.cgi?query=aio_writev)
///
/// # Examples
///
#[cfg_attr(fbsd14, doc = " ```")]
#[cfg_attr(not(fbsd14), doc = " ```no_run")]
/// # use nix::errno::Errno;
/// # use nix::Error;
/// # use nix::sys::aio::*;
/// # use nix::sys::signal::SigevNotify;
/// # use std::{thread, time};
/// # use std::io::IoSlice;
/// # use std::os::unix::io::AsFd;
/// # use tempfile::tempfile;
/// const wbuf0: &[u8] = b"abcdef";
/// const wbuf1: &[u8] = b"123456";
/// let len = wbuf0.len() + wbuf1.len();
/// let wbufs = [IoSlice::new(wbuf0), IoSlice::new(wbuf1)];
/// let mut f = tempfile().unwrap();
/// let mut aiow = Box::pin(
///     AioWritev::new(
///         f.as_fd(),
///         2,   //offset
///         &wbufs,
///         0,   //priority
///         SigevNotify::SigevNone
///     )
/// );
/// aiow.as_mut().submit().unwrap();
/// while (aiow.as_mut().error() == Err(Errno::EINPROGRESS)) {
///     thread::sleep(time::Duration::from_millis(10));
/// }
/// assert_eq!(aiow.as_mut().aio_return().unwrap(), len);
/// ```
#[cfg(target_os = "freebsd")]
#[derive(Debug)]
#[repr(transparent)]
pub struct AioWritev<'a> {
    aiocb: AioCb<'a>,
    _data: PhantomData<&'a [&'a [u8]]>,
    _pin: PhantomPinned,
}

#[cfg(target_os = "freebsd")]
impl<'a> AioWritev<'a> {
    unsafe_pinned!(aiocb: AioCb<'a>);

    /// Returns the number of buffers the operation will read into.
    pub fn iovlen(&self) -> usize {
        self.aiocb.aiocb.0.aio_nbytes
    }

    /// Construct a new `AioWritev`.
    ///
    /// # Arguments
    ///
    /// * `fd`:           File descriptor to write to
    /// * `offs`:         File offset
    /// * `bufs`:         A scatter/gather list of memory buffers.  They must
    ///                   outlive the `AioWritev`.
    /// * `prio`:         If POSIX Prioritized IO is supported, then the
    ///                   operation will be prioritized at the process's
    ///                   priority level minus `prio`
    /// * `sigev_notify`: Determines how you will be notified of event
    ///                   completion.
    pub fn new(
        fd: BorrowedFd<'a>,
        offs: off_t,
        bufs: &[IoSlice<'a>],
        prio: i32,
        sigev_notify: SigevNotify,
    ) -> Self {
        let mut aiocb = AioCb::common_init(fd, prio, sigev_notify);
        // In vectored mode, aio_nbytes stores the length of the iovec array,
        // not the byte count.
        aiocb.aiocb.0.aio_nbytes = bufs.len();
        // casting an immutable buffer to a mutable pointer looks unsafe,
        // but technically its only unsafe to dereference it, not to create
        // it.  Type Safety guarantees that we'll never pass aiocb to
        // aio_read or aio_readv.
        aiocb.aiocb.0.aio_buf = bufs.as_ptr().cast_mut().cast();
        aiocb.aiocb.0.aio_lio_opcode = libc::LIO_WRITEV;
        aiocb.aiocb.0.aio_offset = offs;
        AioWritev {
            aiocb,
            _data: PhantomData,
            _pin: PhantomPinned,
        }
    }

    /// Returns the file offset of the operation.
    pub fn offset(&self) -> off_t {
        self.aiocb.aiocb.0.aio_offset
    }
}

#[cfg(target_os = "freebsd")]
impl<'a> Aio for AioWritev<'a> {
    type Output = usize;

    aio_methods!(aio_writev);
}

#[cfg(target_os = "freebsd")]
impl<'a> AsMut<libc::aiocb> for AioWritev<'a> {
    fn as_mut(&mut self) -> &mut libc::aiocb {
        &mut self.aiocb.aiocb.0
    }
}

#[cfg(target_os = "freebsd")]
impl<'a> AsRef<libc::aiocb> for AioWritev<'a> {
    fn as_ref(&self) -> &libc::aiocb {
        &self.aiocb.aiocb.0
    }
}

/// Cancels outstanding AIO requests for a given file descriptor.
///
/// # Examples
///
/// Issue an aio operation, then cancel all outstanding operations on that file
/// descriptor.
///
/// ```
/// # use nix::errno::Errno;
/// # use nix::Error;
/// # use nix::sys::aio::*;
/// # use nix::sys::signal::SigevNotify;
/// # use std::{thread, time};
/// # use std::io::Write;
/// # use std::os::unix::io::AsFd;
/// # use tempfile::tempfile;
/// let wbuf = b"CDEF";
/// let mut f = tempfile().unwrap();
/// let mut aiocb = Box::pin(AioWrite::new(f.as_fd(),
///     2,   //offset
///     &wbuf[..],
///     0,   //priority
///     SigevNotify::SigevNone));
/// aiocb.as_mut().submit().unwrap();
/// let cs = aio_cancel_all(f.as_fd()).unwrap();
/// if cs == AioCancelStat::AioNotCanceled {
///     while (aiocb.as_mut().error() == Err(Errno::EINPROGRESS)) {
///         thread::sleep(time::Duration::from_millis(10));
///     }
/// }
/// // Must call `aio_return`, but ignore the result
/// let _ = aiocb.as_mut().aio_return();
/// ```
///
/// # References
///
/// [`aio_cancel`](https://pubs.opengroup.org/onlinepubs/9699919799/functions/aio_cancel.html)
pub fn aio_cancel_all<F: AsFd>(fd: F) -> Result<AioCancelStat> {
    match unsafe { libc::aio_cancel(fd.as_fd().as_raw_fd(), ptr::null_mut()) } {
        libc::AIO_CANCELED => Ok(AioCancelStat::AioCanceled),
        libc::AIO_NOTCANCELED => Ok(AioCancelStat::AioNotCanceled),
        libc::AIO_ALLDONE => Ok(AioCancelStat::AioAllDone),
        -1 => Err(Errno::last()),
        _ => panic!("unknown aio_cancel return value"),
    }
}

/// Suspends the calling process until at least one of the specified operations
/// have completed, a signal is delivered, or the timeout has passed.
///
/// If `timeout` is `None`, `aio_suspend` will block indefinitely.
///
/// # Examples
///
/// Use `aio_suspend` to block until an aio operation completes.
///
/// ```
/// # use nix::sys::aio::*;
/// # use nix::sys::signal::SigevNotify;
/// # use std::os::unix::io::AsFd;
/// # use tempfile::tempfile;
/// const WBUF: &[u8] = b"abcdef123456";
/// let mut f = tempfile().unwrap();
/// let mut aiocb = Box::pin(AioWrite::new(f.as_fd(),
///     2,   //offset
///     WBUF,
///     0,   //priority
///     SigevNotify::SigevNone));
/// aiocb.as_mut().submit().unwrap();
/// aio_suspend(&[&*aiocb], None).expect("aio_suspend failed");
/// assert_eq!(aiocb.as_mut().aio_return().unwrap(), WBUF.len());
/// ```
/// # References
///
/// [`aio_suspend`](https://pubs.opengroup.org/onlinepubs/9699919799/functions/aio_suspend.html)
pub fn aio_suspend(
    list: &[&dyn AsRef<libc::aiocb>],
    timeout: Option<TimeSpec>,
) -> Result<()> {
    // Note that this allocation could be eliminated by making the argument
    // generic, and accepting arguments like &[AioWrite].  But that would
    // prevent using aio_suspend to wait on a heterogeneous list of mixed
    // operations.
    let v = list
        .iter()
        .map(|x| x.as_ref() as *const libc::aiocb)
        .collect::<Vec<*const libc::aiocb>>();
    let p = v.as_ptr();
    let timep = match timeout {
        None => ptr::null::<libc::timespec>(),
        Some(x) => x.as_ref() as *const libc::timespec,
    };
    Errno::result(unsafe { libc::aio_suspend(p, list.len() as i32, timep) })
        .map(drop)
}

/// Submits multiple asynchronous I/O requests with a single system call.
///
/// They are not guaranteed to complete atomically, and the order in which the
/// requests are carried out is not specified. Reads, and writes may be freely
/// mixed.
///
/// # Examples
///
/// Use `lio_listio` to submit an aio operation and wait for its completion. In
/// this case, there is no need to use aio_suspend to wait or `error` to poll.
/// This mode is useful for otherwise-synchronous programs that want to execute
/// a handful of I/O operations in parallel.
/// ```
/// # use std::os::unix::io::AsFd;
/// # use nix::sys::aio::*;
/// # use nix::sys::signal::SigevNotify;
/// # use tempfile::tempfile;
/// const WBUF: &[u8] = b"abcdef123456";
/// let mut f = tempfile().unwrap();
/// let mut aiow = Box::pin(AioWrite::new(
///     f.as_fd(),
///     2,      // offset
///     WBUF,
///     0,      // priority
///     SigevNotify::SigevNone
/// ));
/// lio_listio(LioMode::LIO_WAIT, &mut[aiow.as_mut()], SigevNotify::SigevNone)
///     .unwrap();
/// // At this point, we are guaranteed that aiow is complete.
/// assert_eq!(aiow.as_mut().aio_return().unwrap(), WBUF.len());
/// ```
///
/// Use `lio_listio` to submit multiple asynchronous operations with a single
/// syscall, but receive notification individually.  This is an efficient
/// technique for reducing overall context-switch overhead, especially when
/// combined with kqueue.
/// ```
/// # use std::os::unix::io::AsFd;
/// # use std::thread;
/// # use std::time;
/// # use nix::errno::Errno;
/// # use nix::sys::aio::*;
/// # use nix::sys::signal::SigevNotify;
/// # use tempfile::tempfile;
/// const WBUF: &[u8] = b"abcdef123456";
/// let mut f = tempfile().unwrap();
/// let mut aiow = Box::pin(AioWrite::new(
///     f.as_fd(),
///     2,      // offset
///     WBUF,
///     0,      // priority
///     SigevNotify::SigevNone
/// ));
/// lio_listio(LioMode::LIO_NOWAIT, &mut[aiow.as_mut()], SigevNotify::SigevNone)
///     .unwrap();
/// // We must wait for the completion of each individual operation
/// while (aiow.as_mut().error() == Err(Errno::EINPROGRESS)) {
///     thread::sleep(time::Duration::from_millis(10));
/// }
/// assert_eq!(aiow.as_mut().aio_return().unwrap(), WBUF.len());
/// ```
///
/// Use `lio_listio` to submit multiple operations, and receive notification
/// only when all of them are complete.  This can be useful when there is some
/// logical relationship between the operations.  But beware!  Errors or system
/// resource limitations may cause `lio_listio` to return `EIO`, `EAGAIN`, or
/// `EINTR`, in which case some but not all operations may have been submitted.
/// In that case, you must check the status of each individual operation, and
/// possibly resubmit some.
/// ```
/// # use libc::c_int;
/// # use std::os::unix::io::AsFd;
/// # use std::sync::atomic::{AtomicBool, Ordering};
/// # use std::thread;
/// # use std::time;
/// # use nix::errno::Errno;
/// # use nix::sys::aio::*;
/// # use nix::sys::signal::*;
/// # use tempfile::tempfile;
/// pub static SIGNALED: AtomicBool = AtomicBool::new(false);
///
/// extern fn sigfunc(_: c_int) {
///     SIGNALED.store(true, Ordering::Relaxed);
/// }
/// let sa = SigAction::new(SigHandler::Handler(sigfunc),
///                         SaFlags::SA_RESETHAND,
///                         SigSet::empty());
/// SIGNALED.store(false, Ordering::Relaxed);
/// unsafe { sigaction(Signal::SIGUSR2, &sa) }.unwrap();
///
/// const WBUF: &[u8] = b"abcdef123456";
/// let mut f = tempfile().unwrap();
/// let mut aiow = Box::pin(AioWrite::new(
///     f.as_fd(),
///     2,      // offset
///     WBUF,
///     0,      // priority
///     SigevNotify::SigevNone
/// ));
/// let sev = SigevNotify::SigevSignal { signal: Signal::SIGUSR2, si_value: 0 };
/// lio_listio(LioMode::LIO_NOWAIT, &mut[aiow.as_mut()], sev).unwrap();
/// while !SIGNALED.load(Ordering::Relaxed) {
///     thread::sleep(time::Duration::from_millis(10));
/// }
/// // At this point, since `lio_listio` returned success and delivered its
/// // notification, we know that all operations are complete.
/// assert_eq!(aiow.as_mut().aio_return().unwrap(), WBUF.len());
/// ```
#[deprecated(
    since = "0.27.0",
    note = "https://github.com/nix-rust/nix/issues/2017"
)]
pub fn lio_listio(
    mode: LioMode,
    list: &mut [Pin<&mut dyn AsMut<libc::aiocb>>],
    sigev_notify: SigevNotify,
) -> Result<()> {
    let p = list as *mut [Pin<&mut dyn AsMut<libc::aiocb>>]
        as *mut [*mut libc::aiocb] as *mut *mut libc::aiocb;
    let sigev = SigEvent::new(sigev_notify);
    let sigevp = &mut sigev.sigevent() as *mut libc::sigevent;
    Errno::result(unsafe {
        libc::lio_listio(mode as i32, p, list.len() as i32, sigevp)
    })
    .map(drop)
}
