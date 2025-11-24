//! # Integrating x11rb with an Event Loop
//!
//! To integrate x11rb with an event loop,
//! [`std::os::unix::io::AsRawFd`](https://doc.rust-lang.org/std/os/unix/io/trait.AsRawFd.html) is
//! implemented by [`RustConnection`](../rust_connection/struct.RustConnection.html)'s
//! [`DefaultStream`](../rust_connection/struct.DefaultStream.html#impl-AsRawFd) and
//! [`XCBConnection`](../xcb_ffi/struct.XCBConnection.html#impl-AsRawFd). This allows to integrate
//! with an event loop that also handles timeouts or network I/O. See
//! [`xclock_utc`](https://github.com/psychon/x11rb/blob/master/x11rb/examples/xclock_utc.rs) for an
//! example.
//!
//! The general form of such an integration could be as follows:
//! ```no_run
//! #[cfg(unix)]
//! use std::os::unix::io::{AsRawFd, RawFd};
//! #[cfg(windows)]
//! use std::os::windows::io::{AsRawSocket, RawSocket};
//! use x11rb::connection::Connection;
//! use x11rb::rust_connection::RustConnection;
//! use x11rb::errors::ConnectionError;
//!
//! fn main_loop(conn: &RustConnection) -> Result<(), ConnectionError> {
//!     #[cfg(unix)]
//!     let raw_handle = conn.stream().as_raw_fd();
//!     #[cfg(windows)]
//!     let raw_handle = conn.stream().as_raw_socket();
//!     loop {
//!         while let Some(event) = conn.poll_for_event()? {
//!             handle_event(event);
//!         }
//!
//!         poll_for_readable(raw_handle);
//!
//!         // Do other work here.
//!     }
//! }
//! # fn handle_event<T>(event: T) {}
//! # fn poll_for_readable<T>(event: T) {}
//! ```
//! The function `poll_for_readable` could wait for any number of I/O streams (besides the one from
//! x11rb) to become readable. It can also implement timeouts, as seen in the
//! [`xclock_utc` example](https://github.com/psychon/x11rb/blob/master/x11rb/examples/xclock_utc.rs).
//!
//!
//! ## Threads and Races
//!
//! Both [`RustConnection`](../rust_connection/struct.RustConnection.html) and
//! [`XCBConnection`](../xcb_ffi/struct.XCBConnection.html) are `Sync+Send`. However, it is still
//! possible to see races in the presence of threads and an event loop.
//!
//! The underlying problem is that the following two points are not equivalent:
//!
//! 1. A new event is available and can be returned from `conn.poll_for_event()`.
//! 2. The underlying I/O stream is readable.
//!
//! The reason for this is an internal buffer that is required: When an event is received from the
//! X11 server, but we are currently not in `conn.poll_for_event()`, then this event is added to an
//! internal buffer. Thus, it can happen that there is an event available, but the stream is not
//! readable.
//!
//! An example for such an other function is `conn.get_input_focus()?.reply()?`: The
//! `GetInputFocus` request is sent to the server and then `reply()` waits for the reply. It does
//! so by reading X11 packets from the X11 server until the right reply arrives. Any events that
//! are read during this are buffered internally in the `Connection`.
//!
//! If this race occurs, the main loop would sit in `poll_for_readable` and wait, while the already
//! buffered event is available. When something else wakes up the main loop and
//! `conn.poll_for_event()` is called the next time, the event is finally processed.
//!
//! There are two ways around this:
//!
//! 1. Only interact with x11rb from one thread.
//! 2. Use a dedicated thread for waiting for event.
//!
//! In case (1), one can call `conn.poll_for_event()` before waiting for the underlying I/O stream
//! to be readable. Since there are no other threads, nothing can read a new event from the stream
//! after `conn.poll_for_event()` returned `None`.
//!
//! Option (2) is to start a thread that calls `conn.wait_for_event()` in a loop. This is basically
//! a dedicated event loop for fetching events from the X11 server. All other threads can now
//! freely use the X11 connection without events possibly getting stuck and only being processed
//! later.
