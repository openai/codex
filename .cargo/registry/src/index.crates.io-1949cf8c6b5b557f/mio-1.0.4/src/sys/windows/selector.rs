use super::afd::{self, Afd, AfdPollInfo};
use super::io_status_block::IoStatusBlock;
use super::Event;
use crate::sys::Events;

cfg_net! {
    use crate::sys::event::{
        ERROR_FLAGS, READABLE_FLAGS, READ_CLOSED_FLAGS, WRITABLE_FLAGS, WRITE_CLOSED_FLAGS,
    };
    use crate::Interest;
}

use super::iocp::{CompletionPort, CompletionStatus};
use std::collections::VecDeque;
use std::ffi::c_void;
use std::io;
use std::marker::PhantomPinned;
use std::os::windows::io::RawSocket;
use std::pin::Pin;
#[cfg(debug_assertions)]
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use windows_sys::Win32::Foundation::{
    ERROR_INVALID_HANDLE, ERROR_IO_PENDING, HANDLE, STATUS_CANCELLED, WAIT_TIMEOUT,
};
use windows_sys::Win32::System::IO::OVERLAPPED;

#[derive(Debug)]
struct AfdGroup {
    #[cfg_attr(not(feature = "net"), allow(dead_code))]
    cp: Arc<CompletionPort>,
    afd_group: Mutex<Vec<Arc<Afd>>>,
}

impl AfdGroup {
    pub fn new(cp: Arc<CompletionPort>) -> AfdGroup {
        AfdGroup {
            afd_group: Mutex::new(Vec::new()),
            cp,
        }
    }

    pub fn release_unused_afd(&self) {
        let mut afd_group = self.afd_group.lock().unwrap();
        afd_group.retain(|g| Arc::strong_count(g) > 1);
    }
}

cfg_io_source! {
    const POLL_GROUP_MAX_GROUP_SIZE: usize = 32;

    impl AfdGroup {
        pub fn acquire(&self) -> io::Result<Arc<Afd>> {
            let mut afd_group = self.afd_group.lock().unwrap();
            if afd_group.len() == 0 {
                self._alloc_afd_group(&mut afd_group)?;
            } else {
                // + 1 reference in Vec
                if Arc::strong_count(afd_group.last().unwrap()) > POLL_GROUP_MAX_GROUP_SIZE  {
                    self._alloc_afd_group(&mut afd_group)?;
                }
            }

            match afd_group.last() {
                Some(arc) => Ok(arc.clone()),
                None => unreachable!(
                    "Cannot acquire afd, {:#?}, afd_group: {:#?}",
                    self, afd_group
                ),
            }
        }

        fn _alloc_afd_group(&self, afd_group: &mut Vec<Arc<Afd>>) -> io::Result<()> {
            let afd = Afd::new(&self.cp)?;
            let arc = Arc::new(afd);
            afd_group.push(arc);
            Ok(())
        }
    }
}

#[derive(Debug)]
enum SockPollStatus {
    Idle,
    Pending,
    Cancelled,
}

#[derive(Debug)]
pub struct SockState {
    iosb: IoStatusBlock,
    poll_info: AfdPollInfo,
    afd: Arc<Afd>,

    base_socket: RawSocket,

    user_evts: u32,
    pending_evts: u32,

    user_data: u64,

    poll_status: SockPollStatus,
    delete_pending: bool,

    // last raw os error
    error: Option<i32>,

    _pinned: PhantomPinned,
}

