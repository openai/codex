#[cfg(not(any(solarish, target_os = "haiku", target_os = "nto", target_os = "vita")))]
use super::types::FileType;
use crate::backend::c;
use crate::backend::conv::owned_fd;
use crate::fd::{AsFd, BorrowedFd, OwnedFd};
use crate::ffi::{CStr, CString};
use crate::fs::{fcntl_getfl, openat, Mode, OFlags};
#[cfg(not(target_os = "vita"))]
use crate::fs::{fstat, Stat};
#[cfg(not(any(
    solarish,
    target_os = "haiku",
    target_os = "netbsd",
    target_os = "nto",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi",
)))]
use crate::fs::{fstatfs, StatFs};
#[cfg(not(any(
    solarish,
    target_os = "haiku",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
use crate::fs::{fstatvfs, StatVfs};
use crate::io;
#[cfg(not(any(target_os = "fuchsia", target_os = "vita", target_os = "wasi")))]
#[cfg(feature = "process")]
use crate::process::fchdir;
use alloc::borrow::ToOwned;
#[cfg(not(any(linux_like, target_os = "hurd")))]
use c::readdir as libc_readdir;
#[cfg(any(linux_like, target_os = "hurd"))]
use c::readdir64 as libc_readdir;
use core::fmt;
use core::ptr::NonNull;
use libc_errno::{errno, set_errno, Errno};

/// `DIR*`
pub struct Dir {
    /// The `libc` `DIR` pointer.
    libc_dir: NonNull<c::DIR>,

    /// Have we seen any errors in this iteration?
    any_errors: bool,
}

impl Dir {
    /// Take ownership of `fd` and construct a `Dir` that reads entries from
    /// the given directory file descriptor.
    #[inline]
    pub fn new<Fd: Into<OwnedFd>>(fd: Fd) -> io::Result<Self> {
        Self::_new(fd.into())
    }

    #[inline]
    fn _new(fd: OwnedFd) -> io::Result<Self> {
        let raw = owned_fd(fd);
        unsafe {
            let libc_dir = c::fdopendir(raw);

            if let Some(libc_dir) = NonNull::new(libc_dir) {
                Ok(Self {
                    libc_dir,
                    any_errors: false,
                })
            } else {
                let err = io::Errno::last_os_error();
                let _ = c::close(raw);
                Err(err)
            }
        }
    }

    /// Borrow `fd` and construct a `Dir` that reads entries from the given
    /// directory file descriptor.
    #[inline]
    pub fn read_from<Fd: AsFd>(fd: Fd) -> io::Result<Self> {
        Self::_read_from(fd.as_fd())
    }

    #[inline]
    #[allow(unused_mut)]
    fn _read_from(fd: BorrowedFd<'_>) -> io::Result<Self> {
        let mut any_errors = false;

        // Given an arbitrary `OwnedFd`, it's impossible to know whether the
        // user holds a `dup`'d copy which could continue to modify the
        // file description state, which would cause Undefined Behavior after
        // our call to `fdopendir`. To prevent this, we obtain an independent
        // `OwnedFd`.
        let flags = fcntl_getfl(fd)?;
        let fd_for_dir = match openat(fd, cstr!("."), flags | OFlags::CLOEXEC, Mode::empty()) {
            Ok(fd) => fd,
            #[cfg(not(target_os = "wasi"))]
            Err(io::Errno::NOENT) => {
                // If "." doesn't exist, it means the directory was removed.
                // We treat that as iterating through a directory with no
                // entries.
                any_errors = true;
                crate::io::dup(fd)?
            }
            Err(err) => return Err(err),
        };

        let raw = owned_fd(fd_for_dir);
        unsafe {
            let libc_dir = c::fdopendir(raw);

            if let Some(libc_dir) = NonNull::new(libc_dir) {
                Ok(Self {
                    libc_dir,
                    any_errors,
                })
            } else {
                let err = io::Errno::last_os_error();
                let _ = c::close(raw);
                Err(err)
            }
        }
    }

    /// `rewinddir(self)`
    #[inline]
    pub fn rewind(&mut self) {
        self.any_errors = false;
        unsafe { c::rewinddir(self.libc_dir.as_ptr()) }
    }

