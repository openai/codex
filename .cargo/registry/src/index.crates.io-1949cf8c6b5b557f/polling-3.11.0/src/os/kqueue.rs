//! Functionality that is only available for `kqueue`-based platforms.

use crate::sys::{mode_to_flags, SourceId};
use crate::{PollMode, Poller};

use std::io;
use std::marker::PhantomData;
use std::process::Child;
use std::time::Duration;

use rustix::event::kqueue;

use super::__private::PollerSealed;
use __private::FilterSealed;

// TODO(notgull): We should also have EVFILT_AIO, EVFILT_VNODE and EVFILT_USER. However, the current
// API makes it difficult to effectively express events from these filters. At the next breaking
// change, we should change `Event` to be a struct with private fields, and encode additional
// information in there.

/// Functionality that is only available for `kqueue`-based platforms.
///
/// `kqueue` is able to monitor much more than just read/write readiness on file descriptors. Using
/// this extension trait, you can monitor for signals, process exits, and more. See the implementors
/// of the [`Filter`] trait for more information.
pub trait PollerKqueueExt<F: Filter>: PollerSealed {
    /// Add a filter to the poller.
    ///
    /// This is similar to [`add`][Poller::add], but it allows you to specify a filter instead of
    /// a socket. See the implementors of the [`Filter`] trait for more information.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use polling::{Events, Poller, PollMode};
    /// use polling::os::kqueue::{Filter, PollerKqueueExt, Signal};
    ///
    /// let poller = Poller::new().unwrap();
    ///
    /// // Register the SIGINT signal.
    /// poller.add_filter(Signal(rustix::process::Signal::INT.as_raw()), 0, PollMode::Oneshot).unwrap();
    ///
    /// // Wait for the signal.
    /// let mut events = Events::new();
    /// poller.wait(&mut events, None).unwrap();
    /// # let _ = events;
    /// ```
    fn add_filter(&self, filter: F, key: usize, mode: PollMode) -> io::Result<()>;

    /// Modify a filter in the poller.
    ///
    /// This is similar to [`modify`][Poller::modify], but it allows you to specify a filter
    /// instead of a socket. See the implementors of the [`Filter`] trait for more information.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use polling::{Events, Poller, PollMode};
    /// use polling::os::kqueue::{Filter, PollerKqueueExt, Signal};
    ///
    /// let poller = Poller::new().unwrap();
    ///
    /// // Register the SIGINT signal.
    /// poller.add_filter(Signal(rustix::process::Signal::INT.as_raw()), 0, PollMode::Oneshot).unwrap();
    ///
    /// // Re-register with a different key.
    /// poller.modify_filter(Signal(rustix::process::Signal::INT.as_raw()), 1, PollMode::Oneshot).unwrap();
    ///
    /// // Wait for the signal.
    /// let mut events = Events::new();
    /// poller.wait(&mut events, None).unwrap();
    /// # let _ = events;
    /// ```
    fn modify_filter(&self, filter: F, key: usize, mode: PollMode) -> io::Result<()>;

    /// Remove a filter from the poller.
    ///
    /// This is used to remove filters that were previously added with
    /// [`add_filter`](PollerKqueueExt::add_filter).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use polling::{Poller, PollMode};
    /// use polling::os::kqueue::{Filter, PollerKqueueExt, Signal};
    ///
    /// let poller = Poller::new().unwrap();
    ///
    /// // Register the SIGINT signal.
    /// poller.add_filter(Signal(rustix::process::Signal::INT.as_raw()), 0, PollMode::Oneshot).unwrap();
    ///
    /// // Remove the filter.
    /// poller.delete_filter(Signal(rustix::process::Signal::INT.as_raw())).unwrap();
    /// ```
    fn delete_filter(&self, filter: F) -> io::Result<()>;
}

impl<F: Filter> PollerKqueueExt<F> for Poller {
    #[inline(always)]
    fn add_filter(&self, filter: F, key: usize, mode: PollMode) -> io::Result<()> {
        // No difference between adding and modifying in kqueue.
        self.poller.add_source(filter.source_id())?;
        self.modify_filter(filter, key, mode)
    }

    fn modify_filter(&self, filter: F, key: usize, mode: PollMode) -> io::Result<()> {
        self.poller.has_source(filter.source_id())?;

        // Convert the filter into a kevent.
        let event = filter.filter(kqueue::EventFlags::ADD | mode_to_flags(mode), key);

        // Modify the filter.
        self.poller.submit_changes([event])
    }

    fn delete_filter(&self, filter: F) -> io::Result<()> {
        // Convert the filter into a kevent.
        let event = filter.filter(kqueue::EventFlags::DELETE, 0);

        // Delete the filter.
        self.poller.submit_changes([event])?;

        self.poller.remove_source(filter.source_id())
    }
}

