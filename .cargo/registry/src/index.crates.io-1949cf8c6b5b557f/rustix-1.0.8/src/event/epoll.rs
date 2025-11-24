//! Linux `epoll` support.
//!
//! # Examples
//!
//! ```no_run
//! # #[cfg(feature = "net")]
//! # fn main() -> std::io::Result<()> {
//! use rustix::buffer::spare_capacity;
//! use rustix::event::epoll;
//! use rustix::fd::AsFd;
//! use rustix::io::{ioctl_fionbio, read, write};
//! use rustix::net::{
//!     accept, bind, listen, socket, AddressFamily, Ipv4Addr, SocketAddrV4, SocketType,
//! };
//! use std::collections::HashMap;
//! use std::os::unix::io::AsRawFd;
//!
//! // Create a socket and listen on it.
//! let listen_sock = socket(AddressFamily::INET, SocketType::STREAM, None)?;
//! bind(&listen_sock, &SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))?;
//! listen(&listen_sock, 1)?;
//!
//! // Create an epoll object. Using `Owning` here means the epoll object will
//! // take ownership of the file descriptors registered with it.
//! let epoll = epoll::create(epoll::CreateFlags::CLOEXEC)?;
//!
//! // Register the socket with the epoll object.
//! epoll::add(
//!     &epoll,
//!     &listen_sock,
//!     epoll::EventData::new_u64(1),
//!     epoll::EventFlags::IN,
//! )?;
//!
//! // Keep track of the sockets we've opened.
//! let mut next_id = epoll::EventData::new_u64(2);
//! let mut sockets = HashMap::new();
//!
//! // Process events.
//! let mut event_list = Vec::with_capacity(4);
//! loop {
//!     epoll::wait(&epoll, spare_capacity(&mut event_list), None)?;
//!     for event in event_list.drain(..) {
//!         let target = event.data;
//!         if target.u64() == 1 {
//!             // Accept a new connection, set it to non-blocking, and
//!             // register to be notified when it's ready to write to.
//!             let conn_sock = accept(&listen_sock)?;
//!             ioctl_fionbio(&conn_sock, true)?;
//!             epoll::add(
//!                 &epoll,
//!                 &conn_sock,
//!                 next_id,
//!                 epoll::EventFlags::OUT | epoll::EventFlags::ET,
//!             )?;
//!
//!             // Keep track of the socket.
//!             sockets.insert(next_id, conn_sock);
//!             next_id = epoll::EventData::new_u64(next_id.u64() + 1);
//!         } else {
//!             // Write a message to the stream and then unregister it.
//!             let target = sockets.remove(&target).unwrap();
//!             write(&target, b"hello\n")?;
//!             let _ = epoll::delete(&epoll, &target)?;
//!         }
//!     }
//! }
//! # }
//! # #[cfg(not(feature = "net"))]
//! # fn main() {}
//! ```

#![allow(unsafe_code)]
#![allow(unused_qualifications)]

use super::epoll;
pub use crate::backend::event::epoll::*;
use crate::backend::event::syscalls;
use crate::buffer::Buffer;
use crate::fd::{AsFd, OwnedFd};
use crate::io;
use crate::timespec::Timespec;
use core::ffi::c_void;
use core::hash::{Hash, Hasher};

/// `epoll_create1(flags)`—Creates a new epoll object.
///
/// Use the [`epoll::CreateFlags::CLOEXEC`] flag to prevent the resulting file
/// descriptor from being implicitly passed across `exec` boundaries.
///
/// # References
///  - [Linux]
///  - [illumos]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/epoll_create.2.html
/// [illumos]: https://www.illumos.org/man/3C/epoll_create
#[inline]
#[doc(alias = "epoll_create1")]
pub fn create(flags: epoll::CreateFlags) -> io::Result<OwnedFd> {
    syscalls::epoll_create(flags)
}

/// `epoll_ctl(self, EPOLL_CTL_ADD, data, event)`—Adds an element to an epoll
/// object.
///
/// This registers interest in any of the events set in `event_flags` occurring
/// on the file descriptor associated with `data`.
///
/// `close`ing a file descriptor does not necessarily unregister interest which
/// can lead to spurious events being returned from [`epoll::wait`]. If a file
/// descriptor is an `Arc<dyn SystemResource>`, then `epoll` can be thought to
/// maintain a `Weak<dyn SystemResource>` to the file descriptor. Check the
/// [faq] for details.
///
/// # References
///  - [Linux]
///  - [illumos]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/epoll_ctl.2.html
/// [illumos]: https://www.illumos.org/man/3C/epoll_ctl
/// [faq]: https://man7.org/linux/man-pages/man7/epoll.7.html#:~:text=Will%20closing%20a%20file%20descriptor%20cause%20it%20to%20be%20removed%20from%20all%0A%20%20%20%20%20%20%20%20%20%20epoll%20interest%20lists%3F
#[doc(alias = "epoll_ctl")]
#[inline]
pub fn add<EpollFd: AsFd, SourceFd: AsFd>(
    epoll: EpollFd,
    source: SourceFd,
    data: epoll::EventData,
    event_flags: epoll::EventFlags,
) -> io::Result<()> {
    syscalls::epoll_add(
        epoll.as_fd(),
        source.as_fd(),
        &Event {
            flags: event_flags,
            data,
            #[cfg(all(libc, target_os = "redox"))]
            _pad: 0,
        },
    )
}