impl SockState {
    fn update(&mut self, self_arc: &Pin<Arc<Mutex<SockState>>>) -> io::Result<()> {
        assert!(!self.delete_pending);

        // make sure to reset previous error before a new update
        self.error = None;

        if let SockPollStatus::Pending = self.poll_status {
            if (self.user_evts & afd::KNOWN_EVENTS & !self.pending_evts) == 0 {
                /* All the events the user is interested in are already being monitored by
                 * the pending poll operation. It might spuriously complete because of an
                 * event that we're no longer interested in; when that happens we'll submit
                 * a new poll operation with the updated event mask. */
            } else {
                /* A poll operation is already pending, but it's not monitoring for all the
                 * events that the user is interested in. Therefore, cancel the pending
                 * poll operation; when we receive it's completion package, a new poll
                 * operation will be submitted with the correct event mask. */
                if let Err(e) = self.cancel() {
                    self.error = e.raw_os_error();
                    return Err(e);
                }
                return Ok(());
            }
        } else if let SockPollStatus::Cancelled = self.poll_status {
            /* The poll operation has already been cancelled, we're still waiting for
             * it to return. For now, there's nothing that needs to be done. */
        } else if let SockPollStatus::Idle = self.poll_status {
            /* No poll operation is pending; start one. */
            self.poll_info.exclusive = 0;
            self.poll_info.number_of_handles = 1;
            self.poll_info.timeout = i64::MAX;
            self.poll_info.handles[0].handle = self.base_socket as HANDLE;
            self.poll_info.handles[0].status = 0;
            self.poll_info.handles[0].events = self.user_evts | afd::POLL_LOCAL_CLOSE;

            // Increase the ref count as the memory will be used by the kernel.
            let overlapped_ptr = into_overlapped(self_arc.clone());

            let result = unsafe {
                self.afd
                    .poll(&mut self.poll_info, &mut *self.iosb, overlapped_ptr)
            };
            if let Err(e) = result {
                let code = e.raw_os_error().unwrap();
                if code == ERROR_IO_PENDING as i32 {
                    /* Overlapped poll operation in progress; this is expected. */
                } else {
                    // Since the operation failed it means the kernel won't be
                    // using the memory any more.
                    drop(from_overlapped(overlapped_ptr as *mut _));
                    if code == ERROR_INVALID_HANDLE as i32 {
                        /* Socket closed; it'll be dropped. */
                        self.mark_delete();
                        return Ok(());
                    } else {
                        self.error = e.raw_os_error();
                        return Err(e);
                    }
                }
            }

            self.poll_status = SockPollStatus::Pending;
            self.pending_evts = self.user_evts;
        } else {
            unreachable!("Invalid poll status during update, {:#?}", self)
        }

        Ok(())
    }

    fn cancel(&mut self) -> io::Result<()> {
        match self.poll_status {
            SockPollStatus::Pending => {}
            _ => unreachable!("Invalid poll status during cancel, {:#?}", self),
        };
        unsafe {
            self.afd.cancel(&mut *self.iosb)?;
        }
        self.poll_status = SockPollStatus::Cancelled;
        self.pending_evts = 0;
        Ok(())
    }

    // This is the function called from the overlapped using as Arc<Mutex<SockState>>. Watch out for reference counting.
    fn feed_event(&mut self) -> Option<Event> {
        self.poll_status = SockPollStatus::Idle;
        self.pending_evts = 0;

        let mut afd_events = 0;
        // We use the status info in IO_STATUS_BLOCK to determine the socket poll status. It is unsafe to use a pointer of IO_STATUS_BLOCK.
        unsafe {
            if self.delete_pending {
                return None;
            } else if self.iosb.Anonymous.Status == STATUS_CANCELLED {
                /* The poll request was cancelled by CancelIoEx. */
            } else if self.iosb.Anonymous.Status < 0 {
                /* The overlapped request itself failed in an unexpected way. */
                afd_events = afd::POLL_CONNECT_FAIL;
            } else if self.poll_info.number_of_handles < 1 {
                /* This poll operation succeeded but didn't report any socket events. */
            } else if self.poll_info.handles[0].events & afd::POLL_LOCAL_CLOSE != 0 {
                /* The poll operation reported that the socket was closed. */
                self.mark_delete();
                return None;
            } else {
                afd_events = self.poll_info.handles[0].events;
            }
        }

        afd_events &= self.user_evts;

        if afd_events == 0 {
            return None;
        }

        // In mio, we have to simulate Edge-triggered behavior to match API usage.
        // The strategy here is to intercept all read/write from user that could cause WouldBlock usage,
        // then reregister the socket to reset the interests.
        self.user_evts &= !afd_events;

        Some(Event {
            data: self.user_data,
            flags: afd_events,
        })
    }

