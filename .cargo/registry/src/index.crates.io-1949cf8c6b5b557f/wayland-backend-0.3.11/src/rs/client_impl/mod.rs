//! Client-side rust implementation of a Wayland protocol backend

use std::{
    fmt,
    os::unix::{
        io::{AsRawFd, BorrowedFd, OwnedFd, RawFd},
        net::UnixStream,
    },
    sync::{Arc, Condvar, Mutex, MutexGuard, Weak},
};

use crate::{
    core_interfaces::WL_DISPLAY_INTERFACE,
    debug,
    protocol::{
        check_for_signature, same_interface, same_interface_or_anonymous, AllowNull, Argument,
        ArgumentType, Interface, Message, ObjectInfo, ProtocolError, ANONYMOUS_INTERFACE,
        INLINE_ARGS,
    },
};
use smallvec::SmallVec;

use super::{
    client::*,
    map::{Object, ObjectMap, SERVER_ID_LIMIT},
    socket::{BufferedSocket, Socket},
    wire::MessageParseError,
};

#[derive(Debug, Clone)]
struct Data {
    client_destroyed: bool,
    server_destroyed: bool,
    user_data: Arc<dyn ObjectData>,
    serial: u32,
}

/// An ID representing a Wayland object
#[derive(Clone)]
pub struct InnerObjectId {
    serial: u32,
    id: u32,
    interface: &'static Interface,
}

impl std::cmp::PartialEq for InnerObjectId {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.serial == other.serial
            && same_interface(self.interface, other.interface)
    }
}

impl std::cmp::Eq for InnerObjectId {}

impl std::hash::Hash for InnerObjectId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.serial.hash(state);
        self.id.hash(state);
    }
}

impl fmt::Display for InnerObjectId {
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.interface.name, self.id)
    }
}

impl fmt::Debug for InnerObjectId {
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ObjectId({}, {})", self, self.serial)
    }
}

impl InnerObjectId {
    pub fn is_null(&self) -> bool {
        self.id == 0
    }

    pub fn interface(&self) -> &'static Interface {
        self.interface
    }

    pub fn protocol_id(&self) -> u32 {
        self.id
    }
}

#[derive(Debug)]
struct ProtocolState {
    socket: BufferedSocket,
    map: ObjectMap<Data>,
    last_error: Option<WaylandError>,
    last_serial: u32,
    debug: bool,
}

#[derive(Debug)]
struct ReadingState {
    prepared_reads: usize,
    read_condvar: Arc<Condvar>,
    read_serial: usize,
}

#[derive(Debug)]
pub struct ConnectionState {
    protocol: Mutex<ProtocolState>,
    read: Mutex<ReadingState>,
}

impl ConnectionState {
    fn lock_protocol(&self) -> MutexGuard<'_, ProtocolState> {
        self.protocol.lock().unwrap()
    }

    fn lock_read(&self) -> MutexGuard<'_, ReadingState> {
        self.read.lock().unwrap()
    }
}

#[derive(Clone, Debug)]
pub struct InnerBackend {
    state: Arc<ConnectionState>,
}

#[derive(Clone, Debug)]
pub struct WeakInnerBackend {
    state: Weak<ConnectionState>,
}

impl WeakInnerBackend {
    pub fn upgrade(&self) -> Option<InnerBackend> {
        Weak::upgrade(&self.state).map(|state| InnerBackend { state })
    }
}

impl PartialEq for InnerBackend {
    fn eq(&self, rhs: &Self) -> bool {
        Arc::ptr_eq(&self.state, &rhs.state)
    }
}

impl Eq for InnerBackend {}

impl InnerBackend {
    pub fn downgrade(&self) -> WeakInnerBackend {
        WeakInnerBackend { state: Arc::downgrade(&self.state) }
    }

