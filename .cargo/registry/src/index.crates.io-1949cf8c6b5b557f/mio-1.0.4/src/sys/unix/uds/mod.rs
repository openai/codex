#[cfg(target_os = "android")]
use std::os::android::net::SocketAddrExt;
#[cfg(target_os = "linux")]
use std::os::linux::net::SocketAddrExt;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::FromRawFd;
use std::os::unix::net::SocketAddr;
use std::{io, mem, ptr};

pub(crate) mod datagram;
pub(crate) mod listener;
pub(crate) mod stream;

const UNNAMED_ADDRESS: &[u8] = &[];

/// Get the `sun_path` field offset of `sockaddr_un` for the target OS.
///
/// On Linux, this function equates to the same value as
/// `size_of::<sa_family_t>()`, but some other implementations include
/// other fields before `sun_path`, so the expression more portably
/// describes the size of the address structure.
fn path_offset(sockaddr: &libc::sockaddr_un) -> usize {
    let base = sockaddr as *const _ as usize;
    let path = &sockaddr.sun_path as *const _ as usize;
    path - base
}

/// Converts a Rust `SocketAddr` into the system representation.
fn unix_addr(address: &SocketAddr) -> (libc::sockaddr_un, libc::socklen_t) {
    // SAFETY: `libc::sockaddr_un` zero filled is properly initialized.
    //
    // `0` is a valid value for `sockaddr_un::sun_family`; it is
    // `libc::AF_UNSPEC`.
    //
    // `[0; 108]` is a valid value for `sockaddr_un::sun_path`; it begins an
    // abstract path.
    let mut sockaddr = unsafe { mem::zeroed::<libc::sockaddr_un>() };

    sockaddr.sun_family = libc::AF_UNIX as libc::sa_family_t;

    #[allow(unused_mut)] // Only used with abstract namespaces.
    let mut offset = 0;
    let addr = match address.as_pathname() {
        Some(path) => path.as_os_str().as_bytes(),
        #[cfg(any(target_os = "android", target_os = "linux"))]
        None => match address.as_abstract_name() {
            Some(name) => {
                offset += 1;
                name
            }
            None => UNNAMED_ADDRESS,
        },
        #[cfg(not(any(target_os = "android", target_os = "linux")))]
        None => UNNAMED_ADDRESS,
    };

    // SAFETY: `addr` and `sockaddr.sun_path` are not overlapping and both point
    // to valid memory.
    // SAFETY: since `addr` is a valid Unix address, it must not be larger than
    // `SUN_LEN` bytes, thus we won't overwrite the size of sockaddr.sun_path.
    // SAFETY: null byte is already written because we zeroed the address above.
    debug_assert!(offset + addr.len() <= sockaddr.sun_path.len());
    unsafe {
        ptr::copy_nonoverlapping(
            addr.as_ptr(),
            sockaddr.sun_path.as_mut_ptr().add(offset).cast(),
            addr.len(),
        )
    };

    let mut addrlen = path_offset(&sockaddr) + addr.len();
    // +1 for null byte at the end of the path, not needed for abstract
    // namespaces (which start with a null byte).
    match addr.first() {
        Some(&0) | None => {}
        Some(_) => addrlen += 1,
    }

    // SAFETY: the length is fine to cast to `socklen_t` as it's 32 bits and the
    // address can be at most `SUN_LEN` bytes.
    (sockaddr, addrlen as _)
}

fn pair<T>(flags: libc::c_int) -> io::Result<(T, T)>
where
    T: FromRawFd,
{
    #[cfg(not(any(
        target_os = "aix",
        target_os = "haiku",
        target_os = "ios",
        target_os = "macos",
        target_os = "nto",
        target_os = "tvos",
        target_os = "visionos",
        target_os = "watchos",
        target_os = "espidf",
        target_os = "vita",
    )))]
    let flags = flags | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC;

    let mut fds = [-1; 2];
    syscall!(socketpair(libc::AF_UNIX, flags, 0, fds.as_mut_ptr()))?;
    let pair = unsafe { (T::from_raw_fd(fds[0]), T::from_raw_fd(fds[1])) };

    // Darwin (and others) doesn't have SOCK_NONBLOCK or SOCK_CLOEXEC.
    //
    // In order to set those flags, additional `fcntl` sys calls must be
    // performed. If a `fnctl` fails after the sockets have been created,
    // the file descriptors will leak. Creating `pair` above ensures that if
    // there is an error, the file descriptors are closed.
    #[cfg(any(
        target_os = "aix",
        target_os = "haiku",
        target_os = "ios",
        target_os = "macos",
        target_os = "nto",
        target_os = "tvos",
        target_os = "visionos",
        target_os = "watchos",
        target_os = "espidf",
        target_os = "vita",
    ))]
    {
        syscall!(fcntl(fds[0], libc::F_SETFL, libc::O_NONBLOCK))?;
        #[cfg(not(any(target_os = "espidf", target_os = "vita", target_os = "nto")))]
        syscall!(fcntl(fds[0], libc::F_SETFD, libc::FD_CLOEXEC))?;
        syscall!(fcntl(fds[1], libc::F_SETFL, libc::O_NONBLOCK))?;
        #[cfg(not(any(target_os = "espidf", target_os = "vita", target_os = "nto")))]
        syscall!(fcntl(fds[1], libc::F_SETFD, libc::FD_CLOEXEC))?;
    }

    Ok(pair)
}

#[cfg(test)]
mod tests {
    use std::os::unix::net::SocketAddr;
    use std::path::Path;
    use std::str;

    use super::{path_offset, unix_addr};

    #[test]
    fn pathname_address() {
        const PATH: &str = "./foo/bar.txt";
        const PATH_LEN: usize = 13;

        // Pathname addresses do have a null terminator, so `socklen` is
        // expected to be `PATH_LEN` + `offset` + 1.
        let address = SocketAddr::from_pathname(Path::new(PATH)).unwrap();
        let (sockaddr, actual) = unix_addr(&address);
        let offset = path_offset(&sockaddr);
        let expected = PATH_LEN + offset + 1;
        assert_eq!(expected as libc::socklen_t, actual)
    }

    #[test]
    #[cfg(any(target_os = "android", target_os = "linux"))]
    fn abstract_address() {
        #[cfg(target_os = "android")]
        use std::os::android::net::SocketAddrExt;
        #[cfg(target_os = "linux")]
        use std::os::linux::net::SocketAddrExt;

        const PATH: &[u8] = &[0, 116, 111, 107, 105, 111];
        const PATH_LEN: usize = 6;

        // Abstract addresses do not have a null terminator, so `socklen` is
        // expected to be `PATH_LEN` + `offset`.
        let address = SocketAddr::from_abstract_name(PATH).unwrap();
        let (sockaddr, actual) = unix_addr(&address);
        let offset = path_offset(&sockaddr);
        let expected = PATH_LEN + offset;
        assert_eq!(expected as libc::socklen_t, actual)
    }
}
