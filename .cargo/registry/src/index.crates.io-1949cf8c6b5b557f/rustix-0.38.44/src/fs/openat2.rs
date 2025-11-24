use crate::fd::OwnedFd;
use crate::{backend, io, path};
use backend::fd::AsFd;
use backend::fs::types::{Mode, OFlags, ResolveFlags};

/// `openat2(dirfd, path, OpenHow { oflags, mode, resolve }, sizeof(OpenHow))`
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/openat2.2.html
#[inline]
pub fn openat2<Fd: AsFd, P: path::Arg>(
    dirfd: Fd,
    path: P,
    oflags: OFlags,
    mode: Mode,
    resolve: ResolveFlags,
) -> io::Result<OwnedFd> {
    path.into_with_c_str(|path| {
        backend::fs::syscalls::openat2(dirfd.as_fd(), path, oflags, mode, resolve)
    })
}