    pub fn connect(stream: UnixStream) -> Result<Self, NoWaylandLib> {
        let socket = BufferedSocket::new(Socket::from(stream));
        let mut map = ObjectMap::new();
        map.insert_at(
            1,
            Object {
                interface: &WL_DISPLAY_INTERFACE,
                version: 1,
                data: Data {
                    client_destroyed: false,
                    server_destroyed: false,
                    user_data: Arc::new(DumbObjectData),
                    serial: 0,
                },
            },
        )
        .unwrap();

        let debug = debug::has_debug_client_env();

        Ok(Self {
            state: Arc::new(ConnectionState {
                protocol: Mutex::new(ProtocolState {
                    socket,
                    map,
                    last_error: None,
                    last_serial: 0,
                    debug,
                }),
                read: Mutex::new(ReadingState {
                    prepared_reads: 0,
                    read_condvar: Arc::new(Condvar::new()),
                    read_serial: 0,
                }),
            }),
        })
    }

    /// Flush all pending outgoing requests to the server
    pub fn flush(&self) -> Result<(), WaylandError> {
        let mut guard = self.state.lock_protocol();
        guard.no_last_error()?;
        if let Err(e) = guard.socket.flush() {
            return Err(guard.store_if_not_wouldblock_and_return_error(e));
        }
        Ok(())
    }

    pub fn poll_fd(&self) -> BorrowedFd<'_> {
        let raw_fd = self.state.lock_protocol().socket.as_raw_fd();
        // This allows the lifetime of the BorrowedFd to be tied to &self rather than the lock guard,
        // which is the real safety concern
        unsafe { BorrowedFd::borrow_raw(raw_fd) }
    }
}

#[derive(Debug)]
pub struct InnerReadEventsGuard {
    state: Arc<ConnectionState>,
    done: bool,
}

impl InnerReadEventsGuard {
    /// Create a new reading guard
    ///
    /// This call will not block, but event callbacks may be invoked in the process
    /// of preparing the guard.
    pub fn try_new(backend: InnerBackend) -> Option<Self> {
        backend.state.lock_read().prepared_reads += 1;
        Some(Self { state: backend.state, done: false })
    }

    /// Access the Wayland socket FD for polling
    pub fn connection_fd(&self) -> BorrowedFd<'_> {
        let raw_fd = self.state.lock_protocol().socket.as_raw_fd();
        // This allows the lifetime of the BorrowedFd to be tied to &self rather than the lock guard,
        // which is the real safety concern
        unsafe { BorrowedFd::borrow_raw(raw_fd) }
    }

    /// Attempt to read events from the Wayland socket
    ///
    /// If multiple threads have a live reading guard, this method will block until all of them
    /// are either dropped or have their `read()` method invoked, at which point on of the threads
    /// will read events from the socket and invoke the callbacks for the received events. All
    /// threads will then resume their execution.
    ///
    /// This returns the number of dispatched events, or `0` if an other thread handled the dispatching.
    /// If no events are available to read from the socket, this returns a `WouldBlock` IO error.
    pub fn read(mut self) -> Result<usize, WaylandError> {
        let mut guard = self.state.lock_read();
        guard.prepared_reads -= 1;
        self.done = true;
        if guard.prepared_reads == 0 {
            // We should be the one reading
            let ret = dispatch_events(self.state.clone());
            // wake up other threads
            guard.read_serial = guard.read_serial.wrapping_add(1);
            guard.read_condvar.notify_all();
            // forward the return value
            ret
        } else {
            // We should wait for an other thread to read (or cancel)
            let serial = guard.read_serial;
            let condvar = guard.read_condvar.clone();
            let _guard =
                condvar.wait_while(guard, |backend| serial == backend.read_serial).unwrap();
            self.state.lock_protocol().no_last_error()?;
            Ok(0)
        }
    }
}

impl Drop for InnerReadEventsGuard {
    fn drop(&mut self) {
        if !self.done {
            let mut guard = self.state.lock_read();
            guard.prepared_reads -= 1;
            if guard.prepared_reads == 0 {
                // Cancel the read
                guard.read_serial = guard.read_serial.wrapping_add(1);
                guard.read_condvar.notify_all();
            }
        }
    }
}

impl InnerBackend {
    pub fn display_id(&self) -> ObjectId {
        ObjectId { id: InnerObjectId { serial: 0, id: 1, interface: &WL_DISPLAY_INTERFACE } }
    }

    pub fn last_error(&self) -> Option<WaylandError> {
        self.state.lock_protocol().last_error.clone()
    }

    pub fn info(&self, id: ObjectId) -> Result<ObjectInfo, InvalidId> {
        let object = self.state.lock_protocol().get_object(id.id.clone())?;
        if object.data.client_destroyed {
            Err(InvalidId)
        } else {
            Ok(ObjectInfo { id: id.id.id, interface: object.interface, version: object.version })
        }
    }

