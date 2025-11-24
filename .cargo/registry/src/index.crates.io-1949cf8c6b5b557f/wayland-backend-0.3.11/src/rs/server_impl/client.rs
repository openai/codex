use std::{
    ffi::CString,
    os::unix::{
        io::{AsFd, BorrowedFd, OwnedFd, RawFd},
        net::UnixStream,
    },
    sync::Arc,
};

use crate::{
    core_interfaces::{WL_CALLBACK_INTERFACE, WL_DISPLAY_INTERFACE, WL_REGISTRY_INTERFACE},
    debug,
    protocol::{
        check_for_signature, same_interface, same_interface_or_anonymous, AllowNull, Argument,
        ArgumentType, Interface, Message, ObjectInfo, ProtocolError, ANONYMOUS_INTERFACE,
        INLINE_ARGS,
    },
    rs::map::SERVER_ID_LIMIT,
    types::server::{DisconnectReason, InvalidId},
};

use smallvec::SmallVec;

use crate::rs::{
    map::{Object, ObjectMap},
    socket::{BufferedSocket, Socket},
    wire::MessageParseError,
};

use super::{
    handle::PendingDestructor, registry::Registry, ClientData, ClientId, Credentials, Data,
    DumbObjectData, GlobalHandler, InnerClientId, InnerGlobalId, InnerObjectId, ObjectData,
    ObjectId, UninitObjectData,
};

type ArgSmallVec<Fd> = SmallVec<[Argument<ObjectId, Fd>; INLINE_ARGS]>;

#[repr(u32)]
#[allow(dead_code)]
pub(crate) enum DisplayError {
    InvalidObject = 0,
    InvalidMethod = 1,
    NoMemory = 2,
    Implementation = 3,
}

#[derive(Debug)]
pub(crate) struct Client<D: 'static> {
    socket: BufferedSocket,
    pub(crate) map: ObjectMap<Data<D>>,
    debug: bool,
    last_serial: u32,
    pub(crate) id: InnerClientId,
    pub(crate) killed: bool,
    pub(crate) data: Arc<dyn ClientData>,
}

impl<D> Client<D> {
    fn next_serial(&mut self) -> u32 {
        self.last_serial = self.last_serial.wrapping_add(1);
        self.last_serial
    }
}

impl<D> Client<D> {
    pub(crate) fn new(
        stream: UnixStream,
        id: InnerClientId,
        debug: bool,
        data: Arc<dyn ClientData>,
    ) -> Self {
        let socket = BufferedSocket::new(Socket::from(stream));
        let mut map = ObjectMap::new();
        map.insert_at(
            1,
            Object {
                interface: &WL_DISPLAY_INTERFACE,
                version: 1,
                data: Data { user_data: Arc::new(DumbObjectData), serial: 0 },
            },
        )
        .unwrap();

        data.initialized(ClientId { id: id.clone() });

        Self { socket, map, debug, id, killed: false, last_serial: 0, data }
    }

    pub(crate) fn create_object(
        &mut self,
        interface: &'static Interface,
        version: u32,
        user_data: Arc<dyn ObjectData<D>>,
    ) -> InnerObjectId {
        let serial = self.next_serial();
        let id = self.map.server_insert_new(Object {
            interface,
            version,
            data: Data { serial, user_data },
        });
        InnerObjectId { id, serial, client_id: self.id.clone(), interface }
    }

    pub(crate) fn object_info(&self, id: InnerObjectId) -> Result<ObjectInfo, InvalidId> {
        let object = self.get_object(id.clone())?;
        Ok(ObjectInfo { id: id.id, interface: object.interface, version: object.version })
    }

