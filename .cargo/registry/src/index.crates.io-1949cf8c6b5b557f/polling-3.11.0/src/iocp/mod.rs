//! Bindings to Windows I/O Completion Ports.
//!
//! I/O Completion Ports is a completion-based API rather than a polling-based API, like
//! epoll or kqueue. Therefore, we have to adapt the IOCP API to the crate's API.
//!
//! WinSock is powered by the Auxiliary Function Driver (AFD) subsystem, which can be
//! accessed directly by using unstable `ntdll` functions. AFD exposes features that are not
//! available through the normal WinSock interface, such as IOCTL_AFD_POLL. This function is
//! similar to the exposed `WSAPoll` method. However, once the targeted socket is "ready",
//! a completion packet is queued to an I/O completion port.
//!
//! We take advantage of IOCTL_AFD_POLL to "translate" this crate's polling-based API
//! to the one Windows expects. When a device is added to the `Poller`, an IOCTL_AFD_POLL
//! operation is started and queued to the IOCP. To modify a currently registered device
//! (e.g. with `modify()` or `delete()`), the ongoing POLL is cancelled and then restarted
//! with new parameters. When the POLL eventually completes, the packet is posted to the IOCP.
//! From here it's a simple matter of using `GetQueuedCompletionStatusEx` to read the packets
//! from the IOCP and react accordingly. Notifying the poller is trivial, because we can
//! simply post a packet to the IOCP to wake it up.
//!
//! The main disadvantage of this strategy is that it relies on unstable Windows APIs.
//! However, as `libuv` (the backing I/O library for Node.JS) relies on the same unstable
//! AFD strategy, it is unlikely to be broken without plenty of advanced warning.
//!
//! Previously, this crate used the `wepoll` library for polling. `wepoll` uses a similar
//! AFD-based strategy for polling.

mod afd;
mod port;

use afd::{base_socket, Afd, AfdPollInfo, AfdPollMask, HasAfdInfo, IoStatusBlock};
use port::{IoCompletionPort, OverlappedEntry};

use windows_sys::Win32::Foundation::{ERROR_INVALID_HANDLE, ERROR_IO_PENDING, STATUS_CANCELLED};
use windows_sys::Win32::System::Threading::{
    RegisterWaitForSingleObject, UnregisterWait, INFINITE, WT_EXECUTELONGFUNCTION,
    WT_EXECUTEONLYONCE,
};

use crate::{Event, PollMode};

use concurrent_queue::ConcurrentQueue;
use pin_project_lite::pin_project;

use std::cell::UnsafeCell;
use std::collections::hash_map::{Entry, HashMap};
use std::ffi::c_void;
use std::fmt;
use std::io;
use std::marker::PhantomPinned;
use std::mem::{forget, MaybeUninit};
use std::os::windows::io::{
    AsHandle, AsRawHandle, AsRawSocket, BorrowedHandle, BorrowedSocket, RawHandle, RawSocket,
};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, RwLock, Weak};
use std::time::{Duration, Instant};

/// Macro to lock and ignore lock poisoning.
macro_rules! lock {
    ($lock_result:expr) => {{
        $lock_result.unwrap_or_else(|e| e.into_inner())
    }};
}

/// Interface to I/O completion ports.
#[derive(Debug)]
pub(super) struct Poller {
    /// The I/O completion port.
    port: Arc<IoCompletionPort<Packet>>,

    /// List of currently active AFD instances.
    ///
    /// AFD acts as the actual source of the socket events. It's essentially running `WSAPoll` on
    /// the sockets and then posting the events to the IOCP.
    ///
    /// AFD instances can be keyed to an unlimited number of sockets. However, each AFD instance
    /// polls their sockets linearly. Therefore, it is best to limit the number of sockets each AFD
    /// instance is responsible for. The limit of 32 is chosen because that's what `wepoll` uses.
    ///
    /// Weak references are kept here so that the AFD handle is automatically dropped when the last
    /// associated socket is dropped.
    afd: Mutex<Vec<Weak<Afd<Packet>>>>,

    /// The state of the sources registered with this poller.
    ///
    /// Each source is keyed by its raw socket ID.
    sources: RwLock<HashMap<RawSocket, Packet>>,

    /// The state of the waitable handles registered with this poller.
    waitables: RwLock<HashMap<RawHandle, Packet>>,

    /// Sockets with pending updates.
    ///
    /// This list contains packets with sockets that need to have their AFD state adjusted by
    /// calling the `update()` function on them. It's best to queue up packets as they need to
    /// be updated and then run all of the updates before we start waiting on the IOCP, rather than
    /// updating them as we come. If we're waiting on the IOCP updates should be run immediately.
    pending_updates: ConcurrentQueue<Packet>,

    /// Are we currently polling?
    ///
    /// This indicates whether or not we are blocking on the IOCP, and is used to determine
    /// whether pending updates should be run immediately or queued.
    polling: AtomicBool,

    /// The packet used to notify the poller.
    ///
    /// This is a special-case packet that is used to wake up the poller when it is waiting.
    notifier: Packet,
}

