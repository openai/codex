//! Kernel event notification mechanism
//!
//! # See Also
//! [kqueue(2)](https://www.freebsd.org/cgi/man.cgi?query=kqueue)

use crate::{Errno, Result};
#[cfg(not(target_os = "netbsd"))]
use libc::{c_int, c_long, intptr_t, time_t, timespec, uintptr_t};
#[cfg(target_os = "netbsd")]
use libc::{c_long, intptr_t, size_t, time_t, timespec, uintptr_t};
use std::convert::TryInto;
use std::mem;
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd};
use std::ptr;

/// A kernel event queue.  Used to notify a process of various asynchronous
/// events.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct KEvent {
    kevent: libc::kevent,
}

/// A kernel event queue.
///
/// Used by the kernel to notify the process of various types of asynchronous
/// events.
#[repr(transparent)]
#[derive(Debug)]
pub struct Kqueue(OwnedFd);

impl AsFd for Kqueue {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl From<Kqueue> for OwnedFd {
    fn from(value: Kqueue) -> Self {
        value.0
    }
}

impl Kqueue {
    /// Create a new kernel event queue.
    pub fn new() -> Result<Self> {
        let res = unsafe { libc::kqueue() };

        Errno::result(res).map(|fd| unsafe { Self(OwnedFd::from_raw_fd(fd)) })
    }

