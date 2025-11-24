//! libc syscalls supporting `rustix::fs`.

use crate::backend::c;
#[cfg(any(not(target_os = "redox"), feature = "alloc"))]
use crate::backend::conv::ret_usize;
use crate::backend::conv::{borrowed_fd, c_str, ret, ret_c_int, ret_off_t, ret_owned_fd};
use crate::fd::{BorrowedFd, OwnedFd};
#[allow(unused_imports)]
use crate::ffi;
use crate::ffi::CStr;
#[cfg(all(apple, feature = "alloc"))]
use crate::ffi::CString;
#[cfg(not(any(target_os = "espidf", target_os = "horizon", target_os = "vita")))]
use crate::fs::Access;
#[cfg(not(any(target_os = "espidf", target_os = "redox")))]
use crate::fs::AtFlags;
#[cfg(not(any(
    netbsdlike,
    target_os = "dragonfly",
    target_os = "espidf",
    target_os = "horizon",
    target_os = "nto",
    target_os = "redox",
    target_os = "vita",
)))]
use crate::fs::FallocateFlags;
#[cfg(not(any(
    target_os = "espidf",
    target_os = "horizon",
    target_os = "vita",
    target_os = "wasi"
)))]
use crate::fs::FlockOperation;
#[cfg(any(linux_kernel, target_os = "freebsd"))]
use crate::fs::MemfdFlags;
#[cfg(any(linux_kernel, apple))]
use crate::fs::RenameFlags;
#[cfg(any(linux_kernel, target_os = "freebsd", target_os = "fuchsia"))]
use crate::fs::SealFlags;
#[cfg(not(any(
    solarish,
    target_os = "espidf",
    target_os = "haiku",
    target_os = "horizon",
    target_os = "netbsd",
    target_os = "nto",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi",
)))]
use crate::fs::StatFs;
#[cfg(not(any(target_os = "espidf", target_os = "vita")))]
use crate::fs::Timestamps;
#[cfg(not(any(
    apple,
    target_os = "espidf",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
use crate::fs::{Dev, FileType};
use crate::fs::{Mode, OFlags, SeekFrom, Stat};
#[cfg(not(any(target_os = "haiku", target_os = "redox", target_os = "wasi")))]
use crate::fs::{StatVfs, StatVfsMountFlags};
use crate::io;
#[cfg(all(target_env = "gnu", fix_y2038))]
use crate::timespec::LibcTimespec;
#[cfg(not(target_os = "wasi"))]
use crate::ugid::{Gid, Uid};
#[cfg(all(apple, feature = "alloc"))]
use alloc::vec;
use core::mem::MaybeUninit;
#[cfg(apple)]
use {
    crate::backend::conv::nonnegative_ret,
    crate::fs::{copyfile_state_t, CloneFlags, CopyfileFlags},
};
#[cfg(not(any(
    apple,
    netbsdlike,
    target_os = "dragonfly",
    target_os = "espidf",
    target_os = "haiku",
    target_os = "horizon",
    target_os = "redox",
    target_os = "solaris",
    target_os = "vita",
)))]
use {crate::fs::Advice, core::num::NonZeroU64};
#[cfg(any(apple, linux_kernel, target_os = "hurd"))]
use {crate::fs::XattrFlags, core::mem::size_of, core::ptr::null_mut};
#[cfg(linux_kernel)]
use {
    crate::fs::{ResolveFlags, Statx, StatxFlags, CWD},
    core::ptr::null,
};

#[cfg(all(target_env = "gnu", fix_y2038))]
weak!(fn __utimensat64(c::c_int, *const ffi::c_char, *const LibcTimespec, c::c_int) -> c::c_int);
#[cfg(all(target_env = "gnu", fix_y2038))]
weak!(fn __futimens64(c::c_int, *const LibcTimespec) -> c::c_int);

/// Use a direct syscall (via libc) for `open`.
///
/// This is only currently necessary as a workaround for old glibc; see below.
#[cfg(all(unix, target_env = "gnu"))]
fn open_via_syscall(path: &CStr, oflags: OFlags, mode: Mode) -> io::Result<OwnedFd> {
    // Linux on aarch64, loongarch64 and riscv64 has no `open` syscall so use
    // `openat`.
    #[cfg(any(
        target_arch = "aarch64",
        target_arch = "riscv32",
        target_arch = "riscv64",
        target_arch = "csky",
        target_arch = "loongarch64"
    ))]
    {
        openat_via_syscall(CWD, path, oflags, mode)
    }

    // Use the `open` syscall.
    #[cfg(not(any(
        target_arch = "aarch64",
        target_arch = "riscv32",
        target_arch = "riscv64",
        target_arch = "csky",
        target_arch = "loongarch64"
    )))]
    unsafe {
        syscall! {
            fn open(
                pathname: *const ffi::c_char,
                oflags: c::c_int,
                mode: c::mode_t
            ) via SYS_open -> c::c_int
        }

        ret_owned_fd(open(
            c_str(path),
            bitflags_bits!(oflags),
            bitflags_bits!(mode),
        ))
    }
}

pub(crate) fn open(path: &CStr, oflags: OFlags, mode: Mode) -> io::Result<OwnedFd> {
    // Work around <https://sourceware.org/bugzilla/show_bug.cgi?id=17523>.
    // glibc versions before 2.25 don't handle `O_TMPFILE` correctly.
    #[cfg(all(
        unix,
        target_env = "gnu",
        not(target_os = "hurd"),
        not(target_os = "freebsd")
    ))]
    if oflags.contains(OFlags::TMPFILE) && crate::backend::if_glibc_is_less_than_2_25() {
        return open_via_syscall(path, oflags, mode);
    }

    // On these platforms, `mode_t` is `u16` and can't be passed directly to a
    // variadic function.
    #[cfg(any(
        apple,
        freebsdlike,
        all(target_os = "android", target_pointer_width = "32")
    ))]
    let mode: c::c_uint = mode.bits().into();

    // Otherwise, cast to `mode_t` as that's what `open` is documented to take.
    #[cfg(not(any(
        apple,
        freebsdlike,
        all(target_os = "android", target_pointer_width = "32")
    )))]
    let mode: c::mode_t = mode.bits() as _;

    unsafe { ret_owned_fd(c::open(c_str(path), bitflags_bits!(oflags), mode)) }
}

/// Use a direct syscall (via libc) for `openat`.
///
/// This is only currently necessary as a workaround for old glibc; see below.
#[cfg(all(unix, target_env = "gnu", not(target_os = "hurd")))]
fn openat_via_syscall(
    dirfd: BorrowedFd<'_>,
    path: &CStr,
    oflags: OFlags,
    mode: Mode,
) -> io::Result<OwnedFd> {
    syscall! {
        fn openat(
            base_dirfd: c::c_int,
            pathname: *const ffi::c_char,
            oflags: c::c_int,
            mode: c::mode_t
        ) via SYS_openat -> c::c_int
    }

    unsafe {
        ret_owned_fd(openat(
            borrowed_fd(dirfd),
            c_str(path),
            bitflags_bits!(oflags),
            bitflags_bits!(mode),
        ))
    }
}

#[cfg(not(target_os = "redox"))]
pub(crate) fn openat(
    dirfd: BorrowedFd<'_>,
    path: &CStr,
    oflags: OFlags,
    mode: Mode,
) -> io::Result<OwnedFd> {
    // Work around <https://sourceware.org/bugzilla/show_bug.cgi?id=17523>.
    // glibc versions before 2.25 don't handle `O_TMPFILE` correctly.
    #[cfg(all(
        unix,
        target_env = "gnu",
        not(target_os = "hurd"),
        not(target_os = "freebsd")
    ))]
    if oflags.contains(OFlags::TMPFILE) && crate::backend::if_glibc_is_less_than_2_25() {
        return openat_via_syscall(dirfd, path, oflags, mode);
    }

    // On these platforms, `mode_t` is `u16` and can't be passed directly to a
    // variadic function.
    #[cfg(any(
        apple,
        freebsdlike,
        all(target_os = "android", target_pointer_width = "32")
    ))]
    let mode: c::c_uint = mode.bits().into();

    // Otherwise, cast to `mode_t` as that's what `open` is documented to take.
    #[cfg(not(any(
        apple,
        freebsdlike,
        all(target_os = "android", target_pointer_width = "32")
    )))]
    let mode: c::mode_t = mode.bits() as _;

    unsafe {
        ret_owned_fd(c::openat(
            borrowed_fd(dirfd),
            c_str(path),
            bitflags_bits!(oflags),
            mode,
        ))
    }
}

#[cfg(not(any(
    solarish,
    target_os = "espidf",
    target_os = "haiku",
    target_os = "horizon",
    target_os = "netbsd",
    target_os = "nto",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi",
)))]
#[inline]
pub(crate) fn statfs(filename: &CStr) -> io::Result<StatFs> {
    unsafe {
        let mut result = MaybeUninit::<StatFs>::uninit();
        ret(c::statfs(c_str(filename), result.as_mut_ptr()))?;
        Ok(result.assume_init())
    }
}

#[cfg(not(any(target_os = "haiku", target_os = "redox", target_os = "wasi")))]
#[inline]
pub(crate) fn statvfs(filename: &CStr) -> io::Result<StatVfs> {
    unsafe {
        let mut result = MaybeUninit::<c::statvfs>::uninit();
        ret(c::statvfs(c_str(filename), result.as_mut_ptr()))?;
        Ok(libc_statvfs_to_statvfs(result.assume_init()))
    }
}

#[cfg(feature = "alloc")]
#[inline]
pub(crate) fn readlink(path: &CStr, buf: &mut [u8]) -> io::Result<usize> {
    unsafe { ret_usize(c::readlink(c_str(path), buf.as_mut_ptr().cast(), buf.len()) as isize) }
}

#[cfg(not(target_os = "redox"))]
#[inline]
pub(crate) unsafe fn readlinkat(
    dirfd: BorrowedFd<'_>,
    path: &CStr,
    buf: (*mut u8, usize),
) -> io::Result<usize> {
    ret_usize(c::readlinkat(borrowed_fd(dirfd), c_str(path), buf.0.cast(), buf.1) as isize)
}

pub(crate) fn mkdir(path: &CStr, mode: Mode) -> io::Result<()> {
    unsafe { ret(c::mkdir(c_str(path), mode.bits() as c::mode_t)) }
}

#[cfg(not(target_os = "redox"))]
pub(crate) fn mkdirat(dirfd: BorrowedFd<'_>, path: &CStr, mode: Mode) -> io::Result<()> {
    unsafe {
        ret(c::mkdirat(
            borrowed_fd(dirfd),
            c_str(path),
            mode.bits() as c::mode_t,
        ))
    }
}

#[cfg(linux_kernel)]
pub(crate) fn getdents_uninit(
    fd: BorrowedFd<'_>,
    buf: &mut [MaybeUninit<u8>],
) -> io::Result<usize> {
    syscall! {
        fn getdents64(
            fd: c::c_int,
            dirp: *mut c::c_void,
            count: usize
        ) via SYS_getdents64 -> c::ssize_t
    }
    unsafe {
        ret_usize(getdents64(
            borrowed_fd(fd),
            buf.as_mut_ptr().cast::<c::c_void>(),
            buf.len(),
        ))
    }
}

pub(crate) fn link(old_path: &CStr, new_path: &CStr) -> io::Result<()> {
    unsafe { ret(c::link(c_str(old_path), c_str(new_path))) }
}

