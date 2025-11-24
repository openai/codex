//! Bindings to poll (VxWorks, Fuchsia, other Unix systems).

use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::Instant;

#[cfg(not(target_os = "hermit"))]
use rustix::fd::{AsFd, AsRawFd, BorrowedFd};
#[cfg(target_os = "hermit")]
use std::os::hermit::io::{AsFd, AsRawFd, BorrowedFd};

use syscall::{poll, PollFd, PollFlags};

// std::os::unix doesn't exist on Fuchsia
type RawFd = std::os::raw::c_int;

use crate::{Event, PollMode};

/// Interface to poll.
#[derive(Debug)]
pub struct Poller {
    /// File descriptors to poll.
    fds: Mutex<Fds>,
    /// Notification pipe for waking up the poller.
    ///
    /// On all platforms except ESP IDF, the `pipe` syscall is used.
    /// On ESP IDF, the `eventfd` syscall is used instead.
    notify: notify::Notify,
    /// The number of operations (`add`, `modify` or `delete`) that are currently waiting on the
    /// mutex to become free. When this is nonzero, `wait` must be suspended until it reaches zero
    /// again.
    waiting_operations: AtomicUsize,
    /// Whether `wait` has been notified by the user.
    notified: AtomicBool,
    /// The condition variable that gets notified when `waiting_operations` reaches zero or
    /// `notified` becomes true.
    ///
    /// This is used with the `fds` mutex.
    operations_complete: Condvar,
}

/// The file descriptors to poll in a `Poller`.
#[derive(Debug)]
struct Fds {
    /// The list of `pollfds` taken by poll.
    ///
    /// The first file descriptor is always present and is used to notify the poller. It is also
    /// stored in `notify_read`.
    poll_fds: Vec<PollFd<'static>>,
    /// The map of each file descriptor to data associated with it. This does not include the file
    /// descriptors `notify_read` or `notify_write`.
    fd_data: HashMap<RawFd, FdData>,
}

/// Data associated with a file descriptor in a poller.
#[derive(Debug)]
struct FdData {
    /// The index into `poll_fds` this file descriptor is.
    poll_fds_index: usize,
    /// The key of the `Event` associated with this file descriptor.
    key: usize,
    /// Whether to remove this file descriptor from the poller on the next call to `wait`.
    remove: bool,
}

impl Poller {
    /// Creates a new poller.
    pub fn new() -> io::Result<Poller> {
        let notify = notify::Notify::new()?;

        #[cfg(feature = "tracing")]
        tracing::trace!(?notify, "new");

        Ok(Self {
            fds: Mutex::new(Fds {
                poll_fds: vec![PollFd::from_borrowed_fd(
                    // SAFETY: `notify.fd()` will remain valid until we drop `self`.
                    unsafe { BorrowedFd::borrow_raw(notify.fd().as_raw_fd()) },
                    notify.poll_flags(),
                )],
                fd_data: HashMap::new(),
            }),
            notify,
            waiting_operations: AtomicUsize::new(0),
            operations_complete: Condvar::new(),
            notified: AtomicBool::new(false),
        })
    }

    /// Whether this poller supports level-triggered events.
    pub fn supports_level(&self) -> bool {
        true
    }

    /// Whether the poller supports edge-triggered events.
    pub fn supports_edge(&self) -> bool {
        false
    }

    /// Adds a new file descriptor.
    pub fn add(&self, fd: RawFd, ev: Event, mode: PollMode) -> io::Result<()> {
        if self.notify.has_fd(fd) {
            return Err(io::Error::from(io::ErrorKind::InvalidInput));
        }

        #[cfg(feature = "tracing")]
        let span = tracing::trace_span!(
            "add",
            notify_read = ?self.notify.fd().as_raw_fd(),
            ?fd,
            ?ev,
        );
        #[cfg(feature = "tracing")]
        let _enter = span.enter();

        self.modify_fds(|fds| {
            if fds.fd_data.contains_key(&fd) {
                return Err(io::Error::from(io::ErrorKind::AlreadyExists));
            }

            let poll_fds_index = fds.poll_fds.len();
            fds.fd_data.insert(
                fd,
                FdData {
                    poll_fds_index,
                    key: ev.key,
                    remove: cvt_mode_as_remove(mode)?,
                },
            );

            fds.poll_fds.push(PollFd::from_borrowed_fd(
                // SAFETY: Until we have I/O safety, assume that `fd` is valid forever.
                unsafe { BorrowedFd::borrow_raw(fd) },
                poll_events(ev),
            ));

            Ok(())
        })
    }

