use std::{
    any::Any,
    fmt,
    os::unix::{
        io::{BorrowedFd, OwnedFd, RawFd},
        net::UnixStream,
    },
    sync::Arc,
};

#[cfg(doc)]
use std::io::ErrorKind::WouldBlock;

use crate::protocol::{Interface, Message, ObjectInfo};

use super::client_impl;

pub use crate::types::client::{InvalidId, NoWaylandLib, WaylandError};

/// A trait representing your data associated to an object
///
/// You will only be given access to it as a `&` reference, so you
/// need to handle interior mutability by yourself.
///
/// The methods of this trait will be invoked internally every time a
/// new object is created to initialize its data.
pub trait ObjectData: downcast_rs::DowncastSync {
    /// Dispatch an event for the associated object
    ///
    /// If the event has a `NewId` argument, the callback must return the object data
    /// for the newly created object
    fn event(
        self: Arc<Self>,
        backend: &Backend,
        msg: Message<ObjectId, OwnedFd>,
    ) -> Option<Arc<dyn ObjectData>>;

    /// Notification that the object has been destroyed and is no longer active
    fn destroyed(&self, object_id: ObjectId);

    /// Helper for forwarding a Debug implementation of your `ObjectData` type
    ///
    /// By default will just print `ObjectData { ... }`
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn debug(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectData").finish_non_exhaustive()
    }

    /// Helper for accessing user data
    ///
    /// This function is used to back the `Proxy::data()` function in `wayland_client`.  By default,
    /// it returns `self` (via [`Downcast`][downcast_rs::DowncastSync]), but this may be overridden to allow downcasting user data
    /// without needing to have access to the full type.
    fn data_as_any(&self) -> &dyn Any {
        self.as_any()
    }
}

impl std::fmt::Debug for dyn ObjectData {
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.debug(f)
    }
}

downcast_rs::impl_downcast!(sync ObjectData);

/// An ID representing a Wayland object
///
/// The backend internally tracks which IDs are still valid, invalidates them when the protocol object they
/// represent is destroyed. As such even though the Wayland protocol reuses IDs, you can confidently compare
/// two `ObjectId` for equality, they will only compare as equal if they both represent the same protocol
/// object.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ObjectId {
    pub(crate) id: client_impl::InnerObjectId,
}

impl fmt::Display for ObjectId {
    #[cfg_attr(unstable_coverage, coverage(off))]
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.id.fmt(f)
    }
}

impl fmt::Debug for ObjectId {
    #[cfg_attr(unstable_coverage, coverage(off))]
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.id.fmt(f)
    }
}

impl ObjectId {
    /// Check if this is a null ID
    ///
    /// **Note:** This is not the same as checking if the ID is still valid, which cannot be done without the
    /// [`Backend`]. A null ID is the ID equivalent of a null pointer: it never has been valid and never will
    /// be.
    #[inline]
    pub fn is_null(&self) -> bool {
        self.id.is_null()
    }

    /// Create a null object ID
    ///
    /// This object ID is always invalid, and should be used as placeholder in requests that create objects,
    /// or for request with an optional `Object` argument.
    ///
    /// See [`Backend::send_request()`] for details.
    #[inline]
    pub fn null() -> ObjectId {
        client_impl::InnerBackend::null_id()
    }

    /// Interface of the represented object
    #[inline]
    pub fn interface(&self) -> &'static Interface {
        self.id.interface()
    }

    /// Return the protocol-level numerical ID of this object
    ///
    /// Protocol IDs are reused after object destruction, so this should not be used as a unique identifier,
    /// instead use the [`ObjectId`] directly, it implements [`Clone`], [`PartialEq`], [`Eq`] and [`Hash`].
    #[inline]
    pub fn protocol_id(&self) -> u32 {
        self.id.protocol_id()
    }
}

/// A Wayland client backend
///
/// This type hosts all the interface for interacting with the wayland protocol. It can be
/// cloned, all clones refer to the same underlying connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Backend {
    pub(crate) backend: client_impl::InnerBackend,
}

/// A weak handle to a [`Backend`]
///
/// This handle behaves similarly to [`Weak`][std::sync::Weak], and can be used to keep access to
/// the backend without actually preventing it from being dropped.
#[derive(Clone, Debug)]
pub struct WeakBackend {
    inner: client_impl::WeakInnerBackend,
}

