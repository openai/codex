use std::{
    os::unix::io::{AsRawFd, BorrowedFd, OwnedFd},
    sync::{Arc, Mutex},
};

use super::{
    handle::State, ClientId, Data, GlobalHandler, GlobalId, Handle, InnerClientId, InnerGlobalId,
    InnerHandle, InnerObjectId, ObjectId,
};
use crate::{
    core_interfaces::{WL_DISPLAY_INTERFACE, WL_REGISTRY_INTERFACE},
    protocol::{same_interface, Argument, Message},
    rs::map::Object,
    types::server::InitError,
};

#[cfg(any(target_os = "linux", target_os = "android"))]
use rustix::event::{epoll, Timespec};

#[cfg(any(
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "macos"
))]
use rustix::event::kqueue::*;
use smallvec::SmallVec;

#[derive(Debug)]
pub struct InnerBackend<D: 'static> {
    state: Arc<Mutex<State<D>>>,
}

impl<D> InnerBackend<D> {
    pub fn new() -> Result<Self, InitError> {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let poll_fd = epoll::create(epoll::CreateFlags::CLOEXEC)
            .map_err(Into::into)
            .map_err(InitError::Io)?;

        #[cfg(any(
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd",
            target_os = "macos"
        ))]
        let poll_fd = kqueue().map_err(Into::into).map_err(InitError::Io)?;

        Ok(Self { state: Arc::new(Mutex::new(State::new(poll_fd))) })
    }

    pub fn flush(&self, client: Option<ClientId>) -> std::io::Result<()> {
        self.state.lock().unwrap().flush(client)
    }

    pub fn handle(&self) -> Handle {
        Handle { handle: InnerHandle { state: self.state.clone() as Arc<_> } }
    }

    pub fn poll_fd(&self) -> BorrowedFd<'_> {
        let raw_fd = self.state.lock().unwrap().poll_fd.as_raw_fd();
        // This allows the lifetime of the BorrowedFd to be tied to &self rather than the lock guard,
        // which is the real safety concern
        unsafe { BorrowedFd::borrow_raw(raw_fd) }
    }

    pub fn dispatch_client(
        &self,
        data: &mut D,
        client_id: InnerClientId,
    ) -> std::io::Result<usize> {
        let ret = self.dispatch_events_for(data, client_id);
        let cleanup = self.state.lock().unwrap().cleanup();
        cleanup(&self.handle(), data);
        ret
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn dispatch_all_clients(&self, data: &mut D) -> std::io::Result<usize> {
        use std::os::unix::io::AsFd;

        let poll_fd = self.poll_fd();
        let mut dispatched = 0;
        let mut events = Vec::<epoll::Event>::with_capacity(32);
        loop {
            let buffer = rustix::buffer::spare_capacity(&mut events);
            epoll::wait(poll_fd.as_fd(), buffer, Some(&Timespec::default()))?;

            if events.is_empty() {
                break;
            }

            for event in events.drain(..) {
                let id = InnerClientId::from_u64(event.data.u64());
                // remove the cb while we call it, to gracefully handle reentrancy
                if let Ok(count) = self.dispatch_events_for(data, id) {
                    dispatched += count;
                }
            }
            let cleanup = self.state.lock().unwrap().cleanup();
            cleanup(&self.handle(), data);
        }

        Ok(dispatched)
    }

    #[cfg(any(
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "macos"
    ))]
    pub fn dispatch_all_clients(&self, data: &mut D) -> std::io::Result<usize> {
        use std::time::Duration;

        let poll_fd = self.poll_fd();
        let mut dispatched = 0;
        let mut events = Vec::<Event>::with_capacity(32);
        loop {
            let buffer = rustix::buffer::spare_capacity(&mut events);
            let nevents = unsafe { kevent(&poll_fd, &[], buffer, Some(Duration::ZERO))? };

            if nevents == 0 {
                break;
            }

            for event in events.drain(..) {
                let id = InnerClientId::from_u64(event.udata() as u64);
                // remove the cb while we call it, to gracefully handle reentrancy
                if let Ok(count) = self.dispatch_events_for(data, id) {
                    dispatched += count;
                }
            }
            let cleanup = self.state.lock().unwrap().cleanup();
            cleanup(&self.handle(), data);
        }

        Ok(dispatched)
    }

    pub(crate) fn dispatch_events_for(
        &self,
        data: &mut D,
        client_id: InnerClientId,
    ) -> std::io::Result<usize> {
        let mut dispatched = 0;
        let handle = self.handle();
        let mut state = self.state.lock().unwrap();
        loop {
            let action = {
                let state = &mut *state;
                if let Ok(client) = state.clients.get_client_mut(client_id.clone()) {
                    let (message, object) = match client.next_request() {
                        Ok(v) => v,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            if dispatched > 0 {
                                break;
                            } else {
                                return Err(e);
                            }
                        }
                        Err(e) => {
                            #[cfg(any(target_os = "linux", target_os = "android"))]
                            {
                                epoll::delete(&state.poll_fd, client)?;
                            }

                            #[cfg(any(
                                target_os = "dragonfly",
                                target_os = "freebsd",
                                target_os = "netbsd",
                                target_os = "openbsd",
                                target_os = "macos"
                            ))]
                            {
                                use rustix::event::kqueue::*;
                                use std::os::unix::io::{AsFd, AsRawFd};

                                let evt = Event::new(
                                    EventFilter::Read(client.as_fd().as_raw_fd()),
                                    EventFlags::DELETE,
                                    client_id.as_u64() as *mut _,
                                );

                                let events: &mut [Event] = &mut [];
                                unsafe {
                                    kevent(&state.poll_fd, &[evt], events, None).map(|_| ())?;
                                }
                            }
                            return Err(e);
                        }
                    };
                    dispatched += 1;
                    if same_interface(object.interface, &WL_DISPLAY_INTERFACE) {
                        client.handle_display_request(message, &mut state.registry);
                        continue;
                    } else if same_interface(object.interface, &WL_REGISTRY_INTERFACE) {
                        if let Some((client, global, object, handler)) =
                            client.handle_registry_request(message, &mut state.registry)
                        {
                            DispatchAction::Bind { client, global, object, handler }
                        } else {
                            continue;
                        }
                    } else {
                        let object_id = InnerObjectId {
                            id: message.sender_id,
                            serial: object.data.serial,
                            interface: object.interface,
                            client_id: client.id.clone(),
                        };
                        let opcode = message.opcode;
                        let (arguments, is_destructor, created_id) =
                            match client.process_request(&object, message) {
                                Some(args) => args,
                                None => continue,
                            };
                        // Return the whole set to invoke the callback while handle is not borrower via client
                        DispatchAction::Request {
                            object,
                            object_id,
                            opcode,
                            arguments,
                            is_destructor,
                            created_id,
                        }
                    }
                } else {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Invalid client ID",
                    ));
                }
            };
            match action {
                DispatchAction::Request {
                    object,
                    object_id,
                    opcode,
                    arguments,
                    is_destructor,
                    created_id,
                } => {
                    // temporarily unlock the state Mutex while this request is dispatched
                    std::mem::drop(state);
                    let ret = object.data.user_data.clone().request(
                        &handle.clone(),
                        data,
                        ClientId { id: client_id.clone() },
                        Message {
                            sender_id: ObjectId { id: object_id.clone() },
                            opcode,
                            args: arguments,
                        },
                    );
                    if is_destructor {
                        object.data.user_data.clone().destroyed(
                            &handle.clone(),
                            data,
                            ClientId { id: client_id.clone() },
                            ObjectId { id: object_id.clone() },
                        );
                    }
                    // acquire the lock again and continue
                    state = self.state.lock().unwrap();
                    if is_destructor {
                        if let Ok(client) = state.clients.get_client_mut(client_id.clone()) {
                            client.send_delete_id(object_id);
                        }
                    }
                    match (created_id, ret) {
                        (Some(child_id), Some(child_data)) => {
                            if let Ok(client) = state.clients.get_client_mut(client_id.clone()) {
                                client
                                    .map
                                    .with(child_id.id, |obj| obj.data.user_data = child_data)
                                    .unwrap();
                            }
                        }
                        (None, None) => {}
                        (Some(child_id), None) => {
                            // Allow the callback to not return any data if the client is already dead (typically
                            // if the callback provoked a protocol error)
                            if let Ok(client) = state.clients.get_client(client_id.clone()) {
                                if !client.killed {
                                    panic!(
                                        "Callback creating object {child_id} did not provide any object data."
                                    );
                                }
                            }
                        }
                        (None, Some(_)) => {
                            panic!("An object data was returned from a callback not creating any object");
                        }
                    }
                    // dropping the object calls destructors from which users could call into wayland-backend again.
                    // so lets release and relock the state again, to avoid a deadlock
                    std::mem::drop(state);
                    std::mem::drop(object);
                    state = self.state.lock().unwrap();
                }
                DispatchAction::Bind { object, client, global, handler } => {
                    // temporarily unlock the state Mutex while this request is dispatched
                    std::mem::drop(state);
                    let child_data = handler.bind(
                        &handle.clone(),
                        data,
                        ClientId { id: client.clone() },
                        GlobalId { id: global },
                        ObjectId { id: object.clone() },
                    );
                    // acquire the lock again and continue
                    state = self.state.lock().unwrap();
                    if let Ok(client) = state.clients.get_client_mut(client.clone()) {
                        client.map.with(object.id, |obj| obj.data.user_data = child_data).unwrap();
                    }
                }
            }
        }
        Ok(dispatched)
    }
}

enum DispatchAction<D: 'static> {
    Request {
        object: Object<Data<D>>,
        object_id: InnerObjectId,
        opcode: u16,
        arguments: SmallVec<[Argument<ObjectId, OwnedFd>; 4]>,
        is_destructor: bool,
        created_id: Option<InnerObjectId>,
    },
    Bind {
        object: InnerObjectId,
        client: InnerClientId,
        global: InnerGlobalId,
        handler: Arc<dyn GlobalHandler<D>>,
    },
}