    /// Register new events with the kqueue, and return any pending events to
    /// the user.
    ///
    /// This method will block until either the timeout expires, or a registered
    /// event triggers a notification.
    ///
    /// # Arguments
    /// - `changelist` - Any new kevents to register for notifications.
    /// - `eventlist` - Storage space for the kernel to return notifications.
    /// - `timeout` - An optional timeout.
    ///
    /// # Returns
    /// Returns the number of events placed in the `eventlist`.  If an error
    /// occurs while processing an element of the `changelist` and there is
    /// enough room in the `eventlist`, then the event will be placed in the
    /// `eventlist` with `EV_ERROR` set in `flags` and the system error in
    /// `data`.
    pub fn kevent(
        &self,
        changelist: &[KEvent],
        eventlist: &mut [KEvent],
        timeout_opt: Option<timespec>,
    ) -> Result<usize> {
        let res = unsafe {
            libc::kevent(
                self.0.as_raw_fd(),
                changelist.as_ptr().cast(),
                changelist.len() as type_of_nchanges,
                eventlist.as_mut_ptr().cast(),
                eventlist.len() as type_of_nchanges,
                if let Some(ref timeout) = timeout_opt {
                    timeout as *const timespec
                } else {
                    ptr::null()
                },
            )
        };
        Errno::result(res).map(|r| r as usize)
    }
}

#[cfg(any(freebsdlike, apple_targets, target_os = "openbsd"))]
type type_of_udata = *mut libc::c_void;
#[cfg(target_os = "netbsd")]
type type_of_udata = intptr_t;

#[cfg(target_os = "netbsd")]
type type_of_event_filter = u32;
#[cfg(not(target_os = "netbsd"))]
type type_of_event_filter = i16;
libc_enum! {
    #[cfg_attr(target_os = "netbsd", repr(u32))]
    #[cfg_attr(not(target_os = "netbsd"), repr(i16))]
    #[non_exhaustive]
    /// Kqueue filter types.  These are all the different types of event that a
    /// kqueue can notify for.
    pub enum EventFilter {
        /// Notifies on the completion of a POSIX AIO operation.
        EVFILT_AIO,
        #[cfg(target_os = "freebsd")]
        /// Returns whenever there is no remaining data in the write buffer
        EVFILT_EMPTY,
        #[cfg(target_os = "dragonfly")]
        /// Takes a descriptor as the identifier, and returns whenever one of
        /// the specified exceptional conditions has occurred on the descriptor.
        EVFILT_EXCEPT,
        #[cfg(any(freebsdlike, apple_targets))]
        /// Establishes a file system monitor.
        EVFILT_FS,
        #[cfg(target_os = "freebsd")]
        /// Notify for completion of a list of POSIX AIO operations.
        /// # See Also
        /// [lio_listio(2)](https://www.freebsd.org/cgi/man.cgi?query=lio_listio)
        EVFILT_LIO,
        #[cfg(apple_targets)]
        /// Mach portsets
        EVFILT_MACHPORT,
        /// Notifies when a process performs one or more of the requested
        /// events.
        EVFILT_PROC,
        /// Returns events associated with the process referenced by a given
        /// process descriptor, created by `pdfork()`. The events to monitor are:
        ///
        /// - NOTE_EXIT: the process has exited. The exit status will be stored in data.
        #[cfg(target_os = "freebsd")]
        EVFILT_PROCDESC,
        /// Takes a file descriptor as the identifier, and notifies whenever
        /// there is data available to read.
        EVFILT_READ,
        #[cfg(target_os = "freebsd")]
        #[doc(hidden)]
        #[deprecated(since = "0.27.0", note = "Never fully implemented by the OS")]
        EVFILT_SENDFILE,
        /// Takes a signal number to monitor as the identifier and notifies when
        /// the given signal is delivered to the process.
        EVFILT_SIGNAL,
        /// Establishes a timer and notifies when the timer expires.
        EVFILT_TIMER,
        #[cfg(any(freebsdlike, apple_targets))]
        /// Notifies only when explicitly requested by the user.
        EVFILT_USER,
        #[cfg(apple_targets)]
        /// Virtual memory events
        EVFILT_VM,
        /// Notifies when a requested event happens on a specified file.
        EVFILT_VNODE,
        /// Takes a file descriptor as the identifier, and notifies whenever
        /// it is possible to write to the file without blocking.
        EVFILT_WRITE,
    }
    impl TryFrom<type_of_event_filter>
}

#[cfg(any(freebsdlike, apple_targets, target_os = "openbsd"))]
#[doc(hidden)]
pub type type_of_event_flag = u16;
#[cfg(target_os = "netbsd")]
#[doc(hidden)]
pub type type_of_event_flag = u32;
libc_bitflags! {
    /// Event flags.  See the man page for details.
    // There's no useful documentation we can write for the individual flags
    // that wouldn't simply be repeating the man page.
    pub struct EventFlag: type_of_event_flag {
        #[allow(missing_docs)]
        EV_ADD;
        #[allow(missing_docs)]
        EV_CLEAR;
        #[allow(missing_docs)]
        EV_DELETE;
        #[allow(missing_docs)]
        EV_DISABLE;
        #[cfg(bsd)]
        #[allow(missing_docs)]
        EV_DISPATCH;
        #[cfg(target_os = "freebsd")]
        #[allow(missing_docs)]
        EV_DROP;
        #[allow(missing_docs)]
        EV_ENABLE;
        #[allow(missing_docs)]
        EV_EOF;
        #[allow(missing_docs)]
        EV_ERROR;
        #[cfg(apple_targets)]
        #[allow(missing_docs)]
        EV_FLAG0;
        #[allow(missing_docs)]
        EV_FLAG1;
        #[cfg(target_os = "dragonfly")]
        #[allow(missing_docs)]
        EV_NODATA;
        #[allow(missing_docs)]
        EV_ONESHOT;
        #[cfg(apple_targets)]
        #[allow(missing_docs)]
        EV_OOBAND;
        #[cfg(apple_targets)]
        #[allow(missing_docs)]
        EV_POLL;
        #[cfg(bsd)]
        #[allow(missing_docs)]
        EV_RECEIPT;
    }
}

libc_bitflags!(
    /// Filter-specific flags.  See the man page for details.
    // There's no useful documentation we can write for the individual flags
    // that wouldn't simply be repeating the man page.
    #[allow(missing_docs)]
    pub struct FilterFlag: u32 {
        #[cfg(apple_targets)]
        #[allow(missing_docs)]
        NOTE_ABSOLUTE;
        #[allow(missing_docs)]
        NOTE_ATTRIB;
        #[allow(missing_docs)]
        NOTE_CHILD;
        #[allow(missing_docs)]
        NOTE_DELETE;
        #[cfg(target_os = "openbsd")]
        #[allow(missing_docs)]
        NOTE_EOF;
        #[allow(missing_docs)]
        NOTE_EXEC;
        #[allow(missing_docs)]
        NOTE_EXIT;
        #[cfg(apple_targets)]
        #[allow(missing_docs)]
        NOTE_EXITSTATUS;
        #[allow(missing_docs)]
        NOTE_EXTEND;
        #[cfg(any(apple_targets, freebsdlike))]
        #[allow(missing_docs)]
        NOTE_FFAND;
        #[cfg(any(apple_targets, freebsdlike))]
        #[allow(missing_docs)]
        NOTE_FFCOPY;
        #[cfg(any(apple_targets, freebsdlike))]
        #[allow(missing_docs)]
        NOTE_FFCTRLMASK;
        #[cfg(any(apple_targets, freebsdlike))]
        #[allow(missing_docs)]
        NOTE_FFLAGSMASK;
        #[cfg(any(apple_targets, freebsdlike))]
        #[allow(missing_docs)]
        NOTE_FFNOP;
        #[cfg(any(apple_targets, freebsdlike))]
        #[allow(missing_docs)]
        NOTE_FFOR;
        #[allow(missing_docs)]
        NOTE_FORK;
        #[allow(missing_docs)]
        NOTE_LINK;
        #[allow(missing_docs)]
        NOTE_LOWAT;
        #[cfg(target_os = "freebsd")]
        #[allow(missing_docs)]
        NOTE_MSECONDS;
        #[cfg(apple_targets)]
        #[allow(missing_docs)]
        NOTE_NONE;
        #[cfg(any(
            apple_targets,
            target_os = "freebsd"))]
        #[allow(missing_docs)]
        NOTE_NSECONDS;
        #[cfg(target_os = "dragonfly")]
        #[allow(missing_docs)]
        NOTE_OOB;
        #[allow(missing_docs)]
        NOTE_PCTRLMASK;
        #[allow(missing_docs)]
        NOTE_PDATAMASK;
        #[allow(missing_docs)]
        NOTE_RENAME;
        #[allow(missing_docs)]
        NOTE_REVOKE;
        #[cfg(any(
            apple_targets,
            target_os = "freebsd"))]
        #[allow(missing_docs)]
        NOTE_SECONDS;
        #[cfg(apple_targets)]
        #[allow(missing_docs)]
        NOTE_SIGNAL;
        #[allow(missing_docs)]
        NOTE_TRACK;
        #[allow(missing_docs)]
        NOTE_TRACKERR;
        #[cfg(any(apple_targets, freebsdlike))]
        #[allow(missing_docs)]
        NOTE_TRIGGER;
        #[cfg(target_os = "openbsd")]
        #[allow(missing_docs)]
        NOTE_TRUNCATE;
        #[cfg(any(
            apple_targets,
            target_os = "freebsd"))]
        #[allow(missing_docs)]
        NOTE_USECONDS;
        #[cfg(apple_targets)]
        #[allow(missing_docs)]
        NOTE_VM_ERROR;
        #[cfg(apple_targets)]
        #[allow(missing_docs)]
        NOTE_VM_PRESSURE;
        #[cfg(apple_targets)]
        #[allow(missing_docs)]
        NOTE_VM_PRESSURE_SUDDEN_TERMINATE;
        #[cfg(apple_targets)]
        #[allow(missing_docs)]
        NOTE_VM_PRESSURE_TERMINATE;
        #[allow(missing_docs)]
        NOTE_WRITE;
    }
);

