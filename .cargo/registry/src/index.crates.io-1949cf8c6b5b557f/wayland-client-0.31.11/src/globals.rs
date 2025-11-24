//! Helpers for handling the initialization of an app
//!
//! At the startup of your Wayland app, the initial step is generally to retrieve the list of globals
//! advertized by the compositor from the registry. Using the [`Dispatch`] mechanism for this task can be
//! very unpractical, this is why this module provides a special helper for handling the registry.
//!
//! The entry point of this helper is the [`registry_queue_init`] function. Given a reference to your
//! [`Connection`] it will create an [`EventQueue`], retrieve the initial list of globals, and register a
//! handler using your provided `Dispatch<WlRegistry,_>` implementation for handling dynamic registry events.
//!
//! ## Example
//!
//! ```no_run
//! use wayland_client::{
//!     Connection, Dispatch, QueueHandle,
//!     globals::{registry_queue_init, Global, GlobalListContents},
//!     protocol::{wl_registry, wl_compositor},
//! };
//! # use std::sync::Mutex;
//! # struct State;
//!
//! // You need to provide a Dispatch<WlRegistry, GlobalListContents> impl for your app
//! impl wayland_client::Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
//!     fn event(
//!         state: &mut State,
//!         proxy: &wl_registry::WlRegistry,
//!         event: wl_registry::Event,
//!         // This mutex contains an up-to-date list of the currently known globals
//!         // including the one that was just added or destroyed
//!         data: &GlobalListContents,
//!         conn: &Connection,
//!         qhandle: &QueueHandle<State>,
//!     ) {
//!         /* react to dynamic global events here */
//!     }
//! }
//!
//! let conn = Connection::connect_to_env().unwrap();
//! let (globals, queue) = registry_queue_init::<State>(&conn).unwrap();
//!
//! # impl wayland_client::Dispatch<wl_compositor::WlCompositor, ()> for State {
//! #     fn event(
//! #         state: &mut State,
//! #         proxy: &wl_compositor::WlCompositor,
//! #         event: wl_compositor::Event,
//! #         data: &(),
//! #         conn: &Connection,
//! #         qhandle: &QueueHandle<State>,
//! #     ) {}
//! # }
//! // now you can bind the globals you need for your app
//! let compositor: wl_compositor::WlCompositor = globals.bind(&queue.handle(), 4..=5, ()).unwrap();
//! ```

use std::{
    fmt,
    ops::RangeInclusive,
    os::unix::io::OwnedFd,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use wayland_backend::{
    client::{Backend, InvalidId, ObjectData, ObjectId, WaylandError},
    protocol::Message,
};

use crate::{
    protocol::{wl_display, wl_registry},
    Connection, Dispatch, EventQueue, Proxy, QueueHandle,
};

/// Initialize a new event queue with its associated registry and retrieve the initial list of globals
///
/// See [the module level documentation][self] for more.
pub fn registry_queue_init<State>(
    conn: &Connection,
) -> Result<(GlobalList, EventQueue<State>), GlobalError>
where
    State: Dispatch<wl_registry::WlRegistry, GlobalListContents> + 'static,
{
    let event_queue = conn.new_event_queue();
    let display = conn.display();
    let data = Arc::new(RegistryState {
        globals: GlobalListContents { contents: Default::default() },
        handle: event_queue.handle(),
        initial_roundtrip_done: AtomicBool::new(false),
    });
    let registry = display.send_constructor(wl_display::Request::GetRegistry {}, data.clone())?;
    // We don't need to dispatch the event queue as for now nothing will be sent to it
    conn.roundtrip()?;
    data.initial_roundtrip_done.store(true, Ordering::Relaxed);
    Ok((GlobalList { registry }, event_queue))
}

/// A helper for global initialization.
///
/// See [the module level documentation][self] for more.
#[derive(Debug)]
pub struct GlobalList {
    registry: wl_registry::WlRegistry,
}

impl GlobalList {
    /// Access the contents of the list of globals
    pub fn contents(&self) -> &GlobalListContents {
        self.registry.data::<GlobalListContents>().unwrap()
    }

    /// Binds a global, returning a new protocol object associated with the global.
    ///
    /// The `version` specifies the range of versions that should be bound. This function will guarantee the
    /// version of the returned protocol object is the lower of the maximum requested version and the advertised
    /// version.
    ///
    /// If the lower bound of the `version` is less than the version advertised by the server, then
    /// [`BindError::UnsupportedVersion`] is returned.
    ///
    /// ## Multi-instance/Device globals.
    ///
    /// This function is not intended to be used with globals that have multiple instances such as `wl_output`
    /// and `wl_seat`. These types of globals need their own initialization mechanism because these
    /// multi-instance globals may be removed at runtime. To handle then, you should instead rely on the
    /// `Dispatch` implementation for `WlRegistry` of your `State`.
    ///
    /// # Panics
    ///
    /// This function will panic if the maximum requested version is greater than the known maximum version of
    /// the interface. The known maximum version is determined by the code generated using wayland-scanner.
    pub fn bind<I, State, U>(
        &self,
        qh: &QueueHandle<State>,
        version: RangeInclusive<u32>,
        udata: U,
    ) -> Result<I, BindError>
    where
        I: Proxy + 'static,
        State: Dispatch<I, U> + 'static,
        U: Send + Sync + 'static,
    {
        let version_start = *version.start();
        let version_end = *version.end();
        let interface = I::interface();

        if *version.end() > interface.version {
            // This is a panic because it's a compile-time programmer error, not a runtime error.
            panic!("Maximum version ({}) of {} was higher than the proxy's maximum version ({}); outdated wayland XML files?",
                version.end(), interface.name, interface.version);
        }

        let globals = &self.registry.data::<GlobalListContents>().unwrap().contents;
        let guard = globals.lock().unwrap();
        let (name, version) = guard
            .iter()
            // Find the with the correct interface
            .filter_map(|Global { name, interface: interface_name, version }| {
                // TODO: then_some
                if interface.name == &interface_name[..] {
                    Some((*name, *version))
                } else {
                    None
                }
            })
            .next()
            .ok_or(BindError::NotPresent)?;

        // Test version requirements
        if version < version_start {
            return Err(BindError::UnsupportedVersion);
        }

        // To get the version to bind, take the lower of the version advertised by the server and the maximum
        // requested version.
        let version = version.min(version_end);

        Ok(self.registry.bind(name, version, qh, udata))
    }

    /// Returns the [`WlRegistry`][wl_registry] protocol object.
    ///
    /// This may be used if more direct control when creating globals is needed.
    pub fn registry(&self) -> &wl_registry::WlRegistry {
        &self.registry
    }
}

/// An error that may occur when initializing the global list.
#[derive(Debug)]
pub enum GlobalError {
    /// The backend generated an error
    Backend(WaylandError),

    /// An invalid object id was acted upon.
    InvalidId(InvalidId),
}

impl std::error::Error for GlobalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            GlobalError::Backend(source) => Some(source),
            GlobalError::InvalidId(source) => std::error::Error::source(source),
        }
    }
}