    pub fn is_pending_deletion(&self) -> bool {
        self.delete_pending
    }

    pub fn mark_delete(&mut self) {
        if !self.delete_pending {
            if let SockPollStatus::Pending = self.poll_status {
                drop(self.cancel());
            }

            self.delete_pending = true;
        }
    }

    fn has_error(&self) -> bool {
        self.error.is_some()
    }
}

cfg_io_source! {
    impl SockState {
        fn new(raw_socket: RawSocket, afd: Arc<Afd>) -> io::Result<SockState> {
            Ok(SockState {
                iosb: IoStatusBlock::zeroed(),
                poll_info: AfdPollInfo::zeroed(),
                afd,
                base_socket: get_base_socket(raw_socket)?,
                user_evts: 0,
                pending_evts: 0,
                user_data: 0,
                poll_status: SockPollStatus::Idle,
                delete_pending: false,
                error: None,
                _pinned: PhantomPinned,
            })
        }

        /// True if need to be added on update queue, false otherwise.
        fn set_event(&mut self, ev: Event) -> bool {
            /* afd::POLL_CONNECT_FAIL and afd::POLL_ABORT are always reported, even when not requested by the caller. */
            let events = ev.flags | afd::POLL_CONNECT_FAIL | afd::POLL_ABORT;

            self.user_evts = events;
            self.user_data = ev.data;

            (events & !self.pending_evts) != 0
        }
    }
}

impl Drop for SockState {
    fn drop(&mut self) {
        self.mark_delete();
    }
}

/// Converts the pointer to a `SockState` into a raw pointer.
/// To revert see `from_overlapped`.
fn into_overlapped(sock_state: Pin<Arc<Mutex<SockState>>>) -> *mut c_void {
    let overlapped_ptr: *const Mutex<SockState> =
        unsafe { Arc::into_raw(Pin::into_inner_unchecked(sock_state)) };
    overlapped_ptr as *mut _
}

/// Convert a raw overlapped pointer into a reference to `SockState`.
/// Reverts `into_overlapped`.
fn from_overlapped(ptr: *mut OVERLAPPED) -> Pin<Arc<Mutex<SockState>>> {
    let sock_ptr: *const Mutex<SockState> = ptr as *const _;
    unsafe { Pin::new_unchecked(Arc::from_raw(sock_ptr)) }
}

/// Each Selector has a globally unique(ish) ID associated with it. This ID
/// gets tracked by `TcpStream`, `TcpListener`, etc... when they are first
/// registered with the `Selector`. If a type that is previously associated with
/// a `Selector` attempts to register itself with a different `Selector`, the
/// operation will return with an error. This matches windows behavior.
#[cfg(debug_assertions)]
static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

/// Windows implementation of `sys::Selector`
///
/// Edge-triggered event notification is simulated by resetting internal event flag of each socket state `SockState`
/// and setting all events back by intercepting all requests that could cause `io::ErrorKind::WouldBlock` happening.
///
/// This selector is currently only support socket due to `Afd` driver is winsock2 specific.
#[derive(Debug)]
pub struct Selector {
    #[cfg(debug_assertions)]
    id: usize,
    pub(super) inner: Arc<SelectorInner>,
}

