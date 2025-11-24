use std::{
    ffi::CString,
    fmt,
    os::unix::{
        io::{BorrowedFd, OwnedFd, RawFd},
        net::UnixStream,
    },
    sync::Arc,
};

use crate::protocol::{Interface, Message, ObjectInfo};
pub use crate::types::server::{Credentials, DisconnectReason, GlobalInfo, InitError, InvalidId};

use super::server_impl;

/// A trait representing your data associated to an object
///
/// You will only be given access to it as a `&` reference, so you
/// need to handle interior mutability by yourself.
///
/// The methods of this trait will be invoked internally every time a
/// new object is created to initialize its data.
pub trait ObjectData<D>: downcast_rs::DowncastSync {
    /// Dispatch a request for the associated object
    ///
    /// If the request has a `NewId` argument, the callback must return the object data
    /// for the newly created object
    fn request(
        self: Arc<Self>,
        handle: &Handle,
        data: &mut D,
        client_id: ClientId,
        msg: Message<ObjectId, OwnedFd>,
    ) -> Option<Arc<dyn ObjectData<D>>>;
    /// Notification that the object has been destroyed and is no longer active
    fn destroyed(
        self: Arc<Self>,
        handle: &Handle,
        data: &mut D,
        client_id: ClientId,
        object_id: ObjectId,
    );
    /// Helper for forwarding a Debug implementation of your `ObjectData` type
    ///
    /// By default will just print `ObjectData { ... }`
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn debug(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectData").finish_non_exhaustive()
    }
}

downcast_rs::impl_downcast!(sync ObjectData<D>);

impl<D: 'static> std::fmt::Debug for dyn ObjectData<D> {
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.debug(f)
    }
}

/// A trait representing the handling of new bound globals
pub trait GlobalHandler<D>: downcast_rs::DowncastSync {
    /// Check if given client is allowed to interact with given global
    ///
    /// If this function returns false, the client will not be notified of the existence
    /// of this global, and any attempt to bind it will result in a protocol error as if
    /// the global did not exist.
    ///
    /// Default implementation always return true.
    fn can_view(
        &self,
        _client_id: ClientId,
        _client_data: &Arc<dyn ClientData>,
        _global_id: GlobalId,
    ) -> bool {
        true
    }
    /// A global has been bound
    ///
    /// Given client bound given global, creating given object.
    ///
    /// The method must return the object data for the newly created object.
    fn bind(
        self: Arc<Self>,
        handle: &Handle,
        data: &mut D,
        client_id: ClientId,
        global_id: GlobalId,
        object_id: ObjectId,
    ) -> Arc<dyn ObjectData<D>>;
    /// Helper for forwarding a Debug implementation of your `GlobalHandler` type
    ///
    /// By default will just print `GlobalHandler { ... }`
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn debug(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GlobalHandler").finish_non_exhaustive()
    }
}

impl<D: 'static> std::fmt::Debug for dyn GlobalHandler<D> {
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.debug(f)
    }
}

downcast_rs::impl_downcast!(sync GlobalHandler<D>);

/// A trait representing your data associated to a client
pub trait ClientData: downcast_rs::DowncastSync {
    /// Notification that the client was initialized
    fn initialized(&self, _client_id: ClientId) {}
    /// Notification that the client is disconnected
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
    /// Helper for forwarding a Debug implementation of your `ClientData` type
    ///
    /// By default will just print `GlobalHandler { ... }`
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn debug(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientData").finish_non_exhaustive()
    }
}

impl std::fmt::Debug for dyn ClientData {
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.debug(f)
    }
}

impl ClientData for () {}

downcast_rs::impl_downcast!(sync ClientData);

/// An ID representing a Wayland object
///
/// The backend internally tracks which IDs are still valid, invalidates them when the protocol object they
/// represent is destroyed. As such even though the Wayland protocol reuses IDs, you still confidently compare
/// two `ObjectId` for equality, they will only compare as equal if they both represent the same protocol
/// object from the same client.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ObjectId {
    pub(crate) id: server_impl::InnerObjectId,
}