unsafe impl Send for Poller {}
unsafe impl Sync for Poller {}

impl Poller {
    /// Creates a new poller.
    pub(super) fn new() -> io::Result<Self> {
        // Make sure AFD is able to be used.
        if let Err(e) = afd::NtdllImports::force_load() {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                AfdError::new("failed to initialize unstable Windows functions", e),
            ));
        }

        // Create and destroy a single AFD to test if we support it.
        Afd::<Packet>::new().map_err(|e| {
            io::Error::new(
                io::ErrorKind::Unsupported,
                AfdError::new("failed to initialize \\Device\\Afd", e),
            )
        })?;

        let port = IoCompletionPort::new(0)?;
        #[cfg(feature = "tracing")]
        tracing::trace!(handle = ?port, "new");

        Ok(Poller {
            #[allow(clippy::arc_with_non_send_sync)]
            port: Arc::new(port),
            afd: Mutex::new(vec![]),
            sources: RwLock::new(HashMap::new()),
            waitables: RwLock::new(HashMap::new()),
            pending_updates: ConcurrentQueue::bounded(1024),
            polling: AtomicBool::new(false),
            notifier: Arc::pin(
                PacketInner::Wakeup {
                    _pinned: PhantomPinned,
                }
                .into(),
            ),
        })
    }

    /// Whether this poller supports level-triggered events.
    pub(super) fn supports_level(&self) -> bool {
        true
    }

    /// Whether this poller supports edge-triggered events.
    pub(super) fn supports_edge(&self) -> bool {
        false
    }

    /// Add a new source to the poller.
    ///
    /// # Safety
    ///
    /// The socket must be a valid socket and must last until it is deleted.
    pub(super) unsafe fn add(
        &self,
        socket: RawSocket,
        interest: Event,
        mode: PollMode,
    ) -> io::Result<()> {
        #[cfg(feature = "tracing")]
        let span = tracing::trace_span!(
            "add",
            handle = ?self.port,
            sock = ?socket,
            ev = ?interest,
        );
        #[cfg(feature = "tracing")]
        let _enter = span.enter();

        // We don't support edge-triggered events.
        if matches!(mode, PollMode::Edge | PollMode::EdgeOneshot) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "edge-triggered events are not supported",
            ));
        }

        // Create a new packet.
        let socket_state = {
            // Create a new socket state and assign an AFD handle to it.
            let state = SocketState {
                socket,
                base_socket: base_socket(socket)?,
                interest,
                interest_error: true,
                afd: self.afd_handle()?,
                mode,
                waiting_on_delete: false,
                status: SocketStatus::Idle,
            };

            // We wrap this socket state in a Packet so the IOCP can use it.
            Arc::pin(IoStatusBlock::from(PacketInner::Socket {
                packet: UnsafeCell::new(AfdPollInfo::default()),
                socket: Mutex::new(state),
            }))
        };

        // Keep track of the source in the poller.
        {
            let mut sources = lock!(self.sources.write());

            match sources.entry(socket) {
                Entry::Vacant(v) => {
                    v.insert(Pin::<Arc<_>>::clone(&socket_state));
                }

                Entry::Occupied(_) => {
                    return Err(io::Error::from(io::ErrorKind::AlreadyExists));
                }
            }
        }

        // Update the packet.
        self.update_packet(socket_state)
    }

    /// Update a source in the poller.
    pub(super) fn modify(
        &self,
        socket: BorrowedSocket<'_>,
        interest: Event,
        mode: PollMode,
    ) -> io::Result<()> {
        #[cfg(feature = "tracing")]
        let span = tracing::trace_span!(
            "modify",
            handle = ?self.port,
            sock = ?socket,
            ev = ?interest,
        );
        #[cfg(feature = "tracing")]
        let _enter = span.enter();

        // We don't support edge-triggered events.
        if matches!(mode, PollMode::Edge | PollMode::EdgeOneshot) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "edge-triggered events are not supported",
            ));
        }

        // Get a reference to the source.
        let source = {
            let sources = lock!(self.sources.read());

            sources
                .get(&socket.as_raw_socket())
                .cloned()
                .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?
        };

        // Set the new event.
        if source.as_ref().set_events(interest, mode) {
            // The packet needs to be updated.
            self.update_packet(source)?;
        }

        Ok(())
    }

    /// Delete a source from the poller.
    pub(super) fn delete(&self, socket: BorrowedSocket<'_>) -> io::Result<()> {
        #[cfg(feature = "tracing")]
        let span = tracing::trace_span!(
            "remove",
            handle = ?self.port,
            sock = ?socket,
        );
        #[cfg(feature = "tracing")]
        let _enter = span.enter();

        // Remove the source from our associative map.
        let source = {
            let mut sources = lock!(self.sources.write());

            match sources.remove(&socket.as_raw_socket()) {
                Some(s) => s,
                None => {
                    // If the source has already been removed, then we can just return.
                    return Ok(());
                }
            }
        };

        // Indicate to the source that it is being deleted.
        // This cancels any ongoing AFD_IOCTL_POLL operations.
        source.begin_delete()
    }

    /// Add a new waitable to the poller.
    pub(super) fn add_waitable(
        &self,
        handle: RawHandle,
        interest: Event,
        mode: PollMode,
    ) -> io::Result<()> {
        #[cfg(feature = "tracing")]
        tracing::trace!(
            "add_waitable: handle={:?}, waitable={:p}, ev={:?}",
            self.port,
            handle,
            interest
        );

        // We don't support edge-triggered events.
        if matches!(mode, PollMode::Edge | PollMode::EdgeOneshot) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "edge-triggered events are not supported",
            ));
        }

        // Create a new packet.
        let handle_state = {
            let state = WaitableState {
                handle,
                port: Arc::downgrade(&self.port),
                interest,
                mode,
                status: WaitableStatus::Idle,
            };

            Arc::pin(IoStatusBlock::from(PacketInner::Waitable {
                handle: Mutex::new(state),
            }))
        };

        // Keep track of the source in the poller.
        {
            let mut sources = lock!(self.waitables.write());

            match sources.entry(handle) {
                Entry::Vacant(v) => {
                    v.insert(Pin::<Arc<_>>::clone(&handle_state));
                }

                Entry::Occupied(_) => {
                    return Err(io::Error::from(io::ErrorKind::AlreadyExists));
                }
            }
        }

        // Update the packet.
        self.update_packet(handle_state)
    }

    /// Update a waitable in the poller.
    pub(crate) fn modify_waitable(
        &self,
        waitable: RawHandle,
        interest: Event,
        mode: PollMode,
    ) -> io::Result<()> {
        #[cfg(feature = "tracing")]
        tracing::trace!(
            "modify_waitable: handle={:?}, waitable={:p}, ev={:?}",
            self.port,
            waitable,
            interest
        );

        // We don't support edge-triggered events.
        if matches!(mode, PollMode::Edge | PollMode::EdgeOneshot) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "edge-triggered events are not supported",
            ));
        }

        // Get a reference to the source.
        let source = {
            let sources = lock!(self.waitables.read());

            sources
                .get(&waitable)
                .cloned()
                .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?
        };

        // Set the new event.
        if source.as_ref().set_events(interest, mode) {
            self.update_packet(source)?;
        }

        Ok(())
    }

    /// Delete a waitable from the poller.
    pub(super) fn remove_waitable(&self, waitable: RawHandle) -> io::Result<()> {
        #[cfg(feature = "tracing")]
        tracing::trace!("remove: handle={:?}, waitable={:p}", self.port, waitable);

        // Get a reference to the source.
        let source = {
            let mut sources = lock!(self.waitables.write());

            match sources.remove(&waitable) {
                Some(s) => s,
                None => {
                    // If the source has already been removed, then we can just return.
                    return Ok(());
                }
            }
        };

        // Indicate to the source that it is being deleted.
        // This cancels any ongoing AFD_IOCTL_POLL operations.
        source.begin_delete()
    }

    /// Wait for events.
    pub(super) fn wait_deadline(
        &self,
        events: &mut Events,
        deadline: Option<Instant>,
    ) -> io::Result<()> {
        #[cfg(feature = "tracing")]
        let span = tracing::trace_span!(
            "wait",
            handle = ?self.port,
            ?deadline,
        );
        #[cfg(feature = "tracing")]
        let _enter = span.enter();

        let mut notified = false;

        loop {
            let mut new_events = 0;

            // Indicate that we are now polling.
            let was_polling = self.polling.swap(true, Ordering::SeqCst);
            debug_assert!(!was_polling);

            // Even if we panic, we want to make sure we indicate that polling has stopped.
            let guard = CallOnDrop(|| {
                let was_polling = self.polling.swap(false, Ordering::SeqCst);
                debug_assert!(was_polling);
            });

            // Process every entry in the queue before we start polling.
            self.drain_update_queue(false)?;

            // Get the time to wait for.
            let timeout = deadline.map(|t| t.saturating_duration_since(Instant::now()));

            // Wait for I/O events.
            let _len = self.port.wait(&mut events.completions, timeout)?;
            #[cfg(feature = "tracing")]
            tracing::trace!(
                handle = ?self.port,
                res = ?_len,
                "new events");

            // We are no longer polling.
            drop(guard);

            // Process all of the events.
            for entry in events.completions.drain(..) {
                let packet = entry.into_packet();

                // Feed the event into the packet.
                match packet.feed_event(self)? {
                    FeedEventResult::NoEvent => {}
                    FeedEventResult::Event(event) => {
                        events.packets.push(event);
                        new_events += 1;
                    }
                    FeedEventResult::Notified => {
                        notified = true;
                    }
                }
            }

            // Break if there was a notification or at least one event, or if deadline is reached.
            let timeout_is_empty = timeout.is_some_and(|t| t.is_zero());
            if notified || new_events > 0 || timeout_is_empty {
                break;
            }

            #[cfg(feature = "tracing")]
            tracing::trace!("wait: no events found, re-entering polling loop");
        }

        Ok(())
    }

    /// Notify this poller.
    pub(super) fn notify(&self) -> io::Result<()> {
        // Push the notify packet into the IOCP.
        self.port.post(0, 0, self.notifier.clone())
    }

    /// Push an IOCP packet into the queue.
    pub(super) fn post(&self, packet: CompletionPacket) -> io::Result<()> {
        self.port.post(0, 0, packet.0)
    }

    /// Run an update on a packet.
    fn update_packet(&self, mut packet: Packet) -> io::Result<()> {
        loop {
            // If we are currently polling, we need to update the packet immediately.
            if self.polling.load(Ordering::Acquire) {
                packet.update()?;
                return Ok(());
            }

            // Try to queue the update.
            match self.pending_updates.push(packet) {
                Ok(()) => return Ok(()),
                Err(p) => packet = p.into_inner(),
            }

            // If we failed to queue the update, we need to drain the queue first.
            self.drain_update_queue(true)?;

            // Loop back and try again.
        }
    }

    /// Drain the update queue.
    fn drain_update_queue(&self, limit: bool) -> io::Result<()> {
        // Determine how many packets to process.
        let max = if limit {
            // Only drain the queue's capacity, since this could in theory run forever.
            self.pending_updates.capacity().unwrap()
        } else {
            // Less of a concern if we're draining the queue prior to a poll operation.
            usize::MAX
        };

        self.pending_updates
            .try_iter()
            .take(max)
            .try_for_each(|packet| packet.update())
    }

    /// Get a handle to the AFD reference.
    ///
    /// This finds an AFD handle with less than 32 associated sockets, or creates a new one if
    /// one does not exist.
    fn afd_handle(&self) -> io::Result<Arc<Afd<Packet>>> {
        const AFD_MAX_SIZE: usize = 32;

        // Crawl the list and see if there are any existing AFD instances that we can use.
        // While we're here, remove any unused AFD pointers.
        let mut afd_handles = lock!(self.afd.lock());
        let mut i = 0;
        while i < afd_handles.len() {
            // Get the reference count of the AFD instance.
            let refcount = Weak::strong_count(&afd_handles[i]);

            match refcount {
                0 => {
                    // Prune the AFD pointer if it has no references.
                    afd_handles.swap_remove(i);
                }

                refcount if refcount >= AFD_MAX_SIZE => {
                    // Skip this one, since it is already at the maximum size.
                    i += 1;
                }

                _ => {
                    // We can use this AFD instance.
                    match afd_handles[i].upgrade() {
                        Some(afd) => return Ok(afd),
                        None => {
                            // The last socket dropped the AFD before we could acquire it.
                            // Prune the AFD pointer and continue.
                            afd_handles.swap_remove(i);
                        }
                    }
                }
            }
        }

        // No available handles, create a new AFD instance.
        #[allow(clippy::arc_with_non_send_sync)]
        let afd = Arc::new(Afd::new()?);

        // Register the AFD instance with the I/O completion port.
        self.port.register(&*afd, true)?;

        // Insert a weak pointer to the AFD instance into the list for other sockets.
        afd_handles.push(Arc::downgrade(&afd));

        Ok(afd)
    }
}

