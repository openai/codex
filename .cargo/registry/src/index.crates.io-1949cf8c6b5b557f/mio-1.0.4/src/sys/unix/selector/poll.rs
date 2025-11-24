// This implementation is based on the one in the `polling` crate.
// Thanks to https://github.com/Kestrer for the original implementation!
// Permission to use this code has been granted by original author:
// https://github.com/tokio-rs/mio/pull/1602#issuecomment-1218441031

use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
#[cfg(not(target_os = "hermit"))]
use std::os::fd::{AsRawFd, RawFd};
// TODO: once <https://github.com/rust-lang/rust/issues/126198> is fixed this
// can use `std::os::fd` and be merged with the above.
#[cfg(target_os = "hermit")]
use std::os::hermit::io::{AsRawFd, RawFd};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use std::{cmp, fmt, io};

use crate::sys::unix::waker::Waker as WakerInternal;
use crate::{Interest, Token};

/// Unique id for use as `SelectorId`.
#[cfg(debug_assertions)]
static NEXT_ID: AtomicUsize = AtomicUsize::new(1);

#[derive(Debug)]
pub struct Selector {
    state: Arc<SelectorState>,
}

impl Selector {
    pub fn new() -> io::Result<Selector> {
        let state = SelectorState::new()?;

        Ok(Selector {
            state: Arc::new(state),
        })
    }

    pub fn try_clone(&self) -> io::Result<Selector> {
        let state = self.state.clone();

        Ok(Selector { state })
    }

    pub fn select(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()> {
        self.state.select(events, timeout)
    }

    pub fn register(&self, fd: RawFd, token: Token, interests: Interest) -> io::Result<()> {
        self.state.register(fd, token, interests)
    }

    #[allow(dead_code)]
    pub(crate) fn register_internal(
        &self,
        fd: RawFd,
        token: Token,
        interests: Interest,
    ) -> io::Result<Arc<RegistrationRecord>> {
        self.state.register_internal(fd, token, interests)
    }

    pub fn reregister(&self, fd: RawFd, token: Token, interests: Interest) -> io::Result<()> {
        self.state.reregister(fd, token, interests)
    }

    pub fn deregister(&self, fd: RawFd) -> io::Result<()> {
        self.state.deregister(fd)
    }

    pub fn wake(&self, token: Token) -> io::Result<()> {
        self.state.wake(token)
    }

    cfg_io_source! {
        #[cfg(debug_assertions)]
        pub fn id(&self) -> usize {
            self.state.id
        }
    }
}

/// Interface to poll.
#[derive(Debug)]
struct SelectorState {
    /// File descriptors to poll.
    fds: Mutex<Fds>,

    /// File descriptors which will be removed before the next poll call.
    ///
    /// When a file descriptor is deregistered while a poll is running, we need to filter
    /// out all removed descriptors after that poll is finished running.
    pending_removal: Mutex<Vec<RawFd>>,

    /// Token associated with Waker that have recently asked to wake.  This will
    /// cause a synthetic behaviour where on any wakeup we add all pending tokens
    /// to the list of emitted events.
    pending_wake_token: Mutex<Option<Token>>,

    /// Data is written to this to wake up the current instance of `wait`, which can occur when the
    /// user notifies it (in which case `notified` would have been set) or when an operation needs
    /// to occur (in which case `waiting_operations` would have been incremented).
    notify_waker: WakerInternal,

    /// The number of operations (`add`, `modify` or `delete`) that are currently waiting on the
    /// mutex to become free. When this is nonzero, `wait` must be suspended until it reaches zero
    /// again.
    waiting_operations: AtomicUsize,
    /// The condition variable that gets notified when `waiting_operations` reaches zero or
    /// `notified` becomes true.
    ///
    /// This is used with the `fds` mutex.
    operations_complete: Condvar,

    /// This selectors id.
    #[cfg(debug_assertions)]
    #[allow(dead_code)]
    id: usize,
}

/// The file descriptors to poll in a `Poller`.
#[derive(Debug, Clone)]
struct Fds {
    /// The list of `pollfds` taken by poll.
    ///
    /// The first file descriptor is always present and is used to notify the poller.
    poll_fds: Vec<PollFd>,
    /// The map of each file descriptor to data associated with it. This does not include the file
    /// descriptors created by the internal notify waker.
    fd_data: HashMap<RawFd, FdData>,
}

/// Transparent wrapper around `libc::pollfd`, used to support `Debug` derives without adding the
/// `extra_traits` feature of `libc`.
#[repr(transparent)]
#[derive(Clone)]
struct PollFd(libc::pollfd);

impl Debug for PollFd {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("pollfd")
            .field("fd", &self.0.fd)
            .field("events", &self.0.events)
            .field("revents", &self.0.revents)
            .finish()
    }
}