    pub(crate) fn send_event(
        &mut self,
        Message { sender_id: object_id, opcode, args }: Message<ObjectId, RawFd>,
        pending_destructors: Option<&mut Vec<super::handle::PendingDestructor<D>>>,
    ) -> Result<(), InvalidId> {
        if self.killed {
            return Ok(());
        }
        let object = self.get_object(object_id.id.clone())?;

        let message_desc = match object.interface.events.get(opcode as usize) {
            Some(msg) => msg,
            None => {
                panic!(
                    "Unknown opcode {} for object {}@{}.",
                    opcode, object.interface.name, object_id.id
                );
            }
        };

        if !check_for_signature(message_desc.signature, &args) {
            panic!(
                "Unexpected signature for event {}@{}.{}: expected {:?}, got {:?}.",
                object.interface.name,
                object_id.id,
                message_desc.name,
                message_desc.signature,
                args
            );
        }

        if self.debug {
            debug::print_send_message(
                object.interface.name,
                object_id.id.id,
                message_desc.name,
                &args,
                false,
            );
        }

        let mut msg_args = SmallVec::with_capacity(args.len());
        let mut arg_interfaces = message_desc.arg_interfaces.iter();
        for (i, arg) in args.into_iter().enumerate() {
            msg_args.push(match arg {
                Argument::Array(a) => Argument::Array(a),
                Argument::Int(i) => Argument::Int(i),
                Argument::Uint(u) => Argument::Uint(u),
                Argument::Str(s) => Argument::Str(s),
                Argument::Fixed(f) => Argument::Fixed(f),
                Argument::Fd(f) => Argument::Fd(f),
                Argument::NewId(o) => {
                    if o.id.id != 0 {
                        if o.id.client_id != self.id {
                            panic!("Attempting to send an event with objects from wrong client.")
                        }
                        let object = self.get_object(o.id.clone())?;
                        let child_interface = match message_desc.child_interface {
                            Some(iface) => iface,
                            None => panic!("Trying to send event {}@{}.{} which creates an object without specifying its interface, this is unsupported.", object_id.id.interface.name, object_id.id, message_desc.name),
                        };
                        if !same_interface(child_interface, object.interface) {
                            panic!("Event {}@{}.{} expects a newid argument of interface {} but {} was provided instead.", object.interface.name, object_id.id, message_desc.name, child_interface.name, object.interface.name);
                        }
                    } else if !matches!(message_desc.signature[i], ArgumentType::NewId) {
                        panic!("Request {}@{}.{} expects an non-null newid argument.", object.interface.name, object_id.id, message_desc.name);
                    }
                    Argument::Object(o.id.id)
                },
                Argument::Object(o) => {
                    let next_interface = arg_interfaces.next().unwrap();
                    if o.id.id != 0 {
                        if o.id.client_id != self.id {
                            panic!("Attempting to send an event with objects from wrong client.")
                        }
                        let arg_object = self.get_object(o.id.clone())?;
                        if !same_interface_or_anonymous(next_interface, arg_object.interface) {
                            panic!("Event {}@{}.{} expects an object argument of interface {} but {} was provided instead.", object.interface.name, object_id.id, message_desc.name, next_interface.name, arg_object.interface.name);
                        }
                    } else if !matches!(message_desc.signature[i], ArgumentType::Object(AllowNull::Yes)) {
                            panic!("Request {}@{}.{} expects an non-null object argument.", object.interface.name, object_id.id, message_desc.name);
                    }
                    Argument::Object(o.id.id)
                }
            });
        }

        let msg = Message { sender_id: object_id.id.id, opcode, args: msg_args };

        if self.socket.write_message(&msg).is_err() {
            self.kill(DisconnectReason::ConnectionClosed);
        }

        // Handle destruction if relevant
        if message_desc.is_destructor {
            self.map.remove(object_id.id.id);
            if let Some(vec) = pending_destructors {
                vec.push((object.data.user_data.clone(), self.id.clone(), object_id.id.clone()));
            }
            self.send_delete_id(object_id.id);
        }

        Ok(())
    }

    pub(crate) fn send_delete_id(&mut self, object_id: InnerObjectId) {
        // We should only send delete_id for objects in the client ID space
        if object_id.id < SERVER_ID_LIMIT {
            let msg = message!(1, 1, [Argument::Uint(object_id.id)]);
            if self.socket.write_message(&msg).is_err() {
                self.kill(DisconnectReason::ConnectionClosed);
            }
        }
        self.map.remove(object_id.id);
    }

    pub(crate) fn get_object_data(
        &self,
        id: InnerObjectId,
    ) -> Result<Arc<dyn ObjectData<D>>, InvalidId> {
        let object = self.get_object(id)?;
        Ok(object.data.user_data)
    }