impl AsRawHandle for Poller {
    fn as_raw_handle(&self) -> RawHandle {
        self.port.as_raw_handle()
    }
}

impl AsHandle for Poller {
    fn as_handle(&self) -> BorrowedHandle<'_> {
        unsafe { BorrowedHandle::borrow_raw(self.as_raw_handle()) }
    }
}

/// The container for events.
pub(super) struct Events {
    /// List of IOCP packets.
    packets: Vec<Event>,

    /// Buffer for completion packets.
    completions: Vec<OverlappedEntry<Packet>>,
}

unsafe impl Send for Events {}

impl Events {
    /// Creates an empty list of events.
    pub fn with_capacity(cap: usize) -> Events {
        Events {
            packets: Vec::with_capacity(cap),
            completions: Vec::with_capacity(cap),
        }
    }

    /// Iterate over I/O events.
    pub fn iter(&self) -> impl Iterator<Item = Event> + '_ {
        self.packets.iter().copied()
    }

    /// Clear the list of events.
    pub fn clear(&mut self) {
        self.packets.clear();
    }

    /// The capacity of the list of events.
    pub fn capacity(&self) -> usize {
        self.packets.capacity()
    }
}

/// Extra information about an event.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct EventExtra {
    /// Flags associated with this event.
    flags: AfdPollMask,
}