    /// `readdir(self)`, where `None` means the end of the directory.
    pub fn read(&mut self) -> Option<io::Result<DirEntry>> {
        // If we've seen errors, don't continue to try to read anything further.
        if self.any_errors {
            return None;
        }

        set_errno(Errno(0));
        let dirent_ptr = unsafe { libc_readdir(self.libc_dir.as_ptr()) };
        if dirent_ptr.is_null() {
            let curr_errno = errno().0;
            if curr_errno == 0 {
                // We successfully reached the end of the stream.
                None
            } else {
                // `errno` is unknown or non-zero, so an error occurred.
                self.any_errors = true;
                Some(Err(io::Errno(curr_errno)))
            }
        } else {
            // We successfully read an entry.
            unsafe {
                let dirent = &*dirent_ptr;

                // We have our own copy of OpenBSD's dirent; check that the
                // layout minimally matches libc's.
                #[cfg(target_os = "openbsd")]
                check_dirent_layout(dirent);

                let result = DirEntry {
                    #[cfg(not(any(
                        solarish,
                        target_os = "aix",
                        target_os = "haiku",
                        target_os = "nto",
                        target_os = "vita"
                    )))]
                    d_type: dirent.d_type,

                    #[cfg(not(any(freebsdlike, netbsdlike, target_os = "vita")))]
                    d_ino: dirent.d_ino,

                    #[cfg(any(freebsdlike, netbsdlike))]
                    d_fileno: dirent.d_fileno,

                    name: CStr::from_ptr(dirent.d_name.as_ptr()).to_owned(),
                };

                Some(Ok(result))
            }
        }
    }

    /// `fstat(self)`
    #[cfg(not(target_os = "vita"))]
    #[inline]
    pub fn stat(&self) -> io::Result<Stat> {
        fstat(unsafe { BorrowedFd::borrow_raw(c::dirfd(self.libc_dir.as_ptr())) })
    }

    /// `fstatfs(self)`
    #[cfg(not(any(
        solarish,
        target_os = "haiku",
        target_os = "netbsd",
        target_os = "nto",
        target_os = "redox",
        target_os = "vita",
        target_os = "wasi",
    )))]
    #[inline]
    pub fn statfs(&self) -> io::Result<StatFs> {
        fstatfs(unsafe { BorrowedFd::borrow_raw(c::dirfd(self.libc_dir.as_ptr())) })
    }

    /// `fstatvfs(self)`
    #[cfg(not(any(
        solarish,
        target_os = "haiku",
        target_os = "redox",
        target_os = "vita",
        target_os = "wasi"
    )))]
    #[inline]
    pub fn statvfs(&self) -> io::Result<StatVfs> {
        fstatvfs(unsafe { BorrowedFd::borrow_raw(c::dirfd(self.libc_dir.as_ptr())) })
    }

    /// `fchdir(self)`
    #[cfg(feature = "process")]
    #[cfg(not(any(target_os = "fuchsia", target_os = "vita", target_os = "wasi")))]
    #[cfg_attr(docsrs, doc(cfg(feature = "process")))]
    #[inline]
    pub fn chdir(&self) -> io::Result<()> {
        fchdir(unsafe { BorrowedFd::borrow_raw(c::dirfd(self.libc_dir.as_ptr())) })
    }
}

/// `Dir` is `Send` and `Sync`, because even though it contains internal
/// state, all methods that modify the state require a `mut &self` and
/// can therefore not be called concurrently. Calling them from different
/// threads sequentially is fine.
unsafe impl Send for Dir {}
unsafe impl Sync for Dir {}

impl Drop for Dir {
    #[inline]
    fn drop(&mut self) {
        unsafe { c::closedir(self.libc_dir.as_ptr()) };
    }
}

impl Iterator for Dir {
    type Item = io::Result<DirEntry>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        Self::read(self)
    }
}

impl fmt::Debug for Dir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = f.debug_struct("Dir");
        #[cfg(not(target_os = "vita"))]
        s.field("fd", unsafe { &c::dirfd(self.libc_dir.as_ptr()) });
        s.finish()
    }
}

/// `struct dirent`
#[derive(Debug)]
pub struct DirEntry {
    #[cfg(not(any(
        solarish,
        target_os = "aix",
        target_os = "haiku",
        target_os = "nto",
        target_os = "vita"
    )))]
    d_type: u8,

    #[cfg(not(any(freebsdlike, netbsdlike, target_os = "vita")))]
    d_ino: c::ino_t,

    #[cfg(any(freebsdlike, netbsdlike))]
    d_fileno: c::ino_t,

    name: CString,
}

impl DirEntry {
    /// Returns the file name of this directory entry.
    #[inline]
    pub fn file_name(&self) -> &CStr {
        &self.name
    }