    /// Modifies an existing file descriptor.
    pub fn modify(&self, fd: BorrowedFd<'_>, ev: Event, mode: PollMode) -> io::Result<()> {
        if self.notify.has_fd(fd.as_raw_fd()) {
            return Err(io::Error::from(io::ErrorKind::InvalidInput));
        }

        #[cfg(feature = "tracing")]
        let span = tracing::trace_span!(
            "modify",
            notify_read = ?self.notify.fd().as_raw_fd(),
            ?fd,
            ?ev,
        );
        #[cfg(feature = "tracing")]
        let _enter = span.enter();

        self.modify_fds(|fds| {
            let data = fds
                .fd_data
                .get_mut(&fd.as_raw_fd())
                .ok_or(io::ErrorKind::NotFound)?;
            data.key = ev.key;
            let poll_fds_index = data.poll_fds_index;

            // SAFETY: This is essentially transmuting a `PollFd<'a>` to a `PollFd<'static>`, which
            // only works if it's removed in time with `delete()`.
            fds.poll_fds[poll_fds_index] = PollFd::from_borrowed_fd(
                unsafe { BorrowedFd::borrow_raw(fd.as_raw_fd()) },
                poll_events(ev),
            );
            data.remove = cvt_mode_as_remove(mode)?;

            Ok(())
        })
    }

    /// Deletes a file descriptor.
    pub fn delete(&self, fd: BorrowedFd<'_>) -> io::Result<()> {
        if self.notify.has_fd(fd.as_raw_fd()) {
            return Err(io::Error::from(io::ErrorKind::InvalidInput));
        }

        #[cfg(feature = "tracing")]
        let span = tracing::trace_span!(
            "delete",
            notify_read = ?self.notify.fd().as_raw_fd(),
            ?fd,
        );
        #[cfg(feature = "tracing")]
        let _enter = span.enter();

        self.modify_fds(|fds| {
            let data = fds
                .fd_data
                .remove(&fd.as_raw_fd())
                .ok_or(io::ErrorKind::NotFound)?;
            fds.poll_fds.swap_remove(data.poll_fds_index);
            if let Some(swapped_pollfd) = fds.poll_fds.get(data.poll_fds_index) {
                fds.fd_data
                    .get_mut(&swapped_pollfd.as_fd().as_raw_fd())
                    .unwrap()
                    .poll_fds_index = data.poll_fds_index;
            }

            Ok(())
        })
    }