impl EventExtra {
    /// Create a new, empty version of this struct.
    #[inline]
    pub const fn empty() -> EventExtra {
        EventExtra {
            flags: AfdPollMask::empty(),
        }
    }

    /// Is this a HUP event?
    #[inline]
    pub fn is_hup(&self) -> bool {
        self.flags.intersects(AfdPollMask::ABORT)
    }

    /// Is this a PRI event?
    #[inline]
    pub fn is_pri(&self) -> bool {
        self.flags.intersects(AfdPollMask::RECEIVE_EXPEDITED)
    }

    /// Set up a listener for HUP events.
    #[inline]
    pub fn set_hup(&mut self, active: bool) {
        self.flags.set(AfdPollMask::ABORT, active);
    }

    /// Set up a listener for PRI events.
    #[inline]
    pub fn set_pri(&mut self, active: bool) {
        self.flags.set(AfdPollMask::RECEIVE_EXPEDITED, active);
    }

    /// Check if TCP connect failed. Deprecated.
    #[inline]
    pub fn is_connect_failed(&self) -> Option<bool> {
        Some(self.flags.intersects(AfdPollMask::CONNECT_FAIL))
    }

    /// Check if TCP connect failed.
    #[inline]
    pub fn is_err(&self) -> Option<bool> {
        Some(self.flags.intersects(AfdPollMask::CONNECT_FAIL))
    }
}