impl Selector {
    pub fn new() -> io::Result<Selector> {
        SelectorInner::new().map(|inner| {
            #[cfg(debug_assertions)]
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed) + 1;
            Selector {
                #[cfg(debug_assertions)]
                id,
                inner: Arc::new(inner),
            }
        })
    }

    pub fn try_clone(&self) -> io::Result<Selector> {
        Ok(Selector {
            #[cfg(debug_assertions)]
            id: self.id,
            inner: Arc::clone(&self.inner),
        })
    }

    /// # Safety
    ///
    /// This requires a mutable reference to self because only a single thread
    /// can poll IOCP at a time.
    pub fn select(&mut self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()> {
        self.inner.select(events, timeout)
    }

    pub(super) fn clone_port(&self) -> Arc<CompletionPort> {
        self.inner.cp.clone()
    }

    #[cfg(feature = "os-ext")]
    pub(super) fn same_port(&self, other: &Arc<CompletionPort>) -> bool {
        Arc::ptr_eq(&self.inner.cp, other)
    }
}

cfg_io_source! {
    use super::InternalState;
    use crate::Token;

    impl Selector {
        pub(super) fn register(
            &self,
            socket: RawSocket,
            token: Token,
            interests: Interest,
        ) -> io::Result<InternalState> {
            SelectorInner::register(&self.inner, socket, token, interests)
        }

        pub(super) fn reregister(
            &self,
            state: Pin<Arc<Mutex<SockState>>>,
            token: Token,
            interests: Interest,
        ) -> io::Result<()> {
            self.inner.reregister(state, token, interests)
        }

        #[cfg(debug_assertions)]
        pub fn id(&self) -> usize {
            self.id
        }
    }
}

#[derive(Debug)]
pub struct SelectorInner {
    pub(super) cp: Arc<CompletionPort>,
    update_queue: Mutex<VecDeque<Pin<Arc<Mutex<SockState>>>>>,
    afd_group: AfdGroup,
    is_polling: AtomicBool,
}

// We have ensured thread safety by introducing lock manually.
unsafe impl Sync for SelectorInner {}

impl SelectorInner {
    pub fn new() -> io::Result<SelectorInner> {
        CompletionPort::new(0).map(|cp| {
            let cp = Arc::new(cp);
            let cp_afd = Arc::clone(&cp);

            SelectorInner {
                cp,
                update_queue: Mutex::new(VecDeque::new()),
                afd_group: AfdGroup::new(cp_afd),
                is_polling: AtomicBool::new(false),
            }
        })
    }

