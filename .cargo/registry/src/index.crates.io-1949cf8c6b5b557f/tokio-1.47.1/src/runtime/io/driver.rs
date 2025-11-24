// Signal handling
cfg_signal_internal_and_unix! {
    mod signal;
}
cfg_tokio_uring! {
    mod uring;
    use uring::UringContext;
    use crate::loom::sync::atomic::AtomicUsize;
}

use crate::io::interest::Interest;
use crate::io::ready::Ready;
use crate::loom::sync::Mutex;
use crate::runtime::driver;
use crate::runtime::io::registration_set;
use crate::runtime::io::{IoDriverMetrics, RegistrationSet, ScheduledIo};

use mio::event::Source;
use std::fmt;
use std::io;
use std::sync::Arc;
use std::time::Duration;

/// I/O driver, backed by Mio.
pub(crate) struct Driver {
    /// True when an event with the signal token is received
    signal_ready: bool,

    /// Reuse the `mio::Events` value across calls to poll.
    events: mio::Events,

    /// The system event queue.
    poll: mio::Poll,
}

/// A reference to an I/O driver.
pub(crate) struct Handle {
    /// Registers I/O resources.
    registry: mio::Registry,

    /// Tracks all registrations
    registrations: RegistrationSet,

    /// State that should be synchronized
    synced: Mutex<registration_set::Synced>,

    /// Used to wake up the reactor from a call to `turn`.
    /// Not supported on `Wasi` due to lack of threading support.
    #[cfg(not(target_os = "wasi"))]
    waker: mio::Waker,

    pub(crate) metrics: IoDriverMetrics,

    #[cfg(all(tokio_uring, feature = "rt", feature = "fs", target_os = "linux",))]
    pub(crate) uring_context: Mutex<UringContext>,

    #[cfg(all(tokio_uring, feature = "rt", feature = "fs", target_os = "linux",))]
    pub(crate) uring_state: AtomicUsize,
}

#[derive(Debug)]
pub(crate) struct ReadyEvent {
    pub(super) tick: u8,
    pub(crate) ready: Ready,
    pub(super) is_shutdown: bool,
}

cfg_net_unix!(
    impl ReadyEvent {
        pub(crate) fn with_ready(&self, ready: Ready) -> Self {
            Self {
                ready,
                tick: self.tick,
                is_shutdown: self.is_shutdown,
            }
        }
    }
);

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub(super) enum Direction {
    Read,
    Write,
}

pub(super) enum Tick {
    Set,
    Clear(u8),
}

const TOKEN_WAKEUP: mio::Token = mio::Token(0);
const TOKEN_SIGNAL: mio::Token = mio::Token(1);

fn _assert_kinds() {
    fn _assert<T: Send + Sync>() {}

    _assert::<Handle>();
}

// ===== impl Driver =====

impl Driver {
    /// Creates a new event loop, returning any error that happened during the
    /// creation.
    pub(crate) fn new(nevents: usize) -> io::Result<(Driver, Handle)> {
        let poll = mio::Poll::new()?;
        #[cfg(not(target_os = "wasi"))]
        let waker = mio::Waker::new(poll.registry(), TOKEN_WAKEUP)?;
        let registry = poll.registry().try_clone()?;

        let driver = Driver {
            signal_ready: false,
            events: mio::Events::with_capacity(nevents),
            poll,
        };

        let (registrations, synced) = RegistrationSet::new();

        let handle = Handle {
            registry,
            registrations,
            synced: Mutex::new(synced),
            #[cfg(not(target_os = "wasi"))]
            waker,
            metrics: IoDriverMetrics::default(),
            #[cfg(all(tokio_uring, feature = "rt", feature = "fs", target_os = "linux",))]
            uring_context: Mutex::new(UringContext::new()),
            #[cfg(all(tokio_uring, feature = "rt", feature = "fs", target_os = "linux",))]
            uring_state: AtomicUsize::new(0),
        };

        Ok((driver, handle))
    }

    pub(crate) fn park(&mut self, rt_handle: &driver::Handle) {
        let handle = rt_handle.io();
        self.turn(handle, None);
    }

    pub(crate) fn park_timeout(&mut self, rt_handle: &driver::Handle, duration: Duration) {
        let handle = rt_handle.io();
        self.turn(handle, Some(duration));
    }

    pub(crate) fn shutdown(&mut self, rt_handle: &driver::Handle) {
        let handle = rt_handle.io();
        let ios = handle.registrations.shutdown(&mut handle.synced.lock());

        // `shutdown()` must be called without holding the lock.
        for io in ios {
            io.shutdown();
        }
    }

