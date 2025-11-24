//! `RawDir` and `RawDirEntry`.

use crate::backend::fs::syscalls::getdents_uninit;
use crate::fd::AsFd;
use crate::ffi::CStr;
use crate::fs::FileType;
use crate::io;
use core::fmt;
use core::mem::{align_of, MaybeUninit};
use linux_raw_sys::general::linux_dirent64;

/// A directory iterator implemented with getdents.
///
/// Note: This implementation does not handle growing the buffer. If this
/// functionality is necessary, you'll need to drop the current iterator,
/// resize the buffer, and then re-create the iterator. The iterator is
/// guaranteed to continue where it left off provided the file descriptor isn't
/// changed. See the example in [`RawDir::new`].
pub struct RawDir<'buf, Fd: AsFd> {
    fd: Fd,
    buf: &'buf mut [MaybeUninit<u8>],
    initialized: usize,
    offset: usize,
}

impl<'buf, Fd: AsFd> RawDir<'buf, Fd> {
    /// Create a new iterator from the given file descriptor and buffer.
    ///
    /// Note: the buffer size may be trimmed to accommodate alignment
    /// requirements.
    ///
    /// # Examples
    ///
    /// ## Simple but non-portable
    ///
    /// These examples are non-portable, because file systems may not have a
    /// maximum file name length. If you can make assumptions that bound
    /// this length, then these examples may suffice.
    ///
    /// Using the heap:
    ///
    /// ```
    /// # use std::mem::MaybeUninit;
    /// # use rustix::fs::{CWD, Mode, OFlags, openat, RawDir};
    /// # use rustix::cstr;
    ///
    /// let fd = openat(
    ///     CWD,
    ///     cstr!("."),
    ///     OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
    ///     Mode::empty(),
    /// )
    /// .unwrap();
    ///
    /// let mut buf = Vec::with_capacity(8192);
    /// let mut iter = RawDir::new(fd, buf.spare_capacity_mut());
    /// while let Some(entry) = iter.next() {
    ///     let entry = entry.unwrap();
    ///     dbg!(&entry);
    /// }
    /// ```
    ///
    /// Using the stack:
    ///
    /// ```
    /// # use std::mem::MaybeUninit;
    /// # use rustix::fs::{CWD, Mode, OFlags, openat, RawDir};
    /// # use rustix::cstr;
    ///
    /// let fd = openat(
    ///     CWD,
    ///     cstr!("."),
    ///     OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
    ///     Mode::empty(),
    /// )
    /// .unwrap();
    ///
    /// let mut buf = [MaybeUninit::uninit(); 2048];
    /// let mut iter = RawDir::new(fd, &mut buf);
    /// while let Some(entry) = iter.next() {
    ///     let entry = entry.unwrap();
    ///     dbg!(&entry);
    /// }
    /// ```
    ///
    /// ## Portable
    ///
    /// Heap allocated growing buffer for supporting directory entries with
    /// arbitrarily large file names:
    ///
    /// ```ignore
    /// # // The `ignore` above can be removed when we can depend on Rust 1.65.
    /// # use std::mem::MaybeUninit;
    /// # use rustix::fs::{CWD, Mode, OFlags, openat, RawDir};
    /// # use rustix::io::Errno;
    /// # use rustix::cstr;
    ///
    /// let fd = openat(
    ///     CWD,
    ///     cstr!("."),
    ///     OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
    ///     Mode::empty(),
    /// )
    /// .unwrap();
    ///
    /// let mut buf = Vec::with_capacity(8192);
    /// 'read: loop {
    ///     'resize: {
    ///         let mut iter = RawDir::new(&fd, buf.spare_capacity_mut());
    ///         while let Some(entry) = iter.next() {
    ///             let entry = match entry {
    ///                 Err(Errno::INVAL) => break 'resize,
    ///                 r => r.unwrap(),
    ///             };
    ///             dbg!(&entry);
    ///         }
    ///         break 'read;
    ///     }
    ///
    ///     let new_capacity = buf.capacity() * 2;
    ///     buf.reserve(new_capacity);
    /// }
    /// ```
    pub fn new(fd: Fd, buf: &'buf mut [MaybeUninit<u8>]) -> Self {
        Self {
            fd,
            buf: {
                let offset = buf.as_ptr().align_offset(align_of::<linux_dirent64>());
                if offset < buf.len() {
                    &mut buf[offset..]
                } else {
                    &mut []
                }
            },
            initialized: 0,
            offset: 0,
        }
    }
}

/// A raw directory entry, similar to [`std::fs::DirEntry`].
///
/// Unlike the std version, this may represent the `.` or `..` entries.
pub struct RawDirEntry<'a> {
    file_name: &'a CStr,
    file_type: u8,
    inode_number: u64,
    next_entry_cookie: i64,
}

impl<'a> fmt::Debug for RawDirEntry<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut f = f.debug_struct("RawDirEntry");
        f.field("file_name", &self.file_name());
        f.field("file_type", &self.file_type());
        f.field("ino", &self.ino());
        f.field("next_entry_cookie", &self.next_entry_cookie());
        f.finish()
    }
}

impl<'a> RawDirEntry<'a> {
    /// Returns the file name of this directory entry.
    #[inline]
    pub fn file_name(&self) -> &CStr {
        self.file_name
    }

    /// Returns the type of this directory entry.
    #[inline]
    pub fn file_type(&self) -> FileType {
        FileType::from_dirent_d_type(self.file_type)
    }

    /// Returns the inode number of this directory entry.
    #[inline]
    #[doc(alias = "inode_number")]
    pub fn ino(&self) -> u64 {
        self.inode_number
    }

    /// Returns the seek cookie to the next directory entry.
    #[inline]
    #[doc(alias = "off")]
    pub fn next_entry_cookie(&self) -> u64 {
        self.next_entry_cookie as u64
    }
}

impl<'buf, Fd: AsFd> RawDir<'buf, Fd> {
    /// Identical to [`Iterator::next`] except that [`Iterator::Item`] borrows
    /// from self.
    ///
    /// Note: this interface will be broken to implement a stdlib iterator API
    /// with GAT support once one becomes available.
    #[allow(unsafe_code)]
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<io::Result<RawDirEntry<'_>>> {
        if self.is_buffer_empty() {
            match getdents_uninit(self.fd.as_fd(), self.buf) {
                Ok(0) => return None,
                Ok(bytes_read) => {
                    self.initialized = bytes_read;
                    self.offset = 0;
                }
                Err(e) => return Some(Err(e)),
            }
        }

        let dirent_ptr = self.buf[self.offset..].as_ptr();
        // SAFETY:
        // - This data is initialized by the check above.
        //   - Assumption: the kernel will not give us partial structs.
        // - Assumption: the kernel uses proper alignment between structs.
        // - The starting pointer is aligned (performed in `RawDir::new`).
        let dirent = unsafe { &*dirent_ptr.cast::<linux_dirent64>() };

        self.offset += usize::from(dirent.d_reclen);

        Some(Ok(RawDirEntry {
            file_type: dirent.d_type,
            inode_number: dirent.d_ino.into(),
            next_entry_cookie: dirent.d_off.into(),
            // SAFETY: The kernel guarantees a NUL-terminated string.
            file_name: unsafe { CStr::from_ptr(dirent.d_name.as_ptr().cast()) },
        }))
    }

    /// Returns true if the internal buffer is empty and will be refilled when
    /// calling [`next`].
    ///
    /// [`next`]: Self::next
    pub fn is_buffer_empty(&self) -> bool {
        self.offset >= self.initialized
    }
}