impl WeakBackend {
    /// Try to upgrade this weak handle to a [`Backend`]
    ///
    /// Returns [`None`] if the associated backend was already dropped.
    pub fn upgrade(&self) -> Option<Backend> {
        self.inner.upgrade().map(|backend| Backend { backend })
    }
}

impl Backend {
    /// Try to initialize a Wayland backend on the provided unix stream
    ///
    /// The provided stream should correspond to an already established unix connection with
    /// the Wayland server.
    ///
    /// This method can only fail on the `sys` backend if the `dlopen` cargo feature was enabled
    /// and the system wayland library could not be found.
    pub fn connect(stream: UnixStream) -> Result<Self, NoWaylandLib> {
        client_impl::InnerBackend::connect(stream).map(|backend| Self { backend })
    }

    /// Get a [`WeakBackend`] from this backend
    pub fn downgrade(&self) -> WeakBackend {
        WeakBackend { inner: self.backend.downgrade() }
    }

    /// Flush all pending outgoing requests to the server
    ///
    /// Most errors on this method mean that the Wayland connection is no longer valid, the only
    /// exception being an IO [`WouldBlock`] error. In that case it means that you should try flushing again
    /// later.
    ///
    /// You can however expect this method returning [`WouldBlock`] to be very rare: it can only occur if
    /// either your client sent a lot of big messages at once, or the server is very laggy.
    pub fn flush(&self) -> Result<(), WaylandError> {
        self.backend.flush()
    }

    /// Access the Wayland socket FD for polling
    #[inline]
    pub fn poll_fd(&self) -> BorrowedFd<'_> {
        self.backend.poll_fd()
    }

    /// Get the object ID for the `wl_display`
    #[inline]
    pub fn display_id(&self) -> ObjectId {
        self.backend.display_id()
    }

    /// Get the last error that occurred on this backend
    ///
    /// If this returns [`Some`], your Wayland connection is already dead.
    #[inline]
    pub fn last_error(&self) -> Option<WaylandError> {
        self.backend.last_error()
    }

    /// Get the detailed protocol information about a wayland object
    ///
    /// Returns an error if the provided object ID is no longer valid.
    #[inline]
    pub fn info(&self, id: ObjectId) -> Result<ObjectInfo, InvalidId> {
        self.backend.info(id)
    }

    /// Sends a request to the server
    ///
    /// Returns an error if the sender ID of the provided message is no longer valid.
    ///
    /// **Panic:**
    ///
    /// Several checks against the protocol specification are done, and this method will panic if they do
    /// not pass:
    ///
    /// - the message opcode must be valid for the sender interface
    /// - the argument list must match the prototype for the message associated with this opcode
    /// - if the method creates a new object, a [`ObjectId::null()`] must be given
    ///   in the argument list at the appropriate place, and a `child_spec` (interface and version)
    ///   can be provided. If one is provided, it'll be checked against the protocol spec. If the
    ///   protocol specification does not define the interface of the created object (notable example
    ///   is `wl_registry.bind`), the `child_spec` must be provided.
    pub fn send_request(
        &self,
        msg: Message<ObjectId, RawFd>,
        data: Option<Arc<dyn ObjectData>>,
        child_spec: Option<(&'static Interface, u32)>,
    ) -> Result<ObjectId, InvalidId> {
        self.backend.send_request(msg, data, child_spec)
    }

    /// Access the object data associated with a given object ID
    ///
    /// Returns an error if the object ID is not longer valid or if it corresponds to a Wayland
    /// object that is not managed by this backend (when multiple libraries share the same Wayland
    /// socket via `libwayland` if using the system backend).
    pub fn get_data(&self, id: ObjectId) -> Result<Arc<dyn ObjectData>, InvalidId> {
        self.backend.get_data(id)
    }

    /// Set the object data associated with a given object ID
    ///
    /// Returns an error if the object ID is not longer valid or if it corresponds to a Wayland
    /// object that is not managed by this backend (when multiple libraries share the same Wayland
    /// socket via `libwayland` if using the system backend).
    pub fn set_data(&self, id: ObjectId, data: Arc<dyn ObjectData>) -> Result<(), InvalidId> {
        self.backend.set_data(id, data)
    }

    /// Create a new reading guard
    ///
    /// This is the first step for actually reading events from the Wayland socket. See
    /// [`ReadEventsGuard`] for how to use it.
    ///
    /// This call will not block, but may return [`None`] if the inner queue of the backend needs to
    /// be dispatched. In which case you should invoke
    /// [`dispatch_inner_queue()`][Self::dispatch_inner_queue()].
    #[inline]
    #[must_use]
    pub fn prepare_read(&self) -> Option<ReadEventsGuard> {
        client_impl::InnerReadEventsGuard::try_new(self.backend.clone())
            .map(|guard| ReadEventsGuard { guard })
    }

    /// Dispatches the inner queue of this backend if necessary
    ///
    /// This function actually only does something when using the system backend. It dispaches an inner
    /// queue that the backend uses to wrap `libwayland`. While this dispatching is generally done in
    /// [`ReadEventsGuard::read()`], if multiple threads are interacting with the
    /// Wayland socket it can happen that this queue was filled by another thread. In that case
    /// [`prepare_read()`][Self::prepare_read()] will return [`None`], and you should invoke
    /// this function instead of using the [`ReadEventsGuard`]
    ///
    /// Returns the number of messages that were dispatched to their [`ObjectData`] callbacks.
    #[inline]
    pub fn dispatch_inner_queue(&self) -> Result<usize, WaylandError> {
        self.backend.dispatch_inner_queue()
    }
}

