use std::{
    env, fmt,
    io::ErrorKind,
    os::unix::io::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd},
    os::unix::net::UnixStream,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use wayland_backend::{
    client::{Backend, InvalidId, ObjectData, ObjectId, ReadEventsGuard, WaylandError},
    protocol::{ObjectInfo, ProtocolError},
};

use crate::{protocol::wl_display::WlDisplay, EventQueue, Proxy};

/// The Wayland connection
///
/// This is the main type representing your connection to the Wayland server, though most of the interaction
/// with the protocol are actually done using other types. The two main uses a simple app has for the
/// [`Connection`] are:
///
/// - Obtaining the initial [`WlDisplay`] through the [`display()`][Self::display()] method.
/// - Creating new [`EventQueue`]s with the [`new_event_queue()`][Self::new_event_queue()] method.
///
/// It can be created through the [`connect_to_env()`][Self::connect_to_env()] method to follow the
/// configuration from the environment (which is what you'll do most of the time), or using the
/// [`from_socket()`][Self::from_socket()] method if you retrieved your connected Wayland socket through
/// other means.
///
/// In case you need to plug yourself into an external Wayland connection that you don't control, you'll
/// likely get access to it as a [`Backend`], in which case you can create a [`Connection`] from it using
/// the [`from_backend()`][Self::from_backend()] method.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connection {
    pub(crate) backend: Backend,
}

impl Connection {
    /// Try to connect to the Wayland server following the environment
    ///
    /// This is the standard way to initialize a Wayland connection.
    pub fn connect_to_env() -> Result<Self, ConnectError> {
        let stream = if let Ok(txt) = env::var("WAYLAND_SOCKET") {
            // We should connect to the provided WAYLAND_SOCKET
            let fd = txt.parse::<i32>().map_err(|_| ConnectError::InvalidFd)?;
            let fd = unsafe { OwnedFd::from_raw_fd(fd) };
            // remove the variable so any child processes don't see it
            env::remove_var("WAYLAND_SOCKET");
            // set the CLOEXEC flag on this FD
            let flags = rustix::io::fcntl_getfd(&fd);
            let result = flags
                .map(|f| f | rustix::io::FdFlags::CLOEXEC)
                .and_then(|f| rustix::io::fcntl_setfd(&fd, f));
            match result {
                Ok(_) => {
                    // setting the O_CLOEXEC worked
                    UnixStream::from(fd)
                }
                Err(_) => {
                    // something went wrong in F_GETFD or F_SETFD
                    return Err(ConnectError::InvalidFd);
                }
            }
        } else {
            let socket_name = env::var_os("WAYLAND_DISPLAY")
                .map(Into::<PathBuf>::into)
                .ok_or(ConnectError::NoCompositor)?;

            let socket_path = if socket_name.is_absolute() {
                socket_name
            } else {
                let mut socket_path = env::var_os("XDG_RUNTIME_DIR")
                    .map(Into::<PathBuf>::into)
                    .ok_or(ConnectError::NoCompositor)?;
                if !socket_path.is_absolute() {
                    return Err(ConnectError::NoCompositor);
                }
                socket_path.push(socket_name);
                socket_path
            };

            UnixStream::connect(socket_path).map_err(|_| ConnectError::NoCompositor)?
        };

        let backend = Backend::connect(stream).map_err(|_| ConnectError::NoWaylandLib)?;
        Ok(Self { backend })
    }

    /// Initialize a Wayland connection from an already existing Unix stream
    pub fn from_socket(stream: UnixStream) -> Result<Self, ConnectError> {
        let backend = Backend::connect(stream).map_err(|_| ConnectError::NoWaylandLib)?;
        Ok(Self { backend })
    }

    /// Get the `WlDisplay` associated with this connection
    pub fn display(&self) -> WlDisplay {
        let display_id = self.backend.display_id();
        Proxy::from_id(self, display_id).unwrap()
    }

    /// Create a new event queue
    pub fn new_event_queue<State>(&self) -> EventQueue<State> {
        EventQueue::new(self.clone())
    }

    /// Wrap an existing [`Backend`] into a [`Connection`]
    pub fn from_backend(backend: Backend) -> Self {
        Self { backend }
    }

    /// Get the [`Backend`] underlying this [`Connection`]
    pub fn backend(&self) -> Backend {
        self.backend.clone()
    }

    /// Flush pending outgoing events to the server
    ///
    /// This needs to be done regularly to ensure the server receives all your requests, though several
    /// dispatching methods do it implicitly (this is stated in their documentation when they do).
    pub fn flush(&self) -> Result<(), WaylandError> {
        self.backend.flush()
    }

    /// Start a synchronized read from the socket
    ///
    /// This is needed if you plan to wait on readiness of the Wayland socket using an event loop. See
    /// [`ReadEventsGuard`] for details. Once the events are received, you'll then need to dispatch them from
    /// their event queues using [`EventQueue::dispatch_pending()`].
    ///
    /// If you don't need to manage multiple event sources, see
    /// [`EventQueue::blocking_dispatch()`] for a simpler mechanism.
    #[must_use]
    pub fn prepare_read(&self) -> Option<ReadEventsGuard> {
        self.backend.prepare_read()
    }

