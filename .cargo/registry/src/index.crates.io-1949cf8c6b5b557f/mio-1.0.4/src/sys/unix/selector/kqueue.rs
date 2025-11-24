use crate::{Interest, Token};
use std::mem::{self, MaybeUninit};
use std::ops::{Deref, DerefMut};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
#[cfg(debug_assertions)]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use std::{cmp, io, ptr, slice};

/// Unique id for use as `SelectorId`.
#[cfg(debug_assertions)]
static NEXT_ID: AtomicUsize = AtomicUsize::new(1);

// Type of the `nchanges` and `nevents` parameters in the `kevent` function.
#[cfg(not(target_os = "netbsd"))]
type Count = libc::c_int;
#[cfg(target_os = "netbsd")]
type Count = libc::size_t;

// Type of the `filter` field in the `kevent` structure.
#[cfg(any(target_os = "dragonfly", target_os = "freebsd", target_os = "openbsd"))]
type Filter = libc::c_short;
#[cfg(any(
    target_os = "ios",
    target_os = "macos",
    target_os = "tvos",
    target_os = "visionos",
    target_os = "watchos"
))]
type Filter = i16;
#[cfg(target_os = "netbsd")]
type Filter = u32;

// Type of the `flags` field in the `kevent` structure.
#[cfg(any(target_os = "dragonfly", target_os = "freebsd", target_os = "openbsd"))]
type Flags = libc::c_ushort;
#[cfg(any(
    target_os = "ios",
    target_os = "macos",
    target_os = "tvos",
    target_os = "visionos",
    target_os = "watchos"
))]
type Flags = u16;
#[cfg(target_os = "netbsd")]
type Flags = u32;

// Type of the `udata` field in the `kevent` structure.
#[cfg(not(target_os = "netbsd"))]
type UData = *mut libc::c_void;
#[cfg(target_os = "netbsd")]
type UData = libc::intptr_t;

macro_rules! kevent {
    ($id: expr, $filter: expr, $flags: expr, $data: expr) => {
        libc::kevent {
            ident: $id as libc::uintptr_t,
            filter: $filter as Filter,
            flags: $flags,
            udata: $data as UData,
            ..unsafe { mem::zeroed() }
        }
    };
}

#[derive(Debug)]
pub struct Selector {
    #[cfg(debug_assertions)]
    id: usize,
    kq: OwnedFd,
}

impl Selector {
    pub fn new() -> io::Result<Selector> {
        // SAFETY: `kqueue(2)` ensures the fd is valid.
        let kq = unsafe { OwnedFd::from_raw_fd(syscall!(kqueue())?) };
        syscall!(fcntl(kq.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC))?;
        Ok(Selector {
            #[cfg(debug_assertions)]
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            kq,
        })
    }

    pub fn try_clone(&self) -> io::Result<Selector> {
        self.kq.try_clone().map(|kq| Selector {
            // It's the same selector, so we use the same id.
            #[cfg(debug_assertions)]
            id: self.id,
            kq,
        })
    }

