use crate::backend::c;
use crate::backend::conv::ret;
#[cfg(feature = "mount")]
use crate::backend::conv::{borrowed_fd, c_str, ret_owned_fd};
#[cfg(feature = "mount")]
use crate::fd::{BorrowedFd, OwnedFd};
use crate::ffi::CStr;
use crate::io;
use core::ptr::null;

#[cfg(linux_kernel)]
pub(crate) fn mount(
    source: Option<&CStr>,
    target: &CStr,
    file_system_type: Option<&CStr>,
    flags: super::types::MountFlagsArg,
    data: Option<&CStr>,
) -> io::Result<()> {
    unsafe {
        ret(c::mount(
            source.map_or_else(null, CStr::as_ptr),
            target.as_ptr(),
            file_system_type.map_or_else(null, CStr::as_ptr),
            flags.0,
            data.map_or_else(null, CStr::as_ptr).cast(),
        ))
    }
}

#[cfg(linux_kernel)]
pub(crate) fn unmount(target: &CStr, flags: super::types::UnmountFlags) -> io::Result<()> {
    unsafe { ret(c::umount2(target.as_ptr(), bitflags_bits!(flags))) }
}

#[cfg(linux_kernel)]
#[cfg(feature = "mount")]
pub(crate) fn fsopen(fs_name: &CStr, flags: super::types::FsOpenFlags) -> io::Result<OwnedFd> {
    syscall! {
        fn fsopen(
            fs_name: *const c::c_char,
            flags: c::c_uint
        ) via SYS_fsopen -> c::c_int
    }
    unsafe { ret_owned_fd(fsopen(c_str(fs_name), flags.bits())) }
}

#[cfg(linux_kernel)]
#[cfg(feature = "mount")]
pub(crate) fn fsmount(
    fs_fd: BorrowedFd<'_>,
    flags: super::types::FsMountFlags,
    attr_flags: super::types::MountAttrFlags,
) -> io::Result<OwnedFd> {
    syscall! {
        fn fsmount(
            fs_fd: c::c_int,
            flags: c::c_uint,
            attr_flags: c::c_uint
        ) via SYS_fsmount -> c::c_int
    }
    unsafe { ret_owned_fd(fsmount(borrowed_fd(fs_fd), flags.bits(), attr_flags.bits())) }
}

#[cfg(linux_kernel)]
#[cfg(feature = "mount")]
pub(crate) fn move_mount(
    from_dfd: BorrowedFd<'_>,
    from_pathname: &CStr,
    to_dfd: BorrowedFd<'_>,
    to_pathname: &CStr,
    flags: super::types::MoveMountFlags,
) -> io::Result<()> {
    syscall! {
        fn move_mount(
            from_dfd: c::c_int,
            from_pathname: *const c::c_char,
            to_dfd: c::c_int,
            to_pathname: *const c::c_char,
            flags: c::c_uint
        ) via SYS_move_mount -> c::c_int
    }
    unsafe {
        ret(move_mount(
            borrowed_fd(from_dfd),
            c_str(from_pathname),
            borrowed_fd(to_dfd),
            c_str(to_pathname),
            flags.bits(),
        ))
    }
}

#[cfg(linux_kernel)]
#[cfg(feature = "mount")]
pub(crate) fn open_tree(
    dfd: BorrowedFd<'_>,
    filename: &CStr,
    flags: super::types::OpenTreeFlags,
) -> io::Result<OwnedFd> {
    syscall! {
        fn open_tree(
            dfd: c::c_int,
            filename: *const c::c_char,
            flags: c::c_uint
        ) via SYS_open_tree -> c::c_int
    }

    unsafe { ret_owned_fd(open_tree(borrowed_fd(dfd), c_str(filename), flags.bits())) }
}

#[cfg(linux_kernel)]
#[cfg(feature = "mount")]
pub(crate) fn fspick(
    dfd: BorrowedFd<'_>,
    path: &CStr,
    flags: super::types::FsPickFlags,
) -> io::Result<OwnedFd> {
    syscall! {
        fn fspick(
            dfd: c::c_int,
            path: *const c::c_char,
            flags: c::c_uint
        ) via SYS_fspick -> c::c_int
    }

    unsafe { ret_owned_fd(fspick(borrowed_fd(dfd), c_str(path), flags.bits())) }
}

