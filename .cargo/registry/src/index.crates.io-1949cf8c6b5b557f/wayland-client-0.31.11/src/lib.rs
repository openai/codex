//! Interface for interacting with the Wayland protocol, client-side.
//!
//! ## General concepts
//!
//! This crate is structured around four main objects: the [`Connection`] and [`EventQueue`] structs,
//! proxies (objects implementing the [`Proxy`] trait), and the [`Dispatch`] trait.
//!
//! The [`Connection`] is the heart of this crate. It represents your connection to the Wayland server, and
//! you'll generally initialize it using the [`Connection::connect_to_env()`] method, which will
//! attempt to open a Wayland connection following the configuration specified by the ! environment.
//!
//! Once you have a [`Connection`], you can create an [`EventQueue`] from it. This [`EventQueue`] will take
//! care of processing events from the Wayland server and delivering them to your processing logic, in the form
//! of a state struct with several [`Dispatch`] implementations (see below).
//!
//! Each of the Wayland objects you can manipulate is represented by a struct implementing the [`Proxy`]
//! trait. Those structs are automatically generated from the wayland XML protocol specification. This crate
//! provides the types generated from the core protocol in the [`protocol`] module. For other standard
//! protocols, see the `wayland-protocols` crate.
//!
//! ## Event dispatching
//!
//! The core event dispatching logic provided by this crate is built around the [`EventQueue`] struct. In
//! this paradigm, receiving and processing events is a two-step process:
//!
//! - First, events are read from the Wayland socket. For each event, the backend figures out which [`EventQueue`]
//!   manages it, and enqueues the event in an internal buffer of that queue.
//! - Then, the [`EventQueue`] empties its internal buffer by sequentially invoking the appropriate
//!   [`Dispatch::event()`] method on the `State` value that was provided to it.
//!
//! The main goal of this structure is to make your `State` accessible without synchronization to most of
//! your event-processing logic, to reduce the plumbing costs. See [`EventQueue`]'s documentation for
//! explanations of how to use it to drive your event loop, and when and how to use multiple
//! event queues in your app.
//!
//! ### The [`Dispatch`] trait and dispatch delegation
//!
//! In this paradigm, your `State` needs to implement `Dispatch<O, _>` for every Wayland object `O` it needs to
//! process events for. This is ensured by the fact that, whenever creating an object using the methods on
//! an other object, you need to pass a [`QueueHandle<State>`] from the [`EventQueue`] that will be
//! managing the newly created object.
//!
//! However, implementing all those traits on your own is a lot of (often uninteresting) work. To make this
//! easier a composition mechanism is provided using the [`delegate_dispatch!`] macro. This way, another
//! library (such as Smithay's Client Toolkit) can provide generic [`Dispatch`] implementations that you
//! can reuse in your own app by delegating those objects to that provided implementation. See the
//! documentation of those traits and macro for details.
//!
//! ## Getting started example
//!
//! As an overview of how this crate is used, here is a commented example of a program that connects to the
//! Wayland server and lists the globals this server advertised through the `wl_registry`:
//!
//! ```rust,no_run
//! use wayland_client::{protocol::wl_registry, Connection, Dispatch, QueueHandle};
//! // This struct represents the state of our app. This simple app does not
//! // need any state, but this type still supports the `Dispatch` implementations.
//! struct AppData;
//!
//! // Implement `Dispatch<WlRegistry, ()> for our state. This provides the logic
//! // to be able to process events for the wl_registry interface.
//! //
//! // The second type parameter is the user-data of our implementation. It is a
//! // mechanism that allows you to associate a value to each particular Wayland
//! // object, and allow different dispatching logic depending on the type of the
//! // associated value.
//! //
//! // In this example, we just use () as we don't have any value to associate. See
//! // the `Dispatch` documentation for more details about this.
//! impl Dispatch<wl_registry::WlRegistry, ()> for AppData {
//!     fn event(
//!         _state: &mut Self,
//!         _: &wl_registry::WlRegistry,
//!         event: wl_registry::Event,
//!         _: &(),
//!         _: &Connection,
//!         _: &QueueHandle<AppData>,
//!     ) {
//!         // When receiving events from the wl_registry, we are only interested in the
//!         // `global` event, which signals a new available global.
//!         // When receiving this event, we just print its characteristics in this example.
//!         if let wl_registry::Event::Global { name, interface, version } = event {
//!             println!("[{}] {} (v{})", name, interface, version);
//!         }
//!     }
//! }
//!
//! // The main function of our program
//! fn main() {
//!     // Create a Wayland connection by connecting to the server through the
//!     // environment-provided configuration.
//!     let conn = Connection::connect_to_env().unwrap();
//!
//!     // Retrieve the WlDisplay Wayland object from the connection. This object is
//!     // the starting point of any Wayland program, from which all other objects will
//!     // be created.
//!     let display = conn.display();
//!
//!     // Create an event queue for our event processing
//!     let mut event_queue = conn.new_event_queue();
//!     // And get its handle to associate new objects to it
//!     let qh = event_queue.handle();
//!
//!     // Create a wl_registry object by sending the wl_display.get_registry request.
//!     // This method takes two arguments: a handle to the queue that the newly created
//!     // wl_registry will be assigned to, and the user-data that should be associated
//!     // with this registry (here it is () as we don't need user-data).
//!     let _registry = display.get_registry(&qh, ());
//!
//!     // At this point everything is ready, and we just need to wait to receive the events
//!     // from the wl_registry. Our callback will print the advertised globals.
//!     println!("Advertised globals:");
//!
//!     // To actually receive the events, we invoke the `roundtrip` method. This method
//!     // is special and you will generally only invoke it during the setup of your program:
//!     // it will block until the server has received and processed all the messages you've
//!     // sent up to now.
//!     //
//!     // In our case, that means it'll block until the server has received our
//!     // wl_display.get_registry request, and as a reaction has sent us a batch of
//!     // wl_registry.global events.
//!     //
//!     // `roundtrip` will then empty the internal buffer of the queue it has been invoked
//!     // on, and thus invoke our `Dispatch` implementation that prints the list of advertised
//!     // globals.
//!     event_queue.roundtrip(&mut AppData).unwrap();
//! }
//! ```
//!
//! ## Advanced use
//!
//! ### Bypassing [`Dispatch`]
//!
//! It may be that for some of your objects, handling them via the [`EventQueue`] is impractical. For example,
//! if processing the events from those objects doesn't require accessing some global state, and/or you need to
//! handle them in a context where cranking an event loop is impractical.
//!
//! In those contexts, this crate also provides some escape hatches to directly interface with the low-level
//! APIs from `wayland-backend`, allowing you to register callbacks for those objects that will be invoked
//! whenever they receive an event and *any* event queue from the program is being dispatched. Those
//! callbacks are more constrained: they don't get a `&mut State` reference, and must be threadsafe. See
//! [`Proxy::send_constructor()`] and [`ObjectData`] for details about how to
//! assign such callbacks to objects.
//!
//! ### Interaction with FFI
//!
//! It can happen that you'll need to interact with Wayland states accross FFI. A typical example would be if
//! you need to use the [`raw-window-handle`](https://docs.rs/raw-window-handle/) crate.
//!
//! In this case, you'll need to do it in two steps, by explicitly working with `wayland-backend`, adding
//! it to your dependencies and enabling its `client_system` feature.
//!
//! - If you need to send pointers to FFI, you can retrive the `*mut wl_proxy` pointers from the proxies by
//!   first getting the [`ObjectId`] using the [`Proxy::id()`] method, and then
//!   using the [`ObjectId::as_ptr()`] method.
//  - If you need to receive pointers from FFI, you need to first create a
//    [`Backend`][backend::Backend] from the `*mut wl_display` using
//    [`Backend::from_external_display()`][backend::Backend::from_foreign_display()], and then
//    make it into a [`Connection`] using [`Connection::from_backend()`]. Similarly, you can make
//    [`ObjectId`]s from the `*mut wl_proxy` pointers using [`ObjectId::from_ptr()`], and then make
//    the proxies using [`Proxy::from_id()`].