    /// Waits for I/O events with an optional deadline.
    pub fn wait_deadline(&self, events: &mut Events, deadline: Option<Instant>) -> io::Result<()> {
        #[cfg(feature = "tracing")]
        let span = tracing::trace_span!(
            "wait",
            notify_read = ?self.notify.fd().as_raw_fd(),
            ?deadline,
        );
        #[cfg(feature = "tracing")]
        let _enter = span.enter();

        let mut fds = self.fds.lock().unwrap();

        loop {
            // Complete all current operations.
            loop {
                if self.notified.swap(false, Ordering::SeqCst) {
                    // `notify` will have sent a notification in case we were polling. We weren't,
                    // so remove it.
                    return self.notify.pop_notification();
                } else if self.waiting_operations.load(Ordering::SeqCst) == 0 {
                    break;
                }

                fds = self.operations_complete.wait(fds).unwrap();
            }

            let timeout =
                deadline.map(|deadline| deadline.saturating_duration_since(Instant::now()));

            // Perform the poll.
            let num_events = poll(&mut fds.poll_fds, timeout)?;
            let notified = !fds.poll_fds[0].revents().is_empty();
            let num_fd_events = if notified { num_events - 1 } else { num_events };
            #[cfg(feature = "tracing")]
            tracing::trace!(?num_events, ?notified, ?num_fd_events, "new events",);

            // Read all notifications.
            if notified {
                self.notify.pop_all_notifications()?;
            }

            // If the only event that occurred during polling was notification and it wasn't to
            // exit, another thread is trying to perform an operation on the fds. Continue the
            // loop.
            if !self.notified.swap(false, Ordering::SeqCst) && num_fd_events == 0 && notified {
                continue;
            }

            // Store the events if there were any.
            if num_fd_events > 0 {
                let fds = &mut *fds;

                events.inner.reserve(num_fd_events);
                for fd_data in fds.fd_data.values_mut() {
                    let poll_fd = &mut fds.poll_fds[fd_data.poll_fds_index];
                    if !poll_fd.revents().is_empty() {
                        // Store event
                        let revents = poll_fd.revents();
                        events.inner.push(Event {
                            key: fd_data.key,
                            readable: revents.intersects(read_events()),
                            writable: revents.intersects(write_events()),
                            extra: EventExtra { flags: revents },
                        });
                        // Remove interest if necessary
                        if fd_data.remove {
                            *poll_fd = PollFd::from_borrowed_fd(
                                unsafe { BorrowedFd::borrow_raw(poll_fd.as_fd().as_raw_fd()) },
                                PollFlags::empty(),
                            );
                        }

                        if events.inner.len() == num_fd_events {
                            break;
                        }
                    }
                }
            }

            break;
        }

        Ok(())
    }

    /// Sends a notification to wake up the current or next `wait()` call.
    pub fn notify(&self) -> io::Result<()> {
        #[cfg(feature = "tracing")]
        let span = tracing::trace_span!(
            "notify",
            notify_read = ?self.notify.fd().as_raw_fd(),
        );
        #[cfg(feature = "tracing")]
        let _enter = span.enter();

        if !self.notified.swap(true, Ordering::SeqCst) {
            self.notify.notify()?;
            self.operations_complete.notify_one();
        }

        Ok(())
    }

    /// Perform a modification on `fds`, interrupting the current caller of `wait` if it's running.
    fn modify_fds(&self, f: impl FnOnce(&mut Fds) -> io::Result<()>) -> io::Result<()> {
        self.waiting_operations.fetch_add(1, Ordering::SeqCst);

        // Wake up the current caller of `wait` if there is one.
        let sent_notification = self.notify.notify().is_ok();

        let mut fds = self.fds.lock().unwrap();

        // If there was no caller of `wait` our notification was not removed from the pipe.
        if sent_notification {
            let _ = self.notify.pop_notification();
        }

        let res = f(&mut fds);

        if self.waiting_operations.fetch_sub(1, Ordering::SeqCst) == 1 {
            self.operations_complete.notify_one();
        }

        res
    }
}

/// Get the input poll events for the given event.
fn poll_events(ev: Event) -> PollFlags {
    (if ev.readable {
        PollFlags::IN | PollFlags::PRI
    } else {
        PollFlags::empty()
    }) | (if ev.writable {
        PollFlags::OUT | PollFlags::WRBAND
    } else {
        PollFlags::empty()
    })
}

/// Returned poll events for reading.
fn read_events() -> PollFlags {
    PollFlags::IN | PollFlags::PRI | PollFlags::HUP | PollFlags::ERR
}

/// Returned poll events for writing.
fn write_events() -> PollFlags {
    PollFlags::OUT | PollFlags::WRBAND | PollFlags::HUP | PollFlags::ERR
}

/// A list of reported I/O events.
pub struct Events {
    inner: Vec<Event>,
}

impl Events {
    /// Creates an empty list.
    pub fn with_capacity(cap: usize) -> Events {
        Self {
            inner: Vec::with_capacity(cap),
        }
    }

    /// Iterates over I/O events.
    pub fn iter(&self) -> impl Iterator<Item = Event> + '_ {
        self.inner.iter().copied()
    }

    /// Clear the list.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Get the capacity of the list.
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }
}