    pub fn select(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()> {
        let timeout = timeout.map(|to| libc::timespec {
            tv_sec: cmp::min(to.as_secs(), libc::time_t::MAX as u64) as libc::time_t,
            // `Duration::subsec_nanos` is guaranteed to be less than one
            // billion (the number of nanoseconds in a second), making the
            // cast to i32 safe. The cast itself is needed for platforms
            // where C's long is only 32 bits.
            tv_nsec: libc::c_long::from(to.subsec_nanos() as i32),
        });
        let timeout = timeout
            .as_ref()
            .map(|s| s as *const _)
            .unwrap_or(ptr::null_mut());

        events.clear();
        syscall!(kevent(
            self.kq.as_raw_fd(),
            ptr::null(),
            0,
            events.as_mut_ptr(),
            events.capacity() as Count,
            timeout,
        ))
        .map(|n_events| {
            // This is safe because `kevent` ensures that `n_events` are
            // assigned.
            unsafe { events.set_len(n_events as usize) };
        })
    }

    pub fn register(&self, fd: RawFd, token: Token, interests: Interest) -> io::Result<()> {
        let flags = libc::EV_CLEAR | libc::EV_RECEIPT | libc::EV_ADD;
        // At most we need two changes, but maybe we only need 1.
        let mut changes: [MaybeUninit<libc::kevent>; 2] =
            [MaybeUninit::uninit(), MaybeUninit::uninit()];
        let mut n_changes = 0;

        if interests.is_writable() {
            let kevent = kevent!(fd, libc::EVFILT_WRITE, flags, token.0);
            changes[n_changes] = MaybeUninit::new(kevent);
            n_changes += 1;
        }

        if interests.is_readable() {
            let kevent = kevent!(fd, libc::EVFILT_READ, flags, token.0);
            changes[n_changes] = MaybeUninit::new(kevent);
            n_changes += 1;
        }

        // Older versions of macOS (OS X 10.11 and 10.10 have been witnessed)
        // can return EPIPE when registering a pipe file descriptor where the
        // other end has already disappeared. For example code that creates a
        // pipe, closes a file descriptor, and then registers the other end will
        // see an EPIPE returned from `register`.
        //
        // It also turns out that kevent will still report events on the file
        // descriptor, telling us that it's readable/hup at least after we've
        // done this registration. As a result we just ignore `EPIPE` here
        // instead of propagating it.
        //
        // More info can be found at tokio-rs/mio#582.
        let changes = unsafe {
            // This is safe because we ensure that at least `n_changes` are in
            // the array.
            slice::from_raw_parts_mut(changes[0].as_mut_ptr(), n_changes)
        };
        kevent_register(self.kq.as_raw_fd(), changes, &[libc::EPIPE as i64])
    }

    pub fn reregister(&self, fd: RawFd, token: Token, interests: Interest) -> io::Result<()> {
        let flags = libc::EV_CLEAR | libc::EV_RECEIPT;
        let write_flags = if interests.is_writable() {
            flags | libc::EV_ADD
        } else {
            flags | libc::EV_DELETE
        };
        let read_flags = if interests.is_readable() {
            flags | libc::EV_ADD
        } else {
            flags | libc::EV_DELETE
        };

        let mut changes: [libc::kevent; 2] = [
            kevent!(fd, libc::EVFILT_WRITE, write_flags, token.0),
            kevent!(fd, libc::EVFILT_READ, read_flags, token.0),
        ];

        // Since there is no way to check with which interests the fd was
        // registered we modify both readable and write, adding it when required
        // and removing it otherwise, ignoring the ENOENT error when it comes
        // up. The ENOENT error informs us that a filter we're trying to remove
        // wasn't there in first place, but we don't really care since our goal
        // is accomplished.
        //
        // For the explanation of ignoring `EPIPE` see `register`.
        kevent_register(
            self.kq.as_raw_fd(),
            &mut changes,
            &[libc::ENOENT as i64, libc::EPIPE as i64],
        )
    }

    pub fn deregister(&self, fd: RawFd) -> io::Result<()> {
        let flags = libc::EV_DELETE | libc::EV_RECEIPT;
        let mut changes: [libc::kevent; 2] = [
            kevent!(fd, libc::EVFILT_WRITE, flags, 0),
            kevent!(fd, libc::EVFILT_READ, flags, 0),
        ];

        // Since there is no way to check with which interests the fd was
        // registered we remove both filters (readable and writeable) and ignore
        // the ENOENT error when it comes up. The ENOENT error informs us that
        // the filter wasn't there in first place, but we don't really care
        // about that since our goal is to remove it.
        kevent_register(self.kq.as_raw_fd(), &mut changes, &[libc::ENOENT as i64])
    }

    // Used by `Waker`.
    #[cfg(any(
        target_os = "freebsd",
        target_os = "ios",
        target_os = "macos",
        target_os = "tvos",
        target_os = "visionos",
        target_os = "watchos"
    ))]
    pub fn setup_waker(&self, token: Token) -> io::Result<()> {
        // First attempt to accept user space notifications.
        let mut kevent = kevent!(
            0,
            libc::EVFILT_USER,
            libc::EV_ADD | libc::EV_CLEAR | libc::EV_RECEIPT,
            token.0
        );

        let kq = self.kq.as_raw_fd();
        syscall!(kevent(kq, &kevent, 1, &mut kevent, 1, ptr::null())).and_then(|_| {
            if (kevent.flags & libc::EV_ERROR) != 0 && kevent.data != 0 {
                Err(io::Error::from_raw_os_error(kevent.data as i32))
            } else {
                Ok(())
            }
        })
    }

    // Used by `Waker`.
    #[cfg(any(
        target_os = "freebsd",
        target_os = "ios",
        target_os = "macos",
        target_os = "tvos",
        target_os = "visionos",
        target_os = "watchos"
    ))]
    pub fn wake(&self, token: Token) -> io::Result<()> {
        let mut kevent = kevent!(
            0,
            libc::EVFILT_USER,
            libc::EV_ADD | libc::EV_RECEIPT,
            token.0
        );
        kevent.fflags = libc::NOTE_TRIGGER;

        let kq = self.kq.as_raw_fd();
        syscall!(kevent(kq, &kevent, 1, &mut kevent, 1, ptr::null())).and_then(|_| {
            if (kevent.flags & libc::EV_ERROR) != 0 && kevent.data != 0 {
                Err(io::Error::from_raw_os_error(kevent.data as i32))
            } else {
                Ok(())
            }
        })
    }
}