/// Data associated with a file descriptor in a poller.
#[derive(Debug, Clone)]
struct FdData {
    /// The index into `poll_fds` this file descriptor is.
    poll_fds_index: usize,
    /// The key of the `Event` associated with this file descriptor.
    token: Token,
    /// Used to communicate with IoSourceState when we need to internally deregister
    /// based on a closed fd.
    shared_record: Arc<RegistrationRecord>,
}

impl SelectorState {
    pub fn new() -> io::Result<SelectorState> {
        let notify_waker = WakerInternal::new_unregistered()?;

        Ok(Self {
            fds: Mutex::new(Fds {
                poll_fds: vec![PollFd(libc::pollfd {
                    fd: notify_waker.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                })],
                fd_data: HashMap::new(),
            }),
            pending_removal: Mutex::new(Vec::new()),
            pending_wake_token: Mutex::new(None),
            notify_waker,
            waiting_operations: AtomicUsize::new(0),
            operations_complete: Condvar::new(),
            #[cfg(debug_assertions)]
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
        })
    }

    pub fn select(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()> {
        events.clear();

        let mut fds = self.fds.lock().unwrap();

        // Keep track of fds that receive POLLHUP or POLLERR (i.e. won't receive further
        // events) and internally deregister them before they are externally deregister'd.  See
        // IoSourceState below to track how the external deregister call will be handled
        // when this state occurs.
        let mut closed_raw_fds = Vec::new();

        loop {
            // Complete all current operations.
            loop {
                if self.waiting_operations.load(Ordering::SeqCst) == 0 {
                    break;
                }

                fds = self.operations_complete.wait(fds).unwrap();
            }

            // Perform the poll.
            trace!("Polling on {:?}", &fds);
            let num_events = poll(&mut fds.poll_fds, timeout)?;
            trace!("Poll finished: {:?}", &fds);

            if num_events == 0 {
                return Ok(());
            }

            let waker_events = fds.poll_fds[0].0.revents;
            let notified = waker_events != 0;
            let mut num_fd_events = if notified { num_events - 1 } else { num_events };

            let pending_wake_token = self.pending_wake_token.lock().unwrap().take();

            if notified {
                self.notify_waker.ack_and_reset();
                if pending_wake_token.is_some() {
                    num_fd_events += 1;
                }
            }

            // We now check whether this poll was performed with descriptors which were pending
            // for removal and filter out any matching.
            let mut pending_removal_guard = self.pending_removal.lock().unwrap();
            let mut pending_removal = std::mem::replace(pending_removal_guard.as_mut(), Vec::new());
            drop(pending_removal_guard);

            // Store the events if there were any.
            if num_fd_events > 0 {
                let fds = &mut *fds;

                events.reserve(num_fd_events);

                // Add synthetic events we picked up from calls to wake()
                if let Some(pending_wake_token) = pending_wake_token {
                    events.push(Event {
                        token: pending_wake_token,
                        events: waker_events,
                    });
                }

                for fd_data in fds.fd_data.values_mut() {
                    let PollFd(poll_fd) = &mut fds.poll_fds[fd_data.poll_fds_index];

                    if pending_removal.contains(&poll_fd.fd) {
                        // Fd was removed while poll was running
                        continue;
                    }

                    if poll_fd.revents != 0 {
                        // Store event
                        events.push(Event {
                            token: fd_data.token,
                            events: poll_fd.revents,
                        });

                        if poll_fd.revents & (libc::POLLHUP | libc::POLLERR) != 0 {
                            pending_removal.push(poll_fd.fd);
                            closed_raw_fds.push(poll_fd.fd);
                        }

                        // Remove the interest which just got triggered the IoSourceState's do_io
                        // wrapper used with this selector will add back the interest using
                        // reregister.
                        poll_fd.events &= !poll_fd.revents;

                        // Minor optimization to potentially avoid looping n times where n is the
                        // number of input fds (i.e. we might loop between m and n times where m is
                        // the number of fds with revents != 0).
                        if events.len() == num_fd_events {
                            break;
                        }
                    }
                }

                break; // No more polling.
            }

            // If we didn't break above it means we got woken up internally (for example for adding an fd), so we poll again.
        }

        drop(fds);
        let _ = self.deregister_all(&closed_raw_fds);

        Ok(())
    }

    pub fn register(&self, fd: RawFd, token: Token, interests: Interest) -> io::Result<()> {
        self.register_internal(fd, token, interests).map(|_| ())
    }

    pub fn register_internal(
        &self,
        fd: RawFd,
        token: Token,
        interests: Interest,
    ) -> io::Result<Arc<RegistrationRecord>> {
        #[cfg(debug_assertions)]
        if fd == self.notify_waker.as_raw_fd() {
            return Err(io::Error::from(io::ErrorKind::InvalidInput));
        }

        // We must handle the unlikely case that the following order of operations happens:
        //
        // register(1 as RawFd)
        // deregister(1 as RawFd)
        // register(1 as RawFd)
        // <poll happens>
        //
        // Fd's pending removal only get cleared when poll has been run. It is possible that
        // between registering and deregistering and then _again_ registering the file descriptor
        // poll never gets called, thus the fd stays stuck in the pending removal list.
        //
        // To avoid this scenario we remove an fd from pending removals when registering it.
        let mut pending_removal = self.pending_removal.lock().unwrap();
        if let Some(idx) = pending_removal.iter().position(|&pending| pending == fd) {
            pending_removal.swap_remove(idx);
        }
        drop(pending_removal);

        self.modify_fds(|fds| {
            if fds.fd_data.contains_key(&fd) {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "I/O source already registered this `Registry` \
                    (an old file descriptor might have been closed without deregistration)",
                ));
            }

            let poll_fds_index = fds.poll_fds.len();
            let record = Arc::new(RegistrationRecord::new());
            fds.fd_data.insert(
                fd,
                FdData {
                    poll_fds_index,
                    token,
                    shared_record: record.clone(),
                },
            );

            fds.poll_fds.push(PollFd(libc::pollfd {
                fd,
                events: interests_to_poll(interests),
                revents: 0,
            }));

            Ok(record)
        })
    }

    pub fn reregister(&self, fd: RawFd, token: Token, interests: Interest) -> io::Result<()> {
        self.modify_fds(|fds| {
            let data = fds.fd_data.get_mut(&fd).ok_or(io::ErrorKind::NotFound)?;
            data.token = token;
            let poll_fds_index = data.poll_fds_index;
            fds.poll_fds[poll_fds_index].0.events = interests_to_poll(interests);

            Ok(())
        })
    }

    pub fn deregister(&self, fd: RawFd) -> io::Result<()> {
        self.deregister_all(&[fd])
            .map_err(|_| io::ErrorKind::NotFound)?;
        Ok(())
    }

    /// Perform a modification on `fds`, interrupting the current caller of `wait` if it's running.
    fn modify_fds<T>(&self, f: impl FnOnce(&mut Fds) -> T) -> T {
        self.waiting_operations.fetch_add(1, Ordering::SeqCst);

        // Wake up the current caller of `wait` if there is one.
        let sent_notification = self.notify_waker.wake().is_ok();

        let mut fds = self.fds.lock().unwrap();

        // If there was no caller of `wait` our notification was not removed from the pipe.
        if sent_notification {
            self.notify_waker.ack_and_reset();
        }

        let res = f(&mut *fds);

        if self.waiting_operations.fetch_sub(1, Ordering::SeqCst) == 1 {
            self.operations_complete.notify_one();
        }

        res
    }

    /// Special optimized version of [Self::deregister] which handles multiple removals
    /// at once.  Ok result if all removals were performed, Err if any entries
    /// were not found.
    fn deregister_all(&self, targets: &[RawFd]) -> Result<(), ()> {
        if targets.is_empty() {
            return Ok(());
        }

        let mut pending_removal = self.pending_removal.lock().unwrap();
        pending_removal.extend(targets);
        drop(pending_removal);

        self.modify_fds(|fds| {
            let mut all_successful = true;

            for target in targets {
                match fds.fd_data.remove(target).ok_or(()) {
                    Ok(data) => {
                        data.shared_record.mark_unregistered();
                        fds.poll_fds.swap_remove(data.poll_fds_index);
                        if let Some(swapped_pollfd) = fds.poll_fds.get(data.poll_fds_index) {
                            fds.fd_data
                                .get_mut(&swapped_pollfd.0.fd)
                                .unwrap()
                                .poll_fds_index = data.poll_fds_index;
                        }
                    }
                    Err(_) => all_successful = false,
                }
            }

            if all_successful {
                Ok(())
            } else {
                Err(())
            }
        })
    }

    pub fn wake(&self, token: Token) -> io::Result<()> {
        self.pending_wake_token.lock().unwrap().replace(token);
        self.notify_waker.wake()
    }
}