    pub(crate) fn set_object_data(
        &mut self,
        id: InnerObjectId,
        data: Arc<dyn ObjectData<D>>,
    ) -> Result<(), InvalidId> {
        self.map
            .with(id.id, |objdata| {
                if objdata.data.serial != id.serial {
                    Err(InvalidId)
                } else {
                    objdata.data.user_data = data;
                    Ok(())
                }
            })
            .unwrap_or(Err(InvalidId))
    }

    pub(crate) fn post_display_error(&mut self, code: DisplayError, message: CString) {
        self.post_error(
            InnerObjectId {
                id: 1,
                interface: &WL_DISPLAY_INTERFACE,
                client_id: self.id.clone(),
                serial: 0,
            },
            code as u32,
            message,
        )
    }

    pub(crate) fn post_error(
        &mut self,
        object_id: InnerObjectId,
        error_code: u32,
        message: CString,
    ) {
        let converted_message = message.to_string_lossy().into();
        // errors are ignored, as the client will be killed anyway
        let _ = self.send_event(
            message!(
                ObjectId {
                    id: InnerObjectId {
                        id: 1,
                        interface: &WL_DISPLAY_INTERFACE,
                        client_id: self.id.clone(),
                        serial: 0
                    }
                },
                0, // wl_display.error
                [
                    Argument::Object(ObjectId { id: object_id.clone() }),
                    Argument::Uint(error_code),
                    Argument::Str(Some(Box::new(message))),
                ],
            ),
            // wl_display.error is not a destructor, this argument will not be used
            None,
        );
        let _ = self.flush();
        self.kill(DisconnectReason::ProtocolError(ProtocolError {
            code: error_code,
            object_id: object_id.id,
            object_interface: object_id.interface.name.into(),
            message: converted_message,
        }));
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub(crate) fn get_credentials(&self) -> Credentials {
        let creds =
            rustix::net::sockopt::socket_peercred(&self.socket).expect("getsockopt failed!?");
        let pid = rustix::process::Pid::as_raw(Some(creds.pid));
        Credentials { pid, uid: creds.uid.as_raw(), gid: creds.gid.as_raw() }
    }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    // for now this only works on linux
    pub(crate) fn get_credentials(&self) -> Credentials {
        Credentials { pid: 0, uid: 0, gid: 0 }
    }

    pub(crate) fn kill(&mut self, reason: DisconnectReason) {
        self.killed = true;
        self.data.disconnected(ClientId { id: self.id.clone() }, reason);
    }

    pub(crate) fn flush(&mut self) -> std::io::Result<()> {
        self.socket.flush()
    }

    pub(crate) fn all_objects(&self) -> impl Iterator<Item = ObjectId> + '_ {
        let client_id = self.id.clone();
        self.map.all_objects().map(move |(id, obj)| ObjectId {
            id: InnerObjectId {
                id,
                client_id: client_id.clone(),
                interface: obj.interface,
                serial: obj.data.serial,
            },
        })
    }

    #[allow(clippy::type_complexity)]
    pub(crate) fn next_request(
        &mut self,
    ) -> std::io::Result<(Message<u32, OwnedFd>, Object<Data<D>>)> {
        if self.killed {
            return Err(rustix::io::Errno::PIPE.into());
        }
        loop {
            let map = &self.map;
            let msg = match self.socket.read_one_message(|id, opcode| {
                map.find(id)
                    .and_then(|o| o.interface.requests.get(opcode as usize))
                    .map(|desc| desc.signature)
            }) {
                Ok(msg) => msg,
                Err(MessageParseError::MissingData) | Err(MessageParseError::MissingFD) => {
                    // need to read more data
                    if let Err(e) = self.socket.fill_incoming_buffers() {
                        if e.kind() != std::io::ErrorKind::WouldBlock {
                            self.kill(DisconnectReason::ConnectionClosed);
                        }
                        return Err(e);
                    }
                    continue;
                }
                Err(MessageParseError::Malformed) => {
                    self.kill(DisconnectReason::ConnectionClosed);
                    return Err(rustix::io::Errno::PROTO.into());
                }
            };

            let obj = self.map.find(msg.sender_id).unwrap();

            if self.debug {
                debug::print_dispatched_message(
                    obj.interface.name,
                    msg.sender_id,
                    obj.interface.requests.get(msg.opcode as usize).unwrap().name,
                    &msg.args,
                );
            }

            return Ok((msg, obj));
        }
    }