#[cfg(not(any(target_os = "espidf", target_os = "redox")))]
pub(crate) fn linkat(
    old_dirfd: BorrowedFd<'_>,
    old_path: &CStr,
    new_dirfd: BorrowedFd<'_>,
    new_path: &CStr,
    flags: AtFlags,
) -> io::Result<()> {
    // macOS ≤ 10.9 lacks `linkat`.
    #[cfg(target_os = "macos")]
    unsafe {
        weak! {
            fn linkat(
                c::c_int,
                *const ffi::c_char,
                c::c_int,
                *const ffi::c_char,
                c::c_int
            ) -> c::c_int
        }
        // If we have `linkat`, use it.
        if let Some(libc_linkat) = linkat.get() {
            return ret(libc_linkat(
                borrowed_fd(old_dirfd),
                c_str(old_path),
                borrowed_fd(new_dirfd),
                c_str(new_path),
                bitflags_bits!(flags),
            ));
        }
        // Otherwise, see if we can emulate the `AT_FDCWD` case.
        if borrowed_fd(old_dirfd) != c::AT_FDCWD || borrowed_fd(new_dirfd) != c::AT_FDCWD {
            return Err(io::Errno::NOSYS);
        }
        if flags.intersects(!AtFlags::SYMLINK_FOLLOW) {
            return Err(io::Errno::INVAL);
        }
        if !flags.is_empty() {
            return Err(io::Errno::OPNOTSUPP);
        }
        ret(c::link(c_str(old_path), c_str(new_path)))
    }

    #[cfg(not(target_os = "macos"))]
    unsafe {
        ret(c::linkat(
            borrowed_fd(old_dirfd),
            c_str(old_path),
            borrowed_fd(new_dirfd),
            c_str(new_path),
            bitflags_bits!(flags),
        ))
    }
}

pub(crate) fn rmdir(path: &CStr) -> io::Result<()> {
    unsafe { ret(c::rmdir(c_str(path))) }
}

pub(crate) fn unlink(path: &CStr) -> io::Result<()> {
    unsafe { ret(c::unlink(c_str(path))) }
}

#[cfg(not(any(target_os = "espidf", target_os = "redox")))]
pub(crate) fn unlinkat(dirfd: BorrowedFd<'_>, path: &CStr, flags: AtFlags) -> io::Result<()> {
    // macOS ≤ 10.9 lacks `unlinkat`.
    #[cfg(target_os = "macos")]
    unsafe {
        weak! {
            fn unlinkat(
                c::c_int,
                *const ffi::c_char,
                c::c_int
            ) -> c::c_int
        }
        // If we have `unlinkat`, use it.
        if let Some(libc_unlinkat) = unlinkat.get() {
            return ret(libc_unlinkat(
                borrowed_fd(dirfd),
                c_str(path),
                bitflags_bits!(flags),
            ));
        }
        // Otherwise, see if we can emulate the `AT_FDCWD` case.
        if borrowed_fd(dirfd) != c::AT_FDCWD {
            return Err(io::Errno::NOSYS);
        }
        if flags.intersects(!AtFlags::REMOVEDIR) {
            return Err(io::Errno::INVAL);
        }
        if flags.contains(AtFlags::REMOVEDIR) {
            ret(c::rmdir(c_str(path)))
        } else {
            ret(c::unlink(c_str(path)))
        }
    }

    #[cfg(not(target_os = "macos"))]
    unsafe {
        ret(c::unlinkat(
            borrowed_fd(dirfd),
            c_str(path),
            bitflags_bits!(flags),
        ))
    }
}

pub(crate) fn rename(old_path: &CStr, new_path: &CStr) -> io::Result<()> {
    unsafe { ret(c::rename(c_str(old_path), c_str(new_path))) }
}

#[cfg(not(target_os = "redox"))]
pub(crate) fn renameat(
    old_dirfd: BorrowedFd<'_>,
    old_path: &CStr,
    new_dirfd: BorrowedFd<'_>,
    new_path: &CStr,
) -> io::Result<()> {
    // macOS ≤ 10.9 lacks `renameat`.
    #[cfg(target_os = "macos")]
    unsafe {
        weak! {
            fn renameat(
                c::c_int,
                *const ffi::c_char,
                c::c_int,
                *const ffi::c_char
            ) -> c::c_int
        }
        // If we have `renameat`, use it.
        if let Some(libc_renameat) = renameat.get() {
            return ret(libc_renameat(
                borrowed_fd(old_dirfd),
                c_str(old_path),
                borrowed_fd(new_dirfd),
                c_str(new_path),
            ));
        }
        // Otherwise, see if we can emulate the `AT_FDCWD` case.
        if borrowed_fd(old_dirfd) != c::AT_FDCWD || borrowed_fd(new_dirfd) != c::AT_FDCWD {
            return Err(io::Errno::NOSYS);
        }
        ret(c::rename(c_str(old_path), c_str(new_path)))
    }

    #[cfg(not(target_os = "macos"))]
    unsafe {
        ret(c::renameat(
            borrowed_fd(old_dirfd),
            c_str(old_path),
            borrowed_fd(new_dirfd),
            c_str(new_path),
        ))
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
pub(crate) fn renameat2(
    old_dirfd: BorrowedFd<'_>,
    old_path: &CStr,
    new_dirfd: BorrowedFd<'_>,
    new_path: &CStr,
    flags: RenameFlags,
) -> io::Result<()> {
    // `renameat2` wasn't supported in glibc until 2.28.
    weak_or_syscall! {
        fn renameat2(
            olddirfd: c::c_int,
            oldpath: *const ffi::c_char,
            newdirfd: c::c_int,
            newpath: *const ffi::c_char,
            flags: c::c_uint
        ) via SYS_renameat2 -> c::c_int
    }

    unsafe {
        ret(renameat2(
            borrowed_fd(old_dirfd),
            c_str(old_path),
            borrowed_fd(new_dirfd),
            c_str(new_path),
            flags.bits(),
        ))
    }
}

#[cfg(any(
    target_os = "android",
    all(target_os = "linux", not(target_env = "gnu")),
))]
#[inline]
pub(crate) fn renameat2(
    old_dirfd: BorrowedFd<'_>,
    old_path: &CStr,
    new_dirfd: BorrowedFd<'_>,
    new_path: &CStr,
    flags: RenameFlags,
) -> io::Result<()> {
    // At present, `libc` only has `renameat2` defined for glibc. If we have
    // no flags, we can use plain `renameat`, but otherwise we use `syscall!`.
    // to call `renameat2` ourselves.
    if flags.is_empty() {
        renameat(old_dirfd, old_path, new_dirfd, new_path)
    } else {
        syscall! {
            fn renameat2(
                olddirfd: c::c_int,
                oldpath: *const ffi::c_char,
                newdirfd: c::c_int,
                newpath: *const ffi::c_char,
                flags: c::c_uint
            ) via SYS_renameat2 -> c::c_int
        }

        unsafe {
            ret(renameat2(
                borrowed_fd(old_dirfd),
                c_str(old_path),
                borrowed_fd(new_dirfd),
                c_str(new_path),
                flags.bits(),
            ))
        }
    }
}

#[cfg(apple)]
pub(crate) fn renameat2(
    old_dirfd: BorrowedFd<'_>,
    old_path: &CStr,
    new_dirfd: BorrowedFd<'_>,
    new_path: &CStr,
    flags: RenameFlags,
) -> io::Result<()> {
    unsafe {
        // macOS < 10.12 lacks `renameatx_np`.
        weak! {
            fn renameatx_np(
                c::c_int,
                *const ffi::c_char,
                c::c_int,
                *const ffi::c_char,
                c::c_uint
            ) -> c::c_int
        }
        // If we have `renameatx_np`, use it.
        if let Some(libc_renameatx_np) = renameatx_np.get() {
            return ret(libc_renameatx_np(
                borrowed_fd(old_dirfd),
                c_str(old_path),
                borrowed_fd(new_dirfd),
                c_str(new_path),
                flags.bits(),
            ));
        }
        // Otherwise, see if we can use `rename`. There's no point in trying
        // `renamex_np` because it was added in the same macOS release as
        // `renameatx_np`.
        if !flags.is_empty()
            || borrowed_fd(old_dirfd) != c::AT_FDCWD
            || borrowed_fd(new_dirfd) != c::AT_FDCWD
        {
            return Err(io::Errno::NOSYS);
        }
        ret(c::rename(c_str(old_path), c_str(new_path)))
    }
}

pub(crate) fn symlink(old_path: &CStr, new_path: &CStr) -> io::Result<()> {
    unsafe { ret(c::symlink(c_str(old_path), c_str(new_path))) }
}

#[cfg(not(target_os = "redox"))]
pub(crate) fn symlinkat(
    old_path: &CStr,
    new_dirfd: BorrowedFd<'_>,
    new_path: &CStr,
) -> io::Result<()> {
    unsafe {
        ret(c::symlinkat(
            c_str(old_path),
            borrowed_fd(new_dirfd),
            c_str(new_path),
        ))
    }
}

pub(crate) fn stat(path: &CStr) -> io::Result<Stat> {
    // See the comments in `fstat` about using `crate::fs::statx` here.
    #[cfg(all(
        linux_kernel,
        any(
            target_pointer_width = "32",
            target_arch = "mips64",
            target_arch = "mips64r6"
        )
    ))]
    {
        match crate::fs::statx(CWD, path, AtFlags::empty(), StatxFlags::BASIC_STATS) {
            Ok(x) => statx_to_stat(x),
            Err(io::Errno::NOSYS) => statat_old(CWD, path, AtFlags::empty()),
            Err(err) => Err(err),
        }
    }

    // Main version: libc is y2038 safe. Or, the platform is not y2038 safe and
    // there's nothing practical we can do.
    #[cfg(not(all(
        linux_kernel,
        any(
            target_pointer_width = "32",
            target_arch = "mips64",
            target_arch = "mips64r6"
        )
    )))]
    unsafe {
        #[cfg(test)]
        assert_eq_size!(Stat, c::stat);

        let mut stat = MaybeUninit::<Stat>::uninit();
        ret(c::stat(c_str(path), stat.as_mut_ptr().cast()))?;
        let stat = stat.assume_init();
        #[cfg(apple)]
        let stat = fix_negative_stat_nsecs(stat);
        Ok(stat)
    }
}

pub(crate) fn lstat(path: &CStr) -> io::Result<Stat> {
    // See the comments in `fstat` about using `crate::fs::statx` here.
    #[cfg(all(
        linux_kernel,
        any(
            target_pointer_width = "32",
            target_arch = "mips64",
            target_arch = "mips64r6"
        )
    ))]
    {
        match crate::fs::statx(
            CWD,
            path,
            AtFlags::SYMLINK_NOFOLLOW,
            StatxFlags::BASIC_STATS,
        ) {
            Ok(x) => statx_to_stat(x),
            Err(io::Errno::NOSYS) => statat_old(CWD, path, AtFlags::SYMLINK_NOFOLLOW),
            Err(err) => Err(err),
        }
    }

    // Main version: libc is y2038 safe. Or, the platform is not y2038 safe and
    // there's nothing practical we can do.
    #[cfg(not(all(
        linux_kernel,
        any(
            target_pointer_width = "32",
            target_arch = "mips64",
            target_arch = "mips64r6"
        )
    )))]
    unsafe {
        #[cfg(test)]
        assert_eq_size!(Stat, c::stat);

        let mut stat = MaybeUninit::<Stat>::uninit();
        ret(c::lstat(c_str(path), stat.as_mut_ptr().cast()))?;
        let stat = stat.assume_init();
        #[cfg(apple)]
        let stat = fix_negative_stat_nsecs(stat);
        Ok(stat)
    }
}