/// Shared record between IoSourceState and SelectorState that allows us to internally
/// deregister partially or fully closed fds (i.e. when we get POLLHUP or PULLERR) without
/// confusing IoSourceState and trying to deregister twice.  This isn't strictly
/// required as technically deregister is idempotent but it is confusing
/// when trying to debug behaviour as we get imbalanced calls to register/deregister and
/// superfluous NotFound errors.
#[derive(Debug)]
pub(crate) struct RegistrationRecord {
    is_unregistered: AtomicBool,
}

impl RegistrationRecord {
    pub fn new() -> Self {
        Self {
            is_unregistered: AtomicBool::new(false),
        }
    }

    pub fn mark_unregistered(&self) {
        self.is_unregistered.store(true, Ordering::Relaxed);
    }

    #[allow(dead_code)]
    pub fn is_registered(&self) -> bool {
        !self.is_unregistered.load(Ordering::Relaxed)
    }
}

#[cfg(target_os = "linux")]
const POLLRDHUP: libc::c_short = libc::POLLRDHUP;
#[cfg(not(target_os = "linux"))]
const POLLRDHUP: libc::c_short = 0;

const READ_EVENTS: libc::c_short = libc::POLLIN | POLLRDHUP;

const WRITE_EVENTS: libc::c_short = libc::POLLOUT;