/// Extra information associated with an event.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct EventExtra {
    /// Flags associated with this event.
    flags: PollFlags,
}

impl EventExtra {
    /// Creates an empty set of extra information.
    #[inline]
    pub const fn empty() -> Self {
        Self {
            flags: PollFlags::empty(),
        }
    }

    /// Set the interrupt flag.
    #[inline]
    pub fn set_hup(&mut self, value: bool) {
        self.flags.set(PollFlags::HUP, value);
    }

    /// Set the priority flag.
    #[inline]
    pub fn set_pri(&mut self, value: bool) {
        self.flags.set(PollFlags::PRI, value);
    }

    /// Is this an interrupt event?
    #[inline]
    pub fn is_hup(&self) -> bool {
        self.flags.contains(PollFlags::HUP)
    }

    /// Is this a priority event?
    #[inline]
    pub fn is_pri(&self) -> bool {
        self.flags.contains(PollFlags::PRI)
    }

    #[inline]
    pub fn is_connect_failed(&self) -> Option<bool> {
        Some(self.flags.contains(PollFlags::ERR) || self.flags.contains(PollFlags::HUP))
    }

    #[inline]
    pub fn is_err(&self) -> Option<bool> {
        Some(self.flags.contains(PollFlags::ERR))
    }
}

fn cvt_mode_as_remove(mode: PollMode) -> io::Result<bool> {
    match mode {
        PollMode::Oneshot => Ok(true),
        PollMode::Level => Ok(false),
        _ => Err(crate::unsupported_error(
            "edge-triggered I/O events are not supported in poll()",
        )),
    }
}

#[cfg(unix)]
mod syscall {
    pub(super) use rustix::event::{PollFd, PollFlags};

    pub(super) use rustix::event::Timespec;
    #[cfg(target_os = "espidf")]
    pub(super) use rustix::event::{eventfd, EventfdFlags};
    #[cfg(target_os = "espidf")]
    pub(super) use rustix::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd, RawFd};
    #[cfg(target_os = "espidf")]
    pub(super) use rustix::io::{read, write};
    use std::io;
    use std::time::Duration;

    /// Safe wrapper around the `poll` system call.
    pub(super) fn poll(fds: &mut [PollFd<'_>], timeout: Option<Duration>) -> io::Result<usize> {
        // Timeout for `poll`. In case of overflow, use no timeout.
        let timeout = match timeout {
            Some(timeout) => Timespec::try_from(timeout).ok(),
            None => None,
        };

        Ok(rustix::event::poll(fds, timeout.as_ref())?)
    }
}

#[cfg(target_os = "hermit")]
mod syscall {
    // TODO: Remove this shim once HermitOS is supported in Rustix.

    use std::fmt;
    use std::io;
    use std::marker::PhantomData;
    use std::ops::BitOr;
    use std::time::Duration;