#[cfg(not(any(target_os = "espidf", target_os = "redox")))]
pub(crate) fn statat(dirfd: BorrowedFd<'_>, path: &CStr, flags: AtFlags) -> io::Result<Stat> {
    // See the comments in `fstat` about using `crate::fs::statx` here.
    #[cfg(all(
        linux_kernel,
        any(
            target_pointer_width = "32",
            target_arch = "mips64",
            target_arch = "mips64r6"
        )
    ))]
    {
        match crate::fs::statx(dirfd, path, flags, StatxFlags::BASIC_STATS) {
            Ok(x) => statx_to_stat(x),
            Err(io::Errno::NOSYS) => statat_old(dirfd, path, flags),
            Err(err) => Err(err),
        }
    }

    // Main version: libc is y2038 safe. Or, the platform is not y2038 safe and
    // there's nothing practical we can do.
    #[cfg(not(all(
        linux_kernel,
        any(
            target_pointer_width = "32",
            target_arch = "mips64",
            target_arch = "mips64r6"
        )
    )))]
    unsafe {
        #[cfg(test)]
        assert_eq_size!(Stat, c::stat);

        let mut stat = MaybeUninit::<Stat>::uninit();
        ret(c::fstatat(
            borrowed_fd(dirfd),
            c_str(path),
            stat.as_mut_ptr().cast(),
            bitflags_bits!(flags),
        ))?;
        let stat = stat.assume_init();
        #[cfg(apple)]
        let stat = fix_negative_stat_nsecs(stat);
        Ok(stat)
    }
}

#[cfg(all(
    linux_kernel,
    any(
        target_pointer_width = "32",
        target_arch = "mips64",
        target_arch = "mips64r6"
    )
))]
fn statat_old(dirfd: BorrowedFd<'_>, path: &CStr, flags: AtFlags) -> io::Result<Stat> {
    unsafe {
        let mut result = MaybeUninit::<c::stat64>::uninit();
        ret(c::fstatat(
            borrowed_fd(dirfd),
            c_str(path),
            result.as_mut_ptr(),
            bitflags_bits!(flags),
        ))?;
        stat64_to_stat(result.assume_init())
    }
}

#[cfg(not(any(
    target_os = "espidf",
    target_os = "horizon",
    target_os = "emscripten",
    target_os = "vita"
)))]
pub(crate) fn access(path: &CStr, access: Access) -> io::Result<()> {
    unsafe { ret(c::access(c_str(path), access.bits())) }
}

#[cfg(not(any(
    target_os = "emscripten",
    target_os = "espidf",
    target_os = "horizon",
    target_os = "redox",
    target_os = "vita"
)))]
pub(crate) fn accessat(
    dirfd: BorrowedFd<'_>,
    path: &CStr,
    access: Access,
    flags: AtFlags,
) -> io::Result<()> {
    // macOS ≤ 10.9 lacks `faccessat`.
    #[cfg(target_os = "macos")]
    unsafe {
        weak! {
            fn faccessat(
                c::c_int,
                *const ffi::c_char,
                c::c_int,
                c::c_int
            ) -> c::c_int
        }
        // If we have `faccessat`, use it.
        if let Some(libc_faccessat) = faccessat.get() {
            return ret(libc_faccessat(
                borrowed_fd(dirfd),
                c_str(path),
                bitflags_bits!(access),
                bitflags_bits!(flags),
            ));
        }
        // Otherwise, see if we can emulate the `AT_FDCWD` case.
        if borrowed_fd(dirfd) != c::AT_FDCWD {
            return Err(io::Errno::NOSYS);
        }
        if flags.intersects(!(AtFlags::EACCESS | AtFlags::SYMLINK_NOFOLLOW)) {
            return Err(io::Errno::INVAL);
        }
        if !flags.is_empty() {
            return Err(io::Errno::OPNOTSUPP);
        }
        ret(c::access(c_str(path), bitflags_bits!(access)))
    }

    #[cfg(not(target_os = "macos"))]
    unsafe {
        ret(c::faccessat(
            borrowed_fd(dirfd),
            c_str(path),
            bitflags_bits!(access),
            bitflags_bits!(flags),
        ))
    }
}

#[cfg(target_os = "emscripten")]
pub(crate) fn access(_path: &CStr, _access: Access) -> io::Result<()> {
    Ok(())
}

#[cfg(target_os = "emscripten")]
pub(crate) fn accessat(
    _dirfd: BorrowedFd<'_>,
    _path: &CStr,
    _access: Access,
    _flags: AtFlags,
) -> io::Result<()> {
    Ok(())
}

#[cfg(not(any(
    target_os = "espidf",
    target_os = "horizon",
    target_os = "redox",
    target_os = "vita"
)))]
pub(crate) fn utimensat(
    dirfd: BorrowedFd<'_>,
    path: &CStr,
    times: &Timestamps,
    flags: AtFlags,
) -> io::Result<()> {
    // Old 32-bit version: libc has `utimensat` but it is not y2038 safe by
    // default. But there may be a `__utimensat64` we can use.
    #[cfg(all(fix_y2038, not(apple)))]
    {
        #[cfg(target_env = "gnu")]
        if let Some(libc_utimensat) = __utimensat64.get() {
            let libc_times: [LibcTimespec; 2] =
                [times.last_access.into(), times.last_modification.into()];

            unsafe {
                return ret(libc_utimensat(
                    borrowed_fd(dirfd),
                    c_str(path),
                    libc_times.as_ptr(),
                    bitflags_bits!(flags),
                ));
            }
        }

        utimensat_old(dirfd, path, times, flags)
    }

    // Main version: libc is y2038 safe and has `utimensat`. Or, the platform
    // is not y2038 safe and there's nothing practical we can do.
    #[cfg(not(any(apple, fix_y2038)))]
    unsafe {
        use crate::utils::as_ptr;

        ret(c::utimensat(
            borrowed_fd(dirfd),
            c_str(path),
            as_ptr(times).cast(),
            bitflags_bits!(flags),
        ))
    }

    // Apple version: `utimensat` was introduced in macOS 10.13.
    #[cfg(apple)]
    unsafe {
        use crate::utils::as_ptr;

        // ABI details
        weak! {
            fn utimensat(
                c::c_int,
                *const ffi::c_char,
                *const c::timespec,
                c::c_int
            ) -> c::c_int
        }
        extern "C" {
            fn setattrlist(
                path: *const ffi::c_char,
                attr_list: *const Attrlist,
                attr_buf: *const c::c_void,
                attr_buf_size: c::size_t,
                options: c::c_ulong,
            ) -> c::c_int;
        }
        const FSOPT_NOFOLLOW: c::c_ulong = 0x0000_0001;

        // If we have `utimensat`, use it.
        if let Some(have_utimensat) = utimensat.get() {
            return ret(have_utimensat(
                borrowed_fd(dirfd),
                c_str(path),
                as_ptr(times).cast(),
                bitflags_bits!(flags),
            ));
        }

        // Convert `times`. We only need this in the child, but do it before
        // calling `fork` because it might fail.
        let (attrbuf_size, times, attrs) = times_to_attrlist(times)?;

        // `setattrlistat` was introduced in 10.13 along with `utimensat`, so
        // if we don't have `utimensat`, we don't have `setattrlistat` either.
        // Emulate it using `fork`, and `fchdir` and [`setattrlist`].
        //
        // [`setattrlist`]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/setattrlist.2.html
        match c::fork() {
            -1 => Err(io::Errno::IO),
            0 => {
                if c::fchdir(borrowed_fd(dirfd)) != 0 {
                    let code = match libc_errno::errno().0 {
                        c::EACCES => 2,
                        c::ENOTDIR => 3,
                        _ => 1,
                    };
                    c::_exit(code);
                }

                let mut flags_arg = 0;
                if flags.contains(AtFlags::SYMLINK_NOFOLLOW) {
                    flags_arg |= FSOPT_NOFOLLOW;
                }

                if setattrlist(
                    c_str(path),
                    &attrs,
                    as_ptr(&times).cast(),
                    attrbuf_size,
                    flags_arg,
                ) != 0
                {
                    // Translate expected `errno` codes into ad-hoc integer
                    // values suitable for exit statuses.
                    let code = match libc_errno::errno().0 {
                        c::EACCES => 2,
                        c::ENOTDIR => 3,
                        c::EPERM => 4,
                        c::EROFS => 5,
                        c::ELOOP => 6,
                        c::ENOENT => 7,
                        c::ENAMETOOLONG => 8,
                        c::EINVAL => 9,
                        c::ESRCH => 10,
                        c::ENOTSUP => 11,
                        _ => 1,
                    };
                    c::_exit(code);
                }

                c::_exit(0);
            }
            child_pid => {
                let mut wstatus = 0;
                let _ = ret_c_int(c::waitpid(child_pid, &mut wstatus, 0))?;
                if c::WIFEXITED(wstatus) {
                    // Translate our ad-hoc exit statuses back to `errno`
                    // codes.
                    match c::WEXITSTATUS(wstatus) {
                        0 => Ok(()),
                        2 => Err(io::Errno::ACCESS),
                        3 => Err(io::Errno::NOTDIR),
                        4 => Err(io::Errno::PERM),
                        5 => Err(io::Errno::ROFS),
                        6 => Err(io::Errno::LOOP),
                        7 => Err(io::Errno::NOENT),
                        8 => Err(io::Errno::NAMETOOLONG),
                        9 => Err(io::Errno::INVAL),
                        10 => Err(io::Errno::SRCH),
                        11 => Err(io::Errno::NOTSUP),
                        _ => Err(io::Errno::IO),
                    }
                } else {
                    Err(io::Errno::IO)
                }
            }
        }
    }
}

#[cfg(all(fix_y2038, not(apple)))]
fn utimensat_old(
    dirfd: BorrowedFd<'_>,
    path: &CStr,
    times: &Timestamps,
    flags: AtFlags,
) -> io::Result<()> {
    let old_times = [
        c::timespec {
            tv_sec: times
                .last_access
                .tv_sec
                .try_into()
                .map_err(|_| io::Errno::OVERFLOW)?,
            tv_nsec: times
                .last_access
                .tv_nsec
                .try_into()
                .map_err(|_| io::Errno::OVERFLOW)?,
        },
        c::timespec {
            tv_sec: times
                .last_modification
                .tv_sec
                .try_into()
                .map_err(|_| io::Errno::OVERFLOW)?,
            tv_nsec: times
                .last_modification
                .tv_nsec
                .try_into()
                .map_err(|_| io::Errno::OVERFLOW)?,
        },
    ];
    unsafe {
        ret(c::utimensat(
            borrowed_fd(dirfd),
            c_str(path),
            old_times.as_ptr(),
            bitflags_bits!(flags),
        ))
    }
}

#[cfg(not(target_os = "wasi"))]
pub(crate) fn chmod(path: &CStr, mode: Mode) -> io::Result<()> {
    unsafe { ret(c::chmod(c_str(path), mode.bits() as c::mode_t)) }
}