const PRIORITY_EVENTS: libc::c_short = libc::POLLPRI;

/// Get the input poll events for the given event.
fn interests_to_poll(interest: Interest) -> libc::c_short {
    let mut kind = 0;

    if interest.is_readable() {
        kind |= READ_EVENTS;
    }

    if interest.is_writable() {
        kind |= WRITE_EVENTS;
    }

    if interest.is_priority() {
        kind |= PRIORITY_EVENTS;
    }

    kind
}

/// Helper function to call poll.
fn poll(fds: &mut [PollFd], timeout: Option<Duration>) -> io::Result<usize> {
    loop {
        // A bug in kernels < 2.6.37 makes timeouts larger than LONG_MAX / CONFIG_HZ
        // (approx. 30 minutes with CONFIG_HZ=1200) effectively infinite on 32 bits
        // architectures. The magic number is the same constant used by libuv.
        #[cfg(target_pointer_width = "32")]
        const MAX_SAFE_TIMEOUT: u128 = 1789569;
        #[cfg(not(target_pointer_width = "32"))]
        const MAX_SAFE_TIMEOUT: u128 = libc::c_int::MAX as u128;

        let timeout = timeout
            .map(|to| {
                // `Duration::as_millis` truncates, so round up. This avoids
                // turning sub-millisecond timeouts into a zero timeout, unless
                // the caller explicitly requests that by specifying a zero
                // timeout.
                let to_ms = to
                    .checked_add(Duration::from_nanos(999_999))
                    .unwrap_or(to)
                    .as_millis();
                cmp::min(MAX_SAFE_TIMEOUT, to_ms) as libc::c_int
            })
            .unwrap_or(-1);

        let res = syscall!(poll(
            fds.as_mut_ptr() as *mut libc::pollfd,
            fds.len() as libc::nfds_t,
            timeout,
        ));

        match res {
            Ok(num_events) => break Ok(num_events as usize),
            // poll returns EAGAIN if we can retry it.
            Err(e) if e.raw_os_error() == Some(libc::EAGAIN) => continue,
            Err(e) => return Err(e),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Event {
    token: Token,
    events: libc::c_short,
}

pub type Events = Vec<Event>;

pub mod event {
    use std::fmt;

    use crate::sys::Event;
    use crate::Token;

    use super::POLLRDHUP;

    pub fn token(event: &Event) -> Token {
        event.token
    }

    pub fn is_readable(event: &Event) -> bool {
        (event.events & libc::POLLIN) != 0 || (event.events & libc::POLLPRI) != 0
    }

    pub fn is_writable(event: &Event) -> bool {
        (event.events & libc::POLLOUT) != 0
    }

    pub fn is_error(event: &Event) -> bool {
        (event.events & libc::POLLERR) != 0
    }

    pub fn is_read_closed(event: &Event) -> bool {
        // Both halves of the socket have closed
        (event.events & libc::POLLHUP) != 0
            // Socket has received FIN or called shutdown(SHUT_RD)
            || (event.events & POLLRDHUP) != 0
    }

    pub fn is_write_closed(event: &Event) -> bool {
        // Both halves of the socket have closed
        (event.events & libc::POLLHUP) != 0
            // Unix pipe write end has closed
            || ((event.events & libc::POLLOUT) != 0 && (event.events & libc::POLLERR) != 0)
            // The other side (read end) of a Unix pipe has closed.
            || (event.events == libc::POLLERR)
    }

    pub fn is_priority(event: &Event) -> bool {
        (event.events & libc::POLLPRI) != 0
    }

    pub fn is_aio(_: &Event) -> bool {
        // Not supported in the kernel, only in libc.
        false
    }

    pub fn is_lio(_: &Event) -> bool {
        // Not supported.
        false
    }

    pub fn debug_details(f: &mut fmt::Formatter<'_>, event: &Event) -> fmt::Result {
        #[allow(clippy::trivially_copy_pass_by_ref)]
        fn check_events(got: &libc::c_short, want: &libc::c_short) -> bool {
            (*got & want) != 0
        }
        debug_detail!(
            EventsDetails(libc::c_short),
            check_events,
            libc::POLLIN,
            libc::POLLPRI,
            libc::POLLOUT,
            libc::POLLRDNORM,
            libc::POLLRDBAND,
            libc::POLLWRNORM,
            libc::POLLWRBAND,
            libc::POLLERR,
            libc::POLLHUP,
        );

        f.debug_struct("poll_event")
            .field("token", &event.token)
            .field("events", &EventsDetails(event.events))
            .finish()
    }
}

#[derive(Debug)]
pub(crate) struct Waker {
    selector: Selector,
    token: Token,
}

impl Waker {
    pub(crate) fn new(selector: &Selector, token: Token) -> io::Result<Waker> {
        Ok(Waker {
            selector: selector.try_clone()?,
            token,
        })
    }

    pub(crate) fn wake(&self) -> io::Result<()> {
        self.selector.wake(self.token)
    }
}

cfg_io_source! {
    use crate::Registry;

    struct InternalState {
        selector: Selector,
        token: Token,
        interests: Interest,
        fd: RawFd,
        shared_record: Arc<RegistrationRecord>,
    }

    impl Drop for InternalState {
        fn drop(&mut self) {
            if self.shared_record.is_registered() {
                let _ = self.selector.deregister(self.fd);
            }
        }
    }

    pub(crate) struct IoSourceState {
        inner: Option<Box<InternalState>>,
    }

    impl IoSourceState {
        pub fn new() -> IoSourceState {
            IoSourceState { inner: None }
        }

        pub fn do_io<T, F, R>(&self, f: F, io: &T) -> io::Result<R>
        where
        F: FnOnce(&T) -> io::Result<R>,
        {
            let result = f(io);

            if let Err(err) = &result {
                if err.kind() == io::ErrorKind::WouldBlock {
                    self.inner.as_ref().map_or(Ok(()), |state| {
                        state
                        .selector
                        .reregister(state.fd, state.token, state.interests)
                    })?;
                }
            }

            result
        }

        pub fn register(
            &mut self,
            registry: &Registry,
            token: Token,
            interests: Interest,
            fd: RawFd,
        ) -> io::Result<()> {
            if self.inner.is_some() {
                Err(io::ErrorKind::AlreadyExists.into())
            } else {
                let selector = registry.selector().try_clone()?;

                selector.register_internal(fd, token, interests).map(move |shared_record| {
                    let state = InternalState {
                        selector,
                        token,
                        interests,
                        fd,
                        shared_record,
                    };

                    self.inner = Some(Box::new(state));
                })
            }
        }

        pub fn reregister(
            &mut self,
            registry: &Registry,
            token: Token,
            interests: Interest,
            fd: RawFd,
        ) -> io::Result<()> {
            match self.inner.as_mut() {
                Some(state) => registry
                .selector()
                .reregister(fd, token, interests)
                .map(|()| {
                    state.token = token;
                    state.interests = interests;
                }),
                None => Err(io::ErrorKind::NotFound.into()),
            }
        }

        pub fn deregister(&mut self, registry: &Registry, fd: RawFd) -> io::Result<()> {
            if let Some(state) = self.inner.take() {
                // Marking unregistered will short circuit the drop behaviour of calling
                // deregister so the call to deregister below is strictly required.
                state.shared_record.mark_unregistered();
            }

            registry.selector().deregister(fd)
        }
    }
}