    fn turn(&mut self, handle: &Handle, max_wait: Option<Duration>) {
        debug_assert!(!handle.registrations.is_shutdown(&handle.synced.lock()));

        handle.release_pending_registrations();

        let events = &mut self.events;

        // Block waiting for an event to happen, peeling out how many events
        // happened.
        match self.poll.poll(events, max_wait) {
            Ok(()) => {}
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
            #[cfg(target_os = "wasi")]
            Err(e) if e.kind() == io::ErrorKind::InvalidInput => {
                // In case of wasm32_wasi this error happens, when trying to poll without subscriptions
                // just return from the park, as there would be nothing, which wakes us up.
            }
            Err(e) => panic!("unexpected error when polling the I/O driver: {e:?}"),
        }

        // Process all the events that came in, dispatching appropriately
        let mut ready_count = 0;
        for event in events.iter() {
            let token = event.token();

            if token == TOKEN_WAKEUP {
                // Nothing to do, the event is used to unblock the I/O driver
            } else if token == TOKEN_SIGNAL {
                self.signal_ready = true;
            } else {
                let ready = Ready::from_mio(event);
                let ptr = super::EXPOSE_IO.from_exposed_addr(token.0);

                // Safety: we ensure that the pointers used as tokens are not freed
                // until they are both deregistered from mio **and** we know the I/O
                // driver is not concurrently polling. The I/O driver holds ownership of
                // an `Arc<ScheduledIo>` so we can safely cast this to a ref.
                let io: &ScheduledIo = unsafe { &*ptr };

                io.set_readiness(Tick::Set, |curr| curr | ready);
                io.wake(ready);

                ready_count += 1;
            }
        }

        #[cfg(all(tokio_uring, feature = "rt", feature = "fs", target_os = "linux",))]
        {
            let mut guard = handle.get_uring().lock();
            let ctx = &mut *guard;
            ctx.dispatch_completions();
        }

        handle.metrics.incr_ready_count_by(ready_count);
    }
}

impl fmt::Debug for Driver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Driver")
    }
}

impl Handle {
    /// Forces a reactor blocked in a call to `turn` to wakeup, or otherwise
    /// makes the next call to `turn` return immediately.
    ///
    /// This method is intended to be used in situations where a notification
    /// needs to otherwise be sent to the main reactor. If the reactor is
    /// currently blocked inside of `turn` then it will wake up and soon return
    /// after this method has been called. If the reactor is not currently
    /// blocked in `turn`, then the next call to `turn` will not block and
    /// return immediately.
    pub(crate) fn unpark(&self) {
        #[cfg(not(target_os = "wasi"))]
        self.waker.wake().expect("failed to wake I/O driver");
    }

    /// Registers an I/O resource with the reactor for a given `mio::Ready` state.
    ///
    /// The registration token is returned.
    pub(super) fn add_source(
        &self,
        source: &mut impl mio::event::Source,
        interest: Interest,
    ) -> io::Result<Arc<ScheduledIo>> {
        let scheduled_io = self.registrations.allocate(&mut self.synced.lock())?;
        let token = scheduled_io.token();

        // we should remove the `scheduled_io` from the `registrations` set if registering
        // the `source` with the OS fails. Otherwise it will leak the `scheduled_io`.
        if let Err(e) = self.registry.register(source, token, interest.to_mio()) {
            // safety: `scheduled_io` is part of the `registrations` set.
            unsafe {
                self.registrations
                    .remove(&mut self.synced.lock(), &scheduled_io)
            };

            return Err(e);
        }

        // TODO: move this logic to `RegistrationSet` and use a `CountedLinkedList`
        self.metrics.incr_fd_count();

        Ok(scheduled_io)
    }

    /// Deregisters an I/O resource from the reactor.
    pub(super) fn deregister_source(
        &self,
        registration: &Arc<ScheduledIo>,
        source: &mut impl Source,
    ) -> io::Result<()> {
        // Deregister the source with the OS poller **first**
        self.registry.deregister(source)?;

        if self
            .registrations
            .deregister(&mut self.synced.lock(), registration)
        {
            self.unpark();
        }

        self.metrics.dec_fd_count();

        Ok(())
    }

    fn release_pending_registrations(&self) {
        if self.registrations.needs_release() {
            self.registrations.release(&mut self.synced.lock());
        }
    }
}

impl fmt::Debug for Handle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Handle")
    }
}

impl Direction {
    pub(super) fn mask(self) -> Ready {
        match self {
            Direction::Read => Ready::READABLE | Ready::READ_CLOSED,
            Direction::Write => Ready::WRITABLE | Ready::WRITE_CLOSED,
        }
    }
}