/// A filter that can be registered into a `kqueue`.
pub trait Filter: FilterSealed {}

unsafe impl<T: FilterSealed + ?Sized> FilterSealed for &T {
    #[inline(always)]
    fn filter(&self, flags: kqueue::EventFlags, key: usize) -> kqueue::Event {
        (**self).filter(flags, key)
    }

    #[inline(always)]
    fn source_id(&self) -> SourceId {
        (**self).source_id()
    }
}

impl<T: Filter + ?Sized> Filter for &T {}

/// Monitor this signal number.
///
/// No matter what `PollMode` is specified, this filter will always be
/// oneshot-only.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Signal(pub std::os::raw::c_int);

unsafe impl FilterSealed for Signal {
    #[inline(always)]
    fn filter(&self, flags: kqueue::EventFlags, key: usize) -> kqueue::Event {
        kqueue::Event::new(
            kqueue::EventFilter::Signal {
                signal: rustix::process::Signal::from_named_raw(self.0)
                    .expect("invalid signal number"),
                times: 0,
            },
            flags | kqueue::EventFlags::RECEIPT,
            key as _,
        )
    }

    #[inline(always)]
    fn source_id(&self) -> SourceId {
        SourceId::Signal(self.0)
    }
}

impl Filter for Signal {}

/// Monitor a child process.
#[derive(Debug)]
pub struct Process<'a> {
    /// The process ID to monitor.
    pid: rustix::process::Pid,

    /// The operation to monitor.
    ops: ProcessOps,

    /// Lifetime of the underlying process.
    _lt: PhantomData<&'a Child>,
}

/// The operations that a monitored process can perform.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum ProcessOps {
    /// The process exited.
    Exit,

    /// The process was forked.
    Fork,

    /// The process executed a new process.
    Exec,
}

impl<'a> Process<'a> {
    /// Monitor a child process.
    ///
    /// # Safety
    ///
    /// Once registered into the `Poller`, the `Child` object must outlive this filter's
    /// registration into the poller.
    pub unsafe fn new(child: &'a Child, ops: ProcessOps) -> Self {
        Self {
            pid: rustix::process::Pid::from_child(child),
            ops,
            _lt: PhantomData,
        }
    }

    /// Create a `Process` from a PID.
    ///
    /// # Safety
    ///
    /// The PID must be tied to an actual child process.
    pub unsafe fn from_pid(pid: std::num::NonZeroI32, ops: ProcessOps) -> Self {
        Self {
            pid: unsafe { rustix::process::Pid::from_raw_unchecked(pid.get()) },
            ops,
            _lt: PhantomData,
        }
    }
}

unsafe impl FilterSealed for Process<'_> {
    #[inline(always)]
    fn filter(&self, flags: kqueue::EventFlags, key: usize) -> kqueue::Event {
        let events = match self.ops {
            ProcessOps::Exit => kqueue::ProcessEvents::EXIT,
            ProcessOps::Fork => kqueue::ProcessEvents::FORK,
            ProcessOps::Exec => kqueue::ProcessEvents::EXEC,
        };

        kqueue::Event::new(
            kqueue::EventFilter::Proc {
                // SAFETY: We know that the PID is nonzero.
                pid: self.pid,
                flags: events,
            },
            flags | kqueue::EventFlags::RECEIPT,
            key as _,
        )
    }

    #[inline(always)]
    fn source_id(&self) -> SourceId {
        // SAFETY: We know that the PID is nonzero
        SourceId::Pid(self.pid)
    }
}

impl Filter for Process<'_> {}

/// Wait for a timeout to expire.
///
/// Modifying the timeout after it has been added to the poller will reset it.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timer {
    /// Identifier for the timer.
    pub id: usize,

    /// The timeout to wait for.
    pub timeout: Duration,
}

unsafe impl FilterSealed for Timer {
    fn filter(&self, flags: kqueue::EventFlags, key: usize) -> kqueue::Event {
        kqueue::Event::new(
            kqueue::EventFilter::Timer {
                ident: self.id as _,
                timer: Some(self.timeout),
            },
            flags | kqueue::EventFlags::RECEIPT,
            key as _,
        )
    }

    #[inline(always)]
    fn source_id(&self) -> SourceId {
        SourceId::Timer(self.id)
    }
}

impl Filter for Timer {}

mod __private {
    use crate::sys::SourceId;
    use rustix::event::kqueue;

    #[doc(hidden)]
    pub unsafe trait FilterSealed {
        /// Get the filter for the given event.
        ///
        /// This filter's flags must have `EV_RECEIPT`.
        fn filter(&self, flags: kqueue::EventFlags, key: usize) -> kqueue::Event;

        /// Get the source ID for this source.
        fn source_id(&self) -> SourceId;
    }
}