#[cfg(not(any(
    linux_kernel,
    target_os = "espidf",
    target_os = "redox",
    target_os = "wasi"
)))]
pub(crate) fn chmodat(
    dirfd: BorrowedFd<'_>,
    path: &CStr,
    mode: Mode,
    flags: AtFlags,
) -> io::Result<()> {
    unsafe {
        ret(c::fchmodat(
            borrowed_fd(dirfd),
            c_str(path),
            mode.bits() as c::mode_t,
            bitflags_bits!(flags),
        ))
    }
}

#[cfg(linux_kernel)]
pub(crate) fn chmodat(
    dirfd: BorrowedFd<'_>,
    path: &CStr,
    mode: Mode,
    flags: AtFlags,
) -> io::Result<()> {
    // Linux's `fchmodat` does not have a flags argument.
    //
    // Use `c::syscall` rather than `c::fchmodat` because some libc
    // implementations, such as musl, add extra logic to `fchmod` to emulate
    // support for `AT_SYMLINK_NOFOLLOW`, which uses `/proc` outside our
    // control.
    syscall! {
        fn fchmodat(
            base_dirfd: c::c_int,
            pathname: *const ffi::c_char,
            mode: c::mode_t
        ) via SYS_fchmodat -> c::c_int
    }
    if flags == AtFlags::SYMLINK_NOFOLLOW {
        return Err(io::Errno::OPNOTSUPP);
    }
    if !flags.is_empty() {
        return Err(io::Errno::INVAL);
    }
    unsafe {
        ret(fchmodat(
            borrowed_fd(dirfd),
            c_str(path),
            mode.bits() as c::mode_t,
        ))
    }
}

#[cfg(apple)]
pub(crate) fn fclonefileat(
    srcfd: BorrowedFd<'_>,
    dst_dirfd: BorrowedFd<'_>,
    dst: &CStr,
    flags: CloneFlags,
) -> io::Result<()> {
    syscall! {
        fn fclonefileat(
            srcfd: BorrowedFd<'_>,
            dst_dirfd: BorrowedFd<'_>,
            dst: *const ffi::c_char,
            flags: c::c_int
        ) via SYS_fclonefileat -> c::c_int
    }

    unsafe {
        ret(fclonefileat(
            srcfd,
            dst_dirfd,
            c_str(dst),
            bitflags_bits!(flags),
        ))
    }
}

#[cfg(not(any(target_os = "espidf", target_os = "redox", target_os = "wasi")))]
pub(crate) fn chownat(
    dirfd: BorrowedFd<'_>,
    path: &CStr,
    owner: Option<Uid>,
    group: Option<Gid>,
    flags: AtFlags,
) -> io::Result<()> {
    unsafe {
        let (ow, gr) = crate::ugid::translate_fchown_args(owner, group);
        ret(c::fchownat(
            borrowed_fd(dirfd),
            c_str(path),
            ow,
            gr,
            bitflags_bits!(flags),
        ))
    }
}

#[cfg(not(any(
    apple,
    target_os = "espidf",
    target_os = "horizon",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
pub(crate) fn mknodat(
    dirfd: BorrowedFd<'_>,
    path: &CStr,
    file_type: FileType,
    mode: Mode,
    dev: Dev,
) -> io::Result<()> {
    unsafe {
        ret(c::mknodat(
            borrowed_fd(dirfd),
            c_str(path),
            (mode.bits() | file_type.as_raw_mode()) as c::mode_t,
            dev.try_into().map_err(|_e| io::Errno::PERM)?,
        ))
    }
}

#[cfg(linux_kernel)]
pub(crate) fn copy_file_range(
    fd_in: BorrowedFd<'_>,
    off_in: Option<&mut u64>,
    fd_out: BorrowedFd<'_>,
    off_out: Option<&mut u64>,
    len: usize,
) -> io::Result<usize> {
    syscall! {
        fn copy_file_range(
            fd_in: c::c_int,
            off_in: *mut c::loff_t,
            fd_out: c::c_int,
            off_out: *mut c::loff_t,
            len: usize,
            flags: c::c_uint
        ) via SYS_copy_file_range -> c::ssize_t
    }

    let mut off_in_val: c::loff_t = 0;
    let mut off_out_val: c::loff_t = 0;
    // Silently cast; we'll get `EINVAL` if the value is negative.
    let off_in_ptr = if let Some(off_in) = &off_in {
        off_in_val = **off_in as i64;
        &mut off_in_val
    } else {
        null_mut()
    };
    let off_out_ptr = if let Some(off_out) = &off_out {
        off_out_val = **off_out as i64;
        &mut off_out_val
    } else {
        null_mut()
    };
    let copied = unsafe {
        ret_usize(copy_file_range(
            borrowed_fd(fd_in),
            off_in_ptr,
            borrowed_fd(fd_out),
            off_out_ptr,
            len,
            0, // no flags are defined yet
        ))?
    };
    if let Some(off_in) = off_in {
        *off_in = off_in_val as u64;
    }
    if let Some(off_out) = off_out {
        *off_out = off_out_val as u64;
    }
    Ok(copied)
}

#[cfg(not(any(
    apple,
    netbsdlike,
    target_os = "dragonfly",
    target_os = "espidf",
    target_os = "haiku",
    target_os = "horizon",
    target_os = "redox",
    target_os = "solaris",
    target_os = "vita",
)))]
pub(crate) fn fadvise(
    fd: BorrowedFd<'_>,
    offset: u64,
    len: Option<NonZeroU64>,
    advice: Advice,
) -> io::Result<()> {
    let offset = offset as i64;
    let len = match len {
        None => 0,
        Some(len) => len.get() as i64,
    };

    // Our public API uses `u64` following the [Rust convention], but the
    // underlying host APIs use a signed `off_t`. Converting these values may
    // turn a very large value into a negative value.
    //
    // On FreeBSD, this could cause `posix_fadvise` to fail with
    // `Errno::INVAL`. Because we don't expose the signed type in our API, we
    // also avoid exposing this artifact of casting an unsigned value to the
    // signed type. To do this, we use a no-op call in this case.
    //
    // [Rust convention]: std::io::SeekFrom::Start
    #[cfg(target_os = "freebsd")]
    if offset < 0 {
        if len < 0 {
            return Err(io::Errno::INVAL);
        }

        return fadvise_noop(fd);

        #[cold]
        fn fadvise_noop(fd: BorrowedFd<'_>) -> io::Result<()> {
            // Use an `fcntl` to report `Errno::BADF` if needed, but otherwise
            // do nothing.
            fcntl_getfl(fd).map(|_| ())
        }
    }

    // Similarly, on FreeBSD, if `offset + len` would overflow an `off_t` in a
    // way that users using a `u64` interface wouldn't be aware of, reduce the
    // length so that we only operate on the range that doesn't overflow.
    #[cfg(target_os = "freebsd")]
    let len = if len > 0 && offset.checked_add(len).is_none() {
        i64::MAX - offset
    } else {
        len
    };

    let err = unsafe { c::posix_fadvise(borrowed_fd(fd), offset, len, advice as c::c_int) };

    // `posix_fadvise` returns its error status rather than using `errno`.
    if err == 0 {
        Ok(())
    } else {
        Err(io::Errno(err))
    }
}

pub(crate) fn fcntl_getfl(fd: BorrowedFd<'_>) -> io::Result<OFlags> {
    let flags = unsafe { ret_c_int(c::fcntl(borrowed_fd(fd), c::F_GETFL))? };
    Ok(OFlags::from_bits_retain(bitcast!(flags)))
}

pub(crate) fn fcntl_setfl(fd: BorrowedFd<'_>, flags: OFlags) -> io::Result<()> {
    unsafe { ret(c::fcntl(borrowed_fd(fd), c::F_SETFL, flags.bits())) }
}

#[cfg(any(linux_kernel, target_os = "freebsd", target_os = "fuchsia"))]
pub(crate) fn fcntl_get_seals(fd: BorrowedFd<'_>) -> io::Result<SealFlags> {
    let flags = unsafe { ret_c_int(c::fcntl(borrowed_fd(fd), c::F_GET_SEALS))? };
    Ok(SealFlags::from_bits_retain(bitcast!(flags)))
}

#[cfg(any(linux_kernel, target_os = "freebsd", target_os = "fuchsia"))]
pub(crate) fn fcntl_add_seals(fd: BorrowedFd<'_>, seals: SealFlags) -> io::Result<()> {
    unsafe { ret(c::fcntl(borrowed_fd(fd), c::F_ADD_SEALS, seals.bits())) }
}

#[cfg(not(any(
    target_os = "emscripten",
    target_os = "espidf",
    target_os = "fuchsia",
    target_os = "horizon",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
#[inline]
pub(crate) fn fcntl_lock(fd: BorrowedFd<'_>, operation: FlockOperation) -> io::Result<()> {
    use c::{flock, F_RDLCK, F_SETLK, F_SETLKW, F_UNLCK, F_WRLCK, SEEK_SET};

    let (cmd, l_type) = match operation {
        FlockOperation::LockShared => (F_SETLKW, F_RDLCK),
        FlockOperation::LockExclusive => (F_SETLKW, F_WRLCK),
        FlockOperation::Unlock => (F_SETLKW, F_UNLCK),
        FlockOperation::NonBlockingLockShared => (F_SETLK, F_RDLCK),
        FlockOperation::NonBlockingLockExclusive => (F_SETLK, F_WRLCK),
        FlockOperation::NonBlockingUnlock => (F_SETLK, F_UNLCK),
    };

    unsafe {
        let mut lock: flock = core::mem::zeroed();
        lock.l_type = l_type as _;

        // When `l_len` is zero, this locks all the bytes from
        // `l_whence`/`l_start` to the end of the file, even as the
        // file grows dynamically.
        lock.l_whence = SEEK_SET as _;
        lock.l_start = 0;
        lock.l_len = 0;

        ret(c::fcntl(borrowed_fd(fd), cmd, &lock))
    }
}

pub(crate) fn seek(fd: BorrowedFd<'_>, pos: SeekFrom) -> io::Result<u64> {
    let (whence, offset) = match pos {
        SeekFrom::Start(pos) => {
            let pos: u64 = pos;
            // Silently cast; we'll get `EINVAL` if the value is negative.
            (c::SEEK_SET, pos as i64)
        }
        SeekFrom::End(offset) => (c::SEEK_END, offset),
        SeekFrom::Current(offset) => (c::SEEK_CUR, offset),
        #[cfg(any(apple, freebsdlike, linux_kernel, solarish))]
        SeekFrom::Data(pos) => {
            let pos: u64 = pos;
            // Silently cast; we'll get `EINVAL` if the value is negative.
            (c::SEEK_DATA, pos as i64)
        }
        #[cfg(any(apple, freebsdlike, linux_kernel, solarish))]
        SeekFrom::Hole(pos) => {
            let pos: u64 = pos;
            // Silently cast; we'll get `EINVAL` if the value is negative.
            (c::SEEK_HOLE, pos as i64)
        }
    };

    // ESP-IDF and Vita don't support 64-bit offsets, for example.
    let offset = offset.try_into().map_err(|_| io::Errno::OVERFLOW)?;

    let offset = unsafe { ret_off_t(c::lseek(borrowed_fd(fd), offset, whence))? };
    Ok(offset as u64)
}

pub(crate) fn tell(fd: BorrowedFd<'_>) -> io::Result<u64> {
    let offset = unsafe { ret_off_t(c::lseek(borrowed_fd(fd), 0, c::SEEK_CUR))? };
    Ok(offset as u64)
}