    pub fn null_id() -> ObjectId {
        ObjectId { id: InnerObjectId { serial: 0, id: 0, interface: &ANONYMOUS_INTERFACE } }
    }

    pub fn send_request(
        &self,
        Message { sender_id: ObjectId { id }, opcode, args }: Message<ObjectId, RawFd>,
        data: Option<Arc<dyn ObjectData>>,
        child_spec: Option<(&'static Interface, u32)>,
    ) -> Result<ObjectId, InvalidId> {
        let mut guard = self.state.lock_protocol();
        let object = guard.get_object(id.clone())?;

        let message_desc = match object.interface.requests.get(opcode as usize) {
            Some(msg) => msg,
            None => {
                panic!("Unknown opcode {} for object {}@{}.", opcode, object.interface.name, id.id);
            }
        };

        if object.data.client_destroyed {
            if guard.debug {
                debug::print_send_message(id.interface.name, id.id, message_desc.name, &args, true);
            }
            return Err(InvalidId);
        }

        if !check_for_signature(message_desc.signature, &args) {
            panic!(
                "Unexpected signature for request {}@{}.{}: expected {:?}, got {:?}.",
                object.interface.name, id.id, message_desc.name, message_desc.signature, args
            );
        }

        // Prepare the child object
        let child_spec = if message_desc
            .signature
            .iter()
            .any(|arg| matches!(arg, ArgumentType::NewId))
        {
            if let Some((iface, version)) = child_spec {
                if let Some(child_interface) = message_desc.child_interface {
                    if !same_interface(child_interface, iface) {
                        panic!(
                            "Error when sending request {}@{}.{}: expected interface {} but got {}",
                            object.interface.name,
                            id.id,
                            message_desc.name,
                            child_interface.name,
                            iface.name
                        );
                    }
                    if version != object.version {
                        panic!(
                            "Error when sending request {}@{}.{}: expected version {} but got {}",
                            object.interface.name,
                            id.id,
                            message_desc.name,
                            object.version,
                            version
                        );
                    }
                }
                Some((iface, version))
            } else if let Some(child_interface) = message_desc.child_interface {
                Some((child_interface, object.version))
            } else {
                panic!(
                    "Error when sending request {}@{}.{}: target interface must be specified for a generic constructor.",
                    object.interface.name,
                    id.id,
                    message_desc.name
                );
            }
        } else {
            None
        };

        let child = if let Some((child_interface, child_version)) = child_spec {
            let child_serial = guard.next_serial();

            let child = Object {
                interface: child_interface,
                version: child_version,
                data: Data {
                    client_destroyed: false,
                    server_destroyed: false,
                    user_data: Arc::new(DumbObjectData),
                    serial: child_serial,
                },
            };

            let child_id = guard.map.client_insert_new(child);

            guard
                .map
                .with(child_id, |obj| {
                    obj.data.user_data = data.expect(
                        "Sending a request creating an object without providing an object data.",
                    );
                })
                .unwrap();
            Some((child_id, child_serial, child_interface))
        } else {
            None
        };

        // Prepare the message in a debug-compatible way
        let args = args.into_iter().map(|arg| {
            if let Argument::NewId(ObjectId { id: p }) = arg {
                if p.id != 0 {
                    panic!("The newid provided when sending request {}@{}.{} is not a placeholder.", object.interface.name, id.id, message_desc.name);
                }
                if let Some((child_id, child_serial, child_interface)) = child {
                    Argument::NewId(ObjectId { id: InnerObjectId { id: child_id, serial: child_serial, interface: child_interface}})
                } else {
                    unreachable!();
                }
            } else {
                arg
            }
        }).collect::<SmallVec<[_; INLINE_ARGS]>>();

        if guard.debug {
            debug::print_send_message(
                object.interface.name,
                id.id,
                message_desc.name,
                &args,
                false,
            );
        }
        #[cfg(feature = "log")]
        crate::log_debug!("Sending {}.{} ({})", id, message_desc.name, debug::DisplaySlice(&args));

        // Send the message

        let mut msg_args = SmallVec::with_capacity(args.len());
        let mut arg_interfaces = message_desc.arg_interfaces.iter();
        for (i, arg) in args.into_iter().enumerate() {
            msg_args.push(match arg {
                Argument::Array(a) => Argument::Array(a),
                Argument::Int(i) => Argument::Int(i),
                Argument::Uint(u) => Argument::Uint(u),
                Argument::Str(s) => Argument::Str(s),
                Argument::Fixed(f) => Argument::Fixed(f),
                Argument::NewId(nid) => Argument::NewId(nid.id.id),
                Argument::Fd(f) => Argument::Fd(f),
                Argument::Object(o) => {
                    let next_interface = arg_interfaces.next().unwrap();
                    if o.id.id != 0 {
                        let arg_object = guard.get_object(o.id.clone())?;
                        if !same_interface_or_anonymous(next_interface, arg_object.interface) {
                            panic!("Request {}@{}.{} expects an argument of interface {} but {} was provided instead.", object.interface.name, id.id, message_desc.name, next_interface.name, arg_object.interface.name);
                        }
                    } else if !matches!(message_desc.signature[i], ArgumentType::Object(AllowNull::Yes)) {
                        panic!("Request {}@{}.{} expects an non-null object argument.", object.interface.name, id.id, message_desc.name);
                    }
                    Argument::Object(o.id.id)
                }
            });
        }

        let msg = Message { sender_id: id.id, opcode, args: msg_args };

        if let Err(err) = guard.socket.write_message(&msg) {
            guard.last_error = Some(WaylandError::Io(err));
        }

        // Handle destruction if relevant
        if message_desc.is_destructor {
            guard
                .map
                .with(id.id, |obj| {
                    obj.data.client_destroyed = true;
                })
                .unwrap();
            object.data.user_data.destroyed(ObjectId { id });
        }
        if let Some((child_id, child_serial, child_interface)) = child {
            Ok(ObjectId {
                id: InnerObjectId {
                    id: child_id,
                    serial: child_serial,
                    interface: child_interface,
                },
            })
        } else {
            Ok(Self::null_id())
        }
    }

    pub fn get_data(&self, id: ObjectId) -> Result<Arc<dyn ObjectData>, InvalidId> {
        let object = self.state.lock_protocol().get_object(id.id)?;
        Ok(object.data.user_data)
    }

    pub fn set_data(&self, id: ObjectId, data: Arc<dyn ObjectData>) -> Result<(), InvalidId> {
        self.state
            .lock_protocol()
            .map
            .with(id.id.id, move |objdata| {
                if objdata.data.serial != id.id.serial {
                    Err(InvalidId)
                } else {
                    objdata.data.user_data = data;
                    Ok(())
                }
            })
            .unwrap_or(Err(InvalidId))
    }

    // Nothing to do here, we don't have an inner queue
    pub fn dispatch_inner_queue(&self) -> Result<usize, WaylandError> {
        Ok(0)
    }
}

impl ProtocolState {
    fn next_serial(&mut self) -> u32 {
        self.last_serial = self.last_serial.wrapping_add(1);
        self.last_serial
    }

