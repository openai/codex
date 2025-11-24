//! Bindings to kqueue (macOS, iOS, tvOS, watchOS, visionOS, FreeBSD, NetBSD, OpenBSD, DragonFly BSD).

use std::collections::HashSet;
use std::io;
use std::os::unix::io::{AsFd, AsRawFd, BorrowedFd, OwnedFd, RawFd};
use std::sync::RwLock;
use std::time::Instant;

use rustix::buffer::spare_capacity;
use rustix::event::{kqueue, Timespec};
use rustix::io::{fcntl_setfd, Errno, FdFlags};

use crate::{Event, PollMode};

/// Interface to kqueue.
#[derive(Debug)]
pub struct Poller {
    /// File descriptor for the kqueue instance.
    kqueue_fd: OwnedFd,

    /// List of sources currently registered in this poller.
    ///
    /// This is used to make sure the same source is not registered twice.
    sources: RwLock<HashSet<SourceId>>,

    /// Notification pipe for waking up the poller.
    ///
    /// On platforms that support `EVFILT_USER`, this uses that to wake up the poller. Otherwise, it
    /// uses a pipe.
    notify: notify::Notify,
}

/// Identifier for a source.
#[doc(hidden)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum SourceId {
    /// Registered file descriptor.
    Fd(RawFd),

    /// Signal.
    Signal(std::os::raw::c_int),

    /// Process ID.
    Pid(rustix::process::Pid),

    /// Timer ID.
    Timer(usize),
}

impl Poller {
    /// Creates a new poller.
    pub fn new() -> io::Result<Poller> {
        // Create a kqueue instance.
        let kqueue_fd = kqueue::kqueue()?;
        fcntl_setfd(&kqueue_fd, FdFlags::CLOEXEC)?;

        let poller = Poller {
            kqueue_fd,
            sources: RwLock::new(HashSet::new()),
            notify: notify::Notify::new()?,
        };

        // Register the notification pipe.
        poller.notify.register(&poller)?;

        #[cfg(feature = "tracing")]
        tracing::trace!(
            kqueue_fd = ?poller.kqueue_fd.as_raw_fd(),
            "new"
        );
        Ok(poller)
    }

    /// Whether this poller supports level-triggered events.
    pub fn supports_level(&self) -> bool {
        true
    }

    /// Whether this poller supports edge-triggered events.
    pub fn supports_edge(&self) -> bool {
        true
    }

    /// Adds a new file descriptor.
    ///
    /// # Safety
    ///
    /// The file descriptor must be valid and it must last until it is deleted.
    pub unsafe fn add(&self, fd: RawFd, ev: Event, mode: PollMode) -> io::Result<()> {
        self.add_source(SourceId::Fd(fd))?;

        // File descriptors don't need to be added explicitly, so just modify the interest.
        self.modify(BorrowedFd::borrow_raw(fd), ev, mode)
    }

    /// Modifies an existing file descriptor.
    pub fn modify(&self, fd: BorrowedFd<'_>, ev: Event, mode: PollMode) -> io::Result<()> {
        #[cfg(feature = "tracing")]
        let span = if !self.notify.has_fd(fd) {
            let span = tracing::trace_span!(
                "add",
                kqueue_fd = ?self.kqueue_fd.as_raw_fd(),
                ?fd,
                ?ev,
            );
            Some(span)
        } else {
            None
        };
        #[cfg(feature = "tracing")]
        let _enter = span.as_ref().map(|s| s.enter());

        self.has_source(SourceId::Fd(fd.as_raw_fd()))?;

        let mode_flags = mode_to_flags(mode);

        let read_flags = if ev.readable {
            kqueue::EventFlags::ADD | mode_flags
        } else {
            kqueue::EventFlags::DELETE
        };
        let write_flags = if ev.writable {
            kqueue::EventFlags::ADD | mode_flags
        } else {
            kqueue::EventFlags::DELETE
        };

        // A list of changes for kqueue.
        let changelist = [
            kqueue::Event::new(
                kqueue::EventFilter::Read(fd.as_raw_fd()),
                read_flags | kqueue::EventFlags::RECEIPT,
                ev.key as _,
            ),
            kqueue::Event::new(
                kqueue::EventFilter::Write(fd.as_raw_fd()),
                write_flags | kqueue::EventFlags::RECEIPT,
                ev.key as _,
            ),
        ];

        // Apply changes.
        self.submit_changes(changelist)
    }