#[allow(missing_docs)]
#[deprecated(since = "0.27.0", note = "Use KEvent::new instead")]
pub fn kqueue() -> Result<Kqueue> {
    Kqueue::new()
}

// KEvent can't derive Send because on some operating systems, udata is defined
// as a void*.  However, KEvent's public API always treats udata as an intptr_t,
// which is safe to Send.
unsafe impl Send for KEvent {}

impl KEvent {
    #[allow(clippy::needless_update)] // Not needless on all platforms.
    /// Construct a new `KEvent` suitable for submission to the kernel via the
    /// `changelist` argument of [`Kqueue::kevent`].
    pub fn new(
        ident: uintptr_t,
        filter: EventFilter,
        flags: EventFlag,
        fflags: FilterFlag,
        data: intptr_t,
        udata: intptr_t,
    ) -> KEvent {
        KEvent {
            kevent: libc::kevent {
                ident,
                filter: filter as type_of_event_filter,
                flags: flags.bits(),
                fflags: fflags.bits(),
                // data can be either i64 or intptr_t, depending on platform
                data: data as _,
                udata: udata as type_of_udata,
                ..unsafe { mem::zeroed() }
            },
        }
    }

    /// Value used to identify this event.  The exact interpretation is
    /// determined by the attached filter, but often is a raw file descriptor.
    pub fn ident(&self) -> uintptr_t {
        self.kevent.ident
    }

