//! Socket address utilities.

use crate::backend::c;
#[cfg(unix)]
use {
    crate::ffi::CStr,
    crate::io,
    crate::path,
    core::cmp::Ordering,
    core::fmt,
    core::hash::{Hash, Hasher},
    core::slice,
};

/// `struct sockaddr_un`
#[cfg(unix)]
#[derive(Clone)]
#[doc(alias = "sockaddr_un")]
pub struct SocketAddrUnix {
    pub(crate) unix: c::sockaddr_un,
    #[cfg(not(any(bsd, target_os = "haiku")))]
    len: c::socklen_t,
}

#[cfg(unix)]
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
            unix.sun_path[i] = *b as c::c_char;
        }

        #[cfg(any(bsd, target_os = "haiku"))]
        {
            unix.sun_len = (offsetof_sun_path() + bytes.len()).try_into().unwrap();
        }

        Ok(Self {
            unix,
            #[cfg(not(any(bsd, target_os = "haiku")))]
            len: (offsetof_sun_path() + bytes.len()).try_into().unwrap(),
        })
    }

    /// Construct a new abstract Unix-domain address from a byte slice.
    #[cfg(linux_kernel)]
    #[inline]
    pub fn new_abstract_name(name: &[u8]) -> io::Result<Self> {
        let mut unix = Self::init();
        if 1 + name.len() > unix.sun_path.len() {
            return Err(io::Errno::NAMETOOLONG);
        }
        unix.sun_path[0] = 0;
        for (i, b) in name.iter().enumerate() {
            unix.sun_path[1 + i] = *b as c::c_char;
        }
        let len = offsetof_sun_path() + 1 + name.len();
        let len = len.try_into().unwrap();
        Ok(Self {
            unix,
            #[cfg(not(any(bsd, target_os = "haiku")))]
            len,
        })
    }

    const fn init() -> c::sockaddr_un {
        c::sockaddr_un {
            #[cfg(any(
                bsd,
                target_os = "aix",
                target_os = "haiku",
                target_os = "nto",
                target_os = "hurd",
            ))]
            sun_len: 0,
            #[cfg(target_os = "vita")]
            ss_len: 0,
            sun_family: c::AF_UNIX as _,
            #[cfg(any(bsd, target_os = "nto"))]
            sun_path: [0; 104],
            #[cfg(not(any(bsd, target_os = "aix", target_os = "haiku", target_os = "nto")))]
            sun_path: [0; 108],
            #[cfg(target_os = "haiku")]
            sun_path: [0; 126],
            #[cfg(target_os = "aix")]
            sun_path: [0; 1023],
        }
    }

    /// For a filesystem path address, return the path.
    #[inline]
    pub fn path(&self) -> Option<&CStr> {
        let len = self.len();
        if len != 0 && self.unix.sun_path[0] != 0 {
            let end = len as usize - offsetof_sun_path();
            let bytes = &self.unix.sun_path[..end];
            // SAFETY: `from_raw_parts` to convert from `&[c_char]` to `&[u8]`.
            // And `from_bytes_with_nul_unchecked` since the string is
            // NUL-terminated.
            unsafe {
                Some(CStr::from_bytes_with_nul_unchecked(slice::from_raw_parts(
                    bytes.as_ptr().cast(),
                    bytes.len(),
                )))
            }
        } else {
            None
        }
    }

    /// For an abstract address, return the identifier.
    #[cfg(linux_kernel)]
    #[inline]
    pub fn abstract_name(&self) -> Option<&[u8]> {
        let len = self.len();
        if len != 0 && self.unix.sun_path[0] == 0 {
            let end = len as usize - offsetof_sun_path();
            let bytes = &self.unix.sun_path[1..end];
            // SAFETY: `from_raw_parts` to convert from `&[c_char]` to `&[u8]`.
            unsafe { Some(slice::from_raw_parts(bytes.as_ptr().cast(), bytes.len())) }
        } else {
            None
        }
    }

    #[inline]
    pub(crate) fn addr_len(&self) -> c::socklen_t {
        #[cfg(not(any(bsd, target_os = "haiku")))]
        {
            self.len
        }
        #[cfg(any(bsd, target_os = "haiku"))]
        {
            c::socklen_t::from(self.unix.sun_len)
        }
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.addr_len() as usize
    }
}

#[cfg(unix)]
impl PartialEq for SocketAddrUnix {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        let self_len = self.len() - offsetof_sun_path();
        let other_len = other.len() - offsetof_sun_path();
        self.unix.sun_path[..self_len].eq(&other.unix.sun_path[..other_len])
    }
}

#[cfg(unix)]
impl Eq for SocketAddrUnix {}

#[cfg(unix)]
impl PartialOrd for SocketAddrUnix {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(unix)]
impl Ord for SocketAddrUnix {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        let self_len = self.len() - offsetof_sun_path();
        let other_len = other.len() - offsetof_sun_path();
        self.unix.sun_path[..self_len].cmp(&other.unix.sun_path[..other_len])
    }
}

#[cfg(unix)]
impl Hash for SocketAddrUnix {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        let self_len = self.len() - offsetof_sun_path();
        self.unix.sun_path[..self_len].hash(state)
    }
}

#[cfg(unix)]
impl fmt::Debug for SocketAddrUnix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(path) = self.path() {
            path.fmt(f)
        } else {
            #[cfg(linux_kernel)]
            if let Some(name) = self.abstract_name() {
                return name.fmt(f);
            }

            "(unnamed)".fmt(f)
        }
    }
}

/// `struct sockaddr_storage` as a raw struct.
pub type SocketAddrStorage = c::sockaddr_storage;

/// Return the offset of the `sun_path` field of `sockaddr_un`.
#[cfg(not(windows))]
#[inline]
pub(crate) fn offsetof_sun_path() -> usize {
    let z = c::sockaddr_un {
        #[cfg(any(
            bsd,
            target_os = "aix",
            target_os = "haiku",
            target_os = "hurd",
            target_os = "nto",
        ))]
        sun_len: 0_u8,
        #[cfg(target_os = "vita")]
        ss_len: 0,
        #[cfg(any(
            bsd,
            target_os = "aix",
            target_os = "espidf",
            target_os = "haiku",
            target_os = "hurd",
            target_os = "nto",
            target_os = "vita"
        ))]
        sun_family: 0_u8,
        #[cfg(not(any(
            bsd,
            target_os = "aix",
            target_os = "espidf",
            target_os = "haiku",
            target_os = "hurd",
            target_os = "nto",
            target_os = "vita"
        )))]
        sun_family: 0_u16,
        #[cfg(any(bsd, target_os = "nto"))]
        sun_path: [0; 104],
        #[cfg(not(any(bsd, target_os = "aix", target_os = "haiku", target_os = "nto")))]
        sun_path: [0; 108],
        #[cfg(target_os = "haiku")]
        sun_path: [0; 126],
        #[cfg(target_os = "aix")]
        sun_path: [0; 1023],
    };
    (crate::utils::as_ptr(&z.sun_path) as usize) - (crate::utils::as_ptr(&z) as usize)
}