    /// # Safety
    ///
    /// May only be calling via `Selector::select`.
    pub fn select(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()> {
        events.clear();

        if timeout.is_none() {
            loop {
                let len = self.select2(&mut events.statuses, &mut events.events, None)?;
                if len == 0 {
                    continue;
                }
                break Ok(());
            }
        } else {
            self.select2(&mut events.statuses, &mut events.events, timeout)?;
            Ok(())
        }
    }

    pub fn select2(
        &self,
        statuses: &mut [CompletionStatus],
        events: &mut Vec<Event>,
        timeout: Option<Duration>,
    ) -> io::Result<usize> {
        assert!(!self.is_polling.swap(true, Ordering::AcqRel));

        unsafe { self.update_sockets_events() }?;

        let result = self.cp.get_many(statuses, timeout);

        self.is_polling.store(false, Ordering::Relaxed);

        match result {
            Ok(iocp_events) => Ok(unsafe { self.feed_events(events, iocp_events) }),
            Err(ref e) if e.raw_os_error() == Some(WAIT_TIMEOUT as i32) => Ok(0),
            Err(e) => Err(e),
        }
    }

    unsafe fn update_sockets_events(&self) -> io::Result<()> {
        let mut update_queue = self.update_queue.lock().unwrap();
        for sock in update_queue.iter_mut() {
            let mut sock_internal = sock.lock().unwrap();
            if !sock_internal.is_pending_deletion() {
                sock_internal.update(sock)?;
            }
        }

        // remove all sock which do not have error, they have afd op pending
        update_queue.retain(|sock| sock.lock().unwrap().has_error());

        self.afd_group.release_unused_afd();
        Ok(())
    }

    // It returns processed count of iocp_events rather than the events itself.
    unsafe fn feed_events(
        &self,
        events: &mut Vec<Event>,
        iocp_events: &[CompletionStatus],
    ) -> usize {
        let mut n = 0;
        let mut update_queue = self.update_queue.lock().unwrap();
        for iocp_event in iocp_events.iter() {
            if iocp_event.overlapped().is_null() {
                events.push(Event::from_completion_status(iocp_event));
                n += 1;
                continue;
            } else if iocp_event.token() % 2 == 1 {
                // Handle is a named pipe. This could be extended to be any non-AFD event.
                let callback = (*(iocp_event.overlapped() as *mut super::Overlapped)).callback;

                let len = events.len();
                callback(iocp_event.entry(), Some(events));
                n += events.len() - len;
                continue;
            }

            let sock_state = from_overlapped(iocp_event.overlapped());
            let mut sock_guard = sock_state.lock().unwrap();
            if let Some(e) = sock_guard.feed_event() {
                events.push(e);
                n += 1;
            }

            if !sock_guard.is_pending_deletion() {
                update_queue.push_back(sock_state.clone());
            }
        }
        self.afd_group.release_unused_afd();
        n
    }
}

cfg_io_source! {
    use std::mem::size_of;
    use std::ptr::null_mut;

    use windows_sys::Win32::Networking::WinSock::{
        WSAGetLastError, WSAIoctl, SIO_BASE_HANDLE, SIO_BSP_HANDLE,
        SIO_BSP_HANDLE_POLL, SIO_BSP_HANDLE_SELECT, SOCKET_ERROR,
    };


    impl SelectorInner {
        fn register(
            this: &Arc<Self>,
            socket: RawSocket,
            token: Token,
            interests: Interest,
        ) -> io::Result<InternalState> {
            let flags = interests_to_afd_flags(interests);

            let sock = {
                let sock = this._alloc_sock_for_rawsocket(socket)?;
                let event = Event {
                    flags,
                    data: token.0 as u64,
                };
                sock.lock().unwrap().set_event(event);
                sock
            };

            let state = InternalState {
                selector: this.clone(),
                token,
                interests,
                sock_state: sock.clone(),
            };

            this.queue_state(sock);
            unsafe { this.update_sockets_events_if_polling()? };

            Ok(state)
        }

        // Directly accessed in `IoSourceState::do_io`.
        pub(super) fn reregister(
            &self,
            state: Pin<Arc<Mutex<SockState>>>,
            token: Token,
            interests: Interest,
        ) -> io::Result<()> {
            {
                let event = Event {
                    flags: interests_to_afd_flags(interests),
                    data: token.0 as u64,
                };

                state.lock().unwrap().set_event(event);
            }

            // FIXME: a sock which has_error true should not be re-added to
            // the update queue because it's already there.
            self.queue_state(state);
            unsafe { self.update_sockets_events_if_polling() }
        }

        /// This function is called by register() and reregister() to start an
        /// IOCTL_AFD_POLL operation corresponding to the registered events, but
        /// only if necessary.
        ///
        /// Since it is not possible to modify or synchronously cancel an AFD_POLL
        /// operation, and there can be only one active AFD_POLL operation per
        /// (socket, completion port) pair at any time, it is expensive to change
        /// a socket's event registration after it has been submitted to the kernel.
        ///
        /// Therefore, if no other threads are polling when interest in a socket
        /// event is (re)registered, the socket is added to the 'update queue', but
        /// the actual syscall to start the IOCTL_AFD_POLL operation is deferred
        /// until just before the GetQueuedCompletionStatusEx() syscall is made.
        ///
        /// However, when another thread is already blocked on
        /// GetQueuedCompletionStatusEx() we tell the kernel about the registered
        /// socket event(s) immediately.
        unsafe fn update_sockets_events_if_polling(&self) -> io::Result<()> {
            if self.is_polling.load(Ordering::Acquire) {
                self.update_sockets_events()
            } else {
                Ok(())
            }
        }

        fn queue_state(&self, sock_state: Pin<Arc<Mutex<SockState>>>) {
            let mut update_queue = self.update_queue.lock().unwrap();
            update_queue.push_back(sock_state);
        }

        fn _alloc_sock_for_rawsocket(
            &self,
            raw_socket: RawSocket,
        ) -> io::Result<Pin<Arc<Mutex<SockState>>>> {
            let afd = self.afd_group.acquire()?;
            Ok(Arc::pin(Mutex::new(SockState::new(raw_socket, afd)?)))
        }
    }

    fn try_get_base_socket(raw_socket: RawSocket, ioctl: u32) -> Result<RawSocket, i32> {
        let mut base_socket: RawSocket = 0;
        let mut bytes: u32 = 0;
        unsafe {
            if WSAIoctl(
                raw_socket as usize,
                ioctl,
                null_mut(),
                0,
                &mut base_socket as *mut _ as *mut c_void,
                size_of::<RawSocket>() as u32,
                &mut bytes,
                null_mut(),
                None,
            ) != SOCKET_ERROR
            {
                Ok(base_socket)
            } else {
                Err(WSAGetLastError())
            }
        }
    }

    fn get_base_socket(raw_socket: RawSocket) -> io::Result<RawSocket> {
        let res = try_get_base_socket(raw_socket, SIO_BASE_HANDLE);
        if let Ok(base_socket) = res {
            return Ok(base_socket);
        }

        // The `SIO_BASE_HANDLE` should not be intercepted by LSPs, therefore
        // it should not fail as long as `raw_socket` is a valid socket. See
        // https://docs.microsoft.com/en-us/windows/win32/winsock/winsock-ioctls.
        // However, at least one known LSP deliberately breaks it, so we try
        // some alternative IOCTLs, starting with the most appropriate one.
        for &ioctl in &[
            SIO_BSP_HANDLE_SELECT,
            SIO_BSP_HANDLE_POLL,
            SIO_BSP_HANDLE,
        ] {
            if let Ok(base_socket) = try_get_base_socket(raw_socket, ioctl) {
                // Since we know now that we're dealing with an LSP (otherwise
                // SIO_BASE_HANDLE wouldn't have failed), only return any result
                // when it is different from the original `raw_socket`.
                if base_socket != raw_socket {
                    return Ok(base_socket);
                }
            }
        }

        // If the alternative IOCTLs also failed, return the original error.
        let os_error = res.unwrap_err();
        let err = io::Error::from_raw_os_error(os_error);
        Err(err)
    }
}

impl Drop for SelectorInner {
    fn drop(&mut self) {
        loop {
            let events_num: usize;
            let mut statuses: [CompletionStatus; 1024] = [CompletionStatus::zero(); 1024];

            let result = self
                .cp
                .get_many(&mut statuses, Some(std::time::Duration::from_millis(0)));
            match result {
                Ok(iocp_events) => {
                    events_num = iocp_events.iter().len();
                    for iocp_event in iocp_events.iter() {
                        if iocp_event.overlapped().is_null() {
                            // Custom event
                        } else if iocp_event.token() % 2 == 1 {
                            // Named pipe, dispatch the event so it can release resources
                            let callback = unsafe {
                                (*(iocp_event.overlapped() as *mut super::Overlapped)).callback
                            };

                            callback(iocp_event.entry(), None);
                        } else {
                            // drain sock state to release memory of Arc reference
                            let _sock_state = from_overlapped(iocp_event.overlapped());
                        }
                    }
                }

                Err(_) => {
                    break;
                }
            }

            if events_num == 0 {
                // continue looping until all completion statuses have been drained
                break;
            }
        }

        self.afd_group.release_unused_afd();
    }
}

cfg_net! {
    fn interests_to_afd_flags(interests: Interest) -> u32 {
        let mut flags = 0;

        if interests.is_readable() {
            flags |= READABLE_FLAGS | READ_CLOSED_FLAGS | ERROR_FLAGS;
        }

        if interests.is_writable() {
            flags |= WRITABLE_FLAGS | WRITE_CLOSED_FLAGS | ERROR_FLAGS;
        }

        flags
    }
}
