//! An API for interfacing with `kqueue`.

use crate::fd::{AsFd, OwnedFd, RawFd};
use crate::pid::Pid;
use crate::signal::Signal;
use crate::{backend, io};

use backend::c::{self, intptr_t, kevent as kevent_t, uintptr_t};
use backend::event::syscalls;

use alloc::vec::Vec;
use core::mem::zeroed;
use core::ptr::slice_from_raw_parts_mut;
use core::time::Duration;

/// A `kqueue` event for use with [`kevent`].
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Event {
    // The layout varies between BSDs and macOS.
    inner: kevent_t,
}

impl Event {
    /// Create a new `Event`.
    #[allow(clippy::needless_update)]
    pub fn new(filter: EventFilter, flags: EventFlags, udata: isize) -> Event {
        let (ident, data, filter, fflags) = match filter {
            EventFilter::Read(fd) => (fd as uintptr_t, 0, c::EVFILT_READ, 0),
            EventFilter::Write(fd) => (fd as _, 0, c::EVFILT_WRITE, 0),
            #[cfg(target_os = "freebsd")]
            EventFilter::Empty(fd) => (fd as _, 0, c::EVFILT_EMPTY, 0),
            EventFilter::Vnode { vnode, flags } => (vnode as _, 0, c::EVFILT_VNODE, flags.bits()),
            EventFilter::Proc { pid, flags } => {
                (Pid::as_raw(Some(pid)) as _, 0, c::EVFILT_PROC, flags.bits())
            }
            EventFilter::Signal { signal, times: _ } => (signal as _, 0, c::EVFILT_SIGNAL, 0),
            EventFilter::Timer { ident, timer } => {
                #[cfg(any(apple, target_os = "freebsd", target_os = "netbsd"))]
                let (data, fflags) = match timer {
                    Some(timer) => {
                        if timer.subsec_millis() == 0 {
                            (timer.as_secs() as _, c::NOTE_SECONDS)
                        } else if timer.subsec_nanos() == 0 {
                            (timer.as_micros() as _, c::NOTE_USECONDS)
                        } else {
                            (timer.as_nanos() as _, c::NOTE_NSECONDS)
                        }
                    }
                    None => (intptr_t::MAX, c::NOTE_SECONDS),
                };
                #[cfg(any(target_os = "dragonfly", target_os = "openbsd"))]
                let (data, fflags) = match timer {
                    Some(timer) => (timer.as_millis() as _, 0),
                    None => (intptr_t::MAX, 0),
                };

                (ident as _, data, c::EVFILT_TIMER, fflags)
            }
            #[cfg(any(apple, freebsdlike))]
            EventFilter::User {
                ident,
                flags,
                user_flags,
            } => (ident as _, 0, c::EVFILT_USER, flags.bits() | user_flags.0),
            EventFilter::Unknown => panic!("unknown filter"),
        };

        Event {
            inner: kevent_t {
                ident,
                filter: filter as _,
                flags: flags.bits() as _,
                fflags,
                data: {
                    // On OpenBSD, data is an `i64` and not an `isize`.
                    data as _
                },
                udata: {
                    // On NetBSD, udata is an `isize` and not a pointer.
                    // TODO: Strict provenance, prevent int-to-ptr cast.
                    udata as _
                },
                ..unsafe { zeroed() }
            },
        }
    }

    /// Get the event flags for this event.
    pub fn flags(&self) -> EventFlags {
        EventFlags::from_bits_retain(self.inner.flags as _)
    }

    /// Get the user data for this event.
    pub fn udata(&self) -> isize {
        // On NetBSD, udata is an isize and not a pointer.
        // TODO: Strict provenance, prevent ptr-to-int cast.

        self.inner.udata as _
    }

    /// Get the raw data for this event.
    pub fn data(&self) -> i64 {
        // On some BSDs, data is an `isize` and not an `i64`.
        self.inner.data as _
    }

