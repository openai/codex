//! Socket address utilities.
//!
//! # Safety
//!
//! This file uses `CStr::from_bytes_with_nul_unchecked` on a string it knows
//! to be NUL-terminated.
#![allow(unsafe_code)]

use crate::backend::c;
use crate::ffi::CStr;
use crate::{io, path};
use core::cmp::Ordering;
use core::hash::{Hash, Hasher};
use core::{fmt, slice};

/// `struct sockaddr_un`
#[derive(Clone)]
#[doc(alias = "sockaddr_un")]
pub struct SocketAddrUnix {
    pub(crate) unix: c::sockaddr_un,
    len: c::socklen_t,
}

impl SocketAddrUnix {
    /// Construct a new Unix-domain address from a filesystem path.
    #[inline]
    pub fn new<P: path::Arg>(path: P) -> io::Result<Self> {
        path.into_with_c_str(Self::_new)
    }

    #[inline]
    fn _new(path: &CStr) -> io::Result<Self> {
        let mut unix = Self::init();
        let bytes = path.to_bytes_with_nul();
        if bytes.len() > unix.sun_path.len() {
            return Err(io::Errno::NAMETOOLONG);
        }
        for (i, b) in bytes.iter().enumerate() {
            unix.sun_path[i] = *b as _;
        }
        let len = offsetof_sun_path() + bytes.len();
        let len = len.try_into().unwrap();
        Ok(Self { unix, len })
    }

    /// Construct a new abstract Unix-domain address from a byte slice.
    #[inline]
    pub fn new_abstract_name(name: &[u8]) -> io::Result<Self> {
        let mut unix = Self::init();
        let id = &mut unix.sun_path[1..];

        // SAFETY: Convert `&mut [c_char]` to `&mut [u8]`.
        let id = unsafe { slice::from_raw_parts_mut(id.as_mut_ptr().cast::<u8>(), id.len()) };

        if let Some(id) = id.get_mut(..name.len()) {
            id.copy_from_slice(name);
            let len = offsetof_sun_path() + 1 + name.len();
            let len = len.try_into().unwrap();
            Ok(Self { unix, len })
        } else {
            Err(io::Errno::NAMETOOLONG)
        }
    }

    const fn init() -> c::sockaddr_un {
        c::sockaddr_un {
            sun_family: c::AF_UNIX as _,
            sun_path: [0; 108],
        }
    }

    /// For a filesystem path address, return the path.
    #[inline]
    pub fn path(&self) -> Option<&CStr> {
        let len = self.len();
        if len != 0 && self.unix.sun_path[0] as u8 != b'\0' {
            let end = len as usize - offsetof_sun_path();
            let bytes = &self.unix.sun_path[..end];

            // SAFETY: Convert `&[c_char]` to `&[u8]`.
            let bytes = unsafe { slice::from_raw_parts(bytes.as_ptr().cast::<u8>(), bytes.len()) };

            // SAFETY: `from_bytes_with_nul_unchecked` since the string is
            // NUL-terminated.
            unsafe { Some(CStr::from_bytes_with_nul_unchecked(bytes)) }
        } else {
            None
        }
    }

    /// For an abstract address, return the identifier.
    #[inline]
    pub fn abstract_name(&self) -> Option<&[u8]> {
        let len = self.len();
        if len != 0 && self.unix.sun_path[0] as u8 == b'\0' {
            let end = len as usize - offsetof_sun_path();
            let bytes = &self.unix.sun_path[1..end];

            // SAFETY: Convert `&[c_char]` to `&[u8]`.
            let bytes = unsafe { slice::from_raw_parts(bytes.as_ptr().cast::<u8>(), bytes.len()) };

            Some(bytes)
        } else {
            None
        }
    }

    #[inline]
    pub(crate) fn addr_len(&self) -> c::socklen_t {
        self.len
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.addr_len() as usize
    }
}

impl PartialEq for SocketAddrUnix {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        let self_len = self.len() - offsetof_sun_path();
        let other_len = other.len() - offsetof_sun_path();
        self.unix.sun_path[..self_len].eq(&other.unix.sun_path[..other_len])
    }
}

impl Eq for SocketAddrUnix {}

impl PartialOrd for SocketAddrUnix {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SocketAddrUnix {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        let self_len = self.len() - offsetof_sun_path();
        let other_len = other.len() - offsetof_sun_path();
        self.unix.sun_path[..self_len].cmp(&other.unix.sun_path[..other_len])
    }
}

impl Hash for SocketAddrUnix {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        let self_len = self.len() - offsetof_sun_path();
        self.unix.sun_path[..self_len].hash(state)
    }
}

impl fmt::Debug for SocketAddrUnix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(path) = self.path() {
            path.fmt(f)
        } else if let Some(name) = self.abstract_name() {
            name.fmt(f)
        } else {
            "(unnamed)".fmt(f)
        }
    }
}

/// `struct sockaddr_storage` as a raw struct.
pub type SocketAddrStorage = c::sockaddr;

/// Return the offset of the `sun_path` field of `sockaddr_un`.
#[inline]
pub(crate) fn offsetof_sun_path() -> usize {
    let z = c::sockaddr_un {
        sun_family: 0_u16,
        sun_path: [0; 108],
    };
    (crate::utils::as_ptr(&z.sun_path) as usize) - (crate::utils::as_ptr(&z) as usize)
}
