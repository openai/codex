use crate::{backend, io, path};
use backend::c;
use backend::fd::AsFd;
use bitflags::bitflags;

bitflags! {
    /// `XATTR_*` constants for use with [`setxattr`], and other `*setxattr`
    /// functions.
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct XattrFlags: c::c_uint {
        /// `XATTR_CREATE`
        const CREATE = c::XATTR_CREATE as c::c_uint;

        /// `XATTR_REPLACE`
        const REPLACE = c::XATTR_REPLACE as c::c_uint;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

/// `getxattr(path, name, value.as_ptr(), value.len())`—Get extended
/// filesystem attributes.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/getxattr.2.html
#[inline]
pub fn getxattr<P: path::Arg, Name: path::Arg>(
    path: P,
    name: Name,
    value: &mut [u8],
) -> io::Result<usize> {
    path.into_with_c_str(|path| {
        name.into_with_c_str(|name| backend::fs::syscalls::getxattr(path, name, value))
    })
}

/// `lgetxattr(path, name, value.as_ptr(), value.len())`—Get extended
/// filesystem attributes, without following symlinks in the last path
/// component.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/lgetxattr.2.html
#[inline]
pub fn lgetxattr<P: path::Arg, Name: path::Arg>(
    path: P,
    name: Name,
    value: &mut [u8],
) -> io::Result<usize> {
    path.into_with_c_str(|path| {
        name.into_with_c_str(|name| backend::fs::syscalls::lgetxattr(path, name, value))
    })
}

/// `fgetxattr(fd, name, value.as_ptr(), value.len())`—Get extended
/// filesystem attributes on an open file descriptor.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/fgetxattr.2.html
#[inline]
pub fn fgetxattr<Fd: AsFd, Name: path::Arg>(
    fd: Fd,
    name: Name,
    value: &mut [u8],
) -> io::Result<usize> {
    name.into_with_c_str(|name| backend::fs::syscalls::fgetxattr(fd.as_fd(), name, value))
}

/// `setxattr(path, name, value.as_ptr(), value.len(), flags)`—Set extended
/// filesystem attributes.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/setxattr.2.html
#[inline]
pub fn setxattr<P: path::Arg, Name: path::Arg>(
    path: P,
    name: Name,
    value: &[u8],
    flags: XattrFlags,
) -> io::Result<()> {
    path.into_with_c_str(|path| {
        name.into_with_c_str(|name| backend::fs::syscalls::setxattr(path, name, value, flags))
    })
}

/// `setxattr(path, name, value.as_ptr(), value.len(), flags)`—Set extended
/// filesystem attributes, without following symlinks in the last path
/// component.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/lsetxattr.2.html
#[inline]
pub fn lsetxattr<P: path::Arg, Name: path::Arg>(
    path: P,
    name: Name,
    value: &[u8],
    flags: XattrFlags,
) -> io::Result<()> {
    path.into_with_c_str(|path| {
        name.into_with_c_str(|name| backend::fs::syscalls::lsetxattr(path, name, value, flags))
    })
}

/// `fsetxattr(fd, name, value.as_ptr(), value.len(), flags)`—Set extended
/// filesystem attributes on an open file descriptor.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/fsetxattr.2.html
#[inline]
pub fn fsetxattr<Fd: AsFd, Name: path::Arg>(
    fd: Fd,
    name: Name,
    value: &[u8],
    flags: XattrFlags,
) -> io::Result<()> {
    name.into_with_c_str(|name| backend::fs::syscalls::fsetxattr(fd.as_fd(), name, value, flags))
}

/// `listxattr(path, list.as_ptr(), list.len())`—List extended filesystem
/// attributes.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/listxattr.2.html
#[inline]
pub fn listxattr<P: path::Arg>(path: P, list: &mut [c::c_char]) -> io::Result<usize> {
    path.into_with_c_str(|path| backend::fs::syscalls::listxattr(path, list))
}

/// `llistxattr(path, list.as_ptr(), list.len())`—List extended filesystem
/// attributes, without following symlinks in the last path component.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/llistxattr.2.html
#[inline]
pub fn llistxattr<P: path::Arg>(path: P, list: &mut [c::c_char]) -> io::Result<usize> {
    path.into_with_c_str(|path| backend::fs::syscalls::llistxattr(path, list))
}

/// `flistxattr(fd, list.as_ptr(), list.len())`—List extended filesystem
/// attributes on an open file descriptor.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/flistxattr.2.html
#[inline]
pub fn flistxattr<Fd: AsFd>(fd: Fd, list: &mut [c::c_char]) -> io::Result<usize> {
    backend::fs::syscalls::flistxattr(fd.as_fd(), list)
}

/// `removexattr(path, name)`—Remove an extended filesystem attribute.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/removexattr.2.html
pub fn removexattr<P: path::Arg, Name: path::Arg>(path: P, name: Name) -> io::Result<()> {
    path.into_with_c_str(|path| {
        name.into_with_c_str(|name| backend::fs::syscalls::removexattr(path, name))
    })
}

/// `lremovexattr(path, name)`—Remove an extended filesystem attribute,
/// without following symlinks in the last path component.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/lremovexattr.2.html
pub fn lremovexattr<P: path::Arg, Name: path::Arg>(path: P, name: Name) -> io::Result<()> {
    path.into_with_c_str(|path| {
        name.into_with_c_str(|name| backend::fs::syscalls::lremovexattr(path, name))
    })
}

/// `fremovexattr(fd, name)`—Remove an extended filesystem attribute on an
/// open file descriptor.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/fremovexattr.2.html
pub fn fremovexattr<Fd: AsFd, Name: path::Arg>(fd: Fd, name: Name) -> io::Result<()> {
    name.into_with_c_str(|name| backend::fs::syscalls::fremovexattr(fd.as_fd(), name))
}
