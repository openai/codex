use crate::fd::{AsFd, BorrowedFd, OwnedFd};
use crate::ffi::{CStr, CString};
use crate::fs::{
    fcntl_getfl, fstat, fstatfs, fstatvfs, openat, FileType, Mode, OFlags, Stat, StatFs, StatVfs,
};
use crate::io;
#[cfg(feature = "process")]
use crate::process::fchdir;
use crate::utils::as_ptr;
use alloc::borrow::ToOwned as _;
use alloc::vec::Vec;
use core::fmt;
use core::mem::size_of;
use linux_raw_sys::general::{linux_dirent64, SEEK_SET};

/// `DIR*`
pub struct Dir {
    /// The `OwnedFd` that we read directory entries from.
    fd: OwnedFd,

    /// Have we seen any errors in this iteration?
    any_errors: bool,

    /// Should we rewind the stream on the next iteration?
    rewind: bool,

    /// The buffer for `linux_dirent64` entries.
    buf: Vec<u8>,

    /// Where we are in the buffer.
    pos: usize,
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
        Ok(Self {
            fd,
            any_errors: false,
            rewind: false,
            buf: Vec::new(),
            pos: 0,
        })
    }

    /// Borrow `fd` and construct a `Dir` that reads entries from the given
    /// directory file descriptor.
    #[inline]
    pub fn read_from<Fd: AsFd>(fd: Fd) -> io::Result<Self> {
        Self::_read_from(fd.as_fd())
    }

    #[inline]
    fn _read_from(fd: BorrowedFd<'_>) -> io::Result<Self> {
        let flags = fcntl_getfl(fd)?;
        let fd_for_dir = openat(fd, cstr!("."), flags | OFlags::CLOEXEC, Mode::empty())?;

        Ok(Self {
            fd: fd_for_dir,
            any_errors: false,
            rewind: false,
            buf: Vec::new(),
            pos: 0,
        })
    }

    /// `rewinddir(self)`
    #[inline]
    pub fn rewind(&mut self) {
        self.any_errors = false;
        self.rewind = true;
        self.pos = self.buf.len();
    }

    /// `seekdir(self, offset)`
    ///
    /// This function is only available on 64-bit platforms because it's
    /// implemented using [`libc::seekdir`] which only supports offsets that
    /// fit in a `c_long`.
    ///
    /// [`libc::seekdir`]: https://docs.rs/libc/*/arm-unknown-linux-gnueabihf/libc/fn.seekdir.html
    // In the linux_raw backend here, we don't use `libc::seekdir` and don't
    // have this limitation, but it's a goal of rustix to support the same API
    // on both the linux_raw and libc backends.
    #[cfg(target_pointer_width = "64")]
    #[cfg_attr(docsrs, doc(cfg(target_pointer_width = "64")))]
    #[doc(alias = "seekdir")]
    #[inline]
    pub fn seek(&mut self, offset: i64) -> io::Result<()> {
        self.any_errors = false;
        self.rewind = false;
        self.pos = self.buf.len();
        match io::retry_on_intr(|| {
            crate::backend::fs::syscalls::_seek(self.fd.as_fd(), offset, SEEK_SET)
        }) {
            Ok(_) => Ok(()),
            Err(err) => {
                self.any_errors = true;
                Err(err)
            }
        }
    }

    /// `readdir(self)`, where `None` means the end of the directory.
    pub fn read(&mut self) -> Option<io::Result<DirEntry>> {
        // If we've seen errors, don't continue to try to read anything
        // further.
        if self.any_errors {
            return None;
        }

        // If a rewind was requested, seek to the beginning.
        if self.rewind {
            self.rewind = false;
            match io::retry_on_intr(|| {
                crate::backend::fs::syscalls::_seek(self.fd.as_fd(), 0, SEEK_SET)
            }) {
                Ok(_) => (),
                Err(err) => {
                    self.any_errors = true;
                    return Some(Err(err));
                }
            }
        }

        // Compute linux_dirent64 field offsets.
        let z = linux_dirent64 {
            d_ino: 0_u64,
            d_off: 0_i64,
            d_type: 0_u8,
            d_reclen: 0_u16,
            d_name: Default::default(),
        };
        let base = as_ptr(&z) as usize;
        let offsetof_d_reclen = (as_ptr(&z.d_reclen) as usize) - base;
        let offsetof_d_name = (as_ptr(&z.d_name) as usize) - base;
        let offsetof_d_ino = (as_ptr(&z.d_ino) as usize) - base;
        let offsetof_d_off = (as_ptr(&z.d_off) as usize) - base;
        let offsetof_d_type = (as_ptr(&z.d_type) as usize) - base;

        // Test if we need more entries, and if so, read more.
        if self.buf.len() - self.pos < size_of::<linux_dirent64>() {
            match self.read_more()? {
                Ok(()) => (),
                Err(err) => return Some(Err(err)),
            }
        }

        // We successfully read an entry. Extract the fields.
        let pos = self.pos;

        // Do an unaligned u16 load.
        let d_reclen = u16::from_ne_bytes([
            self.buf[pos + offsetof_d_reclen],
            self.buf[pos + offsetof_d_reclen + 1],
        ]);
        assert!(self.buf.len() - pos >= d_reclen as usize);
        self.pos += d_reclen as usize;

        // Read the NUL-terminated name from the `d_name` field. Without
        // `unsafe`, we need to scan for the NUL twice: once to obtain a size
        // for the slice, and then once within `CStr::from_bytes_with_nul`.
        let name_start = pos + offsetof_d_name;
        let name_len = self.buf[name_start..]
            .iter()
            .position(|x| *x == b'\0')
            .unwrap();
        let name = CStr::from_bytes_with_nul(&self.buf[name_start..][..=name_len]).unwrap();
        let name = name.to_owned();
        assert!(name.as_bytes().len() <= self.buf.len() - name_start);

        // Do an unaligned `u64` load for `d_ino`.
        let d_ino = u64::from_ne_bytes([
            self.buf[pos + offsetof_d_ino],
            self.buf[pos + offsetof_d_ino + 1],
            self.buf[pos + offsetof_d_ino + 2],
            self.buf[pos + offsetof_d_ino + 3],
            self.buf[pos + offsetof_d_ino + 4],
            self.buf[pos + offsetof_d_ino + 5],
            self.buf[pos + offsetof_d_ino + 6],
            self.buf[pos + offsetof_d_ino + 7],
        ]);

        // Do an unaligned `i64` load for `d_off`.
        let d_off = i64::from_ne_bytes([
            self.buf[pos + offsetof_d_off],
            self.buf[pos + offsetof_d_off + 1],
            self.buf[pos + offsetof_d_off + 2],
            self.buf[pos + offsetof_d_off + 3],
            self.buf[pos + offsetof_d_off + 4],
            self.buf[pos + offsetof_d_off + 5],
            self.buf[pos + offsetof_d_off + 6],
            self.buf[pos + offsetof_d_off + 7],
        ]);

        let d_type = self.buf[pos + offsetof_d_type];

        // Check that our types correspond to the `linux_dirent64` types.
        let _ = linux_dirent64 {
            d_ino,
            d_off,
            d_type,
            d_reclen,
            d_name: Default::default(),
        };

        Some(Ok(DirEntry {
            d_ino,
            d_off,
            d_type,
            name,
        }))
    }

    #[must_use]
    fn read_more(&mut self) -> Option<io::Result<()>> {
        // The first few times we're called, we allocate a relatively small
        // buffer, because many directories are small. If we're called more,
        // use progressively larger allocations, up to a fixed maximum.
        //
        // The specific sizes and policy here have not been tuned in detail yet
        // and may need to be adjusted. In doing so, we should be careful to
        // avoid unbounded buffer growth. This buffer only exists to share the
        // cost of a `getdents` call over many entries, so if it gets too big,
        // cache and heap usage will outweigh the benefit. And ultimately,
        // directories can contain more entries than we can allocate contiguous
        // memory for, so we'll always need to cap the size at some point.
        if self.buf.len() < 1024 * size_of::<linux_dirent64>() {
            self.buf.reserve(32 * size_of::<linux_dirent64>());
        }
        self.buf.resize(self.buf.capacity(), 0);
        let nread = match io::retry_on_intr(|| {
            crate::backend::fs::syscalls::getdents(self.fd.as_fd(), &mut self.buf)
        }) {
            Ok(nread) => nread,
            Err(io::Errno::NOENT) => {
                self.any_errors = true;
                return None;
            }
            Err(err) => {
                self.any_errors = true;
                return Some(Err(err));
            }
        };
        self.buf.resize(nread, 0);
        self.pos = 0;
        if nread == 0 {
            None
        } else {
            Some(Ok(()))
        }
    }

    /// `fstat(self)`
    #[inline]
    pub fn stat(&self) -> io::Result<Stat> {
        fstat(&self.fd)
    }

    /// `fstatfs(self)`
    #[inline]
    pub fn statfs(&self) -> io::Result<StatFs> {
        fstatfs(&self.fd)
    }

    /// `fstatvfs(self)`
    #[inline]
    pub fn statvfs(&self) -> io::Result<StatVfs> {
        fstatvfs(&self.fd)
    }

    /// `fchdir(self)`
    #[cfg(feature = "process")]
    #[cfg_attr(docsrs, doc(cfg(feature = "process")))]
    #[inline]
    pub fn chdir(&self) -> io::Result<()> {
        fchdir(&self.fd)
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
        f.debug_struct("Dir").field("fd", &self.fd).finish()
    }
}

/// `struct dirent`
#[derive(Debug)]
pub struct DirEntry {
    d_ino: u64,
    d_type: u8,
    d_off: i64,
    name: CString,
}

impl DirEntry {
    /// Returns the file name of this directory entry.
    #[inline]
    pub fn file_name(&self) -> &CStr {
        &self.name
    }

    /// Returns the “offset” of this directory entry. This is not a true
    /// numerical offset but an opaque cookie that identifies a position in the
    /// given stream.
    #[inline]
    pub fn offset(&self) -> i64 {
        self.d_off
    }

    /// Returns the type of this directory entry.
    #[inline]
    pub fn file_type(&self) -> FileType {
        FileType::from_dirent_d_type(self.d_type)
    }

    /// Return the inode number of this directory entry.
    #[inline]
    pub fn ino(&self) -> u64 {
        self.d_ino
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // will cause the subsequent `getdents64` to fail.
        crate::io::dup2(&file_fd, &mut dir.fd).unwrap();

        assert!(matches!(dir.next(), Some(Err(_))));
        assert!(dir.next().is_none());
    }
}
