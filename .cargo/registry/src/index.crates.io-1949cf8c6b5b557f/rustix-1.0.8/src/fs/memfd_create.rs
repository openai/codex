use crate::fd::OwnedFd;
use crate::{backend, io, path};
use backend::fs::types::MemfdFlags;

/// `memfd_create(name, flags)`—Create an anonymous file.
///
/// For a higher-level API to this functionality, see the [memfd] crate.
///
/// [memfd]: https://crates.io/crates/memfd
///
/// # References
///  - [Linux]
///  - [glibc]
///  - [FreeBSD]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/memfd_create.2.html
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Memory_002dmapped-I_002fO.html#index-memfd_005fcreate
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?memfd_create
#[inline]
pub fn memfd_create<P: path::Arg>(name: P, flags: MemfdFlags) -> io::Result<OwnedFd> {
    name.into_with_c_str(|name| backend::fs::syscalls::memfd_create(name, flags))
}