    pub(super) use std::os::hermit::io::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};

    /// Create an eventfd.
    pub(super) fn eventfd(count: u64, _flags: EventfdFlags) -> io::Result<OwnedFd> {
        let fd = unsafe { hermit_abi::eventfd(count, 0) };

        if fd < 0 {
            Err(io::Error::from_raw_os_error(unsafe {
                hermit_abi::get_errno()
            }))
        } else {
            Ok(unsafe { OwnedFd::from_raw_fd(fd) })
        }
    }

    /// Read some bytes.
    pub(super) fn read(fd: BorrowedFd<'_>, bytes: &mut [u8]) -> io::Result<usize> {
        let count = unsafe { hermit_abi::read(fd.as_raw_fd(), bytes.as_mut_ptr(), bytes.len()) };

        cvt(count)
    }

    /// Write some bytes.
    pub(super) fn write(fd: BorrowedFd<'_>, bytes: &[u8]) -> io::Result<usize> {
        let count = unsafe { hermit_abi::write(fd.as_raw_fd(), bytes.as_ptr(), bytes.len()) };

        cvt(count)
    }

    /// Safe wrapper around the `poll` system call.
    pub(super) fn poll(fds: &mut [PollFd<'_>], timeout: Option<Duration>) -> io::Result<usize> {
        // Timeout in milliseconds for epoll. In case of overflow, use no timeout.
        let mut timeout_ms = -1;
        if let Some(t) = timeout {
            if let Ok(ms) = i32::try_from(t.as_millis()) {
                // Round up to a whole millisecond.
                if Duration::from_millis(ms as u64) < t {
                    if let Some(ms) = ms.checked_add(1) {
                        timeout_ms = ms;
                    }
                } else {
                    timeout_ms = ms;
                }
            }
        }

        let call = unsafe {
            hermit_abi::poll(
                fds.as_mut_ptr() as *mut hermit_abi::pollfd,
                fds.len(),
                timeout_ms,
            )
        };

        cvt(call as isize)
    }

    /// Safe wrapper around `pollfd`.
    #[repr(transparent)]
    pub(super) struct PollFd<'a> {
        inner: hermit_abi::pollfd,
        _lt: PhantomData<BorrowedFd<'a>>,
    }

    impl<'a> PollFd<'a> {
        pub(super) fn from_borrowed_fd(fd: BorrowedFd<'a>, inflags: PollFlags) -> Self {
            Self {
                inner: hermit_abi::pollfd {
                    fd: fd.as_raw_fd(),
                    events: inflags.0,
                    revents: 0,
                },
                _lt: PhantomData,
            }
        }

        pub(super) fn revents(&self) -> PollFlags {
            PollFlags(self.inner.revents)
        }
    }

    impl AsFd for PollFd<'_> {
        fn as_fd(&self) -> BorrowedFd<'_> {
            unsafe { BorrowedFd::borrow_raw(self.inner.fd) }
        }
    }

    impl fmt::Debug for PollFd<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("PollFd")
                .field("fd", &format_args!("0x{:x}", self.inner.fd))
                .field("events", &PollFlags(self.inner.events))
                .field("revents", &PollFlags(self.inner.revents))
                .finish()
        }
    }

    /// Wrapper around polling flags.
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub(super) struct PollFlags(i16);

    impl PollFlags {
        /// Empty set of flags.
        pub(super) const fn empty() -> Self {
            Self(0)
        }

        pub(super) const IN: PollFlags = PollFlags(hermit_abi::POLLIN);
        pub(super) const OUT: PollFlags = PollFlags(hermit_abi::POLLOUT);
        pub(super) const WRBAND: PollFlags = PollFlags(hermit_abi::POLLWRBAND);
        pub(super) const ERR: PollFlags = PollFlags(hermit_abi::POLLERR);
        pub(super) const HUP: PollFlags = PollFlags(hermit_abi::POLLHUP);
        pub(super) const PRI: PollFlags = PollFlags(hermit_abi::POLLPRI);

        /// Tell if this contains some flags.
        pub(super) fn contains(self, flags: PollFlags) -> bool {
            self.0 & flags.0 != 0
        }

        /// Set a flag.
        pub(super) fn set(&mut self, flags: PollFlags, set: bool) {
            if set {
                self.0 |= flags.0;
            } else {
                self.0 &= !(flags.0);
            }
        }

        /// Tell if this is empty.
        pub(super) fn is_empty(self) -> bool {
            self.0 == 0
        }

        /// Tell if this intersects with some flags.
        pub(super) fn intersects(self, flags: PollFlags) -> bool {
            self.contains(flags)
        }
    }

    impl BitOr for PollFlags {
        type Output = PollFlags;

        fn bitor(self, rhs: Self) -> Self::Output {
            Self(self.0 | rhs.0)
        }
    }

    #[derive(Debug, Copy, Clone)]
    pub(super) struct EventfdFlags;

    impl EventfdFlags {
        pub(super) fn empty() -> Self {
            Self
        }
    }

    /// Convert a number to an actual result.
    #[inline]
    fn cvt(len: isize) -> io::Result<usize> {
        if len < 0 {
            Err(io::Error::from_raw_os_error(unsafe {
                hermit_abi::get_errno()
            }))
        } else {
            Ok(len as usize)
        }
    }
}

#[cfg(not(any(target_os = "espidf", target_os = "hermit")))]
mod notify {
    use std::io;