#[cfg(not(any(linux_kernel, target_os = "wasi")))]
pub(crate) fn fchmod(fd: BorrowedFd<'_>, mode: Mode) -> io::Result<()> {
    unsafe { ret(c::fchmod(borrowed_fd(fd), bitflags_bits!(mode))) }
}

#[cfg(linux_kernel)]
pub(crate) fn fchmod(fd: BorrowedFd<'_>, mode: Mode) -> io::Result<()> {
    // Use `c::syscall` rather than `c::fchmod` because some libc
    // implementations, such as musl, add extra logic to `fchmod` to emulate
    // support for `O_PATH`, which uses `/proc` outside our control and
    // interferes with our own use of `O_PATH`.
    syscall! {
        fn fchmod(
            fd: c::c_int,
            mode: c::mode_t
        ) via SYS_fchmod -> c::c_int
    }
    unsafe { ret(fchmod(borrowed_fd(fd), mode.bits() as c::mode_t)) }
}

#[cfg(not(target_os = "wasi"))]
pub(crate) fn chown(path: &CStr, owner: Option<Uid>, group: Option<Gid>) -> io::Result<()> {
    unsafe {
        let (ow, gr) = crate::ugid::translate_fchown_args(owner, group);
        ret(c::chown(c_str(path), ow, gr))
    }
}

#[cfg(linux_kernel)]
pub(crate) fn fchown(fd: BorrowedFd<'_>, owner: Option<Uid>, group: Option<Gid>) -> io::Result<()> {
    // Use `c::syscall` rather than `c::fchown` because some libc
    // implementations, such as musl, add extra logic to `fchown` to emulate
    // support for `O_PATH`, which uses `/proc` outside our control and
    // interferes with our own use of `O_PATH`.
    syscall! {
        fn fchown(
            fd: c::c_int,
            owner: c::uid_t,
            group: c::gid_t
        ) via SYS_fchown -> c::c_int
    }
    unsafe {
        let (ow, gr) = crate::ugid::translate_fchown_args(owner, group);
        ret(fchown(borrowed_fd(fd), ow, gr))
    }
}

#[cfg(not(any(linux_kernel, target_os = "wasi")))]
pub(crate) fn fchown(fd: BorrowedFd<'_>, owner: Option<Uid>, group: Option<Gid>) -> io::Result<()> {
    unsafe {
        let (ow, gr) = crate::ugid::translate_fchown_args(owner, group);
        ret(c::fchown(borrowed_fd(fd), ow, gr))
    }
}

#[cfg(not(any(
    target_os = "espidf",
    target_os = "horizon",
    target_os = "solaris",
    target_os = "vita",
    target_os = "wasi"
)))]
pub(crate) fn flock(fd: BorrowedFd<'_>, operation: FlockOperation) -> io::Result<()> {
    unsafe { ret(c::flock(borrowed_fd(fd), operation as c::c_int)) }
}

#[cfg(linux_kernel)]
pub(crate) fn syncfs(fd: BorrowedFd<'_>) -> io::Result<()> {
    // Some versions of Android libc lack a `syncfs` function.
    #[cfg(target_os = "android")]
    syscall! {
        fn syncfs(fd: c::c_int) via SYS_syncfs -> c::c_int
    }

    // `syncfs` was added to glibc in 2.20.
    #[cfg(not(target_os = "android"))]
    weak_or_syscall! {
        fn syncfs(fd: c::c_int) via SYS_syncfs -> c::c_int
    }

    unsafe { ret(syncfs(borrowed_fd(fd))) }
}

#[cfg(not(any(
    target_os = "espidf",
    target_os = "horizon",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
pub(crate) fn sync() {
    unsafe { c::sync() }
}

pub(crate) fn fstat(fd: BorrowedFd<'_>) -> io::Result<Stat> {
    // 32-bit and mips64 Linux: `struct stat64` is not y2038 compatible; use
    // `statx`.
    //
    // And, some old platforms don't support `statx`, and some fail with a
    // confusing error code, so we call `crate::fs::statx` to handle that. If
    // `statx` isn't available, fall back to the buggy system call.
    #[cfg(all(
        linux_kernel,
        any(
            target_pointer_width = "32",
            target_arch = "mips64",
            target_arch = "mips64r6"
        )
    ))]
    {
        match crate::fs::statx(fd, cstr!(""), AtFlags::EMPTY_PATH, StatxFlags::BASIC_STATS) {
            Ok(x) => statx_to_stat(x),
            Err(io::Errno::NOSYS) => fstat_old(fd),
            Err(err) => Err(err),
        }
    }

    // Main version: libc is y2038 safe. Or, the platform is not y2038 safe and
    // there's nothing practical we can do.
    #[cfg(not(all(
        linux_kernel,
        any(
            target_pointer_width = "32",
            target_arch = "mips64",
            target_arch = "mips64r6"
        )
    )))]
    unsafe {
        #[cfg(test)]
        assert_eq_size!(Stat, c::stat);

        let mut stat = MaybeUninit::<Stat>::uninit();
        ret(c::fstat(borrowed_fd(fd), stat.as_mut_ptr().cast()))?;
        let stat = stat.assume_init();
        #[cfg(apple)]
        let stat = fix_negative_stat_nsecs(stat);
        Ok(stat)
    }
}

#[cfg(all(
    linux_kernel,
    any(
        target_pointer_width = "32",
        target_arch = "mips64",
        target_arch = "mips64r6"
    )
))]
fn fstat_old(fd: BorrowedFd<'_>) -> io::Result<Stat> {
    unsafe {
        let mut result = MaybeUninit::<c::stat64>::uninit();
        ret(c::fstat(borrowed_fd(fd), result.as_mut_ptr()))?;
        stat64_to_stat(result.assume_init())
    }
}

#[cfg(not(any(
    solarish,
    target_os = "espidf",
    target_os = "haiku",
    target_os = "horizon",
    target_os = "netbsd",
    target_os = "nto",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi",
)))]
pub(crate) fn fstatfs(fd: BorrowedFd<'_>) -> io::Result<StatFs> {
    let mut statfs = MaybeUninit::<StatFs>::uninit();
    unsafe {
        ret(c::fstatfs(borrowed_fd(fd), statfs.as_mut_ptr()))?;
        Ok(statfs.assume_init())
    }
}

#[cfg(not(any(target_os = "haiku", target_os = "redox", target_os = "wasi")))]
pub(crate) fn fstatvfs(fd: BorrowedFd<'_>) -> io::Result<StatVfs> {
    let mut statvfs = MaybeUninit::<c::statvfs>::uninit();
    unsafe {
        ret(c::fstatvfs(borrowed_fd(fd), statvfs.as_mut_ptr()))?;
        Ok(libc_statvfs_to_statvfs(statvfs.assume_init()))
    }
}

#[cfg(not(any(target_os = "haiku", target_os = "redox", target_os = "wasi")))]
fn libc_statvfs_to_statvfs(from: c::statvfs) -> StatVfs {
    StatVfs {
        f_bsize: from.f_bsize as u64,
        f_frsize: from.f_frsize as u64,
        f_blocks: from.f_blocks as u64,
        f_bfree: from.f_bfree as u64,
        f_bavail: from.f_bavail as u64,
        f_files: from.f_files as u64,
        f_ffree: from.f_ffree as u64,
        f_favail: from.f_ffree as u64,
        #[cfg(not(target_os = "aix"))]
        f_fsid: from.f_fsid as u64,
        #[cfg(target_os = "aix")]
        f_fsid: ((from.f_fsid.val[0] as u64) << 32) | from.f_fsid.val[1],
        f_flag: StatVfsMountFlags::from_bits_retain(from.f_flag as u64),
        f_namemax: from.f_namemax as u64,
    }
}

#[cfg(not(any(target_os = "espidf", target_os = "horizon", target_os = "vita")))]
pub(crate) fn futimens(fd: BorrowedFd<'_>, times: &Timestamps) -> io::Result<()> {
    // Old 32-bit version: libc has `futimens` but it is not y2038 safe by
    // default. But there may be a `__futimens64` we can use.
    #[cfg(all(fix_y2038, not(apple)))]
    {
        #[cfg(target_env = "gnu")]
        if let Some(libc_futimens) = __futimens64.get() {
            let libc_times: [LibcTimespec; 2] =
                [times.last_access.into(), times.last_modification.into()];

            unsafe {
                return ret(libc_futimens(borrowed_fd(fd), libc_times.as_ptr()));
            }
        }

        futimens_old(fd, times)
    }

    // Main version: libc is y2038 safe and has `futimens`. Or, the platform
    // is not y2038 safe and there's nothing practical we can do.
    #[cfg(not(any(apple, fix_y2038)))]
    unsafe {
        use crate::utils::as_ptr;

        ret(c::futimens(borrowed_fd(fd), as_ptr(times).cast()))
    }

    // Apple version: `futimens` was introduced in macOS 10.13.
    #[cfg(apple)]
    unsafe {
        use crate::utils::as_ptr;

        // ABI details.
        weak! {
            fn futimens(c::c_int, *const c::timespec) -> c::c_int
        }
        extern "C" {
            fn fsetattrlist(
                fd: c::c_int,
                attr_list: *const Attrlist,
                attr_buf: *const c::c_void,
                attr_buf_size: c::size_t,
                options: c::c_ulong,
            ) -> c::c_int;
        }

        // If we have `futimens`, use it.
        if let Some(have_futimens) = futimens.get() {
            return ret(have_futimens(borrowed_fd(fd), as_ptr(times).cast()));
        }

        // Otherwise use `fsetattrlist`.
        let (attrbuf_size, times, attrs) = times_to_attrlist(times)?;

        ret(fsetattrlist(
            borrowed_fd(fd),
            &attrs,
            as_ptr(&times).cast(),
            attrbuf_size,
            0,
        ))
    }
}

#[cfg(all(fix_y2038, not(apple)))]
fn futimens_old(fd: BorrowedFd<'_>, times: &Timestamps) -> io::Result<()> {
    let old_times = [
        c::timespec {
            tv_sec: times
                .last_access
                .tv_sec
                .try_into()
                .map_err(|_| io::Errno::OVERFLOW)?,
            tv_nsec: times
                .last_access
                .tv_nsec
                .try_into()
                .map_err(|_| io::Errno::OVERFLOW)?,
        },
        c::timespec {
            tv_sec: times
                .last_modification
                .tv_sec
                .try_into()
                .map_err(|_| io::Errno::OVERFLOW)?,
            tv_nsec: times
                .last_modification
                .tv_nsec
                .try_into()
                .map_err(|_| io::Errno::OVERFLOW)?,
        },
    ];

    unsafe { ret(c::futimens(borrowed_fd(fd), old_times.as_ptr())) }
}

#[cfg(not(any(
    apple,
    netbsdlike,
    target_os = "dragonfly",
    target_os = "espidf",
    target_os = "horizon",
    target_os = "nto",
    target_os = "redox",
    target_os = "vita",
)))]
pub(crate) fn fallocate(
    fd: BorrowedFd<'_>,
    mode: FallocateFlags,
    offset: u64,
    len: u64,
) -> io::Result<()> {
    // Silently cast to `i64`; we'll get `EINVAL` if the value is negative.
    let offset = offset as i64;
    let len = len as i64;

    // ESP-IDF and Vita don't support 64-bit offsets, for example.
    let offset = offset.try_into().map_err(|_| io::Errno::OVERFLOW)?;
    let len = len.try_into().map_err(|_| io::Errno::OVERFLOW)?;

    #[cfg(any(linux_kernel, target_os = "fuchsia"))]
    unsafe {
        ret(c::fallocate(
            borrowed_fd(fd),
            bitflags_bits!(mode),
            offset,
            len,
        ))
    }

    #[cfg(not(any(linux_kernel, target_os = "fuchsia")))]
    {
        assert!(mode.is_empty());
        let err = unsafe { c::posix_fallocate(borrowed_fd(fd), offset, len) };

        // `posix_fallocate` returns its error status rather than using
        // `errno`.
        if err == 0 {
            Ok(())
        } else {
            Err(io::Errno(err))
        }
    }
}