impl ObjectId {
    /// Returns whether this object is a null object.
    ///
    /// **Note:** This is not the same as checking if the ID is still valid, which cannot be done without the
    /// [`Backend`]. A null ID is the ID equivalent of a null pointer: it never has been valid and never will
    /// be.
    pub fn is_null(&self) -> bool {
        self.id.is_null()
    }

    /// Returns an object id that represents a null object.
    ///
    /// This object ID is always invalid, and should be used for events with an optional `Object` argument.
    #[inline]
    pub fn null() -> ObjectId {
        server_impl::InnerHandle::null_id()
    }

    /// Returns the interface of this object.
    pub fn interface(&self) -> &'static Interface {
        self.id.interface()
    }

    /// Check if two object IDs are associated with the same client
    ///
    /// *Note:* This may spuriously return `false` if one (or both) of the objects to compare
    /// is no longer valid.
    pub fn same_client_as(&self, other: &Self) -> bool {
        self.id.same_client_as(&other.id)
    }

    /// Return the protocol-level numerical ID of this object
    ///
    /// Protocol IDs are reused after object destruction and each client has its own ID space, so this should
    /// not be used as a unique identifier, instead use the `ObjectId` directly, it implements `Clone`,
    /// `PartialEq`, `Eq` and `Hash`.
    pub fn protocol_id(&self) -> u32 {
        self.id.protocol_id()
    }
}

impl fmt::Display for ObjectId {
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.id.fmt(f)
    }
}

impl fmt::Debug for ObjectId {
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.id.fmt(f)
    }
}

/// An ID representing a Wayland client
///
/// The backend internally tracks which IDs are still valid, invalidates them when the client they represent
/// is disconnected. As such you can confidently compare two `ClientId` for equality, they will only compare
/// as equal if they both represent the same client.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ClientId {
    pub(crate) id: server_impl::InnerClientId,
}

impl fmt::Debug for ClientId {
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.id.fmt(f)
    }
}

/// An Id representing a global
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct GlobalId {
    pub(crate) id: server_impl::InnerGlobalId,
}

impl fmt::Debug for GlobalId {
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.id.fmt(f)
    }
}

/// Main handle of a backend to the Wayland protocol
///
/// This type hosts most of the protocol-related functionality of the backend, and is the
/// main entry point for manipulating Wayland objects. It can be retrieved from the backend via
/// [`Backend::handle()`] and cloned, and is given to you as argument in many callbacks.
#[derive(Clone, Debug)]
pub struct Handle {
    pub(crate) handle: server_impl::InnerHandle,
}

/// A weak reference to a [`Handle`]
///
/// This handle behaves similarly to [`Weak`][std::sync::Weak], and can be used to keep access to
/// the handle without actually preventing it from being dropped.
#[derive(Clone, Debug)]
pub struct WeakHandle {
    pub(crate) handle: server_impl::WeakInnerHandle,
}

impl WeakHandle {
    /// Try to upgrade this weak handle to a [`Handle`]
    ///
    /// Returns [`None`] if the associated backend was already dropped.
    #[inline]
    pub fn upgrade(&self) -> Option<Handle> {
        self.handle.upgrade().map(|handle| Handle { handle })
    }
}

impl Handle {
    /// Get a [`WeakHandle`] from this handle
    #[inline]
    pub fn downgrade(&self) -> WeakHandle {
        WeakHandle { handle: self.handle.downgrade() }
    }

    /// Get the detailed protocol information about a wayland object
    ///
    /// Returns an error if the provided object ID is no longer valid.
    #[inline]
    pub fn object_info(&self, id: ObjectId) -> Result<ObjectInfo, InvalidId> {
        self.handle.object_info(id.id)
    }