/// A packet used to wake up the poller with an event.
#[derive(Debug, Clone)]
pub struct CompletionPacket(Packet);

impl CompletionPacket {
    /// Create a new completion packet with a custom event.
    pub fn new(event: Event) -> Self {
        Self(Arc::pin(IoStatusBlock::from(PacketInner::Custom { event })))
    }

    /// Get the event associated with this packet.
    pub fn event(&self) -> &Event {
        let data = self.0.as_ref().data().project_ref();

        match data {
            PacketInnerProj::Custom { event } => event,
            _ => unreachable!(),
        }
    }
}

/// The type of our completion packet.
///
/// It needs to be pinned, since it contains data that is expected by IOCP not to be moved.
type Packet = Pin<Arc<PacketUnwrapped>>;
type PacketUnwrapped = IoStatusBlock<PacketInner>;

pin_project! {
    /// The inner type of the packet.
    #[project_ref = PacketInnerProj]
    #[project = PacketInnerProjMut]
    enum PacketInner {
        // A packet for a socket.
        Socket {
            // The AFD packet state.
            #[pin]
            packet: UnsafeCell<AfdPollInfo>,

            // The socket state.
            socket: Mutex<SocketState>
        },

        /// A packet for a waitable handle.
        Waitable {
            handle: Mutex<WaitableState>
        },

        /// A custom event sent by the user.
        Custom {
            event: Event,
        },

        // A packet used to wake up the poller.
        Wakeup { #[pin] _pinned: PhantomPinned },
    }
}

unsafe impl Send for PacketInner {}
unsafe impl Sync for PacketInner {}

impl fmt::Debug for PacketInner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wakeup { .. } => f.write_str("Wakeup { .. }"),
            Self::Custom { event } => f.debug_struct("Custom").field("event", event).finish(),
            Self::Socket { socket, .. } => f
                .debug_struct("Socket")
                .field("packet", &"..")
                .field("socket", socket)
                .finish(),
            Self::Waitable { handle } => {
                f.debug_struct("Waitable").field("handle", handle).finish()
            }
        }
    }
}

impl HasAfdInfo for PacketInner {
    fn afd_info(self: Pin<&Self>) -> Pin<&UnsafeCell<AfdPollInfo>> {
        match self.project_ref() {
            PacketInnerProj::Socket { packet, .. } => packet,
            _ => unreachable!(),
        }
    }
}

impl PacketUnwrapped {
    /// Set the new events that this socket is waiting on.
    ///
    /// Returns `true` if we need to be updated.
    fn set_events(self: Pin<&Self>, interest: Event, mode: PollMode) -> bool {
        match self.data().project_ref() {
            PacketInnerProj::Socket { socket, .. } => {
                let mut socket = lock!(socket.lock());
                socket.interest = interest;
                socket.mode = mode;
                socket.interest_error = true;

                // If there was a change, indicate that we need an update.
                match socket.status {
                    SocketStatus::Polling { flags } => {
                        let our_flags = event_to_afd_mask(socket.interest, socket.interest_error);
                        our_flags != flags
                    }
                    _ => true,
                }
            }
            PacketInnerProj::Waitable { handle } => {
                let mut handle = lock!(handle.lock());

                // Set the new interest.
                handle.interest = interest;
                handle.mode = mode;

                // Update if there is no ongoing wait.
                handle.status.is_idle()
            }
            _ => true,
        }
    }

