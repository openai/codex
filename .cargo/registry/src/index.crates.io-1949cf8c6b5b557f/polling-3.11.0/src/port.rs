//! Bindings to event port (illumos, Solaris).

use std::io;
use std::os::unix::io::{AsFd, AsRawFd, BorrowedFd, RawFd};
use std::time::Instant;

use rustix::buffer::spare_capacity;
use rustix::event::{port, PollFlags, Timespec};
use rustix::fd::OwnedFd;
use rustix::io::{fcntl_getfd, fcntl_setfd, FdFlags};

use crate::{Event, PollMode};

/// Interface to event ports.
#[derive(Debug)]
pub struct Poller {
    /// File descriptor for the port instance.
    port_fd: OwnedFd,
}

impl Poller {
    /// Creates a new poller.
    pub fn new() -> io::Result<Poller> {
        let port_fd = port::create()?;
        let flags = fcntl_getfd(&port_fd)?;
        fcntl_setfd(&port_fd, flags | FdFlags::CLOEXEC)?;

        #[cfg(feature = "tracing")]
        tracing::trace!(
            port_fd = ?port_fd.as_raw_fd(),
            "new",
        );

        Ok(Poller { port_fd })
    }

    /// Whether this poller supports level-triggered events.
    pub fn supports_level(&self) -> bool {
        false
    }

    /// Whether this poller supports edge-triggered events.
    pub fn supports_edge(&self) -> bool {
        false
    }

    /// Adds a file descriptor.
    ///
    /// # Safety
    ///
    /// The `fd` must be a valid file descriptor and it must last until it is deleted.
    pub unsafe fn add(&self, fd: RawFd, ev: Event, mode: PollMode) -> io::Result<()> {
        // File descriptors don't need to be added explicitly, so just modify the interest.
        self.modify(BorrowedFd::borrow_raw(fd), ev, mode)
    }

    /// Modifies an existing file descriptor.
    pub fn modify(&self, fd: BorrowedFd<'_>, ev: Event, mode: PollMode) -> io::Result<()> {
        #[cfg(feature = "tracing")]
        let span = tracing::trace_span!(
            "modify",
            port_fd = ?self.port_fd.as_raw_fd(),
            ?fd,
            ?ev,
        );
        #[cfg(feature = "tracing")]
        let _enter = span.enter();

        let mut flags = PollFlags::empty();
        if ev.readable {
            flags |= read_flags();
        }
        if ev.writable {
            flags |= write_flags();
        }

        if mode != PollMode::Oneshot {
            return Err(crate::unsupported_error(
                "this kind of event is not supported with event ports",
            ));
        }

        unsafe {
            port::associate_fd(&self.port_fd, fd, flags, ev.key as _)?;
        }

        Ok(())
    }

    /// Deletes a file descriptor.
    pub fn delete(&self, fd: BorrowedFd<'_>) -> io::Result<()> {
        #[cfg(feature = "tracing")]
        let span = tracing::trace_span!(
            "delete",
            port_fd = ?self.port_fd.as_raw_fd(),
            ?fd,
        );
        #[cfg(feature = "tracing")]
        let _enter = span.enter();

        let result = unsafe { port::dissociate_fd(&self.port_fd, fd) };
        if let Err(e) = result {
            match e {
                rustix::io::Errno::NOENT => return Ok(()),
                _ => return Err(e.into()),
            }
        }

        Ok(())
    }

    /// Waits for I/O events with an optional deadline.
    pub fn wait_deadline(&self, events: &mut Events, deadline: Option<Instant>) -> io::Result<()> {
        #[cfg(feature = "tracing")]
        let span = tracing::trace_span!(
            "wait",
            port_fd = ?self.port_fd.as_raw_fd(),
            ?deadline,
        );
        #[cfg(feature = "tracing")]
        let _enter = span.enter();

        let timeout = deadline.map(|deadline| deadline.saturating_duration_since(Instant::now()));

        // Timeout for `port::getn`. In case of overflow, use no timeout.
        let timeout = match timeout {
            Some(t) => Timespec::try_from(t).ok(),
            None => None,
        };

        // Wait for I/O events.
        let res = port::getn(
            &self.port_fd,
            spare_capacity(&mut events.list),
            1,
            timeout.as_ref(),
        );
        #[cfg(feature = "tracing")]
        tracing::trace!(
            port_fd = ?self.port_fd,
            res = ?events.list.len(),
            "new events"
        );

        // Event ports sets the return value to -1 and returns ETIME on timer expire. The number of
        // returned events is stored in nget, but in our case it should always be 0 since we set
        // nget to 1 initially.
        if let Err(e) = res {
            match e {
                rustix::io::Errno::TIME => {}
                _ => return Err(e.into()),
            }
        }

        Ok(())
    }

    /// Sends a notification to wake up the current or next `wait()` call.
    pub fn notify(&self) -> io::Result<()> {
        const PORT_SOURCE_USER: i32 = 3;

        #[cfg(feature = "tracing")]
        let span = tracing::trace_span!(
            "notify",
            port_fd = ?self.port_fd.as_raw_fd(),
        );
        #[cfg(feature = "tracing")]
        let _enter = span.enter();

        // Use port_send to send a notification to the port.
        port::send(&self.port_fd, PORT_SOURCE_USER, crate::NOTIFY_KEY as _)?;

        Ok(())
    }
}

impl AsRawFd for Poller {
    fn as_raw_fd(&self) -> RawFd {
        self.port_fd.as_raw_fd()
    }
}

impl AsFd for Poller {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.port_fd.as_fd()
    }
}

/// Poll flags for all possible readability events.
fn read_flags() -> PollFlags {
    PollFlags::IN | PollFlags::HUP | PollFlags::ERR | PollFlags::PRI
}

/// Poll flags for all possible writability events.
fn write_flags() -> PollFlags {
    PollFlags::OUT | PollFlags::HUP | PollFlags::ERR
}

/// A list of reported I/O events.
pub struct Events {
    list: Vec<port::Event>,
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
        self.list.iter().map(|ev| {
            let flags = PollFlags::from_bits_truncate(ev.events() as _);
            Event {
                key: ev.userdata() as usize,
                readable: flags.intersects(read_flags()),
                writable: flags.intersects(write_flags()),
                extra: EventExtra { flags },
            }
        })
    }

    /// Clear the list.
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
pub struct EventExtra {
    /// Flags associated with this event.
    flags: PollFlags,
}

impl EventExtra {
    /// Create a new, empty version of this struct.
    #[inline]
    pub const fn empty() -> EventExtra {
        EventExtra {
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
        Some(self.flags.contains(PollFlags::ERR) && self.flags.contains(PollFlags::HUP))
    }

    #[inline]
    pub fn is_err(&self) -> Option<bool> {
        Some(self.flags.contains(PollFlags::ERR))
    }
}
