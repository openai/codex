//! linux_raw syscalls supporting `rustix::mount`.
//!
//! # Safety
//!
//! See the `rustix::backend` module documentation for details.
#![allow(unsafe_code)]
#![allow(clippy::undocumented_unsafe_blocks)]

use crate::backend::conv::{ret, ret_owned_fd, slice, zero};
use crate::fd::{BorrowedFd, OwnedFd};
use crate::ffi::CStr;
use crate::io;

#[inline]
pub(crate) fn mount(
    source: Option<&CStr>,
    target: &CStr,
    file_system_type: Option<&CStr>,
    flags: super::types::MountFlagsArg,
    data: Option<&CStr>,
) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_mount,
            source,
            target,
            file_system_type,
            flags,
            data
        ))
    }
}

#[inline]
pub(crate) fn unmount(target: &CStr, flags: super::types::UnmountFlags) -> io::Result<()> {
    unsafe { ret(syscall_readonly!(__NR_umount2, target, flags)) }
}

#[inline]
pub(crate) fn fsopen(fs_name: &CStr, flags: super::types::FsOpenFlags) -> io::Result<OwnedFd> {
    unsafe { ret_owned_fd(syscall_readonly!(__NR_fsopen, fs_name, flags)) }
}

#[inline]
pub(crate) fn fsmount(
    fs_fd: BorrowedFd<'_>,
    flags: super::types::FsMountFlags,
    attr_flags: super::types::MountAttrFlags,
) -> io::Result<OwnedFd> {
    unsafe { ret_owned_fd(syscall_readonly!(__NR_fsmount, fs_fd, flags, attr_flags)) }
}

#[inline]
pub(crate) fn move_mount(
    from_dfd: BorrowedFd<'_>,
    from_pathname: &CStr,
    to_dfd: BorrowedFd<'_>,
    to_pathname: &CStr,
    flags: super::types::MoveMountFlags,
) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_move_mount,
            from_dfd,
            from_pathname,
            to_dfd,
            to_pathname,
            flags
        ))
    }
}

#[inline]
pub(crate) fn open_tree(
    dfd: BorrowedFd<'_>,
    filename: &CStr,
    flags: super::types::OpenTreeFlags,
) -> io::Result<OwnedFd> {
    unsafe { ret_owned_fd(syscall_readonly!(__NR_open_tree, dfd, filename, flags)) }
}

#[inline]
pub(crate) fn fspick(
    dfd: BorrowedFd<'_>,
    path: &CStr,
    flags: super::types::FsPickFlags,
) -> io::Result<OwnedFd> {
    unsafe { ret_owned_fd(syscall_readonly!(__NR_fspick, dfd, path, flags)) }
}

#[inline]
pub(crate) fn fsconfig_set_flag(fs_fd: BorrowedFd<'_>, key: &CStr) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_fsconfig,
            fs_fd,
            super::types::FsConfigCmd::SetFlag,
            key,
            zero(),
            zero()
        ))
    }
}

#[inline]
pub(crate) fn fsconfig_set_string(
    fs_fd: BorrowedFd<'_>,
    key: &CStr,
    value: &CStr,
) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_fsconfig,
            fs_fd,
            super::types::FsConfigCmd::SetString,
            key,
            value,
            zero()
        ))
    }
}

#[inline]
pub(crate) fn fsconfig_set_binary(
    fs_fd: BorrowedFd<'_>,
    key: &CStr,
    value: &[u8],
) -> io::Result<()> {
    let (value_addr, value_len) = slice(value);
    unsafe {
        ret(syscall_readonly!(
            __NR_fsconfig,
            fs_fd,
            super::types::FsConfigCmd::SetBinary,
            key,
            value_addr,
            value_len
        ))
    }
}

#[inline]
pub(crate) fn fsconfig_set_fd(
    fs_fd: BorrowedFd<'_>,
    key: &CStr,
    fd: BorrowedFd<'_>,
) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_fsconfig,
            fs_fd,
            super::types::FsConfigCmd::SetFd,
            key,
            zero(),
            fd
        ))
    }
}

#[inline]
pub(crate) fn fsconfig_set_path(
    fs_fd: BorrowedFd<'_>,
    key: &CStr,
    path: &CStr,
    fd: BorrowedFd<'_>,
) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_fsconfig,
            fs_fd,
            super::types::FsConfigCmd::SetPath,
            key,
            path,
            fd
        ))
    }
}

#[inline]
pub(crate) fn fsconfig_set_path_empty(
    fs_fd: BorrowedFd<'_>,
    key: &CStr,
    fd: BorrowedFd<'_>,
) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_fsconfig,
            fs_fd,
            super::types::FsConfigCmd::SetPathEmpty,
            key,
            cstr!(""),
            fd
        ))
    }
}

#[inline]
pub(crate) fn fsconfig_create(fs_fd: BorrowedFd<'_>) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_fsconfig,
            fs_fd,
            super::types::FsConfigCmd::Create,
            zero(),
            zero(),
            zero()
        ))
    }
}

#[inline]
pub(crate) fn fsconfig_reconfigure(fs_fd: BorrowedFd<'_>) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_fsconfig,
            fs_fd,
            super::types::FsConfigCmd::Reconfigure,
            zero(),
            zero(),
            zero()
        ))
    }
}

#[inline]
pub(crate) fn fsconfig_create_excl(fs_fd: BorrowedFd<'_>) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_fsconfig,
            fs_fd,
            super::types::FsConfigCmd::CreateExclusive,
            zero(),
            zero(),
            zero()
        ))
    }
}