    /// Update the socket and install the new status in AFD.
    ///
    /// This function does one of the following:
    ///
    /// - Nothing, if the packet is waiting on being dropped anyways.
    /// - Cancels the ongoing poll, if we want to poll for different events than we are currently
    ///   polling for.
    /// - Starts a new AFD_POLL operation, if we are not currently polling.
    fn update(self: Pin<Arc<Self>>) -> io::Result<()> {
        let mut socket = match self.as_ref().data().project_ref() {
            PacketInnerProj::Socket { socket, .. } => lock!(socket.lock()),
            PacketInnerProj::Waitable { handle } => {
                let mut handle = lock!(handle.lock());

                // If there is no interests, or if we have been cancelled, we don't need to update.
                if !handle.interest.readable && !handle.interest.writable {
                    return Ok(());
                }

                // If we are idle, we need to update.
                if !handle.status.is_idle() {
                    return Ok(());
                }

                // Start a new wait.
                let packet = self.clone();
                let wait_handle = WaitHandle::new(
                    handle.handle,
                    move || {
                        let mut handle = match packet.as_ref().data().project_ref() {
                            PacketInnerProj::Waitable { handle } => lock!(handle.lock()),
                            _ => unreachable!(),
                        };

                        // Try to get the IOCP.
                        let iocp = match handle.port.upgrade() {
                            Some(iocp) => iocp,
                            None => return,
                        };

                        // Set us back into the idle state.
                        handle.status = WaitableStatus::Idle;

                        // Push this packet.
                        drop(handle);
                        if let Err(_e) = iocp.post(0, 0, packet) {
                            #[cfg(feature = "tracing")]
                            tracing::error!("failed to post completion packet: {}", _e);
                        }
                    },
                    None,
                    false,
                )?;

                // Set the new status.
                handle.status = WaitableStatus::Waiting(wait_handle);

                return Ok(());
            }
            _ => return Err(io::Error::new(io::ErrorKind::Other, "invalid socket state")),
        };

        // If we are waiting on a delete, just return, dropping the packet.
        if socket.waiting_on_delete {
            return Ok(());
        }

        // Check the current status.
        match socket.status {
            SocketStatus::Polling { flags } => {
                // If we need to poll for events aside from what we are currently polling, we need
                // to update the packet. Cancel the ongoing poll.
                let our_flags = event_to_afd_mask(socket.interest, socket.interest_error);
                if our_flags != flags {
                    return self.cancel(socket);
                }

                // All events that we are currently waiting on are accounted for.
                Ok(())
            }

            SocketStatus::Cancelled => {
                // The ongoing operation was cancelled, and we're still waiting for it to return.
                // For now, wait until the top-level loop calls feed_event().
                Ok(())
            }

            SocketStatus::Idle => {
                // Start a new poll.
                let mask = event_to_afd_mask(socket.interest, socket.interest_error);
                let result = socket.afd.poll(self.clone(), socket.base_socket, mask);

                match result {
                    Ok(()) => {}

                    Err(err)
                        if err.raw_os_error() == Some(ERROR_IO_PENDING as i32)
                            || err.kind() == io::ErrorKind::WouldBlock =>
                    {
                        // The operation is pending.
                    }

                    Err(err) if err.raw_os_error() == Some(ERROR_INVALID_HANDLE as i32) => {
                        // The socket was closed. We need to delete it.
                        // This should happen after we drop it here.
                    }

                    Err(err) => return Err(err),
                }

                // We are now polling for the current events.
                socket.status = SocketStatus::Polling { flags: mask };

                Ok(())
            }
        }
    }

    /// This socket state was notified; see if we need to update it.
    ///
    /// This indicates that this packet was indicated as "ready" by the IOCP and needs to be
    /// processed.
    fn feed_event(self: Pin<Arc<Self>>, poller: &Poller) -> io::Result<FeedEventResult> {
        let inner = self.as_ref().data().project_ref();

        let (afd_info, socket) = match inner {
            PacketInnerProj::Socket { packet, socket } => (packet, socket),
            PacketInnerProj::Custom { event } => {
                // This is a custom event.
                return Ok(FeedEventResult::Event(*event));
            }
            PacketInnerProj::Wakeup { .. } => {
                // The poller was notified.
                return Ok(FeedEventResult::Notified);
            }
            PacketInnerProj::Waitable { handle } => {
                let mut handle = lock!(handle.lock());
                let event = handle.interest;

                // Clear the events if we are in one-shot mode.
                if matches!(handle.mode, PollMode::Oneshot) {
                    handle.interest = Event::none(handle.interest.key);
                }

                // Submit for an update.
                drop(handle);
                poller.update_packet(self)?;

                return Ok(FeedEventResult::Event(event));
            }
        };

        let mut socket_state = lock!(socket.lock());
        let mut event = Event::none(socket_state.interest.key);

        // Put ourselves into the idle state.
        socket_state.status = SocketStatus::Idle;

        // If we are waiting to be deleted, just return and let the drop handler do their thing.
        if socket_state.waiting_on_delete {
            return Ok(FeedEventResult::NoEvent);
        }

        unsafe {
            // SAFETY: The packet is not in transit.
            let iosb = &mut *self.as_ref().iosb().get();

            // Check the status.
            match iosb.Anonymous.Status {
                STATUS_CANCELLED => {
                    // Poll request was cancelled.
                }

                status if status < 0 => {
                    // There was an error, so we signal both ends.
                    event.readable = true;
                    event.writable = true;
                }

                _ => {
                    // Check in on the AFD data.
                    let afd_data = &*afd_info.get();

                    // There was at least one event.
                    if afd_data.handle_count() >= 1 {
                        let events = afd_data.events();

                        // If we closed the socket, remove it from being polled.
                        if events.intersects(AfdPollMask::LOCAL_CLOSE) {
                            let source = lock!(poller.sources.write())
                                .remove(&socket_state.socket)
                                .unwrap();
                            return source.begin_delete().map(|()| FeedEventResult::NoEvent);
                        }

                        // Report socket-related events.
                        let (readable, writable) = afd_mask_to_event(events);
                        event.readable = readable;
                        event.writable = writable;
                        event.extra.flags = events;
                    }
                }
            }
        }

        // Filter out events that the user didn't ask for.
        event.readable &= socket_state.interest.readable;
        event.writable &= socket_state.interest.writable;

        // If this event doesn't have anything that interests us, don't return or
        // update the oneshot state.
        let return_value = if event.readable
            || event.writable
            || event
                .extra
                .flags
                .intersects(socket_state.interest.extra.flags)
        {
            // If we are in oneshot mode, remove the interest.
            if matches!(socket_state.mode, PollMode::Oneshot) {
                socket_state.interest = Event::none(socket_state.interest.key);
                socket_state.interest_error = false;
            }

            FeedEventResult::Event(event)
        } else {
            FeedEventResult::NoEvent
        };

        // Put ourselves in the update queue.
        drop(socket_state);
        poller.update_packet(self)?;

        // Return the event.
        Ok(return_value)
    }

