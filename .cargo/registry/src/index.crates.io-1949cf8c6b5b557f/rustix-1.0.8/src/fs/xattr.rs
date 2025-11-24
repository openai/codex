//! Extended attribute functions.

#![allow(unsafe_code)]

use crate::buffer::Buffer;
use crate::{backend, ffi, io, path};
use backend::c;
use backend::fd::AsFd;
use bitflags::bitflags;

bitflags! {
    /// `XATTR_*` constants for use with [`setxattr`], and other `*setxattr`
    /// functions.
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct XattrFlags: ffi::c_uint {
        /// `XATTR_CREATE`
        const CREATE = c::XATTR_CREATE as c::c_uint;

        /// `XATTR_REPLACE`
        const REPLACE = c::XATTR_REPLACE as c::c_uint;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

/// `getxattr(path, name, value)`—Get extended filesystem attributes.
///
/// For a higher-level API to xattr functionality, see the [xattr] crate.
///
/// [xattr]: https://crates.io/crates/xattr
///
/// # References
///  - [Linux]
///  - [Apple]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/getxattr.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/getxattr.2.html
#[inline]
pub fn getxattr<P: path::Arg, Name: path::Arg, Buf: Buffer<u8>>(
    path: P,
    name: Name,
    mut value: Buf,
) -> io::Result<Buf::Output> {
    path.into_with_c_str(|path| {
        name.into_with_c_str(|name| {
            // SAFETY: `getxattr` behaves.
            let len = unsafe { backend::fs::syscalls::getxattr(path, name, value.parts_mut())? };
            // SAFETY: `getxattr` behaves.
            unsafe { Ok(value.assume_init(len)) }
        })
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
pub fn lgetxattr<P: path::Arg, Name: path::Arg, Buf: Buffer<u8>>(
    path: P,
    name: Name,
    mut value: Buf,
) -> io::Result<Buf::Output> {
    path.into_with_c_str(|path| {
        name.into_with_c_str(|name| {
            // SAFETY: `lgetxattr` behaves.
            let len = unsafe { backend::fs::syscalls::lgetxattr(path, name, value.parts_mut())? };
            // SAFETY: `lgetxattr` behaves.
            unsafe { Ok(value.assume_init(len)) }
        })
    })
}

/// `fgetxattr(fd, name, value.as_ptr(), value.len())`—Get extended
/// filesystem attributes on an open file descriptor.
///
/// # References
///  - [Linux]
///  - [Apple]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/fgetxattr.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/fgetxattr.2.html
#[inline]
pub fn fgetxattr<Fd: AsFd, Name: path::Arg, Buf: Buffer<u8>>(
    fd: Fd,
    name: Name,
    mut value: Buf,
) -> io::Result<Buf::Output> {
    name.into_with_c_str(|name| {
        // SAFETY: `fgetxattr` behaves.
        let len = unsafe { backend::fs::syscalls::fgetxattr(fd.as_fd(), name, value.parts_mut())? };
        // SAFETY: `fgetxattr` behaves.
        unsafe { Ok(value.assume_init(len)) }
    })
}

/// `setxattr(path, name, value.as_ptr(), value.len(), flags)`—Set extended
/// filesystem attributes.
///
/// # References
///  - [Linux]
///  - [Apple]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/setxattr.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/setxattr.2.html
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
///  - [Apple]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/fsetxattr.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/fsetxattr.2.html
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
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/listxattr.2.html
#[inline]
pub fn listxattr<P: path::Arg, Buf: Buffer<u8>>(path: P, mut list: Buf) -> io::Result<Buf::Output> {
    path.into_with_c_str(|path| {
        // SAFETY: `listxattr` behaves.
        let len = unsafe { backend::fs::syscalls::listxattr(path, list.parts_mut())? };
        // SAFETY: `listxattr` behaves.
        unsafe { Ok(list.assume_init(len)) }
    })
}

/// `llistxattr(path, list.as_ptr(), list.len())`—List extended filesystem
/// attributes, without following symlinks in the last path component.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/llistxattr.2.html
#[inline]
pub fn llistxattr<P: path::Arg, Buf: Buffer<u8>>(
    path: P,
    mut list: Buf,
) -> io::Result<Buf::Output> {
    path.into_with_c_str(|path| {
        // SAFETY: `flistxattr` behaves.
        let len = unsafe { backend::fs::syscalls::llistxattr(path, list.parts_mut())? };
        // SAFETY: `flistxattr` behaves.
        unsafe { Ok(list.assume_init(len)) }
    })
}

/// `flistxattr(fd, list.as_ptr(), list.len())`—List extended filesystem
/// attributes on an open file descriptor.
///
/// # References
///  - [Linux]
///  - [Apple]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/flistxattr.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/flistxattr.2.html
#[inline]
pub fn flistxattr<Fd: AsFd, Buf: Buffer<u8>>(fd: Fd, mut list: Buf) -> io::Result<Buf::Output> {
    // SAFETY: `flistxattr` behaves.
    let len = unsafe { backend::fs::syscalls::flistxattr(fd.as_fd(), list.parts_mut())? };
    // SAFETY: `flistxattr` behaves.
    unsafe { Ok(list.assume_init(len)) }
}

/// `removexattr(path, name)`—Remove an extended filesystem attribute.
///
/// # References
///  - [Linux]
///  - [Apple]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/removexattr.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/removexattr.2.html
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
///  - [Apple]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/fremovexattr.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/fremovexattr.2.html
pub fn fremovexattr<Fd: AsFd, Name: path::Arg>(fd: Fd, name: Name) -> io::Result<()> {
    name.into_with_c_str(|name| backend::fs::syscalls::fremovexattr(fd.as_fd(), name))
}