    /// Returns the type of this directory entry.
    #[cfg(not(any(
        solarish,
        target_os = "aix",
        target_os = "haiku",
        target_os = "nto",
        target_os = "vita"
    )))]
    #[inline]
    pub fn file_type(&self) -> FileType {
        FileType::from_dirent_d_type(self.d_type)
    }

    /// Return the inode number of this directory entry.
    #[cfg(not(any(freebsdlike, netbsdlike, target_os = "vita")))]
    #[inline]
    pub fn ino(&self) -> u64 {
        self.d_ino as u64
    }

    /// Return the inode number of this directory entry.
    #[cfg(any(freebsdlike, netbsdlike))]
    #[inline]
    pub fn ino(&self) -> u64 {
        #[allow(clippy::useless_conversion)]
        self.d_fileno.into()
    }
}

/// libc's OpenBSD `dirent` has a private field so we can't construct it
/// directly, so we declare it ourselves to make all fields accessible.
#[cfg(target_os = "openbsd")]
#[repr(C)]
#[derive(Debug)]
struct libc_dirent {
    d_fileno: c::ino_t,
    d_off: c::off_t,
    d_reclen: u16,
    d_type: u8,
    d_namlen: u8,
    __d_padding: [u8; 4],
    d_name: [c::c_char; 256],
}

/// We have our own copy of OpenBSD's dirent; check that the layout
/// minimally matches libc's.
#[cfg(target_os = "openbsd")]
fn check_dirent_layout(dirent: &c::dirent) {
    use crate::utils::as_ptr;

    // Check that the basic layouts match.
    #[cfg(test)]
    {
        assert_eq_size!(libc_dirent, c::dirent);
        assert_eq_size!(libc_dirent, c::dirent);
    }

    // Check that the field offsets match.
    assert_eq!(
        {
            let z = libc_dirent {
                d_fileno: 0_u64,
                d_off: 0_i64,
                d_reclen: 0_u16,
                d_type: 0_u8,
                d_namlen: 0_u8,
                __d_padding: [0_u8; 4],
                d_name: [0 as c::c_char; 256],
            };
            let base = as_ptr(&z) as usize;
            (
                (as_ptr(&z.d_fileno) as usize) - base,
                (as_ptr(&z.d_off) as usize) - base,
                (as_ptr(&z.d_reclen) as usize) - base,
                (as_ptr(&z.d_type) as usize) - base,
                (as_ptr(&z.d_namlen) as usize) - base,
                (as_ptr(&z.d_name) as usize) - base,
            )
        },
        {
            let z = dirent;
            let base = as_ptr(z) as usize;
            (
                (as_ptr(&z.d_fileno) as usize) - base,
                (as_ptr(&z.d_off) as usize) - base,
                (as_ptr(&z.d_reclen) as usize) - base,
                (as_ptr(&z.d_type) as usize) - base,
                (as_ptr(&z.d_namlen) as usize) - base,
                (as_ptr(&z.d_name) as usize) - base,
            )
        }
    );
}

#[test]
fn dir_iterator_handles_io_errors() {
    // create a dir, keep the FD, then delete the dir
    let tmp = tempfile::tempdir().unwrap();
    let fd = crate::fs::openat(
        crate::fs::CWD,
        tmp.path(),
        crate::fs::OFlags::RDONLY | crate::fs::OFlags::CLOEXEC,
        crate::fs::Mode::empty(),
    )
    .unwrap();

    let file_fd = crate::fs::openat(
        &fd,
        tmp.path().join("test.txt"),
        crate::fs::OFlags::WRONLY | crate::fs::OFlags::CREATE,
        crate::fs::Mode::RWXU,
    )
    .unwrap();

    let mut dir = Dir::read_from(&fd).unwrap();

    // Reach inside the `Dir` and replace its directory with a file, which
    // will cause the subsequent `readdir` to fail.
    unsafe {
        let raw_fd = c::dirfd(dir.libc_dir.as_ptr());
        let mut owned_fd: crate::fd::OwnedFd = crate::fd::FromRawFd::from_raw_fd(raw_fd);
        crate::io::dup2(&file_fd, &mut owned_fd).unwrap();
        core::mem::forget(owned_fd);
    }

    // FreeBSD and macOS seem to read some directory entries before we call
    // `.next()`.
    #[cfg(any(apple, freebsdlike))]
    {
        dir.rewind();
    }

    assert!(matches!(dir.next(), Some(Err(_))));
    assert!(dir.next().is_none());
}