    /// Initializes a connection with a client.
    ///
    /// The `data` parameter contains data that will be associated with the client.
    #[inline]
    pub fn insert_client(
        &mut self,
        stream: UnixStream,
        data: Arc<dyn ClientData>,
    ) -> std::io::Result<ClientId> {
        Ok(ClientId { id: self.handle.insert_client(stream, data)? })
    }

    /// Returns the id of the client which owns the object.
    #[inline]
    pub fn get_client(&self, id: ObjectId) -> Result<ClientId, InvalidId> {
        self.handle.get_client(id.id)
    }

    /// Returns the data associated with a client.
    #[inline]
    pub fn get_client_data(&self, id: ClientId) -> Result<Arc<dyn ClientData>, InvalidId> {
        self.handle.get_client_data(id.id)
    }

    /// Retrive the [`Credentials`] of a client
    #[inline]
    pub fn get_client_credentials(&self, id: ClientId) -> Result<Credentials, InvalidId> {
        self.handle.get_client_credentials(id.id)
    }

    /// Invokes a closure for all clients connected to this server
    ///
    /// Note that while this method is running, an internal lock of the backend is held,
    /// as a result invoking other methods of the `Handle` within the closure will deadlock.
    /// You should thus store the relevant `ClientId` in a container of your choice and process
    /// them after this method has returned.
    #[inline]
    pub fn with_all_clients(&self, f: impl FnMut(ClientId)) {
        self.handle.with_all_clients(f)
    }

    /// Invokes a closure for all objects owned by a client.
    ///
    /// Note that while this method is running, an internal lock of the backend is held,
    /// as a result invoking other methods of the `Handle` within the closure will deadlock.
    /// You should thus store the relevant `ObjectId` in a container of your choice and process
    /// them after this method has returned.
    #[inline]
    pub fn with_all_objects_for(
        &self,
        client_id: ClientId,
        f: impl FnMut(ObjectId),
    ) -> Result<(), InvalidId> {
        self.handle.with_all_objects_for(client_id.id, f)
    }

    /// Retrieve the `ObjectId` for a wayland object given its protocol numerical ID
    #[inline]
    pub fn object_for_protocol_id(
        &self,
        client_id: ClientId,
        interface: &'static Interface,
        protocol_id: u32,
    ) -> Result<ObjectId, InvalidId> {
        self.handle.object_for_protocol_id(client_id.id, interface, protocol_id)
    }

