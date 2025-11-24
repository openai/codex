//! Linux `mount`.

use crate::backend::mount::types::{
    InternalMountFlags, MountFlags, MountFlagsArg, MountPropagationFlags, UnmountFlags,
};
use crate::ffi::CStr;
use crate::path::{self, option_into_with_c_str};
use crate::{backend, io};

/// `mount(source, target, filesystemtype, mountflags, data)`
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/mount.2.html
#[inline]
pub fn mount<
    'a,
    Source: path::Arg,
    Target: path::Arg,
    Fs: path::Arg,
    Data: Into<Option<&'a CStr>>,
>(
    source: Source,
    target: Target,
    file_system_type: Fs,
    flags: MountFlags,
    data: Data,
) -> io::Result<()> {
    source.into_with_c_str(|source| {
        target.into_with_c_str(|target| {
            file_system_type.into_with_c_str(|file_system_type| {
                option_into_with_c_str(data.into(), |data| {
                    backend::mount::syscalls::mount(
                        Some(source),
                        target,
                        Some(file_system_type),
                        MountFlagsArg(flags.bits()),
                        data,
                    )
                })
            })
        })
    })
}

/// `mount(NULL, target, NULL, MS_REMOUNT | mountflags, data)`
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/mount.2.html
#[inline]
#[doc(alias = "mount")]
#[doc(alias = "MS_REMOUNT")]
pub fn mount_remount<Target: path::Arg, Data: path::Arg>(
    target: Target,
    flags: MountFlags,
    data: Data,
) -> io::Result<()> {
    target.into_with_c_str(|target| {
        data.into_with_c_str(|data| {
            backend::mount::syscalls::mount(
                None,
                target,
                None,
                MountFlagsArg(InternalMountFlags::REMOUNT.bits() | flags.bits()),
                Some(data),
            )
        })
    })
}

/// `mount(source, target, NULL, MS_BIND, NULL)`
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/mount.2.html
#[inline]
#[doc(alias = "mount")]
#[doc(alias = "MS_BIND")]
pub fn mount_bind<Source: path::Arg, Target: path::Arg>(
    source: Source,
    target: Target,
) -> io::Result<()> {
    source.into_with_c_str(|source| {
        target.into_with_c_str(|target| {
            backend::mount::syscalls::mount(
                Some(source),
                target,
                None,
                MountFlagsArg(MountFlags::BIND.bits()),
                None,
            )
        })
    })
}

/// `mount(source, target, NULL, MS_BIND | MS_REC, NULL)`
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/mount.2.html
#[inline]
#[doc(alias = "mount")]
#[doc(alias = "MS_REC")]
pub fn mount_bind_recursive<Source: path::Arg, Target: path::Arg>(
    source: Source,
    target: Target,
) -> io::Result<()> {
    source.into_with_c_str(|source| {
        target.into_with_c_str(|target| {
            backend::mount::syscalls::mount(
                Some(source),
                target,
                None,
                MountFlagsArg(MountFlags::BIND.bits() | MountPropagationFlags::REC.bits()),
                None,
            )
        })
    })
}

/// `mount(NULL, target, NULL, mountflags, NULL)`
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/mount.2.html
#[inline]
#[doc(alias = "mount")]
pub fn mount_change<Target: path::Arg>(
    target: Target,
    flags: MountPropagationFlags,
) -> io::Result<()> {
    target.into_with_c_str(|target| {
        backend::mount::syscalls::mount(None, target, None, MountFlagsArg(flags.bits()), None)
    })
}

/// `mount(source, target, NULL, MS_MOVE, NULL)`
///
/// This is not the same as the `move_mount` syscall. If you want to use that,
/// use [`move_mount`] instead.
///
/// # References
///  - [Linux]
///
/// [`move_mount`]: crate::mount::move_mount
/// [Linux]: https://man7.org/linux/man-pages/man2/mount.2.html
#[inline]
#[doc(alias = "mount")]
#[doc(alias = "MS_MOVE")]
pub fn mount_move<Source: path::Arg, Target: path::Arg>(
    source: Source,
    target: Target,
) -> io::Result<()> {
    source.into_with_c_str(|source| {
        target.into_with_c_str(|target| {
            backend::mount::syscalls::mount(
                Some(source),
                target,
                None,
                MountFlagsArg(InternalMountFlags::MOVE.bits()),
                None,
            )
        })
    })
}

/// `umount2(target, flags)`
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/umount.2.html
#[inline]
#[doc(alias = "umount", alias = "umount2")]
pub fn unmount<Target: path::Arg>(target: Target, flags: UnmountFlags) -> io::Result<()> {
    target.into_with_c_str(|target| backend::mount::syscalls::unmount(target, flags))
}