    fn get_object(&self, id: InnerObjectId) -> Result<Object<Data<D>>, InvalidId> {
        let object = self.map.find(id.id).ok_or(InvalidId)?;
        if object.data.serial != id.serial {
            return Err(InvalidId);
        }
        Ok(object)
    }

    pub(crate) fn object_for_protocol_id(&self, pid: u32) -> Result<InnerObjectId, InvalidId> {
        let object = self.map.find(pid).ok_or(InvalidId)?;
        Ok(InnerObjectId {
            id: pid,
            client_id: self.id.clone(),
            serial: object.data.serial,
            interface: object.interface,
        })
    }

    fn queue_all_destructors(&mut self, pending_destructors: &mut Vec<PendingDestructor<D>>) {
        pending_destructors.extend(self.map.all_objects().map(|(id, obj)| {
            (
                obj.data.user_data.clone(),
                self.id.clone(),
                InnerObjectId {
                    id,
                    serial: obj.data.serial,
                    client_id: self.id.clone(),
                    interface: obj.interface,
                },
            )
        }));
    }

    pub(crate) fn handle_display_request(
        &mut self,
        message: Message<u32, OwnedFd>,
        registry: &mut Registry<D>,
    ) {
        match message.opcode {
            // wl_display.sync(new id wl_callback)
            0 => {
                if let [Argument::NewId(new_id)] = message.args[..] {
                    let serial = self.next_serial();
                    let callback_obj = Object {
                        interface: &WL_CALLBACK_INTERFACE,
                        version: 1,
                        data: Data { user_data: Arc::new(DumbObjectData), serial },
                    };
                    if let Err(()) = self.map.insert_at(new_id, callback_obj) {
                        self.post_display_error(
                            DisplayError::InvalidObject,
                            CString::new(format!("Invalid new_id: {new_id}.")).unwrap(),
                        );
                        return;
                    }
                    let cb_id = ObjectId {
                        id: InnerObjectId {
                            id: new_id,
                            client_id: self.id.clone(),
                            serial,
                            interface: &WL_CALLBACK_INTERFACE,
                        },
                    };
                    // send wl_callback.done(0) this callback does not have any meaningful destructor to run, we can ignore it
                    self.send_event(message!(cb_id, 0, [Argument::Uint(0)]), None).unwrap();
                } else {
                    unreachable!()
                }
            }
            // wl_display.get_registry(new id wl_registry)
            1 => {
                if let [Argument::NewId(new_id)] = message.args[..] {
                    let serial = self.next_serial();
                    let registry_obj = Object {
                        interface: &WL_REGISTRY_INTERFACE,
                        version: 1,
                        data: Data { user_data: Arc::new(DumbObjectData), serial },
                    };
                    let registry_id = InnerObjectId {
                        id: new_id,
                        serial,
                        client_id: self.id.clone(),
                        interface: &WL_REGISTRY_INTERFACE,
                    };
                    if let Err(()) = self.map.insert_at(new_id, registry_obj) {
                        self.post_display_error(
                            DisplayError::InvalidObject,
                            CString::new(format!("Invalid new_id: {new_id}.")).unwrap(),
                        );
                        return;
                    }
                    let _ = registry.new_registry(registry_id, self);
                } else {
                    unreachable!()
                }
            }
            _ => {
                // unkown opcode, kill the client
                self.post_display_error(
                    DisplayError::InvalidMethod,
                    CString::new(format!(
                        "Unknown opcode {} for interface wl_display.",
                        message.opcode
                    ))
                    .unwrap(),
                );
            }
        }
    }