/// Guard for synchronizing event reading across multiple threads
///
/// If multiple threads need to read events from the Wayland socket concurrently,
/// it is necessary to synchronize their access. Failing to do so may cause some of the
/// threads to not be notified of new events, and sleep much longer than appropriate.
///
/// This guard is provided to ensure the proper synchronization is done. The guard is created using
/// the [`Backend::prepare_read()`] method. And the event reading is
/// triggered by consuming the guard using the [`ReadEventsGuard::read()`] method, synchronizing
/// with other threads as necessary so that only one of the threads will actually perform the socket read.
///
/// If you plan to poll the Wayland socket for readiness, the file descriptor can be retrieved via
/// the [`ReadEventsGuard::connection_fd()`] method. Note that for the synchronization to
/// correctly occur, you must *always* create the `ReadEventsGuard` *before* polling the socket.
///
/// Dropping the guard is valid and will cancel the prepared read.
#[derive(Debug)]
pub struct ReadEventsGuard {
    pub(crate) guard: client_impl::InnerReadEventsGuard,
}

impl ReadEventsGuard {
    /// Access the Wayland socket FD for polling
    #[inline]
    pub fn connection_fd(&self) -> BorrowedFd<'_> {
        self.guard.connection_fd()
    }

    /// Attempt to read events from the Wayland socket
    ///
    /// If multiple threads have a live reading guard, this method will block until all of them
    /// are either dropped or have their `read()` method invoked, at which point one of the threads
    /// will read events from the socket and invoke the callbacks for the received events. All
    /// threads will then resume their execution.
    ///
    /// This returns the number of dispatched events, or `0` if an other thread handled the dispatching.
    /// If no events are available to read from the socket, this returns a [`WouldBlock`] IO error.
    #[inline]
    pub fn read(self) -> Result<usize, WaylandError> {
        self.guard.read()
    }
}
pub(crate) struct DumbObjectData;

impl ObjectData for DumbObjectData {
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn event(
        self: Arc<Self>,
        _handle: &Backend,
        _msg: Message<ObjectId, OwnedFd>,
    ) -> Option<Arc<dyn ObjectData>> {
        unreachable!()
    }

    #[cfg_attr(unstable_coverage, coverage(off))]
    fn destroyed(&self, _object_id: ObjectId) {
        unreachable!()
    }
}

pub(crate) struct UninitObjectData;

impl ObjectData for UninitObjectData {
    #[cfg_attr(unstable_coverage, coverage(off))]
    fn event(
        self: Arc<Self>,
        _handle: &Backend,
        msg: Message<ObjectId, OwnedFd>,
    ) -> Option<Arc<dyn ObjectData>> {
        panic!("Received a message on an uninitialized object: {msg:?}");
    }

    #[cfg_attr(unstable_coverage, coverage(off))]
    fn destroyed(&self, _object_id: ObjectId) {}

    #[cfg_attr(unstable_coverage, coverage(off))]
    fn debug(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UninitObjectData").finish()
    }
}
