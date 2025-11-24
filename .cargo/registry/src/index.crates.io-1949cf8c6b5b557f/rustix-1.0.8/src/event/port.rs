//! Solaris/illumos event ports.
//!
//! # Examples
//!
//! ```
//! # fn test() -> std::io::Result<()> {
//! use rustix::event::port;
//! use rustix::stdio::stdout;
//! use std::io;
//!
//! let some_fd = stdout();
//! let some_userdata = 7 as *mut _;
//!
//! // Create a port.
//! let port = port::create()?;
//!
//! // Associate `some_fd` with the port.
//! unsafe {
//!     port::associate_fd(&port, some_fd, port::PollFlags::IN, some_userdata)?;
//! }
//!
//! // Get a single event.
//! let event = port::get(&port, None)?;
//!
//! assert_eq!(event.userdata(), some_userdata);
//! # Ok(())
//! # }
//! ```

use crate::backend::c;
use crate::backend::event::syscalls;
use crate::buffer::Buffer;
use crate::fd::{AsFd, AsRawFd, OwnedFd};
use crate::timespec::Timespec;
use crate::{ffi, io};

pub use super::PollFlags;

/// The structure representing a port event.
#[repr(transparent)]
#[doc(alias = "port_event")]
pub struct Event(pub(crate) c::port_event);

impl Event {
    /// Get the events associated with this event.
    pub fn events(&self) -> i32 {
        self.0.portev_events
    }

    /// Get the event source associated with this event.
    pub fn object(&self) -> usize {
        self.0.portev_object
    }

    /// Get the userdata associated with this event.
    pub fn userdata(&self) -> *mut ffi::c_void {
        self.0.portev_user
    }
}

/// `port_create()`—Creates a new port.
///
/// # References
///  - [OpenSolaris]
///  - [illumos]
///
/// [OpenSolaris]: https://www.unix.com/man-page/opensolaris/3C/port_create/
/// [illumos]: https://illumos.org/man/3C/port_create
#[doc(alias = "port_create")]
pub fn create() -> io::Result<OwnedFd> {
    syscalls::port_create()
}

/// `port_associate(_, PORT_SOURCE_FD, _, _, _)`—Associates a file descriptor
/// with a port.
///
/// # Safety
///
/// Any `object`s passed into the `port` must be valid for the lifetime of the
/// `port`. Logically, `port` keeps a borrowed reference to the `object` until
/// it is removed via [`dissociate_fd`].
///
/// # References
///  - [OpenSolaris]
///  - [illumos]
///
/// [OpenSolaris]: https://www.unix.com/man-page/opensolaris/3C/port_associate/
/// [illumos]: https://illumos.org/man/3C/port_associate
#[doc(alias = "port_associate")]
pub unsafe fn associate_fd<Fd: AsFd, RawFd: AsRawFd>(
    port: Fd,
    object: RawFd,
    events: PollFlags,
    userdata: *mut ffi::c_void,
) -> io::Result<()> {
    syscalls::port_associate(
        port.as_fd(),
        c::PORT_SOURCE_FD,
        object.as_raw_fd() as _,
        events.bits() as _,
        userdata.cast(),
    )
}

/// `port_dissociate(_, PORT_SOURCE_FD, _)`—Dissociates a file descriptor
/// from a port.
///
/// # Safety
///
/// The file descriptor passed into this function must have been previously
/// associated with the port via [`associate_fd`].
///
/// # References
///  - [OpenSolaris]
///  - [illumos]
///
/// [OpenSolaris]: https://www.unix.com/man-page/opensolaris/3C/port_dissociate
/// [illumos]: https://illumos.org/man/3C/port_dissociate
#[doc(alias = "port_dissociate")]
pub unsafe fn dissociate_fd<Fd: AsFd, RawFd: AsRawFd>(port: Fd, object: RawFd) -> io::Result<()> {
    syscalls::port_dissociate(port.as_fd(), c::PORT_SOURCE_FD, object.as_raw_fd() as _)
}

/// `port_get(port, timeout)`—Gets an event from a port.
///
/// If an unsupported timeout is passed, this function fails with
/// [`io::Errno::INVAL`].
///
/// # References
///  - [OpenSolaris]
///  - [illumos]
///
/// [OpenSolaris]: https://www.unix.com/man-page/opensolaris/3C/port_get/
/// [illumos]: https://illumos.org/man/3C/port_get
#[doc(alias = "port_get")]
pub fn get<Fd: AsFd>(port: Fd, timeout: Option<&Timespec>) -> io::Result<Event> {
    syscalls::port_get(port.as_fd(), timeout)
}

/// `port_getn(port, events, min_events, timeout)`—Gets multiple events from
/// a port.
///
/// If `events` is empty, this does nothing and returns immediately.
///
/// To query the number of events without retrieving any, use [`getn_query`].
///
/// If an unsupported timeout is passed, this function fails with
/// [`io::Errno::INVAL`].
///
/// # References
///  - [OpenSolaris]
///  - [illumos]
///
/// [OpenSolaris]: https://www.unix.com/man-page/opensolaris/3C/port_getn/
/// [illumos]: https://illumos.org/man/3C/port_getn
#[doc(alias = "port_getn")]
pub fn getn<Fd: AsFd, Buf: Buffer<Event>>(
    port: Fd,
    mut events: Buf,
    min_events: u32,
    timeout: Option<&Timespec>,
) -> io::Result<Buf::Output> {
    // SAFETY: `port_getn` behaves.
    let nevents =
        unsafe { syscalls::port_getn(port.as_fd(), events.parts_mut(), min_events, timeout)? };
    // SAFETY: `port_getn` behaves.
    unsafe { Ok(events.assume_init(nevents)) }
}

/// `port_getn(port, NULL, 0, NULL)`—Queries the number of events
/// available from a port.
///
/// To retrieve the events, use [`getn`].
///
/// # References
///  - [OpenSolaris]
///  - [illumos]
///
/// [OpenSolaris]: https://www.unix.com/man-page/opensolaris/3C/port_getn/
/// [illumos]: https://illumos.org/man/3C/port_getn
#[doc(alias = "port_getn")]
pub fn getn_query<Fd: AsFd>(port: Fd) -> io::Result<u32> {
    syscalls::port_getn_query(port.as_fd())
}

/// `port_send(port, events, userdata)`—Sends an event to a port.
///
/// # References
///  - [OpenSolaris]
///  - [illumos]
///
/// [OpenSolaris]: https://www.unix.com/man-page/opensolaris/3C/port_send/
/// [illumos]: https://illumos.org/man/3C/port_send
#[doc(alias = "port_send")]
pub fn send<Fd: AsFd>(port: Fd, events: i32, userdata: *mut ffi::c_void) -> io::Result<()> {
    syscalls::port_send(port.as_fd(), events, userdata.cast())
}