impl std::fmt::Display for GlobalError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GlobalError::Backend(source) => {
                write!(f, "Backend error: {source}")
            }
            GlobalError::InvalidId(source) => write!(f, "{source}"),
        }
    }
}

impl From<WaylandError> for GlobalError {
    fn from(source: WaylandError) -> Self {
        GlobalError::Backend(source)
    }
}

impl From<InvalidId> for GlobalError {
    fn from(source: InvalidId) -> Self {
        GlobalError::InvalidId(source)
    }
}

/// An error that occurs when a binding a global fails.
#[derive(Debug)]
pub enum BindError {
    /// The requested version of the global is not supported.
    UnsupportedVersion,

    /// The requested global was not found in the registry.
    NotPresent,
}

impl std::error::Error for BindError {}

impl fmt::Display for BindError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            BindError::UnsupportedVersion => {
                write!(f, "the requested version of the global is not supported")
            }
            BindError::NotPresent => {
                write!(f, "the requested global was not found in the registry")
            }
        }
    }
}

/// Description of a global.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Global {
    /// The name of the global.
    ///
    /// This is an identifier used by the server to reference some specific global.
    pub name: u32,
    /// The interface of the global.
    ///
    /// This describes what type of protocol object the global is.
    pub interface: String,
    /// The advertised version of the global.
    ///
    /// This specifies the maximum version of the global that may be bound. This means any lower version of
    /// the global may be bound.
    pub version: u32,
}

/// A container representing the current contents of the list of globals
#[derive(Debug)]
pub struct GlobalListContents {
    contents: Mutex<Vec<Global>>,
}

impl GlobalListContents {
    /// Access the list of globals
    ///
    /// Your closure is invoked on the global list, and its return value is forwarded to the return value
    /// of this function. This allows you to process the list without making a copy.
    pub fn with_list<T, F: FnOnce(&[Global]) -> T>(&self, f: F) -> T {
        let guard = self.contents.lock().unwrap();
        f(&guard)
    }

    /// Get a copy of the contents of the list of globals.
    pub fn clone_list(&self) -> Vec<Global> {
        self.contents.lock().unwrap().clone()
    }
}

struct RegistryState<State> {
    globals: GlobalListContents,
    handle: QueueHandle<State>,
    initial_roundtrip_done: AtomicBool,
}

impl<State: 'static> ObjectData for RegistryState<State>
where
    State: Dispatch<wl_registry::WlRegistry, GlobalListContents>,
{
    fn event(
        self: Arc<Self>,
        backend: &Backend,
        msg: Message<ObjectId, OwnedFd>,
    ) -> Option<Arc<dyn ObjectData>> {
        let conn = Connection::from_backend(backend.clone());

        // The registry messages don't contain any fd, so use some type trickery to
        // clone the message
        #[derive(Debug, Clone)]
        enum Void {}
        let msg: Message<ObjectId, Void> = msg.map_fd(|_| unreachable!());
        let to_forward = if self.initial_roundtrip_done.load(Ordering::Relaxed) {
            Some(msg.clone().map_fd(|v| match v {}))
        } else {
            None
        };
        // and restore the type
        let msg = msg.map_fd(|v| match v {});

        // Can't do much if the server sends a malformed message
        if let Ok((_, event)) = wl_registry::WlRegistry::parse_event(&conn, msg) {
            match event {
                wl_registry::Event::Global { name, interface, version } => {
                    let mut guard = self.globals.contents.lock().unwrap();
                    guard.push(Global { name, interface, version });
                }

                wl_registry::Event::GlobalRemove { name: remove } => {
                    let mut guard = self.globals.contents.lock().unwrap();
                    guard.retain(|Global { name, .. }| name != &remove);
                }
            }
        };

        if let Some(msg) = to_forward {
            // forward the message to the event queue as normal
            self.handle
                .inner
                .lock()
                .unwrap()
                .enqueue_event::<wl_registry::WlRegistry, GlobalListContents>(msg, self.clone())
        }

        // We do not create any objects in this event handler.
        None
    }

    fn destroyed(&self, _id: ObjectId) {
        // A registry cannot be destroyed unless disconnected.
    }

    fn data_as_any(&self) -> &dyn std::any::Any {
        &self.globals
    }
}