    /// Submit one or more changes to the kernel queue and check to see if they succeeded.
    pub(crate) fn submit_changes<A>(&self, changelist: A) -> io::Result<()>
    where
        A: Copy + AsRef<[kqueue::Event]> + AsMut<[kqueue::Event]>,
    {
        let mut eventlist = Vec::with_capacity(changelist.as_ref().len());

        // Apply changes.
        {
            let changelist = changelist.as_ref();

            unsafe {
                kqueue::kevent_timespec(
                    &self.kqueue_fd,
                    changelist,
                    spare_capacity(&mut eventlist),
                    None,
                )?;
            }
        }

        // Check for errors.
        for &ev in &eventlist {
            let data = ev.data();

            // Explanation for ignoring EPIPE: https://github.com/tokio-rs/mio/issues/582
            if (ev.flags().contains(kqueue::EventFlags::ERROR))
                && data != 0
                && data != Errno::NOENT.raw_os_error() as _
                && data != Errno::PIPE.raw_os_error() as _
            {
                return Err(io::Error::from_raw_os_error(data as _));
            }
        }

        Ok(())
    }

    /// Add a source to the sources set.
    #[inline]
    pub(crate) fn add_source(&self, source: SourceId) -> io::Result<()> {
        if self
            .sources
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(source)
        {
            Ok(())
        } else {
            Err(io::Error::from(io::ErrorKind::AlreadyExists))
        }
    }

    /// Tell if a source is currently inside the set.
    #[inline]
    pub(crate) fn has_source(&self, source: SourceId) -> io::Result<()> {
        if self
            .sources
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .contains(&source)
        {
            Ok(())
        } else {
            Err(io::Error::from(io::ErrorKind::NotFound))
        }
    }

    /// Remove a source from the sources set.
    #[inline]
    pub(crate) fn remove_source(&self, source: SourceId) -> io::Result<()> {
        if self
            .sources
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&source)
        {
            Ok(())
        } else {
            Err(io::Error::from(io::ErrorKind::NotFound))
        }
    }

    /// Deletes a file descriptor.
    pub fn delete(&self, fd: BorrowedFd<'_>) -> io::Result<()> {
        // Simply delete interest in the file descriptor.
        self.modify(fd, Event::none(0), PollMode::Oneshot)?;

        self.remove_source(SourceId::Fd(fd.as_raw_fd()))
    }

    /// Waits for I/O events with an optional deadline.
    pub fn wait_deadline(&self, events: &mut Events, deadline: Option<Instant>) -> io::Result<()> {
        #[cfg(feature = "tracing")]
        let span = tracing::trace_span!(
            "wait",
            kqueue_fd = ?self.kqueue_fd.as_raw_fd(),
            ?deadline,
        );
        #[cfg(feature = "tracing")]
        let _enter = span.enter();

        let timeout = deadline.map(|deadline| deadline.saturating_duration_since(Instant::now()));

        // Timeout for kevent. In case of overflow, use no timeout.
        let timeout = match timeout {
            Some(t) => Timespec::try_from(t).ok(),
            None => None,
        };

        // Wait for I/O events.
        let changelist = [];
        let _res = unsafe {
            kqueue::kevent_timespec(
                &self.kqueue_fd,
                &changelist,
                spare_capacity(&mut events.list),
                timeout.as_ref(),
            )?
        };

        #[cfg(feature = "tracing")]
        tracing::trace!(
            kqueue_fd = ?self.kqueue_fd.as_raw_fd(),
            res = ?_res,
            "new events",
        );

        // Clear the notification (if received) and re-register interest in it.
        self.notify.reregister(self)?;

        Ok(())
    }

    /// Sends a notification to wake up the current or next `wait()` call.
    pub fn notify(&self) -> io::Result<()> {
        #[cfg(feature = "tracing")]
        let span = tracing::trace_span!(
            "notify",
            kqueue_fd = ?self.kqueue_fd.as_raw_fd(),
        );
        #[cfg(feature = "tracing")]
        let _enter = span.enter();

        self.notify.notify(self).ok();
        Ok(())
    }
}

impl AsRawFd for Poller {
    fn as_raw_fd(&self) -> RawFd {
        self.kqueue_fd.as_raw_fd()
    }
}

impl AsFd for Poller {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.kqueue_fd.as_fd()
    }
}