    /// Begin deleting this socket.
    fn begin_delete(self: Pin<Arc<Self>>) -> io::Result<()> {
        // If we aren't already being deleted, start deleting.
        let mut socket = match self.as_ref().data().project_ref() {
            PacketInnerProj::Socket { socket, .. } => lock!(socket.lock()),
            PacketInnerProj::Waitable { handle } => {
                let mut handle = lock!(handle.lock());

                // Set the status to be cancelled. This drops the wait handle and prevents
                // any further updates.
                handle.status = WaitableStatus::Cancelled;

                return Ok(());
            }
            _ => panic!("can't delete packet that doesn't belong to a socket"),
        };
        if !socket.waiting_on_delete {
            socket.waiting_on_delete = true;

            if matches!(socket.status, SocketStatus::Polling { .. }) {
                // Cancel the ongoing poll.
                self.cancel(socket)?;
            }
        }

        // Either drop it now or wait for it to be dropped later.
        Ok(())
    }

    fn cancel(self: &Pin<Arc<Self>>, mut socket: MutexGuard<'_, SocketState>) -> io::Result<()> {
        assert!(matches!(socket.status, SocketStatus::Polling { .. }));

        // Send the cancel request.
        unsafe {
            socket.afd.cancel(self)?;
        }

        // Move state to cancelled.
        socket.status = SocketStatus::Cancelled;

        Ok(())
    }
}

/// Per-socket state.
#[derive(Debug)]
struct SocketState {
    /// The raw socket handle.
    socket: RawSocket,

    /// The base socket handle.
    base_socket: RawSocket,

    /// The event that this socket is currently waiting on.
    interest: Event,

    /// Whether to listen for error events.
    interest_error: bool,

    /// The current poll mode.
    mode: PollMode,

    /// The AFD instance that this socket is registered with.
    afd: Arc<Afd<Packet>>,

    /// Whether this socket is waiting to be deleted.
    waiting_on_delete: bool,

    /// The current status of the socket.
    status: SocketStatus,
}

/// The mode that a socket can be in.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum SocketStatus {
    /// We are currently not polling.
    Idle,

    /// We are currently polling these events.
    Polling {
        /// The flags we are currently polling for.
        flags: AfdPollMask,
    },

    /// The last poll operation was cancelled, and we're waiting for it to
    /// complete.
    Cancelled,
}

/// Per-waitable handle state.
#[derive(Debug)]
struct WaitableState {
    /// The handle that this state is for.
    handle: RawHandle,

    /// The IO completion port that this handle is registered with.
    port: Weak<IoCompletionPort<Packet>>,

    /// The event that this handle will report.
    interest: Event,

    /// The current poll mode.
    mode: PollMode,

    /// The status of this waitable.
    status: WaitableStatus,
}

#[derive(Debug)]
enum WaitableStatus {
    /// We are not polling.
    Idle,

    /// We are waiting on this handle to become signaled.
    Waiting(#[allow(dead_code)] WaitHandle),

    /// This handle has been cancelled.
    Cancelled,
}

impl WaitableStatus {
    fn is_idle(&self) -> bool {
        matches!(self, WaitableStatus::Idle)
    }
}

/// The result of calling `feed_event`.
#[derive(Debug)]
enum FeedEventResult {
    /// No event was yielded.
    NoEvent,

    /// An event was yielded.
    Event(Event),