    #[allow(clippy::type_complexity)]
    pub(crate) fn handle_registry_request(
        &mut self,
        message: Message<u32, OwnedFd>,
        registry: &mut Registry<D>,
    ) -> Option<(InnerClientId, InnerGlobalId, InnerObjectId, Arc<dyn GlobalHandler<D>>)> {
        match message.opcode {
            // wl_registry.bind(uint name, str interface, uint version, new id)
            0 => {
                if let [Argument::Uint(name), Argument::Str(Some(ref interface_name)), Argument::Uint(version), Argument::NewId(new_id)] =
                    message.args[..]
                {
                    if let Some((interface, global_id, handler)) =
                        registry.check_bind(self, name, interface_name, version)
                    {
                        let serial = self.next_serial();
                        let object = Object {
                            interface,
                            version,
                            data: Data { serial, user_data: Arc::new(UninitObjectData) },
                        };
                        if let Err(()) = self.map.insert_at(new_id, object) {
                            self.post_display_error(
                                DisplayError::InvalidObject,
                                CString::new(format!("Invalid new_id: {new_id}.")).unwrap(),
                            );
                            return None;
                        }
                        Some((
                            self.id.clone(),
                            global_id,
                            InnerObjectId {
                                id: new_id,
                                client_id: self.id.clone(),
                                interface,
                                serial,
                            },
                            handler.clone(),
                        ))
                    } else {
                        self.post_display_error(
                            DisplayError::InvalidObject,
                            CString::new(format!(
                                "Invalid binding of {} version {} for global {}.",
                                interface_name.to_string_lossy(),
                                version,
                                name
                            ))
                            .unwrap(),
                        );
                        None
                    }
                } else {
                    unreachable!()
                }
            }
            _ => {
                // unkown opcode, kill the client
                self.post_display_error(
                    DisplayError::InvalidMethod,
                    CString::new(format!(
                        "Unknown opcode {} for interface wl_registry.",
                        message.opcode
                    ))
                    .unwrap(),
                );
                None
            }
        }
    }

    pub(crate) fn process_request(
        &mut self,
        object: &Object<Data<D>>,
        message: Message<u32, OwnedFd>,
    ) -> Option<(ArgSmallVec<OwnedFd>, bool, Option<InnerObjectId>)> {
        let message_desc = object.interface.requests.get(message.opcode as usize).unwrap();
        // Convert the arguments and create the new object if applicable
        let mut new_args = SmallVec::with_capacity(message.args.len());
        let mut arg_interfaces = message_desc.arg_interfaces.iter();
        let mut created_id = None;
        for (i, arg) in message.args.into_iter().enumerate() {
            new_args.push(match arg {
                Argument::Array(a) => Argument::Array(a),
                Argument::Int(i) => Argument::Int(i),
                Argument::Uint(u) => Argument::Uint(u),
                Argument::Str(s) => Argument::Str(s),
                Argument::Fixed(f) => Argument::Fixed(f),
                Argument::Fd(f) => Argument::Fd(f),
                Argument::Object(o) => {
                    let next_interface = arg_interfaces.next();
                    if o != 0 {
                        // Lookup the object to make the appropriate Id
                        let obj = match self.map.find(o) {
                            Some(o) => o,
                            None => {
                                self.post_display_error(
                                    DisplayError::InvalidObject,
                                    CString::new(format!("Unknown id: {o}.")).unwrap()
                                );
                                return None;
                            }
                        };
                        if let Some(next_interface) = next_interface {
                            if !same_interface_or_anonymous(next_interface, obj.interface) {
                                self.post_display_error(
                                    DisplayError::InvalidObject,
                                    CString::new(format!(
                                        "Invalid object {} in request {}.{}: expected {} but got {}.",
                                        o,
                                        object.interface.name,
                                        message_desc.name,
                                        next_interface.name,
                                        obj.interface.name,
                                    )).unwrap()
                                );
                                return None;
                            }
                        }
                        Argument::Object(ObjectId { id: InnerObjectId { id: o, client_id: self.id.clone(), serial: obj.data.serial, interface: obj.interface }})
                    } else if matches!(message_desc.signature[i], ArgumentType::Object(AllowNull::Yes)) {
                        Argument::Object(ObjectId { id: InnerObjectId { id: 0, client_id: self.id.clone(), serial: 0, interface: &ANONYMOUS_INTERFACE }})
                    } else {
                        self.post_display_error(
                            DisplayError::InvalidObject,
                            CString::new(format!(
                                "Invalid null object in request {}.{}.",
                                object.interface.name,
                                message_desc.name,
                            )).unwrap()
                        );
                        return None;
                    }
                }
                Argument::NewId(new_id) => {
                    // An object should be created
                    let child_interface = match message_desc.child_interface {
                        Some(iface) => iface,
                        None => panic!("Received request {}@{}.{} which creates an object without specifying its interface, this is unsupported.", object.interface.name, message.sender_id, message_desc.name),
                    };

                    let child_udata = Arc::new(UninitObjectData);

                    let child_obj = Object {
                        interface: child_interface,
                        version: object.version,
                        data: Data {
                            user_data: child_udata,
                            serial: self.next_serial(),
                        }
                    };

                    let child_id = InnerObjectId { id: new_id, client_id: self.id.clone(), serial: child_obj.data.serial, interface: child_obj.interface };
                    created_id = Some(child_id.clone());

                    if let Err(()) = self.map.insert_at(new_id, child_obj) {
                        // abort parsing, this is an unrecoverable error
                        self.post_display_error(
                            DisplayError::InvalidObject,
                            CString::new(format!("Invalid new_id: {new_id}.")).unwrap()
                        );
                        return None;
                    }

                    Argument::NewId(ObjectId { id: child_id })
                }
            });
        }
        Some((new_args, message_desc.is_destructor, created_id))
    }
}