    /// Create a new object for given client
    ///
    /// To ensure state coherence of the protocol, the created object should be immediately
    /// sent as a "New ID" argument in an event to the client.
    ///
    /// # Panics
    ///
    /// This method will panic if the type parameter `D` is not same to the same type as the
    /// one the backend was initialized with.
    #[inline]
    pub fn create_object<D: 'static>(
        &self,
        client_id: ClientId,
        interface: &'static Interface,
        version: u32,
        data: Arc<dyn ObjectData<D>>,
    ) -> Result<ObjectId, InvalidId> {
        self.handle.create_object(client_id.id, interface, version, data)
    }

    /// Send an event to the client
    ///
    /// Returns an error if the sender ID of the provided message is no longer valid.
    ///
    /// # Panics
    ///
    /// Checks against the protocol specification are done, and this method will panic if they do
    /// not pass:
    ///
    /// - the message opcode must be valid for the sender interface
    /// - the argument list must match the prototype for the message associated with this opcode
    #[inline]
    pub fn send_event(&self, msg: Message<ObjectId, RawFd>) -> Result<(), InvalidId> {
        self.handle.send_event(msg)
    }

    /// Returns the data associated with an object.
    ///
    /// **Panic:** This method will panic if the type parameter `D` is not same to the same type as the
    /// one the backend was initialized with.
    #[inline]
    pub fn get_object_data<D: 'static>(
        &self,
        id: ObjectId,
    ) -> Result<Arc<dyn ObjectData<D>>, InvalidId> {
        self.handle.get_object_data(id.id)
    }

    /// Returns the data associated with an object as a `dyn Any`
    #[inline]
    pub fn get_object_data_any(
        &self,
        id: ObjectId,
    ) -> Result<Arc<dyn std::any::Any + Send + Sync>, InvalidId> {
        self.handle.get_object_data_any(id.id)
    }

    /// Sets the data associated with some object.
    ///
    /// **Panic:** This method will panic if the type parameter `D` is not same to the same type as the
    /// one the backend was initialized with.
    #[inline]
    pub fn set_object_data<D: 'static>(
        &self,
        id: ObjectId,
        data: Arc<dyn ObjectData<D>>,
    ) -> Result<(), InvalidId> {
        self.handle.set_object_data(id.id, data)
    }

    /// Posts a protocol error on an object. This will also disconnect the client which created the object.
    #[inline]
    pub fn post_error(&self, object_id: ObjectId, error_code: u32, message: CString) {
        self.handle.post_error(object_id.id, error_code, message)
    }

    /// Kills the connection to a client.
    ///
    /// The disconnection reason determines the error message that is sent to the client (if any).
    #[inline]
    pub fn kill_client(&self, client_id: ClientId, reason: DisconnectReason) {
        self.handle.kill_client(client_id.id, reason)
    }

    /// Creates a global of the specified interface and version and then advertises it to clients.
    ///
    /// The clients which the global is advertised to is determined by the implementation of the [`GlobalHandler`].
    ///
    /// **Panic:** This method will panic if the type parameter `D` is not same to the same type as the
    /// one the backend was initialized with.
    #[inline]
    pub fn create_global<D: 'static>(
        &self,
        interface: &'static Interface,
        version: u32,
        handler: Arc<dyn GlobalHandler<D>>,
    ) -> GlobalId {
        GlobalId { id: self.handle.create_global(interface, version, handler) }
    }

    /// Disables a global object that is currently active.
    ///
    /// The global removal will be signaled to all currently connected clients. New clients will not know of
    /// the global, but the associated state and callbacks will not be freed. As such, clients that still try
    /// to bind the global afterwards (because they have not yet realized it was removed) will succeed.
    ///
    /// Invoking this method on an already disabled or removed global does nothing. It is not possible to
    /// re-enable a disabled global, this method is meant to be invoked some time before actually removing
    /// the global, to avoid killing clients because of a race.
    ///
    /// **Panic:** This method will panic if the type parameter `D` is not same to the same type as the
    /// one the backend was initialized with.
    #[inline]
    pub fn disable_global<D: 'static>(&self, id: GlobalId) {
        self.handle.disable_global::<D>(id.id)
    }

    /// Removes a global object and free its ressources.
    ///
    /// The global object will no longer be considered valid by the server, clients trying to bind it will be
    /// killed, and the global ID is freed for re-use.
    ///
    /// It is advised to first disable a global and wait some amount of time before removing it, to ensure all
    /// clients are correctly aware of its removal. Note that clients will generally not expect globals that
    /// represent a capability of the server to be removed, as opposed to globals representing peripherals
    /// (like `wl_output` or `wl_seat`).
    ///
    /// This methods does nothing if the provided `GlobalId` corresponds to an already removed global.
    ///
    /// **Panic:** This method will panic if the type parameter `D` is not same to the same type as the
    /// one the backend was initialized with.
    #[inline]
    pub fn remove_global<D: 'static>(&self, id: GlobalId) {
        self.handle.remove_global::<D>(id.id)
    }

    /// Returns information about a global.
    #[inline]
    pub fn global_info(&self, id: GlobalId) -> Result<GlobalInfo, InvalidId> {
        self.handle.global_info(id.id)
    }

    /// Returns the handler which manages the visibility and notifies when a client has bound the global.
    #[inline]
    pub fn get_global_handler<D: 'static>(
        &self,
        id: GlobalId,
    ) -> Result<Arc<dyn GlobalHandler<D>>, InvalidId> {
        self.handle.get_global_handler(id.id)
    }

    /// Flushes pending events destined for a client.
    ///
    /// If no client is specified, all pending events are flushed to all clients.
    pub fn flush(&mut self, client: Option<ClientId>) -> std::io::Result<()> {
        self.handle.flush(client)
    }
}