/// `epoll_ctl(self, EPOLL_CTL_MOD, target, event)`—Modifies an element in a
/// given epoll object.
///
/// This sets the events of interest with `target` to `events`.
///
/// # References
///  - [Linux]
///  - [illumos]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/epoll_ctl.2.html
/// [illumos]: https://www.illumos.org/man/3C/epoll_ctl
#[doc(alias = "epoll_ctl")]
#[inline]
pub fn modify<EpollFd: AsFd, SourceFd: AsFd>(
    epoll: EpollFd,
    source: SourceFd,
    data: epoll::EventData,
    event_flags: epoll::EventFlags,
) -> io::Result<()> {
    syscalls::epoll_mod(
        epoll.as_fd(),
        source.as_fd(),
        &Event {
            flags: event_flags,
            data,
            #[cfg(all(libc, target_os = "redox"))]
            _pad: 0,
        },
    )
}

/// `epoll_ctl(self, EPOLL_CTL_DEL, target, NULL)`—Removes an element in a
/// given epoll object.
///
/// # References
///  - [Linux]
///  - [illumos]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/epoll_ctl.2.html
/// [illumos]: https://www.illumos.org/man/3C/epoll_ctl
#[doc(alias = "epoll_ctl")]
#[inline]
pub fn delete<EpollFd: AsFd, SourceFd: AsFd>(epoll: EpollFd, source: SourceFd) -> io::Result<()> {
    syscalls::epoll_del(epoll.as_fd(), source.as_fd())
}

/// `epoll_wait(self, events, timeout)`—Waits for registered events of
/// interest.
///
/// For each event of interest, an element is written to `events`.
///
/// Linux versions older than 5.11 (those that don't support `epoll_pwait2`)
/// don't support timeouts greater than `c_int::MAX` milliseconds; if an
/// unsupported timeout is passed, this function fails with
/// [`io::Errno::INVAL`]. Enable the "linux_5_11" feature to enable the full
/// range of timeouts.
///
/// # References
///  - [Linux]
///  - [illumos]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/epoll_wait.2.html
/// [illumos]: https://www.illumos.org/man/3C/epoll_wait
#[doc(alias = "epoll_wait")]
#[inline]
pub fn wait<EpollFd: AsFd, Buf: Buffer<Event>>(
    epoll: EpollFd,
    mut event_list: Buf,
    timeout: Option<&Timespec>,
) -> io::Result<Buf::Output> {
    // SAFETY: `epoll_wait` behaves.
    let nfds = unsafe { syscalls::epoll_wait(epoll.as_fd(), event_list.parts_mut(), timeout)? };
    // SAFETY: `epoll_wait` behaves.
    unsafe { Ok(event_list.assume_init(nfds)) }
}

/// A record of an event that occurred.
#[repr(C)]
#[cfg_attr(all(not(libc), target_arch = "x86_64"), repr(packed))]
#[cfg_attr(
    all(
        libc,
        linux_kernel,
        any(
            all(
                target_arch = "x86",
                not(target_env = "musl"),
                not(target_os = "android"),
            ),
            target_arch = "x86_64",
        )
    ),
    repr(packed)
)]
#[cfg_attr(
    all(solarish, any(target_arch = "x86", target_arch = "x86_64")),
    repr(packed(4))
)]
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct Event {
    /// Which specific event(s) occurred.
    pub flags: EventFlags,
    /// User data.
    pub data: EventData,

    #[cfg(all(libc, target_os = "redox"))]
    _pad: u64,
}

/// Data associated with an [`epoll::Event`]. This can either be a 64-bit
/// integer value or a pointer which preserves pointer provenance.
#[repr(C)]
#[derive(Copy, Clone)]
pub union EventData {
    /// A 64-bit integer value.
    as_u64: u64,

    /// A `*mut c_void` which preserves pointer provenance, extended to be
    /// 64-bit so that if we read the value as a `u64` union field, we don't
    /// get uninitialized memory.
    sixty_four_bit_pointer: SixtyFourBitPointer,
}

impl EventData {
    /// Construct a new value containing a `u64`.
    #[inline]
    pub const fn new_u64(value: u64) -> Self {
        Self { as_u64: value }
    }

    /// Construct a new value containing a `*mut c_void`.
    #[inline]
    pub const fn new_ptr(value: *mut c_void) -> Self {
        Self {
            sixty_four_bit_pointer: SixtyFourBitPointer {
                pointer: value,
                #[cfg(target_pointer_width = "32")]
                _padding: 0,
            },
        }
    }

    /// Return the value as a `u64`.
    ///
    /// If the stored value was a pointer, the pointer is zero-extended to a
    /// `u64`.
    #[inline]
    pub fn u64(self) -> u64 {
        unsafe { self.as_u64 }
    }

    /// Return the value as a `*mut c_void`.
    ///
    /// If the stored value was a `u64`, the least-significant bits of the
    /// `u64` are returned as a pointer value.
    #[inline]
    pub fn ptr(self) -> *mut c_void {
        unsafe { self.sixty_four_bit_pointer.pointer }
    }
}

impl PartialEq for EventData {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.u64() == other.u64()
    }
}

impl Eq for EventData {}

impl Hash for EventData {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.u64().hash(state)
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
struct SixtyFourBitPointer {
    #[cfg(target_endian = "big")]
    #[cfg(target_pointer_width = "32")]
    _padding: u32,

    pointer: *mut c_void,

    #[cfg(target_endian = "little")]
    #[cfg(target_pointer_width = "32")]
    _padding: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::c;

    #[test]
    fn test_epoll_layouts() {
        check_renamed_type!(Event, epoll_event);
        check_renamed_struct_renamed_field!(Event, epoll_event, flags, events);
        #[cfg(libc)]
        check_renamed_struct_renamed_field!(Event, epoll_event, data, u64);
        #[cfg(not(libc))]
        check_renamed_struct_renamed_field!(Event, epoll_event, data, data);
    }
}