impl Drop for Poller {
    fn drop(&mut self) {
        #[cfg(feature = "tracing")]
        let span = tracing::trace_span!(
            "drop",
            kqueue_fd = ?self.kqueue_fd.as_raw_fd(),
        );
        #[cfg(feature = "tracing")]
        let _enter = span.enter();

        let _ = self.notify.deregister(self);
    }
}

/// A list of reported I/O events.
pub struct Events {
    list: Vec<kqueue::Event>,
}

unsafe impl Send for Events {}

impl Events {
    /// Creates an empty list.
    pub fn with_capacity(cap: usize) -> Events {
        Events {
            list: Vec::with_capacity(cap),
        }
    }

    /// Iterates over I/O events.
    pub fn iter(&self) -> impl Iterator<Item = Event> + '_ {
        // On some platforms, closing the read end of a pipe wakes up writers, but the
        // event is reported as EVFILT_READ with the EV_EOF flag.
        //
        // https://github.com/golang/go/commit/23aad448b1e3f7c3b4ba2af90120bde91ac865b4
        self.list.iter().map(|ev| Event {
            key: ev.udata() as usize,
            readable: matches!(
                ev.filter(),
                kqueue::EventFilter::Read(..)
                    | kqueue::EventFilter::Vnode { .. }
                    | kqueue::EventFilter::Proc { .. }
                    | kqueue::EventFilter::Signal { .. }
                    | kqueue::EventFilter::Timer { .. }
            ),
            writable: matches!(ev.filter(), kqueue::EventFilter::Write(..))
                || (matches!(ev.filter(), kqueue::EventFilter::Read(..))
                    && (ev.flags().intersects(kqueue::EventFlags::EOF))),
            extra: EventExtra,
        })
    }

    /// Clears the list.
    pub fn clear(&mut self) {
        self.list.clear();
    }

    /// Get the capacity of the list.
    pub fn capacity(&self) -> usize {
        self.list.capacity()
    }
}

/// Extra information associated with an event.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct EventExtra;

impl EventExtra {
    /// Create a new, empty version of this struct.
    #[inline]
    pub const fn empty() -> EventExtra {
        EventExtra
    }

    /// Set the interrupt flag.
    #[inline]
    pub fn set_hup(&mut self, _value: bool) {
        // No-op.
    }

    /// Set the priority flag.
    #[inline]
    pub fn set_pri(&mut self, _value: bool) {
        // No-op.
    }

    /// Is the interrupt flag set?
    #[inline]
    pub fn is_hup(&self) -> bool {
        false
    }

    /// Is the priority flag set?
    #[inline]
    pub fn is_pri(&self) -> bool {
        false
    }

    #[inline]
    pub fn is_connect_failed(&self) -> Option<bool> {
        None
    }

    #[inline]
    pub fn is_err(&self) -> Option<bool> {
        None
    }
}

pub(crate) fn mode_to_flags(mode: PollMode) -> kqueue::EventFlags {
    use kqueue::EventFlags as EV;

    match mode {
        PollMode::Oneshot => EV::ONESHOT,
        PollMode::Level => EV::empty(),
        PollMode::Edge => EV::CLEAR,
        PollMode::EdgeOneshot => EV::ONESHOT | EV::CLEAR,
    }
}

#[cfg(any(
    target_os = "freebsd",
    target_os = "dragonfly",
    target_vendor = "apple",
))]
mod notify {
    use super::Poller;
    use rustix::event::kqueue;
    use std::io;
    #[cfg(feature = "tracing")]
    use std::os::unix::io::BorrowedFd;

    /// A notification pipe.
    ///
    /// This implementation uses `EVFILT_USER` to avoid allocating a pipe.
    #[derive(Debug)]
    pub(super) struct Notify;

    impl Notify {
        /// Creates a new notification pipe.
        pub(super) fn new() -> io::Result<Self> {
            Ok(Self)
        }

        /// Registers this notification pipe in the `Poller`.
        pub(super) fn register(&self, poller: &Poller) -> io::Result<()> {
            // Register an EVFILT_USER event.
            poller.submit_changes([kqueue::Event::new(
                kqueue::EventFilter::User {
                    ident: 0,
                    flags: kqueue::UserFlags::empty(),
                    user_flags: kqueue::UserDefinedFlags::new(0),
                },
                kqueue::EventFlags::ADD | kqueue::EventFlags::RECEIPT | kqueue::EventFlags::CLEAR,
                crate::NOTIFY_KEY as _,
            )])
        }