#[cfg(apple)]
pub(crate) fn fallocate(
    fd: BorrowedFd<'_>,
    mode: FallocateFlags,
    offset: u64,
    len: u64,
) -> io::Result<()> {
    let offset: i64 = offset.try_into().map_err(|_e| io::Errno::INVAL)?;
    let len = len as i64;

    assert!(mode.is_empty());

    let new_len = offset.checked_add(len).ok_or(io::Errno::FBIG)?;
    let mut store = c::fstore_t {
        fst_flags: c::F_ALLOCATECONTIG,
        fst_posmode: c::F_PEOFPOSMODE,
        fst_offset: 0,
        fst_length: new_len,
        fst_bytesalloc: 0,
    };
    unsafe {
        if c::fcntl(borrowed_fd(fd), c::F_PREALLOCATE, &store) == -1 {
            // Unable to allocate contiguous disk space; attempt to allocate
            // non-contiguously.
            store.fst_flags = c::F_ALLOCATEALL;
            let _ = ret_c_int(c::fcntl(borrowed_fd(fd), c::F_PREALLOCATE, &store))?;
        }
        ret(c::ftruncate(borrowed_fd(fd), new_len))
    }
}

pub(crate) fn fsync(fd: BorrowedFd<'_>) -> io::Result<()> {
    unsafe { ret(c::fsync(borrowed_fd(fd))) }
}

#[cfg(not(any(
    apple,
    target_os = "dragonfly",
    target_os = "espidf",
    target_os = "haiku",
    target_os = "horizon",
    target_os = "redox",
    target_os = "vita",
)))]
pub(crate) fn fdatasync(fd: BorrowedFd<'_>) -> io::Result<()> {
    unsafe { ret(c::fdatasync(borrowed_fd(fd))) }
}

pub(crate) fn ftruncate(fd: BorrowedFd<'_>, length: u64) -> io::Result<()> {
    let length = length.try_into().map_err(|_overflow_err| io::Errno::FBIG)?;
    unsafe { ret(c::ftruncate(borrowed_fd(fd), length)) }
}

#[cfg(any(linux_kernel, target_os = "freebsd"))]
pub(crate) fn memfd_create(name: &CStr, flags: MemfdFlags) -> io::Result<OwnedFd> {
    #[cfg(target_os = "freebsd")]
    weakcall! {
        fn memfd_create(
            name: *const ffi::c_char,
            flags: c::c_uint
        ) -> c::c_int
    }

    #[cfg(linux_kernel)]
    weak_or_syscall! {
        fn memfd_create(
            name: *const ffi::c_char,
            flags: c::c_uint
        ) via SYS_memfd_create -> c::c_int
    }

    unsafe { ret_owned_fd(memfd_create(c_str(name), bitflags_bits!(flags))) }
}

#[cfg(linux_kernel)]
pub(crate) fn openat2(
    dirfd: BorrowedFd<'_>,
    path: &CStr,
    oflags: OFlags,
    mode: Mode,
    resolve: ResolveFlags,
) -> io::Result<OwnedFd> {
    use linux_raw_sys::general::open_how;

    syscall! {
        fn openat2(
            base_dirfd: c::c_int,
            pathname: *const ffi::c_char,
            how: *mut open_how,
            size: usize
        ) via SYS_OPENAT2 -> c::c_int
    }

    let oflags = oflags.bits();
    let mut open_how = open_how {
        flags: u64::from(oflags),
        mode: u64::from(mode.bits()),
        resolve: resolve.bits(),
    };

    unsafe {
        ret_owned_fd(openat2(
            borrowed_fd(dirfd),
            c_str(path),
            &mut open_how,
            size_of::<open_how>(),
        ))
    }
}
#[cfg(all(linux_kernel, target_pointer_width = "32"))]
const SYS_OPENAT2: i32 = 437;
#[cfg(all(linux_kernel, target_pointer_width = "64"))]
const SYS_OPENAT2: i64 = 437;

#[cfg(target_os = "linux")]
pub(crate) fn sendfile(
    out_fd: BorrowedFd<'_>,
    in_fd: BorrowedFd<'_>,
    offset: Option<&mut u64>,
    count: usize,
) -> io::Result<usize> {
    unsafe {
        ret_usize(c::sendfile64(
            borrowed_fd(out_fd),
            borrowed_fd(in_fd),
            offset.map_or(null_mut(), crate::utils::as_mut_ptr).cast(),
            count,
        ))
    }
}

/// Convert from a Linux `statx` value to rustix's `Stat`.
#[cfg(all(linux_kernel, target_pointer_width = "32"))]
fn statx_to_stat(x: crate::fs::Statx) -> io::Result<Stat> {
    Ok(Stat {
        st_dev: crate::fs::makedev(x.stx_dev_major, x.stx_dev_minor).into(),
        st_mode: x.stx_mode.into(),
        st_nlink: x.stx_nlink.into(),
        st_uid: x.stx_uid.into(),
        st_gid: x.stx_gid.into(),
        st_rdev: crate::fs::makedev(x.stx_rdev_major, x.stx_rdev_minor).into(),
        st_size: x.stx_size.try_into().map_err(|_| io::Errno::OVERFLOW)?,
        st_blksize: x.stx_blksize.into(),
        st_blocks: x.stx_blocks.into(),
        st_atime: bitcast!(i64::from(x.stx_atime.tv_sec)),
        st_atime_nsec: x.stx_atime.tv_nsec as _,
        st_mtime: bitcast!(i64::from(x.stx_mtime.tv_sec)),
        st_mtime_nsec: x.stx_mtime.tv_nsec as _,
        st_ctime: bitcast!(i64::from(x.stx_ctime.tv_sec)),
        st_ctime_nsec: x.stx_ctime.tv_nsec as _,
        st_ino: x.stx_ino.into(),
    })
}

/// Convert from a Linux `statx` value to rustix's `Stat`.
///
/// mips64' `struct stat64` in libc has private fields, and `stx_blocks`
#[cfg(all(linux_kernel, any(target_arch = "mips64", target_arch = "mips64r6")))]
fn statx_to_stat(x: crate::fs::Statx) -> io::Result<Stat> {
    let mut result: Stat = unsafe { core::mem::zeroed() };

    result.st_dev = crate::fs::makedev(x.stx_dev_major, x.stx_dev_minor);
    result.st_mode = x.stx_mode.into();
    result.st_nlink = x.stx_nlink.into();
    result.st_uid = x.stx_uid.into();
    result.st_gid = x.stx_gid.into();
    result.st_rdev = crate::fs::makedev(x.stx_rdev_major, x.stx_rdev_minor);
    result.st_size = x.stx_size.try_into().map_err(|_| io::Errno::OVERFLOW)?;
    result.st_blksize = x.stx_blksize.into();
    result.st_blocks = x.stx_blocks.try_into().map_err(|_e| io::Errno::OVERFLOW)?;
    result.st_atime = bitcast!(i64::from(x.stx_atime.tv_sec));
    result.st_atime_nsec = x.stx_atime.tv_nsec as _;
    result.st_mtime = bitcast!(i64::from(x.stx_mtime.tv_sec));
    result.st_mtime_nsec = x.stx_mtime.tv_nsec as _;
    result.st_ctime = bitcast!(i64::from(x.stx_ctime.tv_sec));
    result.st_ctime_nsec = x.stx_ctime.tv_nsec as _;
    result.st_ino = x.stx_ino.into();

    Ok(result)
}

/// Convert from a Linux `stat64` value to rustix's `Stat`.
#[cfg(all(linux_kernel, target_pointer_width = "32"))]
fn stat64_to_stat(s64: c::stat64) -> io::Result<Stat> {
    Ok(Stat {
        st_dev: s64.st_dev.try_into().map_err(|_| io::Errno::OVERFLOW)?,
        st_mode: s64.st_mode.try_into().map_err(|_| io::Errno::OVERFLOW)?,
        st_nlink: s64.st_nlink.try_into().map_err(|_| io::Errno::OVERFLOW)?,
        st_uid: s64.st_uid.try_into().map_err(|_| io::Errno::OVERFLOW)?,
        st_gid: s64.st_gid.try_into().map_err(|_| io::Errno::OVERFLOW)?,
        st_rdev: s64.st_rdev.try_into().map_err(|_| io::Errno::OVERFLOW)?,
        st_size: s64.st_size.try_into().map_err(|_| io::Errno::OVERFLOW)?,
        st_blksize: s64.st_blksize.try_into().map_err(|_| io::Errno::OVERFLOW)?,
        st_blocks: s64.st_blocks.try_into().map_err(|_| io::Errno::OVERFLOW)?,
        st_atime: i64::from(s64.st_atime),
        st_atime_nsec: s64
            .st_atime_nsec
            .try_into()
            .map_err(|_| io::Errno::OVERFLOW)?,
        st_mtime: i64::from(s64.st_mtime),
        st_mtime_nsec: s64
            .st_mtime_nsec
            .try_into()
            .map_err(|_| io::Errno::OVERFLOW)?,
        st_ctime: i64::from(s64.st_ctime),
        st_ctime_nsec: s64
            .st_ctime_nsec
            .try_into()
            .map_err(|_| io::Errno::OVERFLOW)?,
        st_ino: s64.st_ino.try_into().map_err(|_| io::Errno::OVERFLOW)?,
    })
}

/// Convert from a Linux `stat64` value to rustix's `Stat`.
///
/// mips64' `struct stat64` in libc has private fields, and `st_blocks` has
/// type `i64`.
#[cfg(all(linux_kernel, any(target_arch = "mips64", target_arch = "mips64r6")))]
fn stat64_to_stat(s64: c::stat64) -> io::Result<Stat> {
    let mut result: Stat = unsafe { core::mem::zeroed() };

    result.st_dev = s64.st_dev.try_into().map_err(|_| io::Errno::OVERFLOW)?;
    result.st_mode = s64.st_mode.try_into().map_err(|_| io::Errno::OVERFLOW)?;
    result.st_nlink = s64.st_nlink.try_into().map_err(|_| io::Errno::OVERFLOW)?;
    result.st_uid = s64.st_uid.try_into().map_err(|_| io::Errno::OVERFLOW)?;
    result.st_gid = s64.st_gid.try_into().map_err(|_| io::Errno::OVERFLOW)?;
    result.st_rdev = s64.st_rdev.try_into().map_err(|_| io::Errno::OVERFLOW)?;
    result.st_size = s64.st_size.try_into().map_err(|_| io::Errno::OVERFLOW)?;
    result.st_blksize = s64.st_blksize.try_into().map_err(|_| io::Errno::OVERFLOW)?;
    result.st_blocks = s64.st_blocks.try_into().map_err(|_| io::Errno::OVERFLOW)?;
    result.st_atime = i64::from(s64.st_atime) as _;
    result.st_atime_nsec = s64
        .st_atime_nsec
        .try_into()
        .map_err(|_| io::Errno::OVERFLOW)?;
    result.st_mtime = i64::from(s64.st_mtime) as _;
    result.st_mtime_nsec = s64
        .st_mtime_nsec
        .try_into()
        .map_err(|_| io::Errno::OVERFLOW)?;
    result.st_ctime = i64::from(s64.st_ctime) as _;
    result.st_ctime_nsec = s64
        .st_ctime_nsec
        .try_into()
        .map_err(|_| io::Errno::OVERFLOW)?;
    result.st_ino = s64.st_ino.try_into().map_err(|_| io::Errno::OVERFLOW)?;

    Ok(result)
}