/// Register `changes` with `kq`ueue.
fn kevent_register(
    kq: RawFd,
    changes: &mut [libc::kevent],
    ignored_errors: &[i64],
) -> io::Result<()> {
    syscall!(kevent(
        kq,
        changes.as_ptr(),
        changes.len() as Count,
        changes.as_mut_ptr(),
        changes.len() as Count,
        ptr::null(),
    ))
    .map(|_| ())
    .or_else(|err| {
        // According to the manual page of FreeBSD: "When kevent() call fails
        // with EINTR error, all changes in the changelist have been applied",
        // so we can safely ignore it.
        if err.raw_os_error() == Some(libc::EINTR) {
            Ok(())
        } else {
            Err(err)
        }
    })
    .and_then(|()| check_errors(changes, ignored_errors))
}

/// Check all events for possible errors, it returns the first error found.
fn check_errors(events: &[libc::kevent], ignored_errors: &[i64]) -> io::Result<()> {
    for event in events {
        // We can't use references to packed structures (in checking the ignored
        // errors), so we need copy the data out before use.
        let data = event.data as _;
        // Check for the error flag, the actual error will be in the `data`
        // field.
        if (event.flags & libc::EV_ERROR != 0) && data != 0 && !ignored_errors.contains(&data) {
            return Err(io::Error::from_raw_os_error(data as i32));
        }
    }
    Ok(())
}

cfg_io_source! {
    #[cfg(debug_assertions)]
    impl Selector {
        pub fn id(&self) -> usize {
            self.id
        }
    }
}

impl AsRawFd for Selector {
    fn as_raw_fd(&self) -> RawFd {
        self.kq.as_raw_fd()
    }
}

pub type Event = libc::kevent;
pub struct Events(Vec<libc::kevent>);

impl Events {
    pub fn with_capacity(capacity: usize) -> Events {
        Events(Vec::with_capacity(capacity))
    }
}