#[cfg(feature = "mount")]
#[cfg(linux_kernel)]
syscall! {
    fn fsconfig(
        fs_fd: c::c_int,
        cmd: c::c_uint,
        key: *const c::c_char,
        val: *const c::c_char,
        aux: c::c_int
    ) via SYS_fsconfig -> c::c_int
}

#[cfg(linux_kernel)]
#[cfg(feature = "mount")]
pub(crate) fn fsconfig_set_flag(fs_fd: BorrowedFd<'_>, key: &CStr) -> io::Result<()> {
    unsafe {
        ret(fsconfig(
            borrowed_fd(fs_fd),
            super::types::FsConfigCmd::SetFlag as _,
            c_str(key),
            null(),
            0,
        ))
    }
}

#[cfg(linux_kernel)]
#[cfg(feature = "mount")]
pub(crate) fn fsconfig_set_string(
    fs_fd: BorrowedFd<'_>,
    key: &CStr,
    value: &CStr,
) -> io::Result<()> {
    unsafe {
        ret(fsconfig(
            borrowed_fd(fs_fd),
            super::types::FsConfigCmd::SetString as _,
            c_str(key),
            c_str(value),
            0,
        ))
    }
}

#[cfg(linux_kernel)]
#[cfg(feature = "mount")]
pub(crate) fn fsconfig_set_binary(
    fs_fd: BorrowedFd<'_>,
    key: &CStr,
    value: &[u8],
) -> io::Result<()> {
    unsafe {
        ret(fsconfig(
            borrowed_fd(fs_fd),
            super::types::FsConfigCmd::SetBinary as _,
            c_str(key),
            value.as_ptr().cast(),
            value.len().try_into().map_err(|_| io::Errno::OVERFLOW)?,
        ))
    }
}

#[cfg(linux_kernel)]
#[cfg(feature = "mount")]
pub(crate) fn fsconfig_set_fd(
    fs_fd: BorrowedFd<'_>,
    key: &CStr,
    fd: BorrowedFd<'_>,
) -> io::Result<()> {
    unsafe {
        ret(fsconfig(
            borrowed_fd(fs_fd),
            super::types::FsConfigCmd::SetFd as _,
            c_str(key),
            null(),
            borrowed_fd(fd),
        ))
    }
}

#[cfg(linux_kernel)]
#[cfg(feature = "mount")]
pub(crate) fn fsconfig_set_path(
    fs_fd: BorrowedFd<'_>,
    key: &CStr,
    path: &CStr,
    fd: BorrowedFd<'_>,
) -> io::Result<()> {
    unsafe {
        ret(fsconfig(
            borrowed_fd(fs_fd),
            super::types::FsConfigCmd::SetPath as _,
            c_str(key),
            c_str(path),
            borrowed_fd(fd),
        ))
    }
}

#[cfg(linux_kernel)]
#[cfg(feature = "mount")]
pub(crate) fn fsconfig_set_path_empty(
    fs_fd: BorrowedFd<'_>,
    key: &CStr,
    fd: BorrowedFd<'_>,
) -> io::Result<()> {
    unsafe {
        ret(fsconfig(
            borrowed_fd(fs_fd),
            super::types::FsConfigCmd::SetPathEmpty as _,
            c_str(key),
            c_str(cstr!("")),
            borrowed_fd(fd),
        ))
    }
}

#[cfg(linux_kernel)]
#[cfg(feature = "mount")]
pub(crate) fn fsconfig_create(fs_fd: BorrowedFd<'_>) -> io::Result<()> {
    unsafe {
        ret(fsconfig(
            borrowed_fd(fs_fd),
            super::types::FsConfigCmd::Create as _,
            null(),
            null(),
            0,
        ))
    }
}

#[cfg(linux_kernel)]
#[cfg(feature = "mount")]
pub(crate) fn fsconfig_reconfigure(fs_fd: BorrowedFd<'_>) -> io::Result<()> {
    unsafe {
        ret(fsconfig(
            borrowed_fd(fs_fd),
            super::types::FsConfigCmd::Reconfigure as _,
            null(),
            null(),
            0,
        ))
    }
}