#![allow(clippy::needless_doctest_main)]
#![warn(missing_docs, missing_debug_implementations)]
#![forbid(improper_ctypes, unsafe_op_in_unsafe_fn)]
#![cfg_attr(unstable_coverage, feature(coverage_attribute))]
// Doc feature labels can be tested locally by running RUSTDOCFLAGS="--cfg=docsrs" cargo +nightly doc -p <crate>
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

use std::{
    fmt,
    hash::{Hash, Hasher},
    os::unix::io::{BorrowedFd, OwnedFd},
    sync::Arc,
};
use wayland_backend::{
    client::{InvalidId, ObjectData, ObjectId, WaylandError, WeakBackend},
    protocol::{Interface, Message},
};

mod conn;
mod event_queue;
pub mod globals;

/// Backend reexports
pub mod backend {
    pub use wayland_backend::client::{
        Backend, InvalidId, NoWaylandLib, ObjectData, ObjectId, ReadEventsGuard, WaylandError,
        WeakBackend,
    };
    pub use wayland_backend::protocol;
    pub use wayland_backend::smallvec;
}

pub use wayland_backend::protocol::WEnum;

pub use conn::{ConnectError, Connection};
pub use event_queue::{Dispatch, EventQueue, QueueFreezeGuard, QueueHandle, QueueProxyData};