impl Deref for Events {
    type Target = Vec<libc::kevent>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Events {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// `Events` cannot derive `Send` or `Sync` because of the
// `udata: *mut ::c_void` field in `libc::kevent`. However, `Events`'s public
// API treats the `udata` field as a `uintptr_t` which is `Send`. `Sync` is
// safe because with a `events: &Events` value, the only access to the `udata`
// field is through `fn token(event: &Event)` which cannot mutate the field.
unsafe impl Send for Events {}
unsafe impl Sync for Events {}

pub mod event {
    use std::fmt;

    use crate::sys::Event;
    use crate::Token;

    use super::{Filter, Flags};

    pub fn token(event: &Event) -> Token {
        Token(event.udata as usize)
    }

    pub fn is_readable(event: &Event) -> bool {
        event.filter == libc::EVFILT_READ || {
            #[cfg(any(
                target_os = "freebsd",
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos"
            ))]
            // Used by the `Awakener`. On platforms that use `eventfd` or a unix
            // pipe it will emit a readable event so we'll fake that here as
            // well.
            {
                event.filter == libc::EVFILT_USER
            }
            #[cfg(not(any(
                target_os = "freebsd",
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos"
            )))]
            {
                false
            }
        }
    }

    pub fn is_writable(event: &Event) -> bool {
        event.filter == libc::EVFILT_WRITE
    }

    pub fn is_error(event: &Event) -> bool {
        (event.flags & libc::EV_ERROR) != 0 ||
            // When the read end of the socket is closed, EV_EOF is set on
            // flags, and fflags contains the error if there is one.
            (event.flags & libc::EV_EOF) != 0 && event.fflags != 0
    }

    pub fn is_read_closed(event: &Event) -> bool {
        event.filter == libc::EVFILT_READ && event.flags & libc::EV_EOF != 0
    }

    pub fn is_write_closed(event: &Event) -> bool {
        event.filter == libc::EVFILT_WRITE && event.flags & libc::EV_EOF != 0
    }

    pub fn is_priority(_: &Event) -> bool {
        // kqueue doesn't have priority indicators.
        false
    }

    #[allow(unused_variables)] // `event` is not used on some platforms.
    pub fn is_aio(event: &Event) -> bool {
        #[cfg(any(
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "ios",
            target_os = "macos",
            target_os = "tvos",
            target_os = "visionos",
            target_os = "watchos",
        ))]
        {
            event.filter == libc::EVFILT_AIO
        }
        #[cfg(not(any(
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "ios",
            target_os = "macos",
            target_os = "tvos",
            target_os = "visionos",
            target_os = "watchos",
        )))]
        {
            false
        }
    }

    #[allow(unused_variables)] // `event` is only used on FreeBSD.
    pub fn is_lio(event: &Event) -> bool {
        #[cfg(target_os = "freebsd")]
        {
            event.filter == libc::EVFILT_LIO
        }
        #[cfg(not(target_os = "freebsd"))]
        {
            false
        }
    }

    pub fn debug_details(f: &mut fmt::Formatter<'_>, event: &Event) -> fmt::Result {
        debug_detail!(
            FilterDetails(Filter),
            PartialEq::eq,
            libc::EVFILT_READ,
            libc::EVFILT_WRITE,
            libc::EVFILT_AIO,
            libc::EVFILT_VNODE,
            libc::EVFILT_PROC,
            libc::EVFILT_SIGNAL,
            libc::EVFILT_TIMER,
            #[cfg(target_os = "freebsd")]
            libc::EVFILT_PROCDESC,
            #[cfg(any(
                target_os = "freebsd",
                target_os = "dragonfly",
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos",
            ))]
            libc::EVFILT_FS,
            #[cfg(target_os = "freebsd")]
            libc::EVFILT_LIO,
            #[cfg(any(
                target_os = "freebsd",
                target_os = "dragonfly",
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos",
            ))]
            libc::EVFILT_USER,
            #[cfg(target_os = "freebsd")]
            libc::EVFILT_SENDFILE,
            #[cfg(target_os = "freebsd")]
            libc::EVFILT_EMPTY,
            #[cfg(target_os = "dragonfly")]
            libc::EVFILT_EXCEPT,
            #[cfg(any(
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos"
            ))]
            libc::EVFILT_MACHPORT,
            #[cfg(any(
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos"
            ))]
            libc::EVFILT_VM,
        );

        #[allow(clippy::trivially_copy_pass_by_ref)]
        fn check_flag(got: &Flags, want: &Flags) -> bool {
            (got & want) != 0
        }
        debug_detail!(
            FlagsDetails(Flags),
            check_flag,
            libc::EV_ADD,
            libc::EV_DELETE,
            libc::EV_ENABLE,
            libc::EV_DISABLE,
            libc::EV_ONESHOT,
            libc::EV_CLEAR,
            libc::EV_RECEIPT,
            libc::EV_DISPATCH,
            #[cfg(target_os = "freebsd")]
            libc::EV_DROP,
            libc::EV_FLAG1,
            libc::EV_ERROR,
            libc::EV_EOF,
            // Not stable across OS versions on OpenBSD.
            #[cfg(not(target_os = "openbsd"))]
            libc::EV_SYSFLAGS,
            #[cfg(any(
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos"
            ))]
            libc::EV_FLAG0,
            #[cfg(any(
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos"
            ))]
            libc::EV_POLL,
            #[cfg(any(
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos"
            ))]
            libc::EV_OOBAND,
            #[cfg(target_os = "dragonfly")]
            libc::EV_NODATA,
        );

        #[allow(clippy::trivially_copy_pass_by_ref)]
        fn check_fflag(got: &u32, want: &u32) -> bool {
            (got & want) != 0
        }
        debug_detail!(
            FflagsDetails(u32),
            check_fflag,
            #[cfg(any(
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos",
            ))]
            libc::NOTE_TRIGGER,
            #[cfg(any(
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos",
            ))]
            libc::NOTE_FFNOP,
            #[cfg(any(
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos",
            ))]
            libc::NOTE_FFAND,
            #[cfg(any(
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos",
            ))]
            libc::NOTE_FFOR,
            #[cfg(any(
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos",
            ))]
            libc::NOTE_FFCOPY,
            #[cfg(any(
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos",
            ))]
            libc::NOTE_FFCTRLMASK,
            #[cfg(any(
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos",
            ))]
            libc::NOTE_FFLAGSMASK,
            libc::NOTE_LOWAT,
            libc::NOTE_DELETE,
            libc::NOTE_WRITE,
            #[cfg(target_os = "dragonfly")]
            libc::NOTE_OOB,
            #[cfg(target_os = "openbsd")]
            libc::NOTE_EOF,
            #[cfg(any(
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos"
            ))]
            libc::NOTE_EXTEND,
            libc::NOTE_ATTRIB,
            libc::NOTE_LINK,
            libc::NOTE_RENAME,
            libc::NOTE_REVOKE,
            #[cfg(any(
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos"
            ))]
            libc::NOTE_NONE,
            #[cfg(any(target_os = "openbsd"))]
            libc::NOTE_TRUNCATE,
            libc::NOTE_EXIT,
            libc::NOTE_FORK,
            libc::NOTE_EXEC,
            #[cfg(any(
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos"
            ))]
            libc::NOTE_SIGNAL,
            #[cfg(any(
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos"
            ))]
            libc::NOTE_EXITSTATUS,
            #[cfg(any(
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos"
            ))]
            libc::NOTE_EXIT_DETAIL,
            libc::NOTE_PDATAMASK,
            libc::NOTE_PCTRLMASK,
            #[cfg(any(
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd",
            ))]
            libc::NOTE_TRACK,
            #[cfg(any(
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd",
            ))]
            libc::NOTE_TRACKERR,
            #[cfg(any(
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd",
            ))]
            libc::NOTE_CHILD,
            #[cfg(any(
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos"
            ))]
            libc::NOTE_EXIT_DETAIL_MASK,
            #[cfg(any(
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos"
            ))]
            libc::NOTE_EXIT_DECRYPTFAIL,
            #[cfg(any(
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos"
            ))]
            libc::NOTE_EXIT_MEMORY,
            #[cfg(any(
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos"
            ))]
            libc::NOTE_EXIT_CSERROR,
            #[cfg(any(
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos"
            ))]
            libc::NOTE_VM_PRESSURE,
            #[cfg(any(
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos"
            ))]
            libc::NOTE_VM_PRESSURE_TERMINATE,
            #[cfg(any(
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos"
            ))]
            libc::NOTE_VM_PRESSURE_SUDDEN_TERMINATE,
            #[cfg(any(
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos"
            ))]
            libc::NOTE_VM_ERROR,
            #[cfg(any(
                target_os = "freebsd",
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos"
            ))]
            libc::NOTE_SECONDS,
            #[cfg(any(target_os = "freebsd"))]
            libc::NOTE_MSECONDS,
            #[cfg(any(
                target_os = "freebsd",
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos"
            ))]
            libc::NOTE_USECONDS,
            #[cfg(any(
                target_os = "freebsd",
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos"
            ))]
            libc::NOTE_NSECONDS,
            #[cfg(any(
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos"
            ))]
            libc::NOTE_ABSOLUTE,
            #[cfg(any(
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos"
            ))]
            libc::NOTE_LEEWAY,
            #[cfg(any(
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos"
            ))]
            libc::NOTE_CRITICAL,
            #[cfg(any(
                target_os = "ios",
                target_os = "macos",
                target_os = "tvos",
                target_os = "visionos",
                target_os = "watchos"
            ))]
            libc::NOTE_BACKGROUND,
        );

        // Can't reference fields in packed structures.
        let ident = event.ident;
        let data = event.data;
        let udata = event.udata;
        f.debug_struct("kevent")
            .field("ident", &ident)
            .field("filter", &FilterDetails(event.filter))
            .field("flags", &FlagsDetails(event.flags))
            .field("fflags", &FflagsDetails(event.fflags))
            .field("data", &data)
            .field("udata", &udata)
            .finish()
    }
}

// No special requirement from the implementation around waking.
pub(crate) use crate::sys::unix::waker::Waker;

cfg_io_source! {
    mod stateless_io_source;
    pub(crate) use stateless_io_source::IoSourceState;
}

#[test]
#[cfg(feature = "os-ext")]
fn does_not_register_rw() {
    use crate::unix::SourceFd;
    use crate::{Poll, Token};

    let kq = unsafe { libc::kqueue() };
    let mut kqf = SourceFd(&kq);
    let poll = Poll::new().unwrap();

    // Registering kqueue fd will fail if write is requested (On anything but
    // some versions of macOS).
    poll.registry()
        .register(&mut kqf, Token(1234), Interest::READABLE)
        .unwrap();
}