    /// The poller has been notified.
    Notified,
}

/// A handle for an ongoing wait operation.
#[derive(Debug)]
struct WaitHandle(RawHandle);

impl Drop for WaitHandle {
    fn drop(&mut self) {
        unsafe {
            UnregisterWait(self.0 as _);
        }
    }
}

impl WaitHandle {
    /// Wait for a waitable handle to become signaled.
    fn new<F>(
        handle: RawHandle,
        callback: F,
        timeout: Option<Duration>,
        long_wait: bool,
    ) -> io::Result<Self>
    where
        F: FnOnce() + Send + Sync + 'static,
    {
        // Make sure a panic in the callback doesn't propagate to the OS.
        struct AbortOnDrop;

        impl Drop for AbortOnDrop {
            fn drop(&mut self) {
                std::process::abort();
            }
        }

        unsafe extern "system" fn wait_callback<F: FnOnce() + Send + Sync + 'static>(
            context: *mut c_void,
            _timer_fired: bool,
        ) {
            let _guard = AbortOnDrop;
            let callback = Box::from_raw(context as *mut F);
            callback();

            // We executed without panicking, so don't abort.
            forget(_guard);
        }

        let mut wait_handle = MaybeUninit::<RawHandle>::uninit();

        let mut flags = WT_EXECUTEONLYONCE;
        if long_wait {
            flags |= WT_EXECUTELONGFUNCTION;
        }

        let res = unsafe {
            RegisterWaitForSingleObject(
                wait_handle.as_mut_ptr().cast::<_>(),
                handle as _,
                Some(wait_callback::<F>),
                Box::into_raw(Box::new(callback)) as _,
                timeout.map_or(INFINITE, dur2timeout),
                flags,
            )
        };

        if res == 0 {
            return Err(io::Error::last_os_error());
        }

        let wait_handle = unsafe { wait_handle.assume_init() };
        Ok(Self(wait_handle))
    }
}

/// Translate an event to the mask expected by AFD.
#[inline]
fn event_to_afd_mask(event: Event, error: bool) -> afd::AfdPollMask {
    event_properties_to_afd_mask(event.readable, event.writable, error) | event.extra.flags
}

/// Translate an event to the mask expected by AFD.
#[inline]
fn event_properties_to_afd_mask(readable: bool, writable: bool, error: bool) -> afd::AfdPollMask {
    use afd::AfdPollMask as AfdPoll;

    let mut mask = AfdPoll::empty();

    if error || readable || writable {
        mask |= AfdPoll::ABORT | AfdPoll::CONNECT_FAIL;
    }

    if readable {
        mask |=
            AfdPoll::RECEIVE | AfdPoll::ACCEPT | AfdPoll::DISCONNECT | AfdPoll::RECEIVE_EXPEDITED;
    }

    if writable {
        mask |= AfdPoll::SEND;
    }

    mask
}

/// Convert the mask reported by AFD to an event.
#[inline]
fn afd_mask_to_event(mask: afd::AfdPollMask) -> (bool, bool) {
    use afd::AfdPollMask as AfdPoll;

    let mut readable = false;
    let mut writable = false;

    if mask.intersects(
        AfdPoll::RECEIVE | AfdPoll::ACCEPT | AfdPoll::DISCONNECT | AfdPoll::RECEIVE_EXPEDITED,
    ) {
        readable = true;
    }

    if mask.intersects(AfdPoll::SEND) {
        writable = true;
    }

    if mask.intersects(AfdPoll::ABORT | AfdPoll::CONNECT_FAIL) {
        readable = true;
        writable = true;
    }

    (readable, writable)
}

// Implementation taken from https://github.com/rust-lang/rust/blob/db5476571d9b27c862b95c1e64764b0ac8980e23/src/libstd/sys/windows/mod.rs
fn dur2timeout(dur: Duration) -> u32 {
    // Note that a duration is a (u64, u32) (seconds, nanoseconds) pair, and the
    // timeouts in windows APIs are typically u32 milliseconds. To translate, we
    // have two pieces to take care of:
    //
    // * Nanosecond precision is rounded up
    // * Greater than u32::MAX milliseconds (50 days) is rounded up to INFINITE
    //   (never time out).
    dur.as_secs()
        .checked_mul(1000)
        .and_then(|ms| ms.checked_add((dur.subsec_nanos() as u64) / 1_000_000))
        .and_then(|ms| {
            if dur.subsec_nanos() % 1_000_000 > 0 {
                ms.checked_add(1)
            } else {
                Some(ms)
            }
        })
        .and_then(|x| u32::try_from(x).ok())
        .unwrap_or(INFINITE)
}

/// An error type that wraps around failing to open AFD.
struct AfdError {
    /// String description of what happened.
    description: &'static str,

    /// The underlying system error.
    system: io::Error,
}

impl AfdError {
    #[inline]
    fn new(description: &'static str, system: io::Error) -> Self {
        Self {
            description,
            system,
        }
    }
}

impl fmt::Debug for AfdError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AfdError")
            .field("description", &self.description)
            .field("system", &self.system)
            .field("note", &"probably caused by old Windows or Wine")
            .finish()
    }
}

impl fmt::Display for AfdError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {}\nThis error is usually caused by running on old Windows or Wine",
            self.description, &self.system
        )
    }
}

impl std::error::Error for AfdError {
    #[inline]
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.system)
    }
}

struct CallOnDrop<F: FnMut()>(F);

impl<F: FnMut()> Drop for CallOnDrop<F> {
    fn drop(&mut self) {
        (self.0)();
    }
}