// internal imports for dispatching logging depending on the `log` feature
#[cfg(feature = "log")]
#[allow(unused_imports)]
use log::{debug as log_debug, error as log_error, info as log_info, warn as log_warn};
#[cfg(not(feature = "log"))]
#[allow(unused_imports)]
use std::{
    eprintln as log_error, eprintln as log_warn, eprintln as log_info, eprintln as log_debug,
};

/// Generated protocol definitions
///
/// This module is automatically generated from the `wayland.xml` protocol specification,
/// and contains the interface definitions for the core Wayland protocol.
#[allow(missing_docs)]
pub mod protocol {
    use self::__interfaces::*;
    use crate as wayland_client;
    pub mod __interfaces {
        wayland_scanner::generate_interfaces!("wayland.xml");
    }
    wayland_scanner::generate_client_code!("wayland.xml");
}

/// Trait representing a Wayland interface
pub trait Proxy: Clone + std::fmt::Debug + Sized {
    /// The event enum for this interface
    type Event;
    /// The request enum for this interface
    type Request<'a>;

    /// The interface description
    fn interface() -> &'static Interface;

    /// The ID of this object
    fn id(&self) -> ObjectId;

    /// The version of this object
    fn version(&self) -> u32;

    /// Checks if the Wayland object associated with this proxy is still alive
    fn is_alive(&self) -> bool {
        if let Some(backend) = self.backend().upgrade() {
            backend.info(self.id()).is_ok()
        } else {
            false
        }
    }

    /// Access the user-data associated with this object
    fn data<U: Send + Sync + 'static>(&self) -> Option<&U>;

    /// Access the raw data associated with this object.
    ///
    /// For objects created using the scanner-generated methods, this will be an instance of the
    /// [`QueueProxyData`] type.
    fn object_data(&self) -> Option<&Arc<dyn ObjectData>>;

    /// Access the backend associated with this object
    fn backend(&self) -> &backend::WeakBackend;

    /// Create an object proxy from its ID
    ///
    /// Returns an error this the provided object ID does not correspond to
    /// the `Self` interface.
    ///
    /// **Note:** This method is mostly meant as an implementation detail to be
    /// used by code generated by wayland-scanner.
    fn from_id(conn: &Connection, id: ObjectId) -> Result<Self, InvalidId>;

    /// Create an inert object proxy
    ///
    /// **Note:** This method is mostly meant as an implementation detail to be
    /// used by code generated by wayland-scanner.
    fn inert(backend: backend::WeakBackend) -> Self;