    /// Do a roundtrip to the server
    ///
    /// This method will block until the Wayland server has processed and answered all your
    /// preceding requests. This is notably useful during the initial setup of an app, to wait for
    /// the initial state from the server.
    ///
    /// See [`EventQueue::roundtrip()`] for a version that includes the dispatching of the event queue.
    pub fn roundtrip(&self) -> Result<usize, WaylandError> {
        let done = Arc::new(SyncData::default());
        let display = self.display();
        self.send_request(
            &display,
            crate::protocol::wl_display::Request::Sync {},
            Some(done.clone()),
        )
        .map_err(|_| WaylandError::Io(rustix::io::Errno::PIPE.into()))?;

        let mut dispatched = 0;

        loop {
            self.backend.flush()?;

            if let Some(guard) = self.backend.prepare_read() {
                dispatched += blocking_read(guard)?;
            } else {
                dispatched += self.backend.dispatch_inner_queue()?;
            }

            // see if the successful read included our callback
            if done.done.load(Ordering::Relaxed) {
                break;
            }
        }

        Ok(dispatched)
    }

    /// Retrieve the protocol error that occured on the connection if any
    ///
    /// If this method returns [`Some`], it means your Wayland connection is already dead.
    pub fn protocol_error(&self) -> Option<ProtocolError> {
        match self.backend.last_error()? {
            WaylandError::Protocol(err) => Some(err),
            WaylandError::Io(_) => None,
        }
    }

    /// Send a request associated with the provided object
    ///
    /// This is a low-level interface used by the code generated by `wayland-scanner`, you will likely
    /// instead use the methods of the types representing each interface, or the [`Proxy::send_request()`] and
    /// [`Proxy::send_constructor()`].
    pub fn send_request<I: Proxy>(
        &self,
        proxy: &I,
        request: I::Request<'_>,
        data: Option<Arc<dyn ObjectData>>,
    ) -> Result<ObjectId, InvalidId> {
        let (msg, child_spec) = proxy.write_request(self, request)?;
        let msg = msg.map_fd(|fd| fd.as_raw_fd());
        self.backend.send_request(msg, data, child_spec)
    }

    /// Get the protocol information related to given object ID
    pub fn object_info(&self, id: ObjectId) -> Result<ObjectInfo, InvalidId> {
        self.backend.info(id)
    }

    /// Get the object data for a given object ID
    ///
    /// This is a low-level interface used by the code generated by `wayland-scanner`, a higher-level
    /// interface for manipulating the user-data assocated to [`Dispatch`][crate::Dispatch] implementations
    /// is given as [`Proxy::data()`]. Also see [`Proxy::object_data()`].
    pub fn get_object_data(&self, id: ObjectId) -> Result<Arc<dyn ObjectData>, InvalidId> {
        self.backend.get_data(id)
    }
}

pub(crate) fn blocking_read(guard: ReadEventsGuard) -> Result<usize, WaylandError> {
    let fd = guard.connection_fd();
    let mut fds = [rustix::event::PollFd::new(
        &fd,
        rustix::event::PollFlags::IN | rustix::event::PollFlags::ERR,
    )];

    loop {
        match rustix::event::poll(&mut fds, None) {
            Ok(_) => break,
            Err(rustix::io::Errno::INTR) => continue,
            Err(e) => return Err(WaylandError::Io(e.into())),
        }
    }

    // at this point the fd is ready
    match guard.read() {
        Ok(n) => Ok(n),
        // if we are still "wouldblock", just return 0; the caller will retry.
        Err(WaylandError::Io(e)) if e.kind() == ErrorKind::WouldBlock => Ok(0),
        Err(e) => Err(e),
    }
}

/// An error when trying to establish a Wayland connection.
#[derive(Debug)]
pub enum ConnectError {
    /// The wayland library could not be loaded.
    NoWaylandLib,

    /// Could not find wayland compositor
    NoCompositor,

    /// `WAYLAND_SOCKET` was set but contained garbage
    InvalidFd,
}

impl std::error::Error for ConnectError {}

impl fmt::Display for ConnectError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ConnectError::NoWaylandLib => {
                write!(f, "The wayland library could not be loaded")
            }
            ConnectError::NoCompositor => {
                write!(f, "Could not find wayland compositor")
            }
            ConnectError::InvalidFd => {
                write!(f, "WAYLAND_SOCKET was set but contained garbage")
            }
        }
    }
}

impl AsFd for Connection {
    /// Provides fd from [`Backend::poll_fd()`] for polling.
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.backend.poll_fd()
    }
}

/*
    wl_callback object data for wl_display.sync
*/

#[derive(Default)]
pub(crate) struct SyncData {
    pub(crate) done: AtomicBool,
}

impl ObjectData for SyncData {
    fn event(
        self: Arc<Self>,
        _handle: &Backend,
        _msg: wayland_backend::protocol::Message<ObjectId, OwnedFd>,
    ) -> Option<Arc<dyn ObjectData>> {
        self.done.store(true, Ordering::Relaxed);
        None
    }

    fn destroyed(&self, _: ObjectId) {}
}