impl<D> AsFd for Client<D> {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.socket.as_fd()
    }
}

#[derive(Debug)]
pub(crate) struct ClientStore<D: 'static> {
    clients: Vec<Option<Client<D>>>,
    last_serial: u32,
    debug: bool,
}

impl<D> ClientStore<D> {
    pub(crate) fn new(debug: bool) -> Self {
        Self { clients: Vec::new(), last_serial: 0, debug }
    }

    pub(crate) fn create_client(
        &mut self,
        stream: UnixStream,
        data: Arc<dyn ClientData>,
    ) -> InnerClientId {
        let serial = self.next_serial();
        // Find the next free place
        let (id, place) = match self.clients.iter_mut().enumerate().find(|(_, c)| c.is_none()) {
            Some((id, place)) => (id, place),
            None => {
                self.clients.push(None);
                (self.clients.len() - 1, self.clients.last_mut().unwrap())
            }
        };

        let id = InnerClientId { id: id as u32, serial };

        *place = Some(Client::new(stream, id.clone(), self.debug, data));

        id
    }

    pub(crate) fn get_client(&self, id: InnerClientId) -> Result<&Client<D>, InvalidId> {
        match self.clients.get(id.id as usize) {
            Some(Some(client)) if client.id == id => Ok(client),
            _ => Err(InvalidId),
        }
    }

    pub(crate) fn get_client_mut(
        &mut self,
        id: InnerClientId,
    ) -> Result<&mut Client<D>, InvalidId> {
        match self.clients.get_mut(id.id as usize) {
            Some(&mut Some(ref mut client)) if client.id == id => Ok(client),
            _ => Err(InvalidId),
        }
    }

    pub(crate) fn cleanup(
        &mut self,
        pending_destructors: &mut Vec<PendingDestructor<D>>,
    ) -> SmallVec<[Client<D>; 1]> {
        let mut cleaned = SmallVec::new();
        for place in &mut self.clients {
            if place.as_ref().map(|client| client.killed).unwrap_or(false) {
                // Remove the client from the store and flush it one last time before dropping it
                let mut client = place.take().unwrap();
                client.queue_all_destructors(pending_destructors);
                let _ = client.flush();
                cleaned.push(client);
            }
        }
        cleaned
    }

    fn next_serial(&mut self) -> u32 {
        self.last_serial = self.last_serial.wrapping_add(1);
        self.last_serial
    }

    pub(crate) fn clients_mut(&mut self) -> impl Iterator<Item = &mut Client<D>> {
        self.clients.iter_mut().flat_map(|o| o.as_mut()).filter(|c| !c.killed)
    }

    pub(crate) fn all_clients_id(&self) -> impl Iterator<Item = ClientId> + '_ {
        self.clients.iter().flat_map(|opt| {
            opt.as_ref().filter(|c| !c.killed).map(|client| ClientId { id: client.id.clone() })
        })
    }
}