    /// Send a request for this object.
    ///
    /// It is an error to use this function on requests that create objects; use
    /// [`send_constructor()`][Self::send_constructor()] for such requests.
    fn send_request(&self, req: Self::Request<'_>) -> Result<(), InvalidId>;

    /// Send a request for this object that creates another object.
    ///
    /// It is an error to use this function on requests that do not create objects; use
    /// [`send_request()`][Self::send_request()] for such requests.
    fn send_constructor<I: Proxy>(
        &self,
        req: Self::Request<'_>,
        data: Arc<dyn ObjectData>,
    ) -> Result<I, InvalidId>;

    /// Parse a event for this object
    ///
    /// **Note:** This method is mostly meant as an implementation detail to be
    /// used by code generated by wayland-scanner.
    fn parse_event(
        conn: &Connection,
        msg: Message<ObjectId, OwnedFd>,
    ) -> Result<(Self, Self::Event), DispatchError>;

    /// Serialize a request for this object
    ///
    /// **Note:** This method is mostly meant as an implementation detail to be
    /// used by code generated by wayland-scanner.
    #[allow(clippy::type_complexity)]
    fn write_request<'a>(
        &self,
        conn: &Connection,
        req: Self::Request<'a>,
    ) -> Result<(Message<ObjectId, BorrowedFd<'a>>, Option<(&'static Interface, u32)>), InvalidId>;

    /// Creates a weak handle to this object
    ///
    /// This weak handle will not keep the user-data associated with the object alive,
    /// and can be converted back to a full proxy using [`Weak::upgrade()`].
    ///
    /// This can be of use if you need to store proxies in the used data of other objects and want
    /// to be sure to avoid reference cycles that would cause memory leaks.
    fn downgrade(&self) -> Weak<Self> {
        Weak { backend: self.backend().clone(), id: self.id(), _iface: std::marker::PhantomData }
    }
}

/// Wayland dispatching error
#[derive(Debug)]
pub enum DispatchError {
    /// The received message does not match the specification for the object's interface.
    BadMessage {
        /// The id of the target object
        sender_id: ObjectId,
        /// The interface of the target object
        interface: &'static str,
        /// The opcode number
        opcode: u16,
    },
    /// The backend generated an error
    Backend(WaylandError),
}

impl std::error::Error for DispatchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DispatchError::BadMessage { .. } => Option::None,
            DispatchError::Backend(source) => Some(source),
        }
    }
}

impl fmt::Display for DispatchError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DispatchError::BadMessage { sender_id, interface, opcode } => {
                write!(f, "Bad message for object {interface}@{sender_id} on opcode {opcode}")
            }
            DispatchError::Backend(source) => {
                write!(f, "Backend error: {source}")
            }
        }
    }
}

impl From<WaylandError> for DispatchError {
    fn from(source: WaylandError) -> Self {
        DispatchError::Backend(source)
    }
}

/// A weak handle to a Wayland object
///
/// This handle does not keep the underlying user data alive, and can be converted back to a full proxy
/// using [`Weak::upgrade()`].
#[derive(Debug, Clone)]
pub struct Weak<I> {
    backend: WeakBackend,
    id: ObjectId,
    _iface: std::marker::PhantomData<I>,
}

impl<I: Proxy> Weak<I> {
    /// Try to upgrade with weak handle back into a full proxy.
    ///
    /// This will fail if either:
    /// - the object represented by this handle has already been destroyed at the protocol level
    /// - the Wayland connection has already been closed
    pub fn upgrade(&self) -> Result<I, InvalidId> {
        let backend = self.backend.upgrade().ok_or(InvalidId)?;
        // Check if the object has been destroyed
        backend.info(self.id.clone())?;
        let conn = Connection::from_backend(backend);
        I::from_id(&conn, self.id.clone())
    }

    /// The underlying [`ObjectId`]
    pub fn id(&self) -> ObjectId {
        self.id.clone()
    }
}

impl<I> PartialEq for Weak<I> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<I> Eq for Weak<I> {}

impl<I> Hash for Weak<I> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl<I: Proxy> PartialEq<I> for Weak<I> {
    fn eq(&self, other: &I) -> bool {
        self.id == other.id()
    }
}