    use rustix::event::PollFlags;
    use rustix::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd, RawFd};
    use rustix::fs::{fcntl_getfl, fcntl_setfl, OFlags};
    use rustix::io::{fcntl_getfd, fcntl_setfd, read, write, FdFlags};
    #[cfg(not(any(target_os = "haiku", target_os = "nto")))]
    use rustix::pipe::pipe_with;
    use rustix::pipe::{pipe, PipeFlags};

    /// A notification pipe.
    ///
    /// This implementation uses a pipe to send notifications.
    #[derive(Debug)]
    pub(super) struct Notify {
        /// The file descriptor of the read half of the notify pipe. This is also stored as the first
        /// file descriptor in `fds.poll_fds`.
        read_pipe: OwnedFd,
        /// The file descriptor of the write half of the notify pipe.
        ///
        /// Data is written to this to wake up the current instance of `Poller::wait`, which can occur when the
        /// user notifies it (in which case `Poller::notified` would have been set) or when an operation needs
        /// to occur (in which case `Poller::waiting_operations` would have been incremented).
        write_pipe: OwnedFd,
    }

    impl Notify {
        /// Creates a new notification pipe.
        pub(super) fn new() -> io::Result<Self> {
            let fallback_pipe = |_| {
                let (read_pipe, write_pipe) = pipe()?;
                fcntl_setfd(&read_pipe, fcntl_getfd(&read_pipe)? | FdFlags::CLOEXEC)?;
                fcntl_setfd(&write_pipe, fcntl_getfd(&write_pipe)? | FdFlags::CLOEXEC)?;
                io::Result::Ok((read_pipe, write_pipe))
            };

            #[cfg(not(any(target_os = "haiku", target_os = "nto")))]
            let (read_pipe, write_pipe) = pipe_with(PipeFlags::CLOEXEC).or_else(fallback_pipe)?;

            #[cfg(any(target_os = "haiku", target_os = "nto"))]
            let (read_pipe, write_pipe) = fallback_pipe(PipeFlags::CLOEXEC)?;

            // Put the reading side into non-blocking mode.
            fcntl_setfl(&read_pipe, fcntl_getfl(&read_pipe)? | OFlags::NONBLOCK)?;

            Ok(Self {
                read_pipe,
                write_pipe,
            })
        }

        /// Provides the file handle of the read half of the notify pipe that needs to be registered by the `Poller`.
        pub(super) fn fd(&self) -> BorrowedFd<'_> {
            self.read_pipe.as_fd()
        }

        /// Provides the poll flags to be used when registering the read half of the notify pipe with the `Poller`.
        pub(super) fn poll_flags(&self) -> PollFlags {
            PollFlags::RDNORM
        }

        /// Notifies the `Poller` instance via the write half of the notify pipe.
        pub(super) fn notify(&self) -> Result<(), io::Error> {
            write(&self.write_pipe, &[0; 1])?;

            Ok(())
        }

        /// Pops a notification (if any) from the pipe.
        pub(super) fn pop_notification(&self) -> Result<(), io::Error> {
            // Pipes on Vita do not guarantee that after `write` call succeeds, the
            // data becomes immediately available for reading on the other side of the pipe.
            // To ensure that the notification is not lost, the read side of the pipe is temporarily
            // switched to blocking for a single `read` call.
            #[cfg(target_os = "vita")]
            rustix::fs::fcntl_setfl(
                &self.read_pipe,
                rustix::fs::fcntl_getfl(&self.read_pipe)? & !rustix::fs::OFlags::NONBLOCK,
            )?;

            let result = read(&self.read_pipe, &mut [0; 1]);

            #[cfg(target_os = "vita")]
            rustix::fs::fcntl_setfl(
                &self.read_pipe,
                rustix::fs::fcntl_getfl(&self.read_pipe)? | rustix::fs::OFlags::NONBLOCK,
            )?;

            result?;

            Ok(())
        }

        /// Pops all notifications from the pipe.
        pub(super) fn pop_all_notifications(&self) -> Result<(), io::Error> {
            while read(&self.read_pipe, &mut [0; 64]).is_ok() {}

            Ok(())
        }

        /// Whether this raw file descriptor is associated with this notifier.
        pub(super) fn has_fd(&self, fd: RawFd) -> bool {
            self.read_pipe.as_raw_fd() == fd || self.write_pipe.as_raw_fd() == fd
        }
    }
}