    /// Get the filter of this event.
    pub fn filter(&self) -> EventFilter {
        match self.inner.filter as _ {
            c::EVFILT_READ => EventFilter::Read(self.inner.ident as _),
            c::EVFILT_WRITE => EventFilter::Write(self.inner.ident as _),
            #[cfg(target_os = "freebsd")]
            c::EVFILT_EMPTY => EventFilter::Empty(self.inner.ident as _),
            c::EVFILT_VNODE => EventFilter::Vnode {
                vnode: self.inner.ident as _,
                flags: VnodeEvents::from_bits_retain(self.inner.fflags),
            },
            c::EVFILT_PROC => EventFilter::Proc {
                pid: Pid::from_raw(self.inner.ident as _).unwrap(),
                flags: ProcessEvents::from_bits_retain(self.inner.fflags),
            },
            c::EVFILT_SIGNAL => EventFilter::Signal {
                signal: Signal::from_raw(self.inner.ident as _).unwrap(),
                times: self.inner.data as _,
            },
            c::EVFILT_TIMER => EventFilter::Timer {
                ident: self.inner.ident as _,
                timer: {
                    let (data, fflags) = (self.inner.data, self.inner.fflags);
                    #[cfg(not(any(apple, target_os = "freebsd", target_os = "netbsd")))]
                    let _ = fflags;
                    #[cfg(any(apple, target_os = "freebsd", target_os = "netbsd"))]
                    match fflags as _ {
                        c::NOTE_SECONDS => Some(Duration::from_secs(data as _)),
                        c::NOTE_USECONDS => Some(Duration::from_micros(data as _)),
                        c::NOTE_NSECONDS => Some(Duration::from_nanos(data as _)),
                        _ => {
                            // Unknown timer flags.
                            None
                        }
                    }
                    #[cfg(any(target_os = "dragonfly", target_os = "openbsd"))]
                    Some(Duration::from_millis(data as _))
                },
            },
            #[cfg(any(apple, freebsdlike))]
            c::EVFILT_USER => EventFilter::User {
                ident: self.inner.ident as _,
                flags: UserFlags::from_bits_retain(self.inner.fflags),
                user_flags: UserDefinedFlags(self.inner.fflags & EVFILT_USER_FLAGS),
            },
            _ => EventFilter::Unknown,
        }
    }
}

/// Bottom 24 bits of a u32.
#[cfg(any(apple, freebsdlike))]
const EVFILT_USER_FLAGS: u32 = 0x00ff_ffff;

/// The possible filters for a `kqueue`.
#[repr(i16)]
#[non_exhaustive]
pub enum EventFilter {
    /// A read filter.
    Read(RawFd),

    /// A write filter.
    Write(RawFd),

    /// An empty filter.
    #[cfg(target_os = "freebsd")]
    Empty(RawFd),

    /// A VNode filter.
    Vnode {
        /// The file descriptor we looked for events in.
        vnode: RawFd,

        /// The flags for this event.
        flags: VnodeEvents,
    },

    /// A process filter.
    Proc {
        /// The process ID we waited on.
        pid: Pid,

        /// The flags for this event.
        flags: ProcessEvents,
    },

    /// A signal filter.
    Signal {
        /// The signal number we waited on.
        signal: Signal,

        /// The number of times the signal has been received since the last
        /// call to kevent.
        times: usize,
    },

    /// A timer filter.
    Timer {
        /// The identifier for this event.
        ident: intptr_t,

        /// The duration for this event.
        timer: Option<Duration>,
    },

    /// A user filter.
    #[cfg(any(apple, freebsdlike))]
    User {
        /// The identifier for this event.
        ident: intptr_t,

        /// The flags for this event.
        flags: UserFlags,

        /// The user-defined flags for this event.
        user_flags: UserDefinedFlags,
    },

    /// This filter is unknown.
    ///
    /// # Panics
    ///
    /// Passing this into `Event::new()` will result in a panic.
    Unknown,
}