#[cfg(linux_kernel)]
#[allow(non_upper_case_globals)]
mod sys {
    use super::{c, ffi, BorrowedFd, Statx};

    weak_or_syscall! {
        pub(super) fn statx(
            dirfd_: BorrowedFd<'_>,
            path: *const ffi::c_char,
            flags: c::c_int,
            mask: c::c_uint,
            buf: *mut Statx
        ) via SYS_statx -> c::c_int
    }
}

#[cfg(linux_kernel)]
#[allow(non_upper_case_globals)]
pub(crate) fn statx(
    dirfd: BorrowedFd<'_>,
    path: &CStr,
    flags: AtFlags,
    mask: StatxFlags,
) -> io::Result<Statx> {
    // If a future Linux kernel adds more fields to `struct statx` and users
    // passing flags unknown to rustix in `StatxFlags`, we could end up
    // writing outside of the buffer. To prevent this possibility, we mask off
    // any flags that we don't know about.
    //
    // This includes `STATX__RESERVED`, which has a value that we know, but
    // which could take on arbitrary new meaning in the future. Linux currently
    // rejects this flag with `EINVAL`, so we do the same.
    //
    // This doesn't rely on `STATX_ALL` because [it's deprecated] and already
    // doesn't represent all the known flags.
    //
    // [it's deprecated]: https://patchwork.kernel.org/project/linux-fsdevel/patch/20200505095915.11275-7-mszeredi@redhat.com/
    #[cfg(not(any(target_os = "android", target_env = "musl")))]
    const STATX__RESERVED: u32 = c::STATX__RESERVED as u32;
    #[cfg(any(target_os = "android", target_env = "musl"))]
    const STATX__RESERVED: u32 = linux_raw_sys::general::STATX__RESERVED;
    if (mask.bits() & STATX__RESERVED) == STATX__RESERVED {
        return Err(io::Errno::INVAL);
    }
    let mask = mask & StatxFlags::all();

    let mut statx_buf = MaybeUninit::<Statx>::uninit();
    unsafe {
        ret(sys::statx(
            dirfd,
            c_str(path),
            bitflags_bits!(flags),
            mask.bits(),
            statx_buf.as_mut_ptr(),
        ))?;
        Ok(statx_buf.assume_init())
    }
}

#[cfg(all(linux_kernel, not(feature = "linux_4_11")))]
#[inline]
pub(crate) fn is_statx_available() -> bool {
    unsafe {
        // Call `statx` with null pointers so that if it fails for any reason
        // other than `EFAULT`, we know it's not supported.
        matches!(
            ret(sys::statx(CWD, null(), 0, 0, null_mut())),
            Err(io::Errno::FAULT)
        )
    }
}

#[cfg(apple)]
pub(crate) unsafe fn fcopyfile(
    from: BorrowedFd<'_>,
    to: BorrowedFd<'_>,
    state: copyfile_state_t,
    flags: CopyfileFlags,
) -> io::Result<()> {
    extern "C" {
        fn fcopyfile(
            from: c::c_int,
            to: c::c_int,
            state: copyfile_state_t,
            flags: c::c_uint,
        ) -> c::c_int;
    }

    nonnegative_ret(fcopyfile(
        borrowed_fd(from),
        borrowed_fd(to),
        state,
        bitflags_bits!(flags),
    ))
}

#[cfg(apple)]
pub(crate) fn copyfile_state_alloc() -> io::Result<copyfile_state_t> {
    extern "C" {
        fn copyfile_state_alloc() -> copyfile_state_t;
    }

    let result = unsafe { copyfile_state_alloc() };
    if result.0.is_null() {
        Err(io::Errno::last_os_error())
    } else {
        Ok(result)
    }
}

#[cfg(apple)]
pub(crate) unsafe fn copyfile_state_free(state: copyfile_state_t) -> io::Result<()> {
    extern "C" {
        fn copyfile_state_free(state: copyfile_state_t) -> c::c_int;
    }

    nonnegative_ret(copyfile_state_free(state))
}

#[cfg(apple)]
const COPYFILE_STATE_COPIED: u32 = 8;

#[cfg(apple)]
pub(crate) unsafe fn copyfile_state_get_copied(state: copyfile_state_t) -> io::Result<u64> {
    let mut copied = MaybeUninit::<u64>::uninit();
    copyfile_state_get(state, COPYFILE_STATE_COPIED, copied.as_mut_ptr().cast())?;
    Ok(copied.assume_init())
}

#[cfg(apple)]
pub(crate) unsafe fn copyfile_state_get(
    state: copyfile_state_t,
    flag: u32,
    dst: *mut c::c_void,
) -> io::Result<()> {
    extern "C" {
        fn copyfile_state_get(state: copyfile_state_t, flag: u32, dst: *mut c::c_void) -> c::c_int;
    }

    nonnegative_ret(copyfile_state_get(state, flag, dst))
}

#[cfg(all(apple, feature = "alloc"))]
pub(crate) fn getpath(fd: BorrowedFd<'_>) -> io::Result<CString> {
    // The use of `PATH_MAX` is generally not encouraged, but it
    // is inevitable in this case because macOS defines `fcntl` with
    // `F_GETPATH` in terms of `MAXPATHLEN`, and there are no
    // alternatives. If a better method is invented, it should be used
    // instead.
    let mut buf = vec![0; c::PATH_MAX as usize];

    // From the [macOS `fcntl` manual page]:
    // `F_GETPATH` - Get the path of the file descriptor `Fildes`. The argument
    //               must be a buffer of size `MAXPATHLEN` or greater.
    //
    // [macOS `fcntl` manual page]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/fcntl.2.html
    unsafe {
        ret(c::fcntl(borrowed_fd(fd), c::F_GETPATH, buf.as_mut_ptr()))?;
    }

    let l = buf.iter().position(|&c| c == 0).unwrap();
    buf.truncate(l);
    buf.shrink_to_fit();

    Ok(CString::new(buf).unwrap())
}

#[cfg(apple)]
pub(crate) fn fcntl_rdadvise(fd: BorrowedFd<'_>, offset: u64, len: u64) -> io::Result<()> {
    // From the [macOS `fcntl` manual page]:
    // `F_RDADVISE` - Issue an advisory read async with no copy to user.
    //
    // The `F_RDADVISE` command operates on the following structure which holds
    // information passed from the user to the system:
    //
    // ```c
    // struct radvisory {
    //      off_t   ra_offset;  /* offset into the file */
    //      int     ra_count;   /* size of the read     */
    // };
    // ```
    //
    // [macOS `fcntl` manual page]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/fcntl.2.html
    let ra_offset = match offset.try_into() {
        Ok(len) => len,
        // If this conversion fails, the user is providing an offset outside
        // any possible file extent, so just ignore it.
        Err(_) => return Ok(()),
    };
    let ra_count = match len.try_into() {
        Ok(len) => len,
        // If this conversion fails, the user is providing a dubiously large
        // hint which is unlikely to improve performance.
        Err(_) => return Ok(()),
    };
    unsafe {
        let radvisory = c::radvisory {
            ra_offset,
            ra_count,
        };
        ret(c::fcntl(borrowed_fd(fd), c::F_RDADVISE, &radvisory))
    }
}

#[cfg(apple)]
pub(crate) fn fcntl_fullfsync(fd: BorrowedFd<'_>) -> io::Result<()> {
    unsafe { ret(c::fcntl(borrowed_fd(fd), c::F_FULLFSYNC)) }
}

#[cfg(apple)]
pub(crate) fn fcntl_nocache(fd: BorrowedFd<'_>, value: bool) -> io::Result<()> {
    unsafe { ret(c::fcntl(borrowed_fd(fd), c::F_NOCACHE, value as c::c_int)) }
}

#[cfg(apple)]
pub(crate) fn fcntl_global_nocache(fd: BorrowedFd<'_>, value: bool) -> io::Result<()> {
    unsafe {
        ret(c::fcntl(
            borrowed_fd(fd),
            c::F_GLOBAL_NOCACHE,
            value as c::c_int,
        ))
    }
}

/// Convert `times` from a `futimens`/`utimensat` argument into `setattrlist`
/// arguments.
#[cfg(apple)]
fn times_to_attrlist(times: &Timestamps) -> io::Result<(c::size_t, [c::timespec; 2], Attrlist)> {
    // ABI details.
    const ATTR_CMN_MODTIME: u32 = 0x0000_0400;
    const ATTR_CMN_ACCTIME: u32 = 0x0000_1000;
    const ATTR_BIT_MAP_COUNT: u16 = 5;

    let mut times = times.clone();

    // If we have any `UTIME_NOW` elements, replace them with the current time.
    if times.last_access.tv_nsec == c::UTIME_NOW.into()
        || times.last_modification.tv_nsec == c::UTIME_NOW.into()
    {
        let now = {
            let mut tv = c::timeval {
                tv_sec: 0,
                tv_usec: 0,
            };
            unsafe {
                let r = c::gettimeofday(&mut tv, null_mut());
                assert_eq!(r, 0);
            }
            c::timespec {
                tv_sec: tv.tv_sec,
                tv_nsec: (tv.tv_usec * 1000) as _,
            }
        };
        if times.last_access.tv_nsec == c::UTIME_NOW.into() {
            times.last_access = crate::timespec::Timespec {
                tv_sec: now.tv_sec.into(),
                tv_nsec: now.tv_nsec as _,
            };
        }
        if times.last_modification.tv_nsec == c::UTIME_NOW.into() {
            times.last_modification = crate::timespec::Timespec {
                tv_sec: now.tv_sec.into(),
                tv_nsec: now.tv_nsec as _,
            };
        }
    }

    // Pack the return values following the rules for [`getattrlist`].
    //
    // [`getattrlist`]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/getattrlist.2.html
    let mut times_size = 0;
    let mut attrs = Attrlist {
        bitmapcount: ATTR_BIT_MAP_COUNT,
        reserved: 0,
        commonattr: 0,
        volattr: 0,
        dirattr: 0,
        fileattr: 0,
        forkattr: 0,
    };
    let mut return_times = [c::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    }; 2];
    let mut times_index = 0;
    if times.last_modification.tv_nsec != c::UTIME_OMIT.into() {
        attrs.commonattr |= ATTR_CMN_MODTIME;
        return_times[times_index] = c::timespec {
            tv_sec: times
                .last_modification
                .tv_sec
                .try_into()
                .map_err(|_| io::Errno::OVERFLOW)?,
            tv_nsec: times.last_modification.tv_nsec as _,
        };
        times_index += 1;
        times_size += size_of::<c::timespec>();
    }
    if times.last_access.tv_nsec != c::UTIME_OMIT.into() {
        attrs.commonattr |= ATTR_CMN_ACCTIME;
        return_times[times_index] = c::timespec {
            tv_sec: times
                .last_access
                .tv_sec
                .try_into()
                .map_err(|_| io::Errno::OVERFLOW)?,
            tv_nsec: times.last_access.tv_nsec as _,
        };
        times_size += size_of::<c::timespec>();
    }

    Ok((times_size, return_times, attrs))
}