#[cfg(any(target_os = "espidf", target_os = "hermit"))]
mod notify {
    use std::io;
    use std::mem;

    use super::syscall::{
        eventfd, read, write, AsFd, AsRawFd, BorrowedFd, EventfdFlags, OwnedFd, PollFlags, RawFd,
    };

    /// A notification pipe.
    ///
    /// This implementation uses the `eventfd` syscall to send notifications.
    #[derive(Debug)]
    pub(super) struct Notify {
        /// The file descriptor of the eventfd object. This is also stored as the first
        /// file descriptor in `fds.poll_fds`.
        ///
        /// Data is written to this to wake up the current instance of `Poller::wait`, which can occur when the
        /// user notifies it (in which case `Poller::notified` would have been set) or when an operation needs
        /// to occur (in which case `Poller::waiting_operations` would have been incremented).
        event_fd: OwnedFd,
    }

    impl Notify {
        /// Creates a new notification pipe.
        pub(super) fn new() -> io::Result<Self> {
            // Note that the eventfd() implementation in ESP-IDF deviates from the specification in the following ways:
            // 1) The file descriptor is always in a non-blocking mode, as if EFD_NONBLOCK was passed as a flag;
            //    passing EFD_NONBLOCK or calling fcntl(.., F_GETFL/F_SETFL) on the eventfd() file descriptor is not supported
            // 2) It always returns the counter value, even if it is 0. This is contrary to the specification which mandates
            //    that it should instead fail with EAGAIN
            //
            // (1) is not a problem for us, as we want the eventfd() file descriptor to be in a non-blocking mode anyway
            // (2) is also not a problem, as long as we don't try to read the counter value in an endless loop when we detect being notified

            let flags = EventfdFlags::empty();
            let event_fd = eventfd(0, flags).map_err(|err| {
                match io::Error::from(err) {
                    err if err.kind() == io::ErrorKind::PermissionDenied => {
                        // EPERM can happen if the eventfd isn't initialized yet.
                        // Tell the user to call esp_vfs_eventfd_register.
                        io::Error::new(
                            io::ErrorKind::PermissionDenied,
                            "failed to initialize eventfd for polling, try calling `esp_vfs_eventfd_register`"
                        )
                    },
                    err => err,
                }
            })?;

            Ok(Self { event_fd })
        }

        /// Provides the eventfd file handle that needs to be registered by the `Poller`.
        pub(super) fn fd(&self) -> BorrowedFd<'_> {
            self.event_fd.as_fd()
        }

        /// Provides the eventfd file handle poll flags to be used when registering it with the `Poller`.
        pub(super) fn poll_flags(&self) -> PollFlags {
            PollFlags::IN
        }

        /// Notifies the `Poller` instance via the eventfd file descriptor.
        pub(super) fn notify(&self) -> Result<(), io::Error> {
            write(self.event_fd.as_fd(), &1u64.to_ne_bytes())?;

            Ok(())
        }

        /// Pops a notification (if any) from the eventfd file descriptor.
        pub(super) fn pop_notification(&self) -> Result<(), io::Error> {
            read(self.event_fd.as_fd(), &mut [0; mem::size_of::<u64>()])?;

            Ok(())
        }

        /// Pops all notifications from the eventfd file descriptor.
        /// Since the eventfd object accumulates all writes in a single 64 bit value,
        /// this operation is - in fact - equivalent to `pop_notification`.
        pub(super) fn pop_all_notifications(&self) -> Result<(), io::Error> {
            let _ = self.pop_notification();

            Ok(())
        }

        /// Whether this raw file descriptor is associated with this notifier.
        pub(super) fn has_fd(&self, fd: RawFd) -> bool {
            self.event_fd.as_raw_fd() == fd
        }
    }
}