bitflags::bitflags! {
    /// The flags for a `kqueue` event specifying actions to perform.
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct EventFlags: u16 {
        /// Add the event to the `kqueue`.
        const ADD = c::EV_ADD as _;

        /// Enable the event.
        const ENABLE = c::EV_ENABLE as _;

        /// Disable the event.
        const DISABLE = c::EV_DISABLE as _;

        /// Delete the event from the `kqueue`.
        const DELETE = c::EV_DELETE as _;

        /// TODO
        const RECEIPT = c::EV_RECEIPT as _;

        /// Clear the event after it is triggered.
        const ONESHOT = c::EV_ONESHOT as _;

        /// TODO
        const CLEAR = c::EV_CLEAR as _;

        /// TODO
        const EOF = c::EV_EOF as _;

        /// TODO
        const ERROR = c::EV_ERROR as _;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags::bitflags! {
    /// The flags for a virtual node event.
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct VnodeEvents: u32 {
        /// The file was deleted.
        const DELETE = c::NOTE_DELETE;

        /// The file was written to.
        const WRITE = c::NOTE_WRITE;

        /// The file was extended.
        const EXTEND = c::NOTE_EXTEND;

        /// The file had its attributes changed.
        const ATTRIBUTES = c::NOTE_ATTRIB;

        /// The file was renamed.
        const RENAME = c::NOTE_RENAME;

        /// Access to the file was revoked.
        const REVOKE = c::NOTE_REVOKE;

        /// The link count of the file has changed.
        const LINK = c::NOTE_LINK;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags::bitflags! {
    /// The flags for a process event.
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct ProcessEvents: u32 {
        /// The process exited.
        const EXIT = c::NOTE_EXIT;

        /// The process forked itself.
        const FORK = c::NOTE_FORK;

        /// The process executed a new process.
        const EXEC = c::NOTE_EXEC;

        /// Follow the process through `fork` calls (write only).
        const TRACK = c::NOTE_TRACK;

        /// An error has occurred with following the process.
        const TRACKERR = c::NOTE_TRACKERR;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

#[cfg(any(apple, freebsdlike))]
bitflags::bitflags! {
    /// The flags for a user event.
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct UserFlags: u32 {
        /// Ignore the user input flags.
        const NOINPUT = c::NOTE_FFNOP;

        /// Bitwise AND `fflags`.
        const AND = c::NOTE_FFAND;

        /// Bitwise OR `fflags`.
        const OR = c::NOTE_FFOR;

        /// Copy `fflags`.
        const COPY = c::NOTE_FFCOPY;

        /// Control mask for operations.
        const CTRLMASK = c::NOTE_FFCTRLMASK;

        /// User defined flags for masks.
        const UDFMASK = c::NOTE_FFLAGSMASK;

        /// Trigger the event.
        const TRIGGER = c::NOTE_TRIGGER;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

/// User-defined flags.
///
/// Only the lower 24 bits are used in this struct.
#[repr(transparent)]
#[cfg(any(apple, freebsdlike))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UserDefinedFlags(u32);

#[cfg(any(apple, freebsdlike))]
impl UserDefinedFlags {
    /// Create a new `UserDefinedFlags` from a `u32`.
    pub fn new(flags: u32) -> Self {
        Self(flags & EVFILT_USER_FLAGS)
    }

    /// Get the underlying `u32`.
    pub fn get(self) -> u32 {
        self.0
    }
}

/// `kqueue()`—Create a new `kqueue` file descriptor.
///
/// # References
///  - [Apple]
///  - [FreeBSD]
///  - [OpenBSD]
///  - [NetBSD]
///  - [DragonFly BSD]
///
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/kqueue.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=kqueue&sektion=2
/// [OpenBSD]: https://man.openbsd.org/kqueue.2
/// [NetBSD]: https://man.netbsd.org/kqueue.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=kqueue&section=2
pub fn kqueue() -> io::Result<OwnedFd> {
    syscalls::kqueue()
}

/// `kevent(kqueue, changelist, eventlist, timeout)`—Wait for events on a
/// `kqueue`.
///
/// Note: in order to receive events, make sure to allocate capacity in the
/// eventlist! Otherwise, the function will return immediately.
///
/// # Safety
///
/// The file descriptors referred to by the `Event` structs must be valid for
/// the lifetime of the `kqueue` file descriptor.
///
/// # References
///  - [Apple]
///  - [FreeBSD]
///  - [OpenBSD]
///  - [NetBSD]
///  - [DragonFly BSD]
///
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/kevent.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=kevent&sektion=2
/// [OpenBSD]: https://man.openbsd.org/kevent.2
/// [NetBSD]: https://man.netbsd.org/kevent.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=kevent&section=2
pub unsafe fn kevent(
    kqueue: impl AsFd,
    changelist: &[Event],
    eventlist: &mut Vec<Event>,
    timeout: Option<Duration>,
) -> io::Result<usize> {
    let timeout = timeout.map(|timeout| backend::c::timespec {
        tv_sec: timeout.as_secs() as _,
        tv_nsec: timeout.subsec_nanos() as _,
    });

    // Populate the event list with events.
    eventlist.set_len(0);
    let out_slice = slice_from_raw_parts_mut(eventlist.as_mut_ptr().cast(), eventlist.capacity());
    let res = syscalls::kevent(
        kqueue.as_fd(),
        changelist,
        &mut *out_slice,
        timeout.as_ref(),
    )
    .map(|res| res as _);

    // Update the event list.
    if let Ok(len) = res {
        eventlist.set_len(len);
    }

    res
}