/// A backend object that represents the state of a wayland server.
///
/// A backend is used to drive a wayland server by receiving requests, dispatching messages to the appropriate
/// handlers and flushes requests to be sent back to the client.
#[derive(Debug)]
pub struct Backend<D: 'static> {
    pub(crate) backend: server_impl::InnerBackend<D>,
}

impl<D> Backend<D> {
    /// Initialize a new Wayland backend
    #[inline]
    pub fn new() -> Result<Self, InitError> {
        Ok(Self { backend: server_impl::InnerBackend::new()? })
    }

    /// Flushes pending events destined for a client.
    ///
    /// If no client is specified, all pending events are flushed to all clients.
    #[inline]
    pub fn flush(&mut self, client: Option<ClientId>) -> std::io::Result<()> {
        self.backend.flush(client)
    }

    /// Returns a handle which represents the server side state of the backend.
    ///
    /// The handle provides a variety of functionality, such as querying information about wayland objects,
    /// obtaining data associated with a client and it's objects, and creating globals.
    #[inline]
    pub fn handle(&self) -> Handle {
        self.backend.handle()
    }

    /// Returns the underlying file descriptor.
    ///
    /// The file descriptor may be monitored for activity with a polling mechanism such as epoll or kqueue.
    /// When it becomes readable, this means there are pending messages that would be dispatched if you call
    /// [`Backend::dispatch_all_clients`].
    ///
    /// The file descriptor should not be used for any other purpose than monitoring it.
    #[inline]
    pub fn poll_fd(&self) -> BorrowedFd<'_> {
        self.backend.poll_fd()
    }

    /// Dispatches all pending messages from the specified client.
    ///
    /// This method will not block if there are no pending messages.
    ///
    /// The provided `data` will be provided to the handler of messages received from the client.
    ///
    /// For performance reasons, use of this function should be integrated with an event loop, monitoring
    /// the file descriptor associated with the client and only calling this method when messages are
    /// available.
    ///
    /// **Note:** This functionality is currently only available on the rust backend, invoking this method on
    /// the system backend will do the same as invoking
    /// [`Backend::dispatch_all_clients()`].
    #[inline]
    pub fn dispatch_single_client(
        &mut self,
        data: &mut D,
        client_id: ClientId,
    ) -> std::io::Result<usize> {
        self.backend.dispatch_client(data, client_id.id)
    }

    /// Dispatches all pending messages from all clients.
    ///
    /// This method will not block if there are no pending messages.
    ///
    /// The provided `data` will be provided to the handler of messages received from the clients.
    ///
    /// For performance reasons, use of this function should be integrated with an event loop, monitoring the
    /// file descriptor retrieved by [`Backend::poll_fd`] and only calling this method when messages are
    /// available.
    #[inline]
    pub fn dispatch_all_clients(&mut self, data: &mut D) -> std::io::Result<usize> {
        self.backend.dispatch_all_clients(data)
    }
}

// Workaround: Some versions of rustc throw a `struct is never constructed`-warning here,
// if the `server_system`-feature is enabled, even though the `rs`-module makes use if it.
#[allow(dead_code)]
pub(crate) struct DumbObjectData;

#[allow(dead_code)]
impl<D> ObjectData<D> for DumbObjectData {
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn request(
        self: Arc<Self>,
        _handle: &Handle,
        _data: &mut D,
        _client_id: ClientId,
        _msg: Message<ObjectId, OwnedFd>,
    ) -> Option<Arc<dyn ObjectData<D>>> {
        unreachable!()
    }

    #[cfg_attr(unstable_coverage, coverage(off))]
    fn destroyed(
        self: Arc<Self>,
        _handle: &Handle,
        _: &mut D,
        _client_id: ClientId,
        _object_id: ObjectId,
    ) {
    }
}