        /// Reregister this notification pipe in the `Poller`.
        pub(super) fn reregister(&self, _poller: &Poller) -> io::Result<()> {
            // We don't need to do anything, it's already registered as EV_CLEAR.
            Ok(())
        }

        /// Notifies the `Poller`.
        pub(super) fn notify(&self, poller: &Poller) -> io::Result<()> {
            // Trigger the EVFILT_USER event.
            poller.submit_changes([kqueue::Event::new(
                kqueue::EventFilter::User {
                    ident: 0,
                    flags: kqueue::UserFlags::TRIGGER,
                    user_flags: kqueue::UserDefinedFlags::new(0),
                },
                kqueue::EventFlags::ADD | kqueue::EventFlags::RECEIPT,
                crate::NOTIFY_KEY as _,
            )])?;

            Ok(())
        }

        /// Deregisters this notification pipe from the `Poller`.
        pub(super) fn deregister(&self, poller: &Poller) -> io::Result<()> {
            // Deregister the EVFILT_USER event.
            poller.submit_changes([kqueue::Event::new(
                kqueue::EventFilter::User {
                    ident: 0,
                    flags: kqueue::UserFlags::empty(),
                    user_flags: kqueue::UserDefinedFlags::new(0),
                },
                kqueue::EventFlags::DELETE | kqueue::EventFlags::RECEIPT,
                crate::NOTIFY_KEY as _,
            )])
        }

        /// Whether this raw file descriptor is associated with this pipe.
        #[cfg(feature = "tracing")]
        pub(super) fn has_fd(&self, _fd: BorrowedFd<'_>) -> bool {
            false
        }
    }
}

#[cfg(not(any(
    target_os = "freebsd",
    target_os = "dragonfly",
    target_vendor = "apple",
)))]
mod notify {
    use super::Poller;
    use crate::{Event, PollMode, NOTIFY_KEY};
    use std::io::{self, prelude::*};
    #[cfg(feature = "tracing")]
    use std::os::unix::io::BorrowedFd;
    use std::os::unix::{
        io::{AsFd, AsRawFd},
        net::UnixStream,
    };

    /// A notification pipe.
    ///
    /// This implementation uses a pipe to send notifications.
    #[derive(Debug)]
    pub(super) struct Notify {
        /// The read end of the pipe.
        read_stream: UnixStream,

        /// The write end of the pipe.
        write_stream: UnixStream,
    }

    impl Notify {
        /// Creates a new notification pipe.
        pub(super) fn new() -> io::Result<Self> {
            let (read_stream, write_stream) = UnixStream::pair()?;
            read_stream.set_nonblocking(true)?;
            write_stream.set_nonblocking(true)?;

            Ok(Self {
                read_stream,
                write_stream,
            })
        }

        /// Registers this notification pipe in the `Poller`.
        pub(super) fn register(&self, poller: &Poller) -> io::Result<()> {
            // Register the read end of this pipe.
            unsafe {
                poller.add(
                    self.read_stream.as_raw_fd(),
                    Event::readable(NOTIFY_KEY),
                    PollMode::Oneshot,
                )
            }
        }

        /// Reregister this notification pipe in the `Poller`.
        pub(super) fn reregister(&self, poller: &Poller) -> io::Result<()> {
            // Clear out the notification.
            while (&self.read_stream).read(&mut [0; 64]).is_ok() {}

            // Reregister the read end of this pipe.
            poller.modify(
                self.read_stream.as_fd(),
                Event::readable(NOTIFY_KEY),
                PollMode::Oneshot,
            )
        }

        /// Notifies the `Poller`.
        #[allow(clippy::unused_io_amount)]
        pub(super) fn notify(&self, _poller: &Poller) -> io::Result<()> {
            // Write to the write end of the pipe
            (&self.write_stream).write(&[1])?;

            Ok(())
        }

        /// Deregisters this notification pipe from the `Poller`.
        pub(super) fn deregister(&self, poller: &Poller) -> io::Result<()> {
            // Deregister the read end of the pipe.
            poller.delete(self.read_stream.as_fd())
        }

        /// Whether this raw file descriptor is associated with this pipe.
        #[cfg(feature = "tracing")]
        pub(super) fn has_fd(&self, fd: BorrowedFd<'_>) -> bool {
            self.read_stream.as_raw_fd() == fd.as_raw_fd()
        }
    }
}
