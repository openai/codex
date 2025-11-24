use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io;
use std::os::windows::io::{AsRawHandle, IntoRawHandle, RawHandle};
use std::path::Path;

use winapi_util as winutil;

// For correctness, it is critical that both file handles remain open while
// their attributes are checked for equality. In particular, the file index
// numbers on a Windows stat object are not guaranteed to remain stable over
// time.
//
// See the docs and remarks on MSDN:
// https://msdn.microsoft.com/en-us/library/windows/desktop/aa363788(v=vs.85).aspx
//
// It gets worse. It appears that the index numbers are not always
// guaranteed to be unique. Namely, ReFS uses 128 bit numbers for unique
// identifiers. This requires a distinct syscall to get `FILE_ID_INFO`
// documented here:
// https://msdn.microsoft.com/en-us/library/windows/desktop/hh802691(v=vs.85).aspx
//
// It seems straight-forward enough to modify this code to use
// `FILE_ID_INFO` when available (minimum Windows Server 2012), but I don't
// have access to such Windows machines.
//
// Two notes.
//
// 1. Java's NIO uses the approach implemented here and appears to ignore
//    `FILE_ID_INFO` altogether. So Java's NIO and this code are
//    susceptible to bugs when running on a file system where
//    `nFileIndex{Low,High}` are not unique.
//
// 2. LLVM has a bug where they fetch the id of a file and continue to use
//    it even after the handle has been closed, so that uniqueness is no
//    longer guaranteed (when `nFileIndex{Low,High}` are unique).
//    bug report: http://lists.llvm.org/pipermail/llvm-bugs/2014-December/037218.html
//
// All said and done, checking whether two files are the same on Windows
// seems quite tricky. Moreover, even if the code is technically incorrect,
// it seems like the chances of actually observing incorrect behavior are
// extremely small. Nevertheless, we mitigate this by checking size too.
//
// In the case where this code is erroneous, two files will be reported
// as equivalent when they are in fact distinct. This will cause the loop
// detection code to report a false positive, which will prevent descending
// into the offending directory. As far as failure modes goes, this isn't
// that bad.

#[derive(Debug)]
pub struct Handle {
    kind: HandleKind,
    key: Option<Key>,
}

#[derive(Debug)]
enum HandleKind {
    /// Used when opening a file or acquiring ownership of a file.
    Owned(winutil::Handle),
    /// Used for stdio.
    Borrowed(winutil::HandleRef),
}

#[derive(Debug, Eq, PartialEq, Hash)]
struct Key {
    volume: u64,
    index: u64,
}

impl Eq for Handle {}

impl PartialEq for Handle {
    fn eq(&self, other: &Handle) -> bool {
        // Need this branch to satisfy `Eq` since `Handle`s with
        // `key.is_none()` wouldn't otherwise.
        if self as *const Handle == other as *const Handle {
            return true;
        } else if self.key.is_none() || other.key.is_none() {
            return false;
        }
        self.key == other.key
    }
}

impl AsRawHandle for crate::Handle {
    fn as_raw_handle(&self) -> RawHandle {
        match self.0.kind {
            HandleKind::Owned(ref h) => h.as_raw_handle(),
            HandleKind::Borrowed(ref h) => h.as_raw_handle(),
        }
    }
}

impl IntoRawHandle for crate::Handle {
    fn into_raw_handle(self) -> RawHandle {
        match self.0.kind {
            HandleKind::Owned(h) => h.into_raw_handle(),
            HandleKind::Borrowed(h) => h.as_raw_handle(),
        }
    }
}

impl Hash for Handle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state);
    }
}

impl Handle {
    pub fn from_path<P: AsRef<Path>>(p: P) -> io::Result<Handle> {
        let h = winutil::Handle::from_path_any(p)?;
        let info = winutil::file::information(&h)?;
        Ok(Handle::from_info(HandleKind::Owned(h), info))
    }

    pub fn from_file(file: File) -> io::Result<Handle> {
        let h = winutil::Handle::from_file(file);
        let info = winutil::file::information(&h)?;
        Ok(Handle::from_info(HandleKind::Owned(h), info))
    }

    fn from_std_handle(h: winutil::HandleRef) -> io::Result<Handle> {
        match winutil::file::information(&h) {
            Ok(info) => Ok(Handle::from_info(HandleKind::Borrowed(h), info)),
            // In a Windows console, if there is no pipe attached to a STD
            // handle, then GetFileInformationByHandle will return an error.
            // We don't really care. The only thing we care about is that
            // this handle is never equivalent to any other handle, which is
            // accomplished by setting key to None.
            Err(_) => Ok(Handle { kind: HandleKind::Borrowed(h), key: None }),
        }
    }

    fn from_info(
        kind: HandleKind,
        info: winutil::file::Information,
    ) -> Handle {
        Handle {
            kind: kind,
            key: Some(Key {
                volume: info.volume_serial_number(),
                index: info.file_index(),
            }),
        }
    }

    pub fn stdin() -> io::Result<Handle> {
        Handle::from_std_handle(winutil::HandleRef::stdin())
    }

    pub fn stdout() -> io::Result<Handle> {
        Handle::from_std_handle(winutil::HandleRef::stdout())
    }

    pub fn stderr() -> io::Result<Handle> {
        Handle::from_std_handle(winutil::HandleRef::stderr())
    }

    pub fn as_file(&self) -> &File {
        match self.kind {
            HandleKind::Owned(ref h) => h.as_file(),
            HandleKind::Borrowed(ref h) => h.as_file(),
        }
    }

    pub fn as_file_mut(&mut self) -> &mut File {
        match self.kind {
            HandleKind::Owned(ref mut h) => h.as_file_mut(),
            HandleKind::Borrowed(ref mut h) => h.as_file_mut(),
        }
    }
}