    #[inline]
    fn no_last_error(&self) -> Result<(), WaylandError> {
        if let Some(ref err) = self.last_error {
            Err(err.clone())
        } else {
            Ok(())
        }
    }

    #[inline]
    fn store_and_return_error(&mut self, err: impl Into<WaylandError>) -> WaylandError {
        let err = err.into();
        crate::log_error!("{err}");
        self.last_error = Some(err.clone());
        err
    }

    #[inline]
    fn store_if_not_wouldblock_and_return_error(&mut self, e: std::io::Error) -> WaylandError {
        if e.kind() != std::io::ErrorKind::WouldBlock {
            self.store_and_return_error(e)
        } else {
            e.into()
        }
    }

    fn get_object(&self, id: InnerObjectId) -> Result<Object<Data>, InvalidId> {
        let object = self.map.find(id.id).ok_or(InvalidId)?;
        if object.data.serial != id.serial {
            return Err(InvalidId);
        }
        Ok(object)
    }

    fn handle_display_event(&mut self, message: Message<u32, OwnedFd>) -> Result<(), WaylandError> {
        if self.debug {
            debug::print_dispatched_message(
                "wl_display",
                message.sender_id,
                if message.opcode == 0 { "error" } else { "delete_id" },
                &message.args,
            );
        }
        match message.opcode {
            0 => {
                // wl_display.error
                if let [Argument::Object(obj), Argument::Uint(code), Argument::Str(Some(ref message))] =
                    message.args[..]
                {
                    let object = self.map.find(obj);
                    let err = WaylandError::Protocol(ProtocolError {
                        code,
                        object_id: obj,
                        object_interface: object
                            .map(|obj| obj.interface.name)
                            .unwrap_or("<unknown>")
                            .into(),
                        message: message.to_string_lossy().into(),
                    });
                    return Err(self.store_and_return_error(err));
                } else {
                    unreachable!()
                }
            }
            1 => {
                // wl_display.delete_id
                if let [Argument::Uint(id)] = message.args[..] {
                    let client_destroyed = self
                        .map
                        .with(id, |obj| {
                            obj.data.server_destroyed = true;
                            obj.data.client_destroyed
                        })
                        .unwrap_or(false);
                    if client_destroyed {
                        self.map.remove(id);
                    }
                } else {
                    unreachable!()
                }
            }
            _ => unreachable!(),
        }
        Ok(())
    }
}

fn dispatch_events(state: Arc<ConnectionState>) -> Result<usize, WaylandError> {
    let backend = Backend { backend: InnerBackend { state } };
    let mut guard = backend.backend.state.lock_protocol();
    guard.no_last_error()?;
    let mut dispatched = 0;
    loop {
        // Attempt to read a message
        let ProtocolState { ref mut socket, ref map, .. } = *guard;
        let message = match socket.read_one_message(|id, opcode| {
            map.find(id)
                .and_then(|o| o.interface.events.get(opcode as usize))
                .map(|desc| desc.signature)
        }) {
            Ok(msg) => msg,
            Err(MessageParseError::MissingData) | Err(MessageParseError::MissingFD) => {
                // need to read more data
                if let Err(e) = guard.socket.fill_incoming_buffers() {
                    if e.kind() != std::io::ErrorKind::WouldBlock {
                        return Err(guard.store_and_return_error(e));
                    } else if dispatched == 0 {
                        return Err(e.into());
                    } else {
                        break;
                    }
                }
                continue;
            }
            Err(MessageParseError::Malformed) => {
                // malformed error, protocol error
                let err = WaylandError::Protocol(ProtocolError {
                    code: 0,
                    object_id: 0,
                    object_interface: "".into(),
                    message: "Malformed Wayland message.".into(),
                });
                return Err(guard.store_and_return_error(err));
            }
        };

        // We got a message, retrieve its associated object & details
        // These lookups must succeed otherwise we would not have been able to parse this message
        let receiver = guard.map.find(message.sender_id).unwrap();
        let message_desc = receiver.interface.events.get(message.opcode as usize).unwrap();

        // Short-circuit display-associated events
        if message.sender_id == 1 {
            guard.handle_display_event(message)?;
            continue;
        }

        let mut created_id = None;

        // Convert the arguments and create the new object if applicable
        let mut args = SmallVec::with_capacity(message.args.len());
        let mut arg_interfaces = message_desc.arg_interfaces.iter();
        for arg in message.args.into_iter() {
            args.push(match arg {
                Argument::Array(a) => Argument::Array(a),
                Argument::Int(i) => Argument::Int(i),
                Argument::Uint(u) => Argument::Uint(u),
                Argument::Str(s) => Argument::Str(s),
                Argument::Fixed(f) => Argument::Fixed(f),
                Argument::Fd(f) => Argument::Fd(f),
                Argument::Object(o) => {
                    if o != 0 {
                        // Lookup the object to make the appropriate Id
                        let obj = match guard.map.find(o) {
                            Some(o) => o,
                            None => {
                                let err = WaylandError::Protocol(ProtocolError {
                                    code: 0,
                                    object_id: 0,
                                    object_interface: "".into(),
                                    message: format!("Unknown object {o}."),
                                });
                                return Err(guard.store_and_return_error(err));
                            }
                        };
                        if let Some(next_interface) = arg_interfaces.next() {
                            if !same_interface_or_anonymous(next_interface, obj.interface) {
                                let err = WaylandError::Protocol(ProtocolError {
                                    code: 0,
                                    object_id: 0,
                                    object_interface: "".into(),
                                    message: format!(
                                        "Protocol error: server sent object {} for interface {}, but it has interface {}.",
                                        o, next_interface.name, obj.interface.name
                                    ),
                                });
                                return Err(guard.store_and_return_error(err));
                            }
                        }
                        Argument::Object(ObjectId { id: InnerObjectId { id: o, serial: obj.data.serial, interface: obj.interface }})
                    } else {
                        Argument::Object(ObjectId { id: InnerObjectId { id: 0, serial: 0, interface: &ANONYMOUS_INTERFACE }})
                    }
                }
                Argument::NewId(new_id) => {
                    // An object should be created
                    let child_interface = match message_desc.child_interface {
                        Some(iface) => iface,
                        None => panic!("Received event {}@{}.{} which creates an object without specifying its interface, this is unsupported.", receiver.interface.name, message.sender_id, message_desc.name),
                    };

                    let child_udata = Arc::new(UninitObjectData);

                    // if this ID belonged to a now destroyed server object, we can replace it
                    if new_id >= SERVER_ID_LIMIT
                        && guard.map.with(new_id, |obj| obj.data.client_destroyed).unwrap_or(false)
                    {
                        guard.map.remove(new_id);
                    }

                    let child_obj = Object {
                        interface: child_interface,
                        version: receiver.version,
                        data: Data {
                            client_destroyed: receiver.data.client_destroyed,
                            server_destroyed: false,
                            user_data: child_udata,
                            serial: guard.next_serial(),
                        }
                    };

                    let child_id = InnerObjectId { id: new_id, serial: child_obj.data.serial, interface: child_obj.interface };
                    created_id = Some(child_id.clone());

                    if let Err(()) = guard.map.insert_at(new_id, child_obj) {
                        // abort parsing, this is an unrecoverable error
                        let err = WaylandError::Protocol(ProtocolError {
                            code: 0,
                            object_id: 0,
                            object_interface: "".into(),
                            message: format!(
                                "Protocol error: server tried to create \
                                an object \"{}\" with invalid id {}.",
                                child_interface.name, new_id
                            ),
                        });
                        return Err(guard.store_and_return_error(err));
                    }

                    Argument::NewId(ObjectId { id: child_id })
                }
            });
        }

        if guard.debug {
            debug::print_dispatched_message(
                receiver.interface.name,
                message.sender_id,
                message_desc.name,
                &args,
            );
        }

        // If this event is send to an already destroyed object (by the client), swallow it
        if receiver.data.client_destroyed {
            continue;
        }

        // Invoke the user callback
        let id = InnerObjectId {
            id: message.sender_id,
            serial: receiver.data.serial,
            interface: receiver.interface,
        };

        // unlock the mutex while we invoke the user callback
        std::mem::drop(guard);
        #[cfg(feature = "log")]
        crate::log_debug!(
            "Dispatching {}.{} ({})",
            id,
            receiver.version,
            debug::DisplaySlice(&args)
        );
        let ret = receiver
            .data
            .user_data
            .clone()
            .event(&backend, Message { sender_id: ObjectId { id }, opcode: message.opcode, args });
        // lock it again to resume dispatching
        guard = backend.backend.state.lock_protocol();

        // If this event is a destructor, destroy the object
        if message_desc.is_destructor {
            guard
                .map
                .with(message.sender_id, |obj| {
                    obj.data.server_destroyed = true;
                    obj.data.client_destroyed = true;
                })
                .unwrap();
            receiver.data.user_data.destroyed(ObjectId {
                id: InnerObjectId {
                    id: message.sender_id,
                    serial: receiver.data.serial,
                    interface: receiver.interface,
                },
            });
        }

        match (created_id, ret) {
            (Some(child_id), Some(child_data)) => {
                guard.map.with(child_id.id, |obj| obj.data.user_data = child_data).unwrap();
            }
            (None, None) => {}
            (Some(child_id), None) => {
                panic!("Callback creating object {child_id} did not provide any object data.");
            }
            (None, Some(_)) => {
                panic!("An object data was returned from a callback not creating any object");
            }
        }

        dispatched += 1;
    }
    Ok(dispatched)
}