/// Support type for `Attrlist`.
#[cfg(apple)]
type Attrgroup = u32;

/// Attribute list for use with [`setattrlist`].
#[cfg(apple)]
#[repr(C)]
struct Attrlist {
    bitmapcount: u16,
    reserved: u16,
    commonattr: Attrgroup,
    volattr: Attrgroup,
    dirattr: Attrgroup,
    fileattr: Attrgroup,
    forkattr: Attrgroup,
}

#[cfg(any(apple, linux_kernel, target_os = "hurd"))]
pub(crate) unsafe fn getxattr(
    path: &CStr,
    name: &CStr,
    value: (*mut u8, usize),
) -> io::Result<usize> {
    #[cfg(not(apple))]
    {
        ret_usize(c::getxattr(
            path.as_ptr(),
            name.as_ptr(),
            value.0.cast::<c::c_void>(),
            value.1,
        ))
    }

    #[cfg(apple)]
    {
        // Passing an empty to slice to `getxattr` leads to `ERANGE` on macOS.
        // Pass null instead.
        let ptr = if value.1 == 0 {
            core::ptr::null_mut()
        } else {
            value.0.cast::<c::c_void>()
        };
        ret_usize(c::getxattr(
            path.as_ptr(),
            name.as_ptr(),
            ptr,
            value.1,
            0,
            0,
        ))
    }
}

#[cfg(any(apple, linux_kernel, target_os = "hurd"))]
pub(crate) unsafe fn lgetxattr(
    path: &CStr,
    name: &CStr,
    value: (*mut u8, usize),
) -> io::Result<usize> {
    #[cfg(not(apple))]
    {
        ret_usize(c::lgetxattr(
            path.as_ptr(),
            name.as_ptr(),
            value.0.cast::<c::c_void>(),
            value.1,
        ))
    }

    #[cfg(apple)]
    {
        // Passing an empty to slice to `getxattr` leads to `ERANGE` on macOS.
        // Pass null instead.
        let ptr = if value.1 == 0 {
            core::ptr::null_mut()
        } else {
            value.0.cast::<c::c_void>()
        };

        ret_usize(c::getxattr(
            path.as_ptr(),
            name.as_ptr(),
            ptr,
            value.1,
            0,
            c::XATTR_NOFOLLOW,
        ))
    }
}

#[cfg(any(apple, linux_kernel, target_os = "hurd"))]
pub(crate) unsafe fn fgetxattr(
    fd: BorrowedFd<'_>,
    name: &CStr,
    value: (*mut u8, usize),
) -> io::Result<usize> {
    #[cfg(not(apple))]
    {
        ret_usize(c::fgetxattr(
            borrowed_fd(fd),
            name.as_ptr(),
            value.0.cast::<c::c_void>(),
            value.1,
        ))
    }

    #[cfg(apple)]
    {
        // Passing an empty to slice to `getxattr` leads to `ERANGE` on macOS.
        // Pass null instead.
        let ptr = if value.1 == 0 {
            core::ptr::null_mut()
        } else {
            value.0.cast::<c::c_void>()
        };
        ret_usize(c::fgetxattr(
            borrowed_fd(fd),
            name.as_ptr(),
            ptr,
            value.1,
            0,
            0,
        ))
    }
}

#[cfg(any(apple, linux_kernel, target_os = "hurd"))]
pub(crate) fn setxattr(
    path: &CStr,
    name: &CStr,
    value: &[u8],
    flags: XattrFlags,
) -> io::Result<()> {
    #[cfg(not(apple))]
    unsafe {
        ret(c::setxattr(
            path.as_ptr(),
            name.as_ptr(),
            value.as_ptr().cast::<c::c_void>(),
            value.len(),
            flags.bits() as i32,
        ))
    }

    #[cfg(apple)]
    unsafe {
        ret(c::setxattr(
            path.as_ptr(),
            name.as_ptr(),
            value.as_ptr().cast::<c::c_void>(),
            value.len(),
            0,
            flags.bits() as i32,
        ))
    }
}

#[cfg(any(apple, linux_kernel, target_os = "hurd"))]
pub(crate) fn lsetxattr(
    path: &CStr,
    name: &CStr,
    value: &[u8],
    flags: XattrFlags,
) -> io::Result<()> {
    #[cfg(not(apple))]
    unsafe {
        ret(c::lsetxattr(
            path.as_ptr(),
            name.as_ptr(),
            value.as_ptr().cast::<c::c_void>(),
            value.len(),
            flags.bits() as i32,
        ))
    }

    #[cfg(apple)]
    unsafe {
        ret(c::setxattr(
            path.as_ptr(),
            name.as_ptr(),
            value.as_ptr().cast::<c::c_void>(),
            value.len(),
            0,
            flags.bits() as i32 | c::XATTR_NOFOLLOW,
        ))
    }
}

#[cfg(any(apple, linux_kernel, target_os = "hurd"))]
pub(crate) fn fsetxattr(
    fd: BorrowedFd<'_>,
    name: &CStr,
    value: &[u8],
    flags: XattrFlags,
) -> io::Result<()> {
    #[cfg(not(apple))]
    unsafe {
        ret(c::fsetxattr(
            borrowed_fd(fd),
            name.as_ptr(),
            value.as_ptr().cast::<c::c_void>(),
            value.len(),
            flags.bits() as i32,
        ))
    }

    #[cfg(apple)]
    unsafe {
        ret(c::fsetxattr(
            borrowed_fd(fd),
            name.as_ptr(),
            value.as_ptr().cast::<c::c_void>(),
            value.len(),
            0,
            flags.bits() as i32,
        ))
    }
}

#[cfg(any(apple, linux_kernel, target_os = "hurd"))]
pub(crate) unsafe fn listxattr(path: &CStr, list: (*mut u8, usize)) -> io::Result<usize> {
    #[cfg(not(apple))]
    {
        ret_usize(c::listxattr(
            path.as_ptr(),
            list.0.cast::<ffi::c_char>(),
            list.1,
        ))
    }

    #[cfg(apple)]
    {
        ret_usize(c::listxattr(
            path.as_ptr(),
            list.0.cast::<ffi::c_char>(),
            list.1,
            0,
        ))
    }
}

#[cfg(any(apple, linux_kernel, target_os = "hurd"))]
pub(crate) unsafe fn llistxattr(path: &CStr, list: (*mut u8, usize)) -> io::Result<usize> {
    #[cfg(not(apple))]
    {
        ret_usize(c::llistxattr(
            path.as_ptr(),
            list.0.cast::<ffi::c_char>(),
            list.1,
        ))
    }

    #[cfg(apple)]
    {
        ret_usize(c::listxattr(
            path.as_ptr(),
            list.0.cast::<ffi::c_char>(),
            list.1,
            c::XATTR_NOFOLLOW,
        ))
    }
}

#[cfg(any(apple, linux_kernel, target_os = "hurd"))]
pub(crate) unsafe fn flistxattr(fd: BorrowedFd<'_>, list: (*mut u8, usize)) -> io::Result<usize> {
    let fd = borrowed_fd(fd);

    #[cfg(not(apple))]
    {
        ret_usize(c::flistxattr(fd, list.0.cast::<ffi::c_char>(), list.1))
    }

    #[cfg(apple)]
    {
        ret_usize(c::flistxattr(fd, list.0.cast::<ffi::c_char>(), list.1, 0))
    }
}

#[cfg(any(apple, linux_kernel, target_os = "hurd"))]
pub(crate) fn removexattr(path: &CStr, name: &CStr) -> io::Result<()> {
    #[cfg(not(apple))]
    unsafe {
        ret(c::removexattr(path.as_ptr(), name.as_ptr()))
    }

    #[cfg(apple)]
    unsafe {
        ret(c::removexattr(path.as_ptr(), name.as_ptr(), 0))
    }
}

#[cfg(any(apple, linux_kernel, target_os = "hurd"))]
pub(crate) fn lremovexattr(path: &CStr, name: &CStr) -> io::Result<()> {
    #[cfg(not(apple))]
    unsafe {
        ret(c::lremovexattr(path.as_ptr(), name.as_ptr()))
    }

    #[cfg(apple)]
    unsafe {
        ret(c::removexattr(
            path.as_ptr(),
            name.as_ptr(),
            c::XATTR_NOFOLLOW,
        ))
    }
}

#[cfg(any(apple, linux_kernel, target_os = "hurd"))]
pub(crate) fn fremovexattr(fd: BorrowedFd<'_>, name: &CStr) -> io::Result<()> {
    let fd = borrowed_fd(fd);

    #[cfg(not(apple))]
    unsafe {
        ret(c::fremovexattr(fd, name.as_ptr()))
    }

    #[cfg(apple)]
    unsafe {
        ret(c::fremovexattr(fd, name.as_ptr(), 0))
    }
}

/// See [`crate::timespec::fix_negative_nsec`] for details.
#[cfg(apple)]
fn fix_negative_stat_nsecs(mut stat: Stat) -> Stat {
    (stat.st_atime, stat.st_atime_nsec) =
        crate::timespec::fix_negative_nsecs(stat.st_atime, stat.st_atime_nsec);
    (stat.st_mtime, stat.st_mtime_nsec) =
        crate::timespec::fix_negative_nsecs(stat.st_mtime, stat.st_mtime_nsec);
    (stat.st_ctime, stat.st_ctime_nsec) =
        crate::timespec::fix_negative_nsecs(stat.st_ctime, stat.st_ctime_nsec);
    stat
}

#[inline]
#[cfg(linux_kernel)]
pub(crate) fn inotify_init1(flags: super::inotify::CreateFlags) -> io::Result<OwnedFd> {
    // SAFETY: `inotify_init1` has no safety preconditions.
    unsafe { ret_owned_fd(c::inotify_init1(bitflags_bits!(flags))) }
}

#[inline]
#[cfg(linux_kernel)]
pub(crate) fn inotify_add_watch(
    inot: BorrowedFd<'_>,
    path: &CStr,
    flags: super::inotify::WatchFlags,
) -> io::Result<i32> {
    // SAFETY: The fd and path we are passing is guaranteed valid by the
    // type system.
    unsafe {
        ret_c_int(c::inotify_add_watch(
            borrowed_fd(inot),
            c_str(path),
            flags.bits(),
        ))
    }
}

#[inline]
#[cfg(linux_kernel)]
pub(crate) fn inotify_rm_watch(inot: BorrowedFd<'_>, wd: i32) -> io::Result<()> {
    // Android's `inotify_rm_watch` takes `u32` despite that
    // `inotify_add_watch` expects a `i32`.
    #[cfg(target_os = "android")]
    let wd = wd as u32;
    // SAFETY: The fd is valid and closing an arbitrary wd is valid.
    unsafe { ret(c::inotify_rm_watch(borrowed_fd(inot), wd)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sizes() {
        #[cfg(linux_kernel)]
        assert_eq_size!(c::loff_t, u64);

        // Assert that `Timestamps` has the expected layout. If we're not fixing
        // y2038, libc's type should match ours. If we are, it's smaller.
        #[cfg(not(fix_y2038))]
        assert_eq_size!([c::timespec; 2], Timestamps);
        #[cfg(fix_y2038)]
        assert!(core::mem::size_of::<[c::timespec; 2]>() < core::mem::size_of::<Timestamps>());
    }
}