    /// Identifies the kernel filter used to process this event.
    ///
    /// Will only return an error if the kernel reports an event via a filter
    /// that is unknown to Nix.
    pub fn filter(&self) -> Result<EventFilter> {
        self.kevent.filter.try_into()
    }

    /// Flags control what the kernel will do when this event is added with
    /// [`Kqueue::kevent`].
    pub fn flags(&self) -> EventFlag {
        EventFlag::from_bits(self.kevent.flags).unwrap()
    }

    /// Filter-specific flags.
    pub fn fflags(&self) -> FilterFlag {
        FilterFlag::from_bits(self.kevent.fflags).unwrap()
    }

    /// Filter-specific data value.
    pub fn data(&self) -> intptr_t {
        self.kevent.data as intptr_t
    }

    /// Opaque user-defined value passed through the kernel unchanged.
    pub fn udata(&self) -> intptr_t {
        self.kevent.udata as intptr_t
    }
}

#[allow(missing_docs)]
#[deprecated(since = "0.27.0", note = "Use Kqueue::kevent instead")]
pub fn kevent(
    kq: &Kqueue,
    changelist: &[KEvent],
    eventlist: &mut [KEvent],
    timeout_ms: usize,
) -> Result<usize> {
    // Convert ms to timespec
    let timeout = timespec {
        tv_sec: (timeout_ms / 1000) as time_t,
        tv_nsec: ((timeout_ms % 1000) * 1_000_000) as c_long,
    };

    kq.kevent(changelist, eventlist, Some(timeout))
}

#[cfg(any(apple_targets, freebsdlike, target_os = "openbsd"))]
type type_of_nchanges = c_int;
#[cfg(target_os = "netbsd")]
type type_of_nchanges = size_t;

#[allow(missing_docs)]
#[deprecated(since = "0.27.0", note = "Use Kqueue::kevent instead")]
pub fn kevent_ts(
    kq: &Kqueue,
    changelist: &[KEvent],
    eventlist: &mut [KEvent],
    timeout_opt: Option<timespec>,
) -> Result<usize> {
    kq.kevent(changelist, eventlist, timeout_opt)
}

/// Modify an existing [`KEvent`].
// Probably should deprecate.  Would anybody ever use it over `KEvent::new`?
#[deprecated(since = "0.27.0", note = "Use Kqueue::kevent instead")]
#[inline]
pub fn ev_set(
    ev: &mut KEvent,
    ident: usize,
    filter: EventFilter,
    flags: EventFlag,
    fflags: FilterFlag,
    udata: intptr_t,
) {
    ev.kevent.ident = ident as uintptr_t;
    ev.kevent.filter = filter as type_of_event_filter;
    ev.kevent.flags = flags.bits();
    ev.kevent.fflags = fflags.bits();
    ev.kevent.data = 0;
    ev.kevent.udata = udata as type_of_udata;
}
