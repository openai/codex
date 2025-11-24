//! Safe wrappers around errno functions
//!
//! # Example
//! ```
//! use nix::errno::Errno;
//!
//! Errno::EIO.set();
//! assert_eq!(Errno::last(), Errno::EIO);
//!
//! Errno::clear();
//! assert_eq!(Errno::last(), Errno::from_raw(0));
//! ```

use crate::Result;
use cfg_if::cfg_if;
use libc::{c_int, c_void};
use std::{error, fmt, io};

pub use self::consts::*;

cfg_if! {
    if #[cfg(any(target_os = "freebsd",
                 apple_targets,))] {
        unsafe fn errno_location() -> *mut c_int {
            unsafe { libc::__error() }
        }
    } else if #[cfg(any(target_os = "android", netbsdlike))] {
        unsafe fn errno_location() -> *mut c_int {
            unsafe { libc::__errno() }
        }
    } else if #[cfg(any(target_os = "linux",
                        target_os = "redox",
                        target_os = "dragonfly",
                        target_os = "fuchsia",
                        target_os = "hurd"))] {
        unsafe fn errno_location() -> *mut c_int {
            unsafe { libc::__errno_location() }
        }
    } else if #[cfg(solarish)] {
        unsafe fn errno_location() -> *mut c_int {
            unsafe { libc::___errno() }
        }
    } else if #[cfg(any(target_os = "haiku",))] {
        unsafe fn errno_location() -> *mut c_int {
            unsafe { libc::_errnop() }
        }
    } else if #[cfg(any(target_os = "aix"))] {
        unsafe fn errno_location() -> *mut c_int {
            unsafe { libc::_Errno() }
        }
    }
}

/// Returns the platform-specific value of errno
#[deprecated(since = "0.28.0", note = "please use `Errno::last_raw()` instead")]
pub fn errno() -> i32 {
    Errno::last_raw()
}

impl Errno {
    /// Returns the current value of errno
    pub fn last() -> Self {
        Self::from_raw(Self::last_raw())
    }

    /// Returns the current raw i32 value of errno
    pub fn last_raw() -> i32 {
        unsafe { *errno_location() }
    }

    /// Sets the value of errno.
    ///
    /// # Example
    /// ```
    /// use nix::errno::Errno;
    ///
    /// Errno::EIO.set();
    ///
    /// assert_eq!(Errno::last(), Errno::EIO);
    /// ```
    pub fn set(self) {
        Self::set_raw(self as i32)
    }

    /// Sets the raw i32 value of errno.
    pub fn set_raw(errno: i32) {
        // Safe because errno is a thread-local variable
        unsafe {
            *errno_location() = errno;
        }
    }

    #[deprecated(
        since = "0.28.0",
        note = "please use `Errno::from_raw()` instead"
    )]
    pub const fn from_i32(err: i32) -> Errno {
        Self::from_raw(err)
    }

    pub const fn from_raw(err: i32) -> Errno {
        #[allow(deprecated)]
        from_i32(err)
    }

    pub fn desc(self) -> &'static str {
        desc(self)
    }

    /// Sets the platform-specific errno to no-error
    ///
    /// ```
    /// use nix::errno::Errno;
    ///
    /// Errno::EIO.set();
    ///
    /// Errno::clear();
    ///
    /// let err = Errno::last();
    /// assert_ne!(err, Errno::EIO);
    /// assert_eq!(err, Errno::from_raw(0));
    /// ```
    pub fn clear() {
        Self::set_raw(0)
    }

    /// Returns `Ok(value)` if it does not contain the sentinel value. This
    /// should not be used when `-1` is not the errno sentinel value.
    #[inline]
    pub fn result<S: ErrnoSentinel + PartialEq<S>>(value: S) -> Result<S> {
        if value == S::sentinel() {
            Err(Self::last())
        } else {
            Ok(value)
        }
    }
}

/// The sentinel value indicates that a function failed and more detailed
/// information about the error can be found in `errno`
pub trait ErrnoSentinel: Sized {
    fn sentinel() -> Self;
}

impl ErrnoSentinel for isize {
    fn sentinel() -> Self {
        -1
    }
}

impl ErrnoSentinel for i32 {
    fn sentinel() -> Self {
        -1
    }
}

impl ErrnoSentinel for i64 {
    fn sentinel() -> Self {
        -1
    }
}

impl ErrnoSentinel for *mut c_void {
    fn sentinel() -> Self {
        -1isize as *mut c_void
    }
}

impl ErrnoSentinel for libc::sighandler_t {
    fn sentinel() -> Self {
        libc::SIG_ERR
    }
}

impl error::Error for Errno {}

impl fmt::Display for Errno {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}: {}", self, self.desc())
    }
}

impl From<Errno> for io::Error {
    fn from(err: Errno) -> Self {
        io::Error::from_raw_os_error(err as i32)
    }
}

impl TryFrom<io::Error> for Errno {
    type Error = io::Error;

    fn try_from(ioerror: io::Error) -> std::result::Result<Self, io::Error> {
        ioerror.raw_os_error().map(Errno::from_raw).ok_or(ioerror)
    }
}

fn desc(errno: Errno) -> &'static str {
    use self::Errno::*;
    match errno {
        UnknownErrno => "Unknown errno",
        EPERM => "Operation not permitted",
        ENOENT => "No such file or directory",
        ESRCH => "No such process",
        EINTR => "Interrupted system call",
        EIO => "I/O error",
        ENXIO => "No such device or address",
        E2BIG => "Argument list too long",
        ENOEXEC => "Exec format error",
        EBADF => "Bad file number",
        ECHILD => "No child processes",
        EAGAIN => "Try again",
        ENOMEM => "Out of memory",
        EACCES => "Permission denied",
        EFAULT => "Bad address",
        #[cfg(not(target_os = "haiku"))]
        ENOTBLK => "Block device required",
        EBUSY => "Device or resource busy",
        EEXIST => "File exists",
        EXDEV => "Cross-device link",
        ENODEV => "No such device",
        ENOTDIR => "Not a directory",
        EISDIR => "Is a directory",
        EINVAL => "Invalid argument",
        ENFILE => "File table overflow",
        EMFILE => "Too many open files",
        ENOTTY => "Not a typewriter",
        ETXTBSY => "Text file busy",
        EFBIG => "File too large",
        ENOSPC => "No space left on device",
        ESPIPE => "Illegal seek",
        EROFS => "Read-only file system",
        EMLINK => "Too many links",
        EPIPE => "Broken pipe",
        EDOM => "Math argument out of domain of func",
        ERANGE => "Math result not representable",
        EDEADLK => "Resource deadlock would occur",
        ENAMETOOLONG => "File name too long",
        ENOLCK => "No record locks available",
        ENOSYS => "Function not implemented",
        ENOTEMPTY => "Directory not empty",
        ELOOP => "Too many symbolic links encountered",
        ENOMSG => "No message of desired type",
        EIDRM => "Identifier removed",
        EINPROGRESS => "Operation now in progress",
        EALREADY => "Operation already in progress",
        ENOTSOCK => "Socket operation on non-socket",
        EDESTADDRREQ => "Destination address required",
        EMSGSIZE => "Message too long",
        EPROTOTYPE => "Protocol wrong type for socket",
        ENOPROTOOPT => "Protocol not available",
        EPROTONOSUPPORT => "Protocol not supported",
        #[cfg(not(target_os = "haiku"))]
        ESOCKTNOSUPPORT => "Socket type not supported",
        #[cfg(not(target_os = "haiku"))]
        EPFNOSUPPORT => "Protocol family not supported",
        #[cfg(not(target_os = "haiku"))]
        EAFNOSUPPORT => "Address family not supported by protocol",
        EADDRINUSE => "Address already in use",
        EADDRNOTAVAIL => "Cannot assign requested address",
        ENETDOWN => "Network is down",
        ENETUNREACH => "Network is unreachable",
        ENETRESET => "Network dropped connection because of reset",
        ECONNABORTED => "Software caused connection abort",
        ECONNRESET => "Connection reset by peer",
        ENOBUFS => "No buffer space available",
        EISCONN => "Transport endpoint is already connected",
        ENOTCONN => "Transport endpoint is not connected",
        ESHUTDOWN => "Cannot send after transport endpoint shutdown",
        #[cfg(not(target_os = "haiku"))]
        ETOOMANYREFS => "Too many references: cannot splice",
        ETIMEDOUT => "Connection timed out",
        ECONNREFUSED => "Connection refused",
        EHOSTDOWN => "Host is down",
        EHOSTUNREACH => "No route to host",

        #[cfg(any(
            linux_android,
            solarish,
            target_os = "aix",
            target_os = "fuchsia"
        ))]
        ECHRNG => "Channel number out of range",

        #[cfg(any(
            linux_android,
            solarish,
            target_os = "aix",
            target_os = "fuchsia"
        ))]
        EL2NSYNC => "Level 2 not synchronized",

        #[cfg(any(
            linux_android,
            solarish,
            target_os = "aix",
            target_os = "fuchsia"
        ))]
        EL3HLT => "Level 3 halted",

        #[cfg(any(
            linux_android,
            solarish,
            target_os = "aix",
            target_os = "fuchsia"
        ))]
        EL3RST => "Level 3 reset",

        #[cfg(any(
            linux_android,
            solarish,
            target_os = "aix",
            target_os = "fuchsia"
        ))]
        ELNRNG => "Link number out of range",

        #[cfg(any(
            linux_android,
            solarish,
            target_os = "aix",
            target_os = "fuchsia"
        ))]
        EUNATCH => "Protocol driver not attached",

        #[cfg(any(
            linux_android,
            solarish,
            target_os = "aix",
            target_os = "fuchsia"
        ))]
        ENOCSI => "No CSI structure available",

        #[cfg(any(
            linux_android,
            solarish,
            target_os = "aix",
            target_os = "fuchsia"
        ))]
        EL2HLT => "Level 2 halted",

        #[cfg(any(linux_android, solarish, target_os = "fuchsia"))]
        EBADE => "Invalid exchange",

        #[cfg(any(linux_android, solarish, target_os = "fuchsia"))]
        EBADR => "Invalid request descriptor",

        #[cfg(any(linux_android, solarish, target_os = "fuchsia"))]
        EXFULL => "Exchange full",

        #[cfg(any(linux_android, solarish, target_os = "fuchsia"))]
        ENOANO => "No anode",

        #[cfg(any(linux_android, solarish, target_os = "fuchsia"))]
        EBADRQC => "Invalid request code",

        #[cfg(any(linux_android, solarish, target_os = "fuchsia"))]
        EBADSLT => "Invalid slot",

        #[cfg(any(linux_android, solarish, target_os = "fuchsia"))]
        EBFONT => "Bad font file format",

        #[cfg(any(
            linux_android,
            solarish,
            target_os = "fuchsia",
            target_os = "hurd"
        ))]
        ENOSTR => "Device not a stream",

        #[cfg(any(
            linux_android,
            solarish,
            target_os = "fuchsia",
            target_os = "hurd"
        ))]
        ENODATA => "No data available",

        #[cfg(any(
            linux_android,
            solarish,
            target_os = "fuchsia",
            target_os = "hurd"
        ))]
        ETIME => "Timer expired",

        #[cfg(any(
            linux_android,
            solarish,
            target_os = "fuchsia",
            target_os = "hurd"
        ))]
        ENOSR => "Out of streams resources",

        #[cfg(any(linux_android, solarish, target_os = "fuchsia"))]
        ENONET => "Machine is not on the network",

        #[cfg(any(linux_android, solarish, target_os = "fuchsia"))]
        ENOPKG => "Package not installed",

        #[cfg(any(
            linux_android,
            solarish,
            target_os = "fuchsia",
            target_os = "hurd"
        ))]
        EREMOTE => "Object is remote",

        #[cfg(any(
            linux_android,
            solarish,
            target_os = "aix",
            target_os = "fuchsia"
        ))]
        ENOLINK => "Link has been severed",

        #[cfg(any(linux_android, solarish, target_os = "fuchsia"))]
        EADV => "Advertise error",

        #[cfg(any(linux_android, solarish, target_os = "fuchsia"))]
        ESRMNT => "Srmount error",

        #[cfg(any(linux_android, solarish, target_os = "fuchsia"))]
        ECOMM => "Communication error on send",

        #[cfg(any(
            linux_android,
            solarish,
            target_os = "aix",
            target_os = "fuchsia",
        ))]
        EPROTO => "Protocol error",

        #[cfg(any(
            linux_android,
            solarish,
            target_os = "aix",
            target_os = "fuchsia"
        ))]
        EMULTIHOP => "Multihop attempted",

        #[cfg(any(linux_android, target_os = "fuchsia"))]
        EDOTDOT => "RFS specific error",

        #[cfg(any(linux_android, target_os = "aix", target_os = "fuchsia"))]
        EBADMSG => "Not a data message",

        #[cfg(solarish)]
        EBADMSG => "Trying to read unreadable message",

        #[cfg(any(
            linux_android,
            target_os = "aix",
            target_os = "fuchsia",
            target_os = "haiku",
            target_os = "hurd"
        ))]
        EOVERFLOW => "Value too large for defined data type",

        #[cfg(any(linux_android, solarish, target_os = "fuchsia"))]
        ENOTUNIQ => "Name not unique on network",

        #[cfg(any(linux_android, solarish, target_os = "fuchsia"))]
        EBADFD => "File descriptor in bad state",

        #[cfg(any(linux_android, solarish, target_os = "fuchsia"))]
        EREMCHG => "Remote address changed",

        #[cfg(any(linux_android, solarish, target_os = "fuchsia"))]
        ELIBACC => "Can not access a needed shared library",

        #[cfg(any(linux_android, solarish, target_os = "fuchsia"))]
        ELIBBAD => "Accessing a corrupted shared library",

        #[cfg(any(linux_android, solarish, target_os = "fuchsia"))]
        ELIBSCN => ".lib section in a.out corrupted",

        #[cfg(any(linux_android, solarish, target_os = "fuchsia"))]
        ELIBMAX => "Attempting to link in too many shared libraries",

        #[cfg(any(
            linux_android,
            solarish,
            target_os = "fuchsia",
            target_os = "hurd"
        ))]
        ELIBEXEC => "Cannot exec a shared library directly",

        #[cfg(any(
            linux_android,
            solarish,
            target_os = "aix",
            target_os = "fuchsia",
            target_os = "openbsd"
        ))]
        EILSEQ => "Illegal byte sequence",

        #[cfg(any(
            linux_android,
            solarish,
            target_os = "aix",
            target_os = "fuchsia"
        ))]
        ERESTART => "Interrupted system call should be restarted",

        #[cfg(any(linux_android, solarish, target_os = "fuchsia"))]
        ESTRPIPE => "Streams pipe error",

        #[cfg(any(linux_android, solarish, target_os = "fuchsia"))]
        EUSERS => "Too many users",

        #[cfg(any(
            linux_android,
            target_os = "fuchsia",
            target_os = "netbsd",
            target_os = "redox"
        ))]
        EOPNOTSUPP => "Operation not supported on transport endpoint",

        #[cfg(any(linux_android, target_os = "fuchsia", target_os = "hurd"))]
        ESTALE => "Stale file handle",

        #[cfg(any(linux_android, target_os = "fuchsia"))]
        EUCLEAN => "Structure needs cleaning",

        #[cfg(any(linux_android, target_os = "fuchsia"))]
        ENOTNAM => "Not a XENIX named type file",

        #[cfg(any(linux_android, target_os = "fuchsia"))]
        ENAVAIL => "No XENIX semaphores available",

        #[cfg(any(linux_android, target_os = "fuchsia"))]
        EISNAM => "Is a named type file",

        #[cfg(any(linux_android, target_os = "fuchsia"))]
        EREMOTEIO => "Remote I/O error",

        #[cfg(any(linux_android, target_os = "fuchsia"))]
        EDQUOT => "Quota exceeded",

        #[cfg(any(
            linux_android,
            target_os = "fuchsia",
            target_os = "openbsd",
            target_os = "dragonfly"
        ))]
        ENOMEDIUM => "No medium found",

        #[cfg(any(
            linux_android,
            target_os = "fuchsia",
            target_os = "openbsd"
        ))]
        EMEDIUMTYPE => "Wrong medium type",

        #[cfg(any(
            linux_android,
            solarish,
            target_os = "fuchsia",
            target_os = "haiku"
        ))]
        ECANCELED => "Operation canceled",

        #[cfg(any(linux_android, target_os = "fuchsia"))]
        ENOKEY => "Required key not available",

        #[cfg(any(linux_android, target_os = "fuchsia"))]
        EKEYEXPIRED => "Key has expired",

        #[cfg(any(linux_android, target_os = "fuchsia"))]
        EKEYREVOKED => "Key has been revoked",

        #[cfg(any(linux_android, target_os = "fuchsia"))]
        EKEYREJECTED => "Key was rejected by service",

        #[cfg(any(
            linux_android,
            target_os = "aix",
            target_os = "fuchsia",
            target_os = "hurd"
        ))]
        EOWNERDEAD => "Owner died",

        #[cfg(solarish)]
        EOWNERDEAD => "Process died with lock",

        #[cfg(any(linux_android, target_os = "aix", target_os = "fuchsia"))]
        ENOTRECOVERABLE => "State not recoverable",

        #[cfg(solarish)]
        ENOTRECOVERABLE => "Lock is not recoverable",

        #[cfg(any(
            all(target_os = "linux", not(target_arch = "mips")),
            target_os = "fuchsia"
        ))]
        ERFKILL => "Operation not possible due to RF-kill",

        #[cfg(any(
            all(target_os = "linux", not(target_arch = "mips")),
            target_os = "fuchsia"
        ))]
        EHWPOISON => "Memory page has hardware error",

        #[cfg(freebsdlike)]
        EDOOFUS => "Programming error",

        #[cfg(any(freebsdlike, target_os = "hurd", target_os = "redox"))]
        EMULTIHOP => "Multihop attempted",

        #[cfg(any(freebsdlike, target_os = "hurd", target_os = "redox"))]
        ENOLINK => "Link has been severed",

        #[cfg(target_os = "freebsd")]
        ENOTCAPABLE => "Capabilities insufficient",

        #[cfg(target_os = "freebsd")]
        ECAPMODE => "Not permitted in capability mode",

        #[cfg(any(bsd, target_os = "hurd"))]
        ENEEDAUTH => "Need authenticator",

        #[cfg(any(bsd, target_os = "redox", solarish))]
        EOVERFLOW => "Value too large to be stored in data type",

        #[cfg(any(
            freebsdlike,
            apple_targets,
            target_os = "netbsd",
            target_os = "redox",
            target_os = "haiku",
            target_os = "hurd"
        ))]
        EILSEQ => "Illegal byte sequence",

        #[cfg(any(bsd, target_os = "haiku"))]
        ENOATTR => "Attribute not found",

        #[cfg(any(
            bsd,
            target_os = "redox",
            target_os = "haiku",
            target_os = "hurd"
        ))]
        EBADMSG => "Bad message",

        #[cfg(any(
            bsd,
            target_os = "haiku",
            target_os = "hurd",
            target_os = "redox"
        ))]
        EPROTO => "Protocol error",

        #[cfg(any(
            freebsdlike,
            apple_targets,
            target_os = "openbsd",
            target_os = "hurd"
        ))]
        ENOTRECOVERABLE => "State not recoverable",

        #[cfg(any(freebsdlike, apple_targets, target_os = "openbsd"))]
        EOWNERDEAD => "Previous owner died",

        #[cfg(any(
            bsd,
            target_os = "aix",
            solarish,
            target_os = "haiku",
            target_os = "hurd"
        ))]
        ENOTSUP => "Operation not supported",

        #[cfg(any(bsd, target_os = "aix", target_os = "hurd"))]
        EPROCLIM => "Too many processes",

        #[cfg(any(
            bsd,
            target_os = "aix",
            target_os = "hurd",
            target_os = "redox"
        ))]
        EUSERS => "Too many users",

        #[cfg(any(
            bsd,
            solarish,
            target_os = "redox",
            target_os = "aix",
            target_os = "haiku",
            target_os = "hurd"
        ))]
        EDQUOT => "Disc quota exceeded",

        #[cfg(any(
            bsd,
            solarish,
            target_os = "redox",
            target_os = "aix",
            target_os = "haiku"
        ))]
        ESTALE => "Stale NFS file handle",

        #[cfg(any(bsd, target_os = "aix", target_os = "redox"))]
        EREMOTE => "Too many levels of remote in path",

        #[cfg(any(bsd, target_os = "hurd"))]
        EBADRPC => "RPC struct is bad",

        #[cfg(any(bsd, target_os = "hurd"))]
        ERPCMISMATCH => "RPC version wrong",

        #[cfg(any(bsd, target_os = "hurd"))]
        EPROGUNAVAIL => "RPC prog. not avail",

        #[cfg(any(bsd, target_os = "hurd"))]
        EPROGMISMATCH => "Program version wrong",

        #[cfg(any(bsd, target_os = "hurd"))]
        EPROCUNAVAIL => "Bad procedure for program",

        #[cfg(any(bsd, target_os = "hurd"))]
        EFTYPE => "Inappropriate file type or format",

        #[cfg(any(bsd, target_os = "hurd"))]
        EAUTH => "Authentication error",

        #[cfg(any(
            bsd,
            target_os = "aix",
            target_os = "hurd",
            target_os = "redox"
        ))]
        ECANCELED => "Operation canceled",

        #[cfg(apple_targets)]
        EPWROFF => "Device power is off",

        #[cfg(apple_targets)]
        EDEVERR => "Device error, e.g. paper out",

        #[cfg(apple_targets)]
        EBADEXEC => "Bad executable",

        #[cfg(apple_targets)]
        EBADARCH => "Bad CPU type in executable",

        #[cfg(apple_targets)]
        ESHLIBVERS => "Shared library version mismatch",

        #[cfg(apple_targets)]
        EBADMACHO => "Malformed Macho file",

        #[cfg(any(apple_targets, target_os = "netbsd", target_os = "haiku"))]
        EMULTIHOP => "Reserved",

        #[cfg(any(
            apple_targets,
            target_os = "aix",
            target_os = "netbsd",
            target_os = "redox"
        ))]
        ENODATA => "No message available on STREAM",

        #[cfg(any(apple_targets, target_os = "netbsd", target_os = "haiku"))]
        ENOLINK => "Reserved",

        #[cfg(any(
            apple_targets,
            target_os = "aix",
            target_os = "netbsd",
            target_os = "redox"
        ))]
        ENOSR => "No STREAM resources",

        #[cfg(any(
            apple_targets,
            target_os = "aix",
            target_os = "netbsd",
            target_os = "redox"
        ))]
        ENOSTR => "Not a STREAM",

        #[cfg(any(
            apple_targets,
            target_os = "aix",
            target_os = "netbsd",
            target_os = "redox"
        ))]
        ETIME => "STREAM ioctl timeout",

        #[cfg(any(apple_targets, solarish, target_os = "aix"))]
        EOPNOTSUPP => "Operation not supported on socket",

        #[cfg(apple_targets)]
        ENOPOLICY => "No such policy registered",

        #[cfg(apple_targets)]
        EQFULL => "Interface output queue is full",

        #[cfg(any(target_os = "openbsd", target_os = "hurd"))]
        EOPNOTSUPP => "Operation not supported",

        #[cfg(target_os = "openbsd")]
        EIPSEC => "IPsec processing failure",

        #[cfg(target_os = "dragonfly")]
        EASYNC => "Async",

        #[cfg(solarish)]
        EDEADLOCK => "Resource deadlock would occur",

        #[cfg(solarish)]
        ELOCKUNMAPPED => "Locked lock was unmapped",

        #[cfg(solarish)]
        ENOTACTIVE => "Facility is not active",

        #[cfg(target_os = "hurd")]
        EBACKGROUND => "Inappropriate operation for background process",

        #[cfg(target_os = "hurd")]
        EDIED => "Translator died",

        #[cfg(target_os = "hurd")]
        EGREGIOUS => "You really blew it this time",

        #[cfg(target_os = "hurd")]
        EIEIO => "Computer bought the farm",

        #[cfg(target_os = "hurd")]
        EGRATUITOUS => "Gratuitous error",
    }
}

#[cfg(any(linux_android, target_os = "fuchsia"))]
mod consts {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(i32)]
    #[non_exhaustive]
    pub enum Errno {
        UnknownErrno = 0,
        EPERM = libc::EPERM,
        ENOENT = libc::ENOENT,
        ESRCH = libc::ESRCH,
        EINTR = libc::EINTR,
        EIO = libc::EIO,
        ENXIO = libc::ENXIO,
        E2BIG = libc::E2BIG,
        ENOEXEC = libc::ENOEXEC,
        EBADF = libc::EBADF,
        ECHILD = libc::ECHILD,
        EAGAIN = libc::EAGAIN,
        ENOMEM = libc::ENOMEM,
        EACCES = libc::EACCES,
        EFAULT = libc::EFAULT,
        ENOTBLK = libc::ENOTBLK,
        EBUSY = libc::EBUSY,
        EEXIST = libc::EEXIST,
        EXDEV = libc::EXDEV,
        ENODEV = libc::ENODEV,
        ENOTDIR = libc::ENOTDIR,
        EISDIR = libc::EISDIR,
        EINVAL = libc::EINVAL,
        ENFILE = libc::ENFILE,
        EMFILE = libc::EMFILE,
        ENOTTY = libc::ENOTTY,
        ETXTBSY = libc::ETXTBSY,
        EFBIG = libc::EFBIG,
        ENOSPC = libc::ENOSPC,
        ESPIPE = libc::ESPIPE,
        EROFS = libc::EROFS,
        EMLINK = libc::EMLINK,
        EPIPE = libc::EPIPE,
        EDOM = libc::EDOM,
        ERANGE = libc::ERANGE,
        EDEADLK = libc::EDEADLK,
        ENAMETOOLONG = libc::ENAMETOOLONG,
        ENOLCK = libc::ENOLCK,
        ENOSYS = libc::ENOSYS,
        ENOTEMPTY = libc::ENOTEMPTY,
        ELOOP = libc::ELOOP,
        ENOMSG = libc::ENOMSG,
        EIDRM = libc::EIDRM,
        ECHRNG = libc::ECHRNG,
        EL2NSYNC = libc::EL2NSYNC,
        EL3HLT = libc::EL3HLT,
        EL3RST = libc::EL3RST,
        ELNRNG = libc::ELNRNG,
        EUNATCH = libc::EUNATCH,
        ENOCSI = libc::ENOCSI,
        EL2HLT = libc::EL2HLT,
        EBADE = libc::EBADE,
        EBADR = libc::EBADR,
        EXFULL = libc::EXFULL,
        ENOANO = libc::ENOANO,
        EBADRQC = libc::EBADRQC,
        EBADSLT = libc::EBADSLT,
        EBFONT = libc::EBFONT,
        ENOSTR = libc::ENOSTR,
        ENODATA = libc::ENODATA,
        ETIME = libc::ETIME,
        ENOSR = libc::ENOSR,
        ENONET = libc::ENONET,
        ENOPKG = libc::ENOPKG,
        EREMOTE = libc::EREMOTE,
        ENOLINK = libc::ENOLINK,
        EADV = libc::EADV,
        ESRMNT = libc::ESRMNT,
        ECOMM = libc::ECOMM,
        EPROTO = libc::EPROTO,
        EMULTIHOP = libc::EMULTIHOP,
        EDOTDOT = libc::EDOTDOT,
        EBADMSG = libc::EBADMSG,
        EOVERFLOW = libc::EOVERFLOW,
        ENOTUNIQ = libc::ENOTUNIQ,
        EBADFD = libc::EBADFD,
        EREMCHG = libc::EREMCHG,
        ELIBACC = libc::ELIBACC,
        ELIBBAD = libc::ELIBBAD,
        ELIBSCN = libc::ELIBSCN,
        ELIBMAX = libc::ELIBMAX,
        ELIBEXEC = libc::ELIBEXEC,
        EILSEQ = libc::EILSEQ,
        ERESTART = libc::ERESTART,
        ESTRPIPE = libc::ESTRPIPE,
        EUSERS = libc::EUSERS,
        ENOTSOCK = libc::ENOTSOCK,
        EDESTADDRREQ = libc::EDESTADDRREQ,
        EMSGSIZE = libc::EMSGSIZE,
        EPROTOTYPE = libc::EPROTOTYPE,
        ENOPROTOOPT = libc::ENOPROTOOPT,
        EPROTONOSUPPORT = libc::EPROTONOSUPPORT,
        ESOCKTNOSUPPORT = libc::ESOCKTNOSUPPORT,
        EOPNOTSUPP = libc::EOPNOTSUPP,
        EPFNOSUPPORT = libc::EPFNOSUPPORT,
        EAFNOSUPPORT = libc::EAFNOSUPPORT,
        EADDRINUSE = libc::EADDRINUSE,
        EADDRNOTAVAIL = libc::EADDRNOTAVAIL,
        ENETDOWN = libc::ENETDOWN,
        ENETUNREACH = libc::ENETUNREACH,
        ENETRESET = libc::ENETRESET,
        ECONNABORTED = libc::ECONNABORTED,
        ECONNRESET = libc::ECONNRESET,
        ENOBUFS = libc::ENOBUFS,
        EISCONN = libc::EISCONN,
        ENOTCONN = libc::ENOTCONN,
        ESHUTDOWN = libc::ESHUTDOWN,
        ETOOMANYREFS = libc::ETOOMANYREFS,
        ETIMEDOUT = libc::ETIMEDOUT,
        ECONNREFUSED = libc::ECONNREFUSED,
        EHOSTDOWN = libc::EHOSTDOWN,
        EHOSTUNREACH = libc::EHOSTUNREACH,
        EALREADY = libc::EALREADY,
        EINPROGRESS = libc::EINPROGRESS,
        ESTALE = libc::ESTALE,
        EUCLEAN = libc::EUCLEAN,
        ENOTNAM = libc::ENOTNAM,
        ENAVAIL = libc::ENAVAIL,
        EISNAM = libc::EISNAM,
        EREMOTEIO = libc::EREMOTEIO,
        EDQUOT = libc::EDQUOT,
        ENOMEDIUM = libc::ENOMEDIUM,
        EMEDIUMTYPE = libc::EMEDIUMTYPE,
        ECANCELED = libc::ECANCELED,
        ENOKEY = libc::ENOKEY,
        EKEYEXPIRED = libc::EKEYEXPIRED,
        EKEYREVOKED = libc::EKEYREVOKED,
        EKEYREJECTED = libc::EKEYREJECTED,
        EOWNERDEAD = libc::EOWNERDEAD,
        ENOTRECOVERABLE = libc::ENOTRECOVERABLE,
        #[cfg(not(any(target_os = "android", target_arch = "mips")))]
        ERFKILL = libc::ERFKILL,
        #[cfg(not(any(target_os = "android", target_arch = "mips")))]
        EHWPOISON = libc::EHWPOISON,
    }

    impl Errno {
        pub const EWOULDBLOCK: Errno = Errno::EAGAIN;
        pub const EDEADLOCK: Errno = Errno::EDEADLK;
        pub const ENOTSUP: Errno = Errno::EOPNOTSUPP;
    }

    #[deprecated(
        since = "0.28.0",
        note = "please use `Errno::from_raw()` instead"
    )]
    pub const fn from_i32(e: i32) -> Errno {
        use self::Errno::*;

        match e {
            libc::EPERM => EPERM,
            libc::ENOENT => ENOENT,
            libc::ESRCH => ESRCH,
            libc::EINTR => EINTR,
            libc::EIO => EIO,
            libc::ENXIO => ENXIO,
            libc::E2BIG => E2BIG,
            libc::ENOEXEC => ENOEXEC,
            libc::EBADF => EBADF,
            libc::ECHILD => ECHILD,
            libc::EAGAIN => EAGAIN,
            libc::ENOMEM => ENOMEM,
            libc::EACCES => EACCES,
            libc::EFAULT => EFAULT,
            libc::ENOTBLK => ENOTBLK,
            libc::EBUSY => EBUSY,
            libc::EEXIST => EEXIST,
            libc::EXDEV => EXDEV,
            libc::ENODEV => ENODEV,
            libc::ENOTDIR => ENOTDIR,
            libc::EISDIR => EISDIR,
            libc::EINVAL => EINVAL,
            libc::ENFILE => ENFILE,
            libc::EMFILE => EMFILE,
            libc::ENOTTY => ENOTTY,
            libc::ETXTBSY => ETXTBSY,
            libc::EFBIG => EFBIG,
            libc::ENOSPC => ENOSPC,
            libc::ESPIPE => ESPIPE,
            libc::EROFS => EROFS,
            libc::EMLINK => EMLINK,
            libc::EPIPE => EPIPE,
            libc::EDOM => EDOM,
            libc::ERANGE => ERANGE,
            libc::EDEADLK => EDEADLK,
            libc::ENAMETOOLONG => ENAMETOOLONG,
            libc::ENOLCK => ENOLCK,
            libc::ENOSYS => ENOSYS,
            libc::ENOTEMPTY => ENOTEMPTY,
            libc::ELOOP => ELOOP,
            libc::ENOMSG => ENOMSG,
            libc::EIDRM => EIDRM,
            libc::ECHRNG => ECHRNG,
            libc::EL2NSYNC => EL2NSYNC,
            libc::EL3HLT => EL3HLT,
            libc::EL3RST => EL3RST,
            libc::ELNRNG => ELNRNG,
            libc::EUNATCH => EUNATCH,
            libc::ENOCSI => ENOCSI,
            libc::EL2HLT => EL2HLT,
            libc::EBADE => EBADE,
            libc::EBADR => EBADR,
            libc::EXFULL => EXFULL,
            libc::ENOANO => ENOANO,
            libc::EBADRQC => EBADRQC,
            libc::EBADSLT => EBADSLT,
            libc::EBFONT => EBFONT,
            libc::ENOSTR => ENOSTR,
            libc::ENODATA => ENODATA,
            libc::ETIME => ETIME,
            libc::ENOSR => ENOSR,
            libc::ENONET => ENONET,
            libc::ENOPKG => ENOPKG,
            libc::EREMOTE => EREMOTE,
            libc::ENOLINK => ENOLINK,
            libc::EADV => EADV,
            libc::ESRMNT => ESRMNT,
            libc::ECOMM => ECOMM,
            libc::EPROTO => EPROTO,
            libc::EMULTIHOP => EMULTIHOP,
            libc::EDOTDOT => EDOTDOT,
            libc::EBADMSG => EBADMSG,
            libc::EOVERFLOW => EOVERFLOW,
            libc::ENOTUNIQ => ENOTUNIQ,
            libc::EBADFD => EBADFD,
            libc::EREMCHG => EREMCHG,
            libc::ELIBACC => ELIBACC,
            libc::ELIBBAD => ELIBBAD,
            libc::ELIBSCN => ELIBSCN,
            libc::ELIBMAX => ELIBMAX,
            libc::ELIBEXEC => ELIBEXEC,
            libc::EILSEQ => EILSEQ,
            libc::ERESTART => ERESTART,
            libc::ESTRPIPE => ESTRPIPE,
            libc::EUSERS => EUSERS,
            libc::ENOTSOCK => ENOTSOCK,
            libc::EDESTADDRREQ => EDESTADDRREQ,
            libc::EMSGSIZE => EMSGSIZE,
            libc::EPROTOTYPE => EPROTOTYPE,
            libc::ENOPROTOOPT => ENOPROTOOPT,
            libc::EPROTONOSUPPORT => EPROTONOSUPPORT,
            libc::ESOCKTNOSUPPORT => ESOCKTNOSUPPORT,
            libc::EOPNOTSUPP => EOPNOTSUPP,
            libc::EPFNOSUPPORT => EPFNOSUPPORT,
            libc::EAFNOSUPPORT => EAFNOSUPPORT,
            libc::EADDRINUSE => EADDRINUSE,
            libc::EADDRNOTAVAIL => EADDRNOTAVAIL,
            libc::ENETDOWN => ENETDOWN,
            libc::ENETUNREACH => ENETUNREACH,
            libc::ENETRESET => ENETRESET,
            libc::ECONNABORTED => ECONNABORTED,
            libc::ECONNRESET => ECONNRESET,
            libc::ENOBUFS => ENOBUFS,
            libc::EISCONN => EISCONN,
            libc::ENOTCONN => ENOTCONN,
            libc::ESHUTDOWN => ESHUTDOWN,
            libc::ETOOMANYREFS => ETOOMANYREFS,
            libc::ETIMEDOUT => ETIMEDOUT,
            libc::ECONNREFUSED => ECONNREFUSED,
            libc::EHOSTDOWN => EHOSTDOWN,
            libc::EHOSTUNREACH => EHOSTUNREACH,
            libc::EALREADY => EALREADY,
            libc::EINPROGRESS => EINPROGRESS,
            libc::ESTALE => ESTALE,
            libc::EUCLEAN => EUCLEAN,
            libc::ENOTNAM => ENOTNAM,
            libc::ENAVAIL => ENAVAIL,
            libc::EISNAM => EISNAM,
            libc::EREMOTEIO => EREMOTEIO,
            libc::EDQUOT => EDQUOT,
            libc::ENOMEDIUM => ENOMEDIUM,
            libc::EMEDIUMTYPE => EMEDIUMTYPE,
            libc::ECANCELED => ECANCELED,
            libc::ENOKEY => ENOKEY,
            libc::EKEYEXPIRED => EKEYEXPIRED,
            libc::EKEYREVOKED => EKEYREVOKED,
            libc::EKEYREJECTED => EKEYREJECTED,
            libc::EOWNERDEAD => EOWNERDEAD,
            libc::ENOTRECOVERABLE => ENOTRECOVERABLE,
            #[cfg(not(any(target_os = "android", target_arch = "mips")))]
            libc::ERFKILL => ERFKILL,
            #[cfg(not(any(target_os = "android", target_arch = "mips")))]
            libc::EHWPOISON => EHWPOISON,
            _ => UnknownErrno,
        }
    }
}

#[cfg(apple_targets)]
mod consts {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(i32)]
    #[non_exhaustive]
    pub enum Errno {
        UnknownErrno = 0,
        EPERM = libc::EPERM,
        ENOENT = libc::ENOENT,
        ESRCH = libc::ESRCH,
        EINTR = libc::EINTR,
        EIO = libc::EIO,
        ENXIO = libc::ENXIO,
        E2BIG = libc::E2BIG,
        ENOEXEC = libc::ENOEXEC,
        EBADF = libc::EBADF,
        ECHILD = libc::ECHILD,
        EDEADLK = libc::EDEADLK,
        ENOMEM = libc::ENOMEM,
        EACCES = libc::EACCES,
        EFAULT = libc::EFAULT,
        ENOTBLK = libc::ENOTBLK,
        EBUSY = libc::EBUSY,
        EEXIST = libc::EEXIST,
        EXDEV = libc::EXDEV,
        ENODEV = libc::ENODEV,
        ENOTDIR = libc::ENOTDIR,
        EISDIR = libc::EISDIR,
        EINVAL = libc::EINVAL,
        ENFILE = libc::ENFILE,
        EMFILE = libc::EMFILE,
        ENOTTY = libc::ENOTTY,
        ETXTBSY = libc::ETXTBSY,
        EFBIG = libc::EFBIG,
        ENOSPC = libc::ENOSPC,
        ESPIPE = libc::ESPIPE,
        EROFS = libc::EROFS,
        EMLINK = libc::EMLINK,
        EPIPE = libc::EPIPE,
        EDOM = libc::EDOM,
        ERANGE = libc::ERANGE,
        EAGAIN = libc::EAGAIN,
        EINPROGRESS = libc::EINPROGRESS,
        EALREADY = libc::EALREADY,
        ENOTSOCK = libc::ENOTSOCK,
        EDESTADDRREQ = libc::EDESTADDRREQ,
        EMSGSIZE = libc::EMSGSIZE,
        EPROTOTYPE = libc::EPROTOTYPE,
        ENOPROTOOPT = libc::ENOPROTOOPT,
        EPROTONOSUPPORT = libc::EPROTONOSUPPORT,
        ESOCKTNOSUPPORT = libc::ESOCKTNOSUPPORT,
        ENOTSUP = libc::ENOTSUP,
        EPFNOSUPPORT = libc::EPFNOSUPPORT,
        EAFNOSUPPORT = libc::EAFNOSUPPORT,
        EADDRINUSE = libc::EADDRINUSE,
        EADDRNOTAVAIL = libc::EADDRNOTAVAIL,
        ENETDOWN = libc::ENETDOWN,
        ENETUNREACH = libc::ENETUNREACH,
        ENETRESET = libc::ENETRESET,
        ECONNABORTED = libc::ECONNABORTED,
        ECONNRESET = libc::ECONNRESET,
        ENOBUFS = libc::ENOBUFS,
        EISCONN = libc::EISCONN,
        ENOTCONN = libc::ENOTCONN,
        ESHUTDOWN = libc::ESHUTDOWN,
        ETOOMANYREFS = libc::ETOOMANYREFS,
        ETIMEDOUT = libc::ETIMEDOUT,
        ECONNREFUSED = libc::ECONNREFUSED,
        ELOOP = libc::ELOOP,
        ENAMETOOLONG = libc::ENAMETOOLONG,
        EHOSTDOWN = libc::EHOSTDOWN,
        EHOSTUNREACH = libc::EHOSTUNREACH,
        ENOTEMPTY = libc::ENOTEMPTY,
        EPROCLIM = libc::EPROCLIM,
        EUSERS = libc::EUSERS,
        EDQUOT = libc::EDQUOT,
        ESTALE = libc::ESTALE,
        EREMOTE = libc::EREMOTE,
        EBADRPC = libc::EBADRPC,
        ERPCMISMATCH = libc::ERPCMISMATCH,
        EPROGUNAVAIL = libc::EPROGUNAVAIL,
        EPROGMISMATCH = libc::EPROGMISMATCH,
        EPROCUNAVAIL = libc::EPROCUNAVAIL,
        ENOLCK = libc::ENOLCK,
        ENOSYS = libc::ENOSYS,
        EFTYPE = libc::EFTYPE,
        EAUTH = libc::EAUTH,
        ENEEDAUTH = libc::ENEEDAUTH,
        EPWROFF = libc::EPWROFF,
        EDEVERR = libc::EDEVERR,
        EOVERFLOW = libc::EOVERFLOW,
        EBADEXEC = libc::EBADEXEC,
        EBADARCH = libc::EBADARCH,
        ESHLIBVERS = libc::ESHLIBVERS,
        EBADMACHO = libc::EBADMACHO,
        ECANCELED = libc::ECANCELED,
        EIDRM = libc::EIDRM,
        ENOMSG = libc::ENOMSG,
        EILSEQ = libc::EILSEQ,
        ENOATTR = libc::ENOATTR,
        EBADMSG = libc::EBADMSG,
        EMULTIHOP = libc::EMULTIHOP,
        ENODATA = libc::ENODATA,
        ENOLINK = libc::ENOLINK,
        ENOSR = libc::ENOSR,
        ENOSTR = libc::ENOSTR,
        EPROTO = libc::EPROTO,
        ETIME = libc::ETIME,
        EOPNOTSUPP = libc::EOPNOTSUPP,
        ENOPOLICY = libc::ENOPOLICY,
        ENOTRECOVERABLE = libc::ENOTRECOVERABLE,
        EOWNERDEAD = libc::EOWNERDEAD,
        EQFULL = libc::EQFULL,
    }

    impl Errno {
        pub const ELAST: Errno = Errno::EQFULL;
        pub const EWOULDBLOCK: Errno = Errno::EAGAIN;
        pub const EDEADLOCK: Errno = Errno::EDEADLK;
    }

    #[deprecated(
        since = "0.28.0",
        note = "please use `Errno::from_raw()` instead"
    )]
    pub const fn from_i32(e: i32) -> Errno {
        use self::Errno::*;

        match e {
            libc::EPERM => EPERM,
            libc::ENOENT => ENOENT,
            libc::ESRCH => ESRCH,
            libc::EINTR => EINTR,
            libc::EIO => EIO,
            libc::ENXIO => ENXIO,
            libc::E2BIG => E2BIG,
            libc::ENOEXEC => ENOEXEC,
            libc::EBADF => EBADF,
            libc::ECHILD => ECHILD,
            libc::EDEADLK => EDEADLK,
            libc::ENOMEM => ENOMEM,
            libc::EACCES => EACCES,
            libc::EFAULT => EFAULT,
            libc::ENOTBLK => ENOTBLK,
            libc::EBUSY => EBUSY,
            libc::EEXIST => EEXIST,
            libc::EXDEV => EXDEV,
            libc::ENODEV => ENODEV,
            libc::ENOTDIR => ENOTDIR,
            libc::EISDIR => EISDIR,
            libc::EINVAL => EINVAL,
            libc::ENFILE => ENFILE,
            libc::EMFILE => EMFILE,
            libc::ENOTTY => ENOTTY,
            libc::ETXTBSY => ETXTBSY,
            libc::EFBIG => EFBIG,
            libc::ENOSPC => ENOSPC,
            libc::ESPIPE => ESPIPE,
            libc::EROFS => EROFS,
            libc::EMLINK => EMLINK,
            libc::EPIPE => EPIPE,
            libc::EDOM => EDOM,
            libc::ERANGE => ERANGE,
            libc::EAGAIN => EAGAIN,
            libc::EINPROGRESS => EINPROGRESS,
            libc::EALREADY => EALREADY,
            libc::ENOTSOCK => ENOTSOCK,
            libc::EDESTADDRREQ => EDESTADDRREQ,
            libc::EMSGSIZE => EMSGSIZE,
            libc::EPROTOTYPE => EPROTOTYPE,
            libc::ENOPROTOOPT => ENOPROTOOPT,
            libc::EPROTONOSUPPORT => EPROTONOSUPPORT,
            libc::ESOCKTNOSUPPORT => ESOCKTNOSUPPORT,
            libc::ENOTSUP => ENOTSUP,
            libc::EPFNOSUPPORT => EPFNOSUPPORT,
            libc::EAFNOSUPPORT => EAFNOSUPPORT,
            libc::EADDRINUSE => EADDRINUSE,
            libc::EADDRNOTAVAIL => EADDRNOTAVAIL,
            libc::ENETDOWN => ENETDOWN,
            libc::ENETUNREACH => ENETUNREACH,
            libc::ENETRESET => ENETRESET,
            libc::ECONNABORTED => ECONNABORTED,
            libc::ECONNRESET => ECONNRESET,
            libc::ENOBUFS => ENOBUFS,
            libc::EISCONN => EISCONN,
            libc::ENOTCONN => ENOTCONN,
            libc::ESHUTDOWN => ESHUTDOWN,
            libc::ETOOMANYREFS => ETOOMANYREFS,
            libc::ETIMEDOUT => ETIMEDOUT,
            libc::ECONNREFUSED => ECONNREFUSED,
            libc::ELOOP => ELOOP,
            libc::ENAMETOOLONG => ENAMETOOLONG,
            libc::EHOSTDOWN => EHOSTDOWN,
            libc::EHOSTUNREACH => EHOSTUNREACH,
            libc::ENOTEMPTY => ENOTEMPTY,
            libc::EPROCLIM => EPROCLIM,
            libc::EUSERS => EUSERS,
            libc::EDQUOT => EDQUOT,
            libc::ESTALE => ESTALE,
            libc::EREMOTE => EREMOTE,
            libc::EBADRPC => EBADRPC,
            libc::ERPCMISMATCH => ERPCMISMATCH,
            libc::EPROGUNAVAIL => EPROGUNAVAIL,
            libc::EPROGMISMATCH => EPROGMISMATCH,
            libc::EPROCUNAVAIL => EPROCUNAVAIL,
            libc::ENOLCK => ENOLCK,
            libc::ENOSYS => ENOSYS,
            libc::EFTYPE => EFTYPE,
            libc::EAUTH => EAUTH,
            libc::ENEEDAUTH => ENEEDAUTH,
            libc::EPWROFF => EPWROFF,
            libc::EDEVERR => EDEVERR,
            libc::EOVERFLOW => EOVERFLOW,
            libc::EBADEXEC => EBADEXEC,
            libc::EBADARCH => EBADARCH,
            libc::ESHLIBVERS => ESHLIBVERS,
            libc::EBADMACHO => EBADMACHO,
            libc::ECANCELED => ECANCELED,
            libc::EIDRM => EIDRM,
            libc::ENOMSG => ENOMSG,
            libc::EILSEQ => EILSEQ,
            libc::ENOATTR => ENOATTR,
            libc::EBADMSG => EBADMSG,
            libc::EMULTIHOP => EMULTIHOP,
            libc::ENODATA => ENODATA,
            libc::ENOLINK => ENOLINK,
            libc::ENOSR => ENOSR,
            libc::ENOSTR => ENOSTR,
            libc::EPROTO => EPROTO,
            libc::ETIME => ETIME,
            libc::EOPNOTSUPP => EOPNOTSUPP,
            libc::ENOPOLICY => ENOPOLICY,
            libc::ENOTRECOVERABLE => ENOTRECOVERABLE,
            libc::EOWNERDEAD => EOWNERDEAD,
            libc::EQFULL => EQFULL,
            _ => UnknownErrno,
        }
    }
}

#[cfg(target_os = "freebsd")]
mod consts {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(i32)]
    #[non_exhaustive]
    pub enum Errno {
        UnknownErrno = 0,
        EPERM = libc::EPERM,
        ENOENT = libc::ENOENT,
        ESRCH = libc::ESRCH,
        EINTR = libc::EINTR,
        EIO = libc::EIO,
        ENXIO = libc::ENXIO,
        E2BIG = libc::E2BIG,
        ENOEXEC = libc::ENOEXEC,
        EBADF = libc::EBADF,
        ECHILD = libc::ECHILD,
        EDEADLK = libc::EDEADLK,
        ENOMEM = libc::ENOMEM,
        EACCES = libc::EACCES,
        EFAULT = libc::EFAULT,
        ENOTBLK = libc::ENOTBLK,
        EBUSY = libc::EBUSY,
        EEXIST = libc::EEXIST,
        EXDEV = libc::EXDEV,
        ENODEV = libc::ENODEV,
        ENOTDIR = libc::ENOTDIR,
        EISDIR = libc::EISDIR,
        EINVAL = libc::EINVAL,
        ENFILE = libc::ENFILE,
        EMFILE = libc::EMFILE,
        ENOTTY = libc::ENOTTY,
        ETXTBSY = libc::ETXTBSY,
        EFBIG = libc::EFBIG,
        ENOSPC = libc::ENOSPC,
        ESPIPE = libc::ESPIPE,
        EROFS = libc::EROFS,
        EMLINK = libc::EMLINK,
        EPIPE = libc::EPIPE,
        EDOM = libc::EDOM,
        ERANGE = libc::ERANGE,
        EAGAIN = libc::EAGAIN,
        EINPROGRESS = libc::EINPROGRESS,
        EALREADY = libc::EALREADY,
        ENOTSOCK = libc::ENOTSOCK,
        EDESTADDRREQ = libc::EDESTADDRREQ,
        EMSGSIZE = libc::EMSGSIZE,
        EPROTOTYPE = libc::EPROTOTYPE,
        ENOPROTOOPT = libc::ENOPROTOOPT,
        EPROTONOSUPPORT = libc::EPROTONOSUPPORT,
        ESOCKTNOSUPPORT = libc::ESOCKTNOSUPPORT,
        ENOTSUP = libc::ENOTSUP,
        EPFNOSUPPORT = libc::EPFNOSUPPORT,
        EAFNOSUPPORT = libc::EAFNOSUPPORT,
        EADDRINUSE = libc::EADDRINUSE,
        EADDRNOTAVAIL = libc::EADDRNOTAVAIL,
        ENETDOWN = libc::ENETDOWN,
        ENETUNREACH = libc::ENETUNREACH,
        ENETRESET = libc::ENETRESET,
        ECONNABORTED = libc::ECONNABORTED,
        ECONNRESET = libc::ECONNRESET,
        ENOBUFS = libc::ENOBUFS,
        EISCONN = libc::EISCONN,
        ENOTCONN = libc::ENOTCONN,
        ESHUTDOWN = libc::ESHUTDOWN,
        ETOOMANYREFS = libc::ETOOMANYREFS,
        ETIMEDOUT = libc::ETIMEDOUT,
        ECONNREFUSED = libc::ECONNREFUSED,
        ELOOP = libc::ELOOP,
        ENAMETOOLONG = libc::ENAMETOOLONG,
        EHOSTDOWN = libc::EHOSTDOWN,
        EHOSTUNREACH = libc::EHOSTUNREACH,
        ENOTEMPTY = libc::ENOTEMPTY,
        EPROCLIM = libc::EPROCLIM,
        EUSERS = libc::EUSERS,
        EDQUOT = libc::EDQUOT,
        ESTALE = libc::ESTALE,
        EREMOTE = libc::EREMOTE,
        EBADRPC = libc::EBADRPC,
        ERPCMISMATCH = libc::ERPCMISMATCH,
        EPROGUNAVAIL = libc::EPROGUNAVAIL,
        EPROGMISMATCH = libc::EPROGMISMATCH,
        EPROCUNAVAIL = libc::EPROCUNAVAIL,
        ENOLCK = libc::ENOLCK,
        ENOSYS = libc::ENOSYS,
        EFTYPE = libc::EFTYPE,
        EAUTH = libc::EAUTH,
        ENEEDAUTH = libc::ENEEDAUTH,
        EIDRM = libc::EIDRM,
        ENOMSG = libc::ENOMSG,
        EOVERFLOW = libc::EOVERFLOW,
        ECANCELED = libc::ECANCELED,
        EILSEQ = libc::EILSEQ,
        ENOATTR = libc::ENOATTR,
        EDOOFUS = libc::EDOOFUS,
        EBADMSG = libc::EBADMSG,
        EMULTIHOP = libc::EMULTIHOP,
        ENOLINK = libc::ENOLINK,
        EPROTO = libc::EPROTO,
        ENOTCAPABLE = libc::ENOTCAPABLE,
        ECAPMODE = libc::ECAPMODE,
        ENOTRECOVERABLE = libc::ENOTRECOVERABLE,
        EOWNERDEAD = libc::EOWNERDEAD,
    }

    impl Errno {
        pub const ELAST: Errno = Errno::EOWNERDEAD;
        pub const EWOULDBLOCK: Errno = Errno::EAGAIN;
        pub const EDEADLOCK: Errno = Errno::EDEADLK;
        pub const EOPNOTSUPP: Errno = Errno::ENOTSUP;
    }

    #[deprecated(
        since = "0.28.0",
        note = "please use `Errno::from_raw()` instead"
    )]
    pub const fn from_i32(e: i32) -> Errno {
        use self::Errno::*;

        match e {
            libc::EPERM => EPERM,
            libc::ENOENT => ENOENT,
            libc::ESRCH => ESRCH,
            libc::EINTR => EINTR,
            libc::EIO => EIO,
            libc::ENXIO => ENXIO,
            libc::E2BIG => E2BIG,
            libc::ENOEXEC => ENOEXEC,
            libc::EBADF => EBADF,
            libc::ECHILD => ECHILD,
            libc::EDEADLK => EDEADLK,
            libc::ENOMEM => ENOMEM,
            libc::EACCES => EACCES,
            libc::EFAULT => EFAULT,
            libc::ENOTBLK => ENOTBLK,
            libc::EBUSY => EBUSY,
            libc::EEXIST => EEXIST,
            libc::EXDEV => EXDEV,
            libc::ENODEV => ENODEV,
            libc::ENOTDIR => ENOTDIR,
            libc::EISDIR => EISDIR,
            libc::EINVAL => EINVAL,
            libc::ENFILE => ENFILE,
            libc::EMFILE => EMFILE,
            libc::ENOTTY => ENOTTY,
            libc::ETXTBSY => ETXTBSY,
            libc::EFBIG => EFBIG,
            libc::ENOSPC => ENOSPC,
            libc::ESPIPE => ESPIPE,
            libc::EROFS => EROFS,
            libc::EMLINK => EMLINK,
            libc::EPIPE => EPIPE,
            libc::EDOM => EDOM,
            libc::ERANGE => ERANGE,
            libc::EAGAIN => EAGAIN,
            libc::EINPROGRESS => EINPROGRESS,
            libc::EALREADY => EALREADY,
            libc::ENOTSOCK => ENOTSOCK,
            libc::EDESTADDRREQ => EDESTADDRREQ,
            libc::EMSGSIZE => EMSGSIZE,
            libc::EPROTOTYPE => EPROTOTYPE,
            libc::ENOPROTOOPT => ENOPROTOOPT,
            libc::EPROTONOSUPPORT => EPROTONOSUPPORT,
            libc::ESOCKTNOSUPPORT => ESOCKTNOSUPPORT,
            libc::ENOTSUP => ENOTSUP,
            libc::EPFNOSUPPORT => EPFNOSUPPORT,
            libc::EAFNOSUPPORT => EAFNOSUPPORT,
            libc::EADDRINUSE => EADDRINUSE,
            libc::EADDRNOTAVAIL => EADDRNOTAVAIL,
            libc::ENETDOWN => ENETDOWN,
            libc::ENETUNREACH => ENETUNREACH,
            libc::ENETRESET => ENETRESET,
            libc::ECONNABORTED => ECONNABORTED,
            libc::ECONNRESET => ECONNRESET,
            libc::ENOBUFS => ENOBUFS,
            libc::EISCONN => EISCONN,
            libc::ENOTCONN => ENOTCONN,
            libc::ESHUTDOWN => ESHUTDOWN,
            libc::ETOOMANYREFS => ETOOMANYREFS,
            libc::ETIMEDOUT => ETIMEDOUT,
            libc::ECONNREFUSED => ECONNREFUSED,
            libc::ELOOP => ELOOP,
            libc::ENAMETOOLONG => ENAMETOOLONG,
            libc::EHOSTDOWN => EHOSTDOWN,
            libc::EHOSTUNREACH => EHOSTUNREACH,
            libc::ENOTEMPTY => ENOTEMPTY,
            libc::EPROCLIM => EPROCLIM,
            libc::EUSERS => EUSERS,
            libc::EDQUOT => EDQUOT,
            libc::ESTALE => ESTALE,
            libc::EREMOTE => EREMOTE,
            libc::EBADRPC => EBADRPC,
            libc::ERPCMISMATCH => ERPCMISMATCH,
            libc::EPROGUNAVAIL => EPROGUNAVAIL,
            libc::EPROGMISMATCH => EPROGMISMATCH,
            libc::EPROCUNAVAIL => EPROCUNAVAIL,
            libc::ENOLCK => ENOLCK,
            libc::ENOSYS => ENOSYS,
            libc::EFTYPE => EFTYPE,
            libc::EAUTH => EAUTH,
            libc::ENEEDAUTH => ENEEDAUTH,
            libc::EIDRM => EIDRM,
            libc::ENOMSG => ENOMSG,
            libc::EOVERFLOW => EOVERFLOW,
            libc::ECANCELED => ECANCELED,
            libc::EILSEQ => EILSEQ,
            libc::ENOATTR => ENOATTR,
            libc::EDOOFUS => EDOOFUS,
            libc::EBADMSG => EBADMSG,
            libc::EMULTIHOP => EMULTIHOP,
            libc::ENOLINK => ENOLINK,
            libc::EPROTO => EPROTO,
            libc::ENOTCAPABLE => ENOTCAPABLE,
            libc::ECAPMODE => ECAPMODE,
            libc::ENOTRECOVERABLE => ENOTRECOVERABLE,
            libc::EOWNERDEAD => EOWNERDEAD,
            _ => UnknownErrno,
        }
    }
}

#[cfg(target_os = "dragonfly")]
mod consts {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(i32)]
    #[non_exhaustive]
    pub enum Errno {
        UnknownErrno = 0,
        EPERM = libc::EPERM,
        ENOENT = libc::ENOENT,
        ESRCH = libc::ESRCH,
        EINTR = libc::EINTR,
        EIO = libc::EIO,
        ENXIO = libc::ENXIO,
        E2BIG = libc::E2BIG,
        ENOEXEC = libc::ENOEXEC,
        EBADF = libc::EBADF,
        ECHILD = libc::ECHILD,
        EDEADLK = libc::EDEADLK,
        ENOMEM = libc::ENOMEM,
        EACCES = libc::EACCES,
        EFAULT = libc::EFAULT,
        ENOTBLK = libc::ENOTBLK,
        EBUSY = libc::EBUSY,
        EEXIST = libc::EEXIST,
        EXDEV = libc::EXDEV,
        ENODEV = libc::ENODEV,
        ENOTDIR = libc::ENOTDIR,
        EISDIR = libc::EISDIR,
        EINVAL = libc::EINVAL,
        ENFILE = libc::ENFILE,
        EMFILE = libc::EMFILE,
        ENOTTY = libc::ENOTTY,
        ETXTBSY = libc::ETXTBSY,
        EFBIG = libc::EFBIG,
        ENOSPC = libc::ENOSPC,
        ESPIPE = libc::ESPIPE,
        EROFS = libc::EROFS,
        EMLINK = libc::EMLINK,
        EPIPE = libc::EPIPE,
        EDOM = libc::EDOM,
        ERANGE = libc::ERANGE,
        EAGAIN = libc::EAGAIN,
        EINPROGRESS = libc::EINPROGRESS,
        EALREADY = libc::EALREADY,
        ENOTSOCK = libc::ENOTSOCK,
        EDESTADDRREQ = libc::EDESTADDRREQ,
        EMSGSIZE = libc::EMSGSIZE,
        EPROTOTYPE = libc::EPROTOTYPE,
        ENOPROTOOPT = libc::ENOPROTOOPT,
        EPROTONOSUPPORT = libc::EPROTONOSUPPORT,
        ESOCKTNOSUPPORT = libc::ESOCKTNOSUPPORT,
        ENOTSUP = libc::ENOTSUP,
        EPFNOSUPPORT = libc::EPFNOSUPPORT,
        EAFNOSUPPORT = libc::EAFNOSUPPORT,
        EADDRINUSE = libc::EADDRINUSE,
        EADDRNOTAVAIL = libc::EADDRNOTAVAIL,
        ENETDOWN = libc::ENETDOWN,
        ENETUNREACH = libc::ENETUNREACH,
        ENETRESET = libc::ENETRESET,
        ECONNABORTED = libc::ECONNABORTED,
        ECONNRESET = libc::ECONNRESET,
        ENOBUFS = libc::ENOBUFS,
        EISCONN = libc::EISCONN,
        ENOTCONN = libc::ENOTCONN,
        ESHUTDOWN = libc::ESHUTDOWN,
        ETOOMANYREFS = libc::ETOOMANYREFS,
        ETIMEDOUT = libc::ETIMEDOUT,
        ECONNREFUSED = libc::ECONNREFUSED,
        ELOOP = libc::ELOOP,
        ENAMETOOLONG = libc::ENAMETOOLONG,
        EHOSTDOWN = libc::EHOSTDOWN,
        EHOSTUNREACH = libc::EHOSTUNREACH,
        ENOTEMPTY = libc::ENOTEMPTY,
        EPROCLIM = libc::EPROCLIM,
        EUSERS = libc::EUSERS,
        EDQUOT = libc::EDQUOT,
        ESTALE = libc::ESTALE,
        EREMOTE = libc::EREMOTE,
        EBADRPC = libc::EBADRPC,
        ERPCMISMATCH = libc::ERPCMISMATCH,
        EPROGUNAVAIL = libc::EPROGUNAVAIL,
        EPROGMISMATCH = libc::EPROGMISMATCH,
        EPROCUNAVAIL = libc::EPROCUNAVAIL,
        ENOLCK = libc::ENOLCK,
        ENOSYS = libc::ENOSYS,
        EFTYPE = libc::EFTYPE,
        EAUTH = libc::EAUTH,
        ENEEDAUTH = libc::ENEEDAUTH,
        EIDRM = libc::EIDRM,
        ENOMSG = libc::ENOMSG,
        EOVERFLOW = libc::EOVERFLOW,
        ECANCELED = libc::ECANCELED,
        EILSEQ = libc::EILSEQ,
        ENOATTR = libc::ENOATTR,
        EDOOFUS = libc::EDOOFUS,
        EBADMSG = libc::EBADMSG,
        EMULTIHOP = libc::EMULTIHOP,
        ENOLINK = libc::ENOLINK,
        EPROTO = libc::EPROTO,
        ENOMEDIUM = libc::ENOMEDIUM,
        ENOTRECOVERABLE = libc::ENOTRECOVERABLE,
        EOWNERDEAD = libc::EOWNERDEAD,
        EASYNC = libc::EASYNC,
    }

    impl Errno {
        pub const ELAST: Errno = Errno::EASYNC;
        pub const EWOULDBLOCK: Errno = Errno::EAGAIN;
        pub const EDEADLOCK: Errno = Errno::EDEADLK;
        pub const EOPNOTSUPP: Errno = Errno::ENOTSUP;
    }

    #[deprecated(
        since = "0.28.0",
        note = "please use `Errno::from_raw()` instead"
    )]
    pub const fn from_i32(e: i32) -> Errno {
        use self::Errno::*;

        match e {
            libc::EPERM => EPERM,
            libc::ENOENT => ENOENT,
            libc::ESRCH => ESRCH,
            libc::EINTR => EINTR,
            libc::EIO => EIO,
            libc::ENXIO => ENXIO,
            libc::E2BIG => E2BIG,
            libc::ENOEXEC => ENOEXEC,
            libc::EBADF => EBADF,
            libc::ECHILD => ECHILD,
            libc::EDEADLK => EDEADLK,
            libc::ENOMEM => ENOMEM,
            libc::EACCES => EACCES,
            libc::EFAULT => EFAULT,
            libc::ENOTBLK => ENOTBLK,
            libc::EBUSY => EBUSY,
            libc::EEXIST => EEXIST,
            libc::EXDEV => EXDEV,
            libc::ENODEV => ENODEV,
            libc::ENOTDIR => ENOTDIR,
            libc::EISDIR => EISDIR,
            libc::EINVAL => EINVAL,
            libc::ENFILE => ENFILE,
            libc::EMFILE => EMFILE,
            libc::ENOTTY => ENOTTY,
            libc::ETXTBSY => ETXTBSY,
            libc::EFBIG => EFBIG,
            libc::ENOSPC => ENOSPC,
            libc::ESPIPE => ESPIPE,
            libc::EROFS => EROFS,
            libc::EMLINK => EMLINK,
            libc::EPIPE => EPIPE,
            libc::EDOM => EDOM,
            libc::ERANGE => ERANGE,
            libc::EAGAIN => EAGAIN,
            libc::EINPROGRESS => EINPROGRESS,
            libc::EALREADY => EALREADY,
            libc::ENOTSOCK => ENOTSOCK,
            libc::EDESTADDRREQ => EDESTADDRREQ,
            libc::EMSGSIZE => EMSGSIZE,
            libc::EPROTOTYPE => EPROTOTYPE,
            libc::ENOPROTOOPT => ENOPROTOOPT,
            libc::EPROTONOSUPPORT => EPROTONOSUPPORT,
            libc::ESOCKTNOSUPPORT => ESOCKTNOSUPPORT,
            libc::ENOTSUP => ENOTSUP,
            libc::EPFNOSUPPORT => EPFNOSUPPORT,
            libc::EAFNOSUPPORT => EAFNOSUPPORT,
            libc::EADDRINUSE => EADDRINUSE,
            libc::EADDRNOTAVAIL => EADDRNOTAVAIL,
            libc::ENETDOWN => ENETDOWN,
            libc::ENETUNREACH => ENETUNREACH,
            libc::ENETRESET => ENETRESET,
            libc::ECONNABORTED => ECONNABORTED,
            libc::ECONNRESET => ECONNRESET,
            libc::ENOBUFS => ENOBUFS,
            libc::EISCONN => EISCONN,
            libc::ENOTCONN => ENOTCONN,
            libc::ESHUTDOWN => ESHUTDOWN,
            libc::ETOOMANYREFS => ETOOMANYREFS,
            libc::ETIMEDOUT => ETIMEDOUT,
            libc::ECONNREFUSED => ECONNREFUSED,
            libc::ELOOP => ELOOP,
            libc::ENAMETOOLONG => ENAMETOOLONG,
            libc::EHOSTDOWN => EHOSTDOWN,
            libc::EHOSTUNREACH => EHOSTUNREACH,
            libc::ENOTEMPTY => ENOTEMPTY,
            libc::EPROCLIM => EPROCLIM,
            libc::EUSERS => EUSERS,
            libc::EDQUOT => EDQUOT,
            libc::ESTALE => ESTALE,
            libc::EREMOTE => EREMOTE,
            libc::EBADRPC => EBADRPC,
            libc::ERPCMISMATCH => ERPCMISMATCH,
            libc::EPROGUNAVAIL => EPROGUNAVAIL,
            libc::EPROGMISMATCH => EPROGMISMATCH,
            libc::EPROCUNAVAIL => EPROCUNAVAIL,
            libc::ENOLCK => ENOLCK,
            libc::ENOSYS => ENOSYS,
            libc::EFTYPE => EFTYPE,
            libc::EAUTH => EAUTH,
            libc::ENEEDAUTH => ENEEDAUTH,
            libc::EIDRM => EIDRM,
            libc::ENOMSG => ENOMSG,
            libc::EOVERFLOW => EOVERFLOW,
            libc::ECANCELED => ECANCELED,
            libc::EILSEQ => EILSEQ,
            libc::ENOATTR => ENOATTR,
            libc::EDOOFUS => EDOOFUS,
            libc::EBADMSG => EBADMSG,
            libc::EMULTIHOP => EMULTIHOP,
            libc::ENOLINK => ENOLINK,
            libc::EPROTO => EPROTO,
            libc::ENOMEDIUM => ENOMEDIUM,
            libc::EASYNC => EASYNC,
            _ => UnknownErrno,
        }
    }
}

#[cfg(target_os = "openbsd")]
mod consts {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(i32)]
    #[non_exhaustive]
    pub enum Errno {
        UnknownErrno = 0,
        EPERM = libc::EPERM,
        ENOENT = libc::ENOENT,
        ESRCH = libc::ESRCH,
        EINTR = libc::EINTR,
        EIO = libc::EIO,
        ENXIO = libc::ENXIO,
        E2BIG = libc::E2BIG,
        ENOEXEC = libc::ENOEXEC,
        EBADF = libc::EBADF,
        ECHILD = libc::ECHILD,
        EDEADLK = libc::EDEADLK,
        ENOMEM = libc::ENOMEM,
        EACCES = libc::EACCES,
        EFAULT = libc::EFAULT,
        ENOTBLK = libc::ENOTBLK,
        EBUSY = libc::EBUSY,
        EEXIST = libc::EEXIST,
        EXDEV = libc::EXDEV,
        ENODEV = libc::ENODEV,
        ENOTDIR = libc::ENOTDIR,
        EISDIR = libc::EISDIR,
        EINVAL = libc::EINVAL,
        ENFILE = libc::ENFILE,
        EMFILE = libc::EMFILE,
        ENOTTY = libc::ENOTTY,
        ETXTBSY = libc::ETXTBSY,
        EFBIG = libc::EFBIG,
        ENOSPC = libc::ENOSPC,
        ESPIPE = libc::ESPIPE,
        EROFS = libc::EROFS,
        EMLINK = libc::EMLINK,
        EPIPE = libc::EPIPE,
        EDOM = libc::EDOM,
        ERANGE = libc::ERANGE,
        EAGAIN = libc::EAGAIN,
        EINPROGRESS = libc::EINPROGRESS,
        EALREADY = libc::EALREADY,
        ENOTSOCK = libc::ENOTSOCK,
        EDESTADDRREQ = libc::EDESTADDRREQ,
        EMSGSIZE = libc::EMSGSIZE,
        EPROTOTYPE = libc::EPROTOTYPE,
        ENOPROTOOPT = libc::ENOPROTOOPT,
        EPROTONOSUPPORT = libc::EPROTONOSUPPORT,
        ESOCKTNOSUPPORT = libc::ESOCKTNOSUPPORT,
        EOPNOTSUPP = libc::EOPNOTSUPP,
        EPFNOSUPPORT = libc::EPFNOSUPPORT,
        EAFNOSUPPORT = libc::EAFNOSUPPORT,
        EADDRINUSE = libc::EADDRINUSE,
        EADDRNOTAVAIL = libc::EADDRNOTAVAIL,
        ENETDOWN = libc::ENETDOWN,
        ENETUNREACH = libc::ENETUNREACH,
        ENETRESET = libc::ENETRESET,
        ECONNABORTED = libc::ECONNABORTED,
        ECONNRESET = libc::ECONNRESET,
        ENOBUFS = libc::ENOBUFS,
        EISCONN = libc::EISCONN,
        ENOTCONN = libc::ENOTCONN,
        ESHUTDOWN = libc::ESHUTDOWN,
        ETOOMANYREFS = libc::ETOOMANYREFS,
        ETIMEDOUT = libc::ETIMEDOUT,
        ECONNREFUSED = libc::ECONNREFUSED,
        ELOOP = libc::ELOOP,
        ENAMETOOLONG = libc::ENAMETOOLONG,
        EHOSTDOWN = libc::EHOSTDOWN,
        EHOSTUNREACH = libc::EHOSTUNREACH,
        ENOTEMPTY = libc::ENOTEMPTY,
        EPROCLIM = libc::EPROCLIM,
        EUSERS = libc::EUSERS,
        EDQUOT = libc::EDQUOT,
        ESTALE = libc::ESTALE,
        EREMOTE = libc::EREMOTE,
        EBADRPC = libc::EBADRPC,
        ERPCMISMATCH = libc::ERPCMISMATCH,
        EPROGUNAVAIL = libc::EPROGUNAVAIL,
        EPROGMISMATCH = libc::EPROGMISMATCH,
        EPROCUNAVAIL = libc::EPROCUNAVAIL,
        ENOLCK = libc::ENOLCK,
        ENOSYS = libc::ENOSYS,
        EFTYPE = libc::EFTYPE,
        EAUTH = libc::EAUTH,
        ENEEDAUTH = libc::ENEEDAUTH,
        EIPSEC = libc::EIPSEC,
        ENOATTR = libc::ENOATTR,
        EILSEQ = libc::EILSEQ,
        ENOMEDIUM = libc::ENOMEDIUM,
        EMEDIUMTYPE = libc::EMEDIUMTYPE,
        EOVERFLOW = libc::EOVERFLOW,
        ECANCELED = libc::ECANCELED,
        EIDRM = libc::EIDRM,
        ENOMSG = libc::ENOMSG,
        ENOTSUP = libc::ENOTSUP,
        EBADMSG = libc::EBADMSG,
        ENOTRECOVERABLE = libc::ENOTRECOVERABLE,
        EOWNERDEAD = libc::EOWNERDEAD,
        EPROTO = libc::EPROTO,
    }

    impl Errno {
        pub const ELAST: Errno = Errno::ENOTSUP;
        pub const EWOULDBLOCK: Errno = Errno::EAGAIN;
    }

    #[deprecated(
        since = "0.28.0",
        note = "please use `Errno::from_raw()` instead"
    )]
    pub const fn from_i32(e: i32) -> Errno {
        use self::Errno::*;

        match e {
            libc::EPERM => EPERM,
            libc::ENOENT => ENOENT,
            libc::ESRCH => ESRCH,
            libc::EINTR => EINTR,
            libc::EIO => EIO,
            libc::ENXIO => ENXIO,
            libc::E2BIG => E2BIG,
            libc::ENOEXEC => ENOEXEC,
            libc::EBADF => EBADF,
            libc::ECHILD => ECHILD,
            libc::EDEADLK => EDEADLK,
            libc::ENOMEM => ENOMEM,
            libc::EACCES => EACCES,
            libc::EFAULT => EFAULT,
            libc::ENOTBLK => ENOTBLK,
            libc::EBUSY => EBUSY,
            libc::EEXIST => EEXIST,
            libc::EXDEV => EXDEV,
            libc::ENODEV => ENODEV,
            libc::ENOTDIR => ENOTDIR,
            libc::EISDIR => EISDIR,
            libc::EINVAL => EINVAL,
            libc::ENFILE => ENFILE,
            libc::EMFILE => EMFILE,
            libc::ENOTTY => ENOTTY,
            libc::ETXTBSY => ETXTBSY,
            libc::EFBIG => EFBIG,
            libc::ENOSPC => ENOSPC,
            libc::ESPIPE => ESPIPE,
            libc::EROFS => EROFS,
            libc::EMLINK => EMLINK,
            libc::EPIPE => EPIPE,
            libc::EDOM => EDOM,
            libc::ERANGE => ERANGE,
            libc::EAGAIN => EAGAIN,
            libc::EINPROGRESS => EINPROGRESS,
            libc::EALREADY => EALREADY,
            libc::ENOTSOCK => ENOTSOCK,
            libc::EDESTADDRREQ => EDESTADDRREQ,
            libc::EMSGSIZE => EMSGSIZE,
            libc::EPROTOTYPE => EPROTOTYPE,
            libc::ENOPROTOOPT => ENOPROTOOPT,
            libc::EPROTONOSUPPORT => EPROTONOSUPPORT,
            libc::ESOCKTNOSUPPORT => ESOCKTNOSUPPORT,
            libc::EOPNOTSUPP => EOPNOTSUPP,
            libc::EPFNOSUPPORT => EPFNOSUPPORT,
            libc::EAFNOSUPPORT => EAFNOSUPPORT,
            libc::EADDRINUSE => EADDRINUSE,
            libc::EADDRNOTAVAIL => EADDRNOTAVAIL,
            libc::ENETDOWN => ENETDOWN,
            libc::ENETUNREACH => ENETUNREACH,
            libc::ENETRESET => ENETRESET,
            libc::ECONNABORTED => ECONNABORTED,
            libc::ECONNRESET => ECONNRESET,
            libc::ENOBUFS => ENOBUFS,
            libc::EISCONN => EISCONN,
            libc::ENOTCONN => ENOTCONN,
            libc::ESHUTDOWN => ESHUTDOWN,
            libc::ETOOMANYREFS => ETOOMANYREFS,
            libc::ETIMEDOUT => ETIMEDOUT,
            libc::ECONNREFUSED => ECONNREFUSED,
            libc::ELOOP => ELOOP,
            libc::ENAMETOOLONG => ENAMETOOLONG,
            libc::EHOSTDOWN => EHOSTDOWN,
            libc::EHOSTUNREACH => EHOSTUNREACH,
            libc::ENOTEMPTY => ENOTEMPTY,
            libc::EPROCLIM => EPROCLIM,
            libc::EUSERS => EUSERS,
            libc::EDQUOT => EDQUOT,
            libc::ESTALE => ESTALE,
            libc::EREMOTE => EREMOTE,
            libc::EBADRPC => EBADRPC,
            libc::ERPCMISMATCH => ERPCMISMATCH,
            libc::EPROGUNAVAIL => EPROGUNAVAIL,
            libc::EPROGMISMATCH => EPROGMISMATCH,
            libc::EPROCUNAVAIL => EPROCUNAVAIL,
            libc::ENOLCK => ENOLCK,
            libc::ENOSYS => ENOSYS,
            libc::EFTYPE => EFTYPE,
            libc::EAUTH => EAUTH,
            libc::ENEEDAUTH => ENEEDAUTH,
            libc::EIPSEC => EIPSEC,
            libc::ENOATTR => ENOATTR,
            libc::EILSEQ => EILSEQ,
            libc::ENOMEDIUM => ENOMEDIUM,
            libc::EMEDIUMTYPE => EMEDIUMTYPE,
            libc::EOVERFLOW => EOVERFLOW,
            libc::ECANCELED => ECANCELED,
            libc::EIDRM => EIDRM,
            libc::ENOMSG => ENOMSG,
            libc::ENOTSUP => ENOTSUP,
            libc::EBADMSG => EBADMSG,
            libc::ENOTRECOVERABLE => ENOTRECOVERABLE,
            libc::EOWNERDEAD => EOWNERDEAD,
            libc::EPROTO => EPROTO,
            _ => UnknownErrno,
        }
    }
}

#[cfg(target_os = "netbsd")]
mod consts {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(i32)]
    #[non_exhaustive]
    pub enum Errno {
        UnknownErrno = 0,
        EPERM = libc::EPERM,
        ENOENT = libc::ENOENT,
        ESRCH = libc::ESRCH,
        EINTR = libc::EINTR,
        EIO = libc::EIO,
        ENXIO = libc::ENXIO,
        E2BIG = libc::E2BIG,
        ENOEXEC = libc::ENOEXEC,
        EBADF = libc::EBADF,
        ECHILD = libc::ECHILD,
        EDEADLK = libc::EDEADLK,
        ENOMEM = libc::ENOMEM,
        EACCES = libc::EACCES,
        EFAULT = libc::EFAULT,
        ENOTBLK = libc::ENOTBLK,
        EBUSY = libc::EBUSY,
        EEXIST = libc::EEXIST,
        EXDEV = libc::EXDEV,
        ENODEV = libc::ENODEV,
        ENOTDIR = libc::ENOTDIR,
        EISDIR = libc::EISDIR,
        EINVAL = libc::EINVAL,
        ENFILE = libc::ENFILE,
        EMFILE = libc::EMFILE,
        ENOTTY = libc::ENOTTY,
        ETXTBSY = libc::ETXTBSY,
        EFBIG = libc::EFBIG,
        ENOSPC = libc::ENOSPC,
        ESPIPE = libc::ESPIPE,
        EROFS = libc::EROFS,
        EMLINK = libc::EMLINK,
        EPIPE = libc::EPIPE,
        EDOM = libc::EDOM,
        ERANGE = libc::ERANGE,
        EAGAIN = libc::EAGAIN,
        EINPROGRESS = libc::EINPROGRESS,
        EALREADY = libc::EALREADY,
        ENOTSOCK = libc::ENOTSOCK,
        EDESTADDRREQ = libc::EDESTADDRREQ,
        EMSGSIZE = libc::EMSGSIZE,
        EPROTOTYPE = libc::EPROTOTYPE,
        ENOPROTOOPT = libc::ENOPROTOOPT,
        EPROTONOSUPPORT = libc::EPROTONOSUPPORT,
        ESOCKTNOSUPPORT = libc::ESOCKTNOSUPPORT,
        EOPNOTSUPP = libc::EOPNOTSUPP,
        EPFNOSUPPORT = libc::EPFNOSUPPORT,
        EAFNOSUPPORT = libc::EAFNOSUPPORT,
        EADDRINUSE = libc::EADDRINUSE,
        EADDRNOTAVAIL = libc::EADDRNOTAVAIL,
        ENETDOWN = libc::ENETDOWN,
        ENETUNREACH = libc::ENETUNREACH,
        ENETRESET = libc::ENETRESET,
        ECONNABORTED = libc::ECONNABORTED,
        ECONNRESET = libc::ECONNRESET,
        ENOBUFS = libc::ENOBUFS,
        EISCONN = libc::EISCONN,
        ENOTCONN = libc::ENOTCONN,
        ESHUTDOWN = libc::ESHUTDOWN,
        ETOOMANYREFS = libc::ETOOMANYREFS,
        ETIMEDOUT = libc::ETIMEDOUT,
        ECONNREFUSED = libc::ECONNREFUSED,
        ELOOP = libc::ELOOP,
        ENAMETOOLONG = libc::ENAMETOOLONG,
        EHOSTDOWN = libc::EHOSTDOWN,
        EHOSTUNREACH = libc::EHOSTUNREACH,
        ENOTEMPTY = libc::ENOTEMPTY,
        EPROCLIM = libc::EPROCLIM,
        EUSERS = libc::EUSERS,
        EDQUOT = libc::EDQUOT,
        ESTALE = libc::ESTALE,
        EREMOTE = libc::EREMOTE,
        EBADRPC = libc::EBADRPC,
        ERPCMISMATCH = libc::ERPCMISMATCH,
        EPROGUNAVAIL = libc::EPROGUNAVAIL,
        EPROGMISMATCH = libc::EPROGMISMATCH,
        EPROCUNAVAIL = libc::EPROCUNAVAIL,
        ENOLCK = libc::ENOLCK,
        ENOSYS = libc::ENOSYS,
        EFTYPE = libc::EFTYPE,
        EAUTH = libc::EAUTH,
        ENEEDAUTH = libc::ENEEDAUTH,
        EIDRM = libc::EIDRM,
        ENOMSG = libc::ENOMSG,
        EOVERFLOW = libc::EOVERFLOW,
        EILSEQ = libc::EILSEQ,
        ENOTSUP = libc::ENOTSUP,
        ECANCELED = libc::ECANCELED,
        EBADMSG = libc::EBADMSG,
        ENODATA = libc::ENODATA,
        ENOSR = libc::ENOSR,
        ENOSTR = libc::ENOSTR,
        ETIME = libc::ETIME,
        ENOATTR = libc::ENOATTR,
        EMULTIHOP = libc::EMULTIHOP,
        ENOLINK = libc::ENOLINK,
        EPROTO = libc::EPROTO,
    }

    impl Errno {
        pub const ELAST: Errno = Errno::ENOTSUP;
        pub const EWOULDBLOCK: Errno = Errno::EAGAIN;
    }

    #[deprecated(
        since = "0.28.0",
        note = "please use `Errno::from_raw()` instead"
    )]
    pub const fn from_i32(e: i32) -> Errno {
        use self::Errno::*;

        match e {
            libc::EPERM => EPERM,
            libc::ENOENT => ENOENT,
            libc::ESRCH => ESRCH,
            libc::EINTR => EINTR,
            libc::EIO => EIO,
            libc::ENXIO => ENXIO,
            libc::E2BIG => E2BIG,
            libc::ENOEXEC => ENOEXEC,
            libc::EBADF => EBADF,
            libc::ECHILD => ECHILD,
            libc::EDEADLK => EDEADLK,
            libc::ENOMEM => ENOMEM,
            libc::EACCES => EACCES,
            libc::EFAULT => EFAULT,
            libc::ENOTBLK => ENOTBLK,
            libc::EBUSY => EBUSY,
            libc::EEXIST => EEXIST,
            libc::EXDEV => EXDEV,
            libc::ENODEV => ENODEV,
            libc::ENOTDIR => ENOTDIR,
            libc::EISDIR => EISDIR,
            libc::EINVAL => EINVAL,
            libc::ENFILE => ENFILE,
            libc::EMFILE => EMFILE,
            libc::ENOTTY => ENOTTY,
            libc::ETXTBSY => ETXTBSY,
            libc::EFBIG => EFBIG,
            libc::ENOSPC => ENOSPC,
            libc::ESPIPE => ESPIPE,
            libc::EROFS => EROFS,
            libc::EMLINK => EMLINK,
            libc::EPIPE => EPIPE,
            libc::EDOM => EDOM,
            libc::ERANGE => ERANGE,
            libc::EAGAIN => EAGAIN,
            libc::EINPROGRESS => EINPROGRESS,
            libc::EALREADY => EALREADY,
            libc::ENOTSOCK => ENOTSOCK,
            libc::EDESTADDRREQ => EDESTADDRREQ,
            libc::EMSGSIZE => EMSGSIZE,
            libc::EPROTOTYPE => EPROTOTYPE,
            libc::ENOPROTOOPT => ENOPROTOOPT,
            libc::EPROTONOSUPPORT => EPROTONOSUPPORT,
            libc::ESOCKTNOSUPPORT => ESOCKTNOSUPPORT,
            libc::EOPNOTSUPP => EOPNOTSUPP,
            libc::EPFNOSUPPORT => EPFNOSUPPORT,
            libc::EAFNOSUPPORT => EAFNOSUPPORT,
            libc::EADDRINUSE => EADDRINUSE,
            libc::EADDRNOTAVAIL => EADDRNOTAVAIL,
            libc::ENETDOWN => ENETDOWN,
            libc::ENETUNREACH => ENETUNREACH,
            libc::ENETRESET => ENETRESET,
            libc::ECONNABORTED => ECONNABORTED,
            libc::ECONNRESET => ECONNRESET,
            libc::ENOBUFS => ENOBUFS,
            libc::EISCONN => EISCONN,
            libc::ENOTCONN => ENOTCONN,
            libc::ESHUTDOWN => ESHUTDOWN,
            libc::ETOOMANYREFS => ETOOMANYREFS,
            libc::ETIMEDOUT => ETIMEDOUT,
            libc::ECONNREFUSED => ECONNREFUSED,
            libc::ELOOP => ELOOP,
            libc::ENAMETOOLONG => ENAMETOOLONG,
            libc::EHOSTDOWN => EHOSTDOWN,
            libc::EHOSTUNREACH => EHOSTUNREACH,
            libc::ENOTEMPTY => ENOTEMPTY,
            libc::EPROCLIM => EPROCLIM,
            libc::EUSERS => EUSERS,
            libc::EDQUOT => EDQUOT,
            libc::ESTALE => ESTALE,
            libc::EREMOTE => EREMOTE,
            libc::EBADRPC => EBADRPC,
            libc::ERPCMISMATCH => ERPCMISMATCH,
            libc::EPROGUNAVAIL => EPROGUNAVAIL,
            libc::EPROGMISMATCH => EPROGMISMATCH,
            libc::EPROCUNAVAIL => EPROCUNAVAIL,
            libc::ENOLCK => ENOLCK,
            libc::ENOSYS => ENOSYS,
            libc::EFTYPE => EFTYPE,
            libc::EAUTH => EAUTH,
            libc::ENEEDAUTH => ENEEDAUTH,
            libc::EIDRM => EIDRM,
            libc::ENOMSG => ENOMSG,
            libc::EOVERFLOW => EOVERFLOW,
            libc::EILSEQ => EILSEQ,
            libc::ENOTSUP => ENOTSUP,
            libc::ECANCELED => ECANCELED,
            libc::EBADMSG => EBADMSG,
            libc::ENODATA => ENODATA,
            libc::ENOSR => ENOSR,
            libc::ENOSTR => ENOSTR,
            libc::ETIME => ETIME,
            libc::ENOATTR => ENOATTR,
            libc::EMULTIHOP => EMULTIHOP,
            libc::ENOLINK => ENOLINK,
            libc::EPROTO => EPROTO,
            _ => UnknownErrno,
        }
    }
}

#[cfg(target_os = "redox")]
mod consts {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(i32)]
    #[non_exhaustive]
    pub enum Errno {
        UnknownErrno = 0,
        EPERM = libc::EPERM,
        ENOENT = libc::ENOENT,
        ESRCH = libc::ESRCH,
        EINTR = libc::EINTR,
        EIO = libc::EIO,
        ENXIO = libc::ENXIO,
        E2BIG = libc::E2BIG,
        ENOEXEC = libc::ENOEXEC,
        EBADF = libc::EBADF,
        ECHILD = libc::ECHILD,
        EDEADLK = libc::EDEADLK,
        ENOMEM = libc::ENOMEM,
        EACCES = libc::EACCES,
        EFAULT = libc::EFAULT,
        ENOTBLK = libc::ENOTBLK,
        EBUSY = libc::EBUSY,
        EEXIST = libc::EEXIST,
        EXDEV = libc::EXDEV,
        ENODEV = libc::ENODEV,
        ENOTDIR = libc::ENOTDIR,
        EISDIR = libc::EISDIR,
        EINVAL = libc::EINVAL,
        ENFILE = libc::ENFILE,
        EMFILE = libc::EMFILE,
        ENOTTY = libc::ENOTTY,
        ETXTBSY = libc::ETXTBSY,
        EFBIG = libc::EFBIG,
        ENOSPC = libc::ENOSPC,
        ESPIPE = libc::ESPIPE,
        EROFS = libc::EROFS,
        EMLINK = libc::EMLINK,
        EPIPE = libc::EPIPE,
        EDOM = libc::EDOM,
        ERANGE = libc::ERANGE,
        EAGAIN = libc::EAGAIN,
        EINPROGRESS = libc::EINPROGRESS,
        EALREADY = libc::EALREADY,
        ENOTSOCK = libc::ENOTSOCK,
        EDESTADDRREQ = libc::EDESTADDRREQ,
        EMSGSIZE = libc::EMSGSIZE,
        EPROTOTYPE = libc::EPROTOTYPE,
        ENOPROTOOPT = libc::ENOPROTOOPT,
        EPROTONOSUPPORT = libc::EPROTONOSUPPORT,
        ESOCKTNOSUPPORT = libc::ESOCKTNOSUPPORT,
        EOPNOTSUPP = libc::EOPNOTSUPP,
        EPFNOSUPPORT = libc::EPFNOSUPPORT,
        EAFNOSUPPORT = libc::EAFNOSUPPORT,
        EADDRINUSE = libc::EADDRINUSE,
        EADDRNOTAVAIL = libc::EADDRNOTAVAIL,
        ENETDOWN = libc::ENETDOWN,
        ENETUNREACH = libc::ENETUNREACH,
        ENETRESET = libc::ENETRESET,
        ECONNABORTED = libc::ECONNABORTED,
        ECONNRESET = libc::ECONNRESET,
        ENOBUFS = libc::ENOBUFS,
        EISCONN = libc::EISCONN,
        ENOTCONN = libc::ENOTCONN,
        ESHUTDOWN = libc::ESHUTDOWN,
        ETOOMANYREFS = libc::ETOOMANYREFS,
        ETIMEDOUT = libc::ETIMEDOUT,
        ECONNREFUSED = libc::ECONNREFUSED,
        ELOOP = libc::ELOOP,
        ENAMETOOLONG = libc::ENAMETOOLONG,
        EHOSTDOWN = libc::EHOSTDOWN,
        EHOSTUNREACH = libc::EHOSTUNREACH,
        ENOTEMPTY = libc::ENOTEMPTY,
        EUSERS = libc::EUSERS,
        EDQUOT = libc::EDQUOT,
        ESTALE = libc::ESTALE,
        EREMOTE = libc::EREMOTE,
        ENOLCK = libc::ENOLCK,
        ENOSYS = libc::ENOSYS,
        EIDRM = libc::EIDRM,
        ENOMSG = libc::ENOMSG,
        EOVERFLOW = libc::EOVERFLOW,
        EILSEQ = libc::EILSEQ,
        ECANCELED = libc::ECANCELED,
        EBADMSG = libc::EBADMSG,
        ENODATA = libc::ENODATA,
        ENOSR = libc::ENOSR,
        ENOSTR = libc::ENOSTR,
        ETIME = libc::ETIME,
        EMULTIHOP = libc::EMULTIHOP,
        ENOLINK = libc::ENOLINK,
        EPROTO = libc::EPROTO,
    }

    impl Errno {
        pub const EWOULDBLOCK: Errno = Errno::EAGAIN;
    }

    #[deprecated(
        since = "0.28.0",
        note = "please use `Errno::from_raw()` instead"
    )]
    pub const fn from_i32(e: i32) -> Errno {
        use self::Errno::*;

        match e {
            libc::EPERM => EPERM,
            libc::ENOENT => ENOENT,
            libc::ESRCH => ESRCH,
            libc::EINTR => EINTR,
            libc::EIO => EIO,
            libc::ENXIO => ENXIO,
            libc::E2BIG => E2BIG,
            libc::ENOEXEC => ENOEXEC,
            libc::EBADF => EBADF,
            libc::ECHILD => ECHILD,
            libc::EDEADLK => EDEADLK,
            libc::ENOMEM => ENOMEM,
            libc::EACCES => EACCES,
            libc::EFAULT => EFAULT,
            libc::ENOTBLK => ENOTBLK,
            libc::EBUSY => EBUSY,
            libc::EEXIST => EEXIST,
            libc::EXDEV => EXDEV,
            libc::ENODEV => ENODEV,
            libc::ENOTDIR => ENOTDIR,
            libc::EISDIR => EISDIR,
            libc::EINVAL => EINVAL,
            libc::ENFILE => ENFILE,
            libc::EMFILE => EMFILE,
            libc::ENOTTY => ENOTTY,
            libc::ETXTBSY => ETXTBSY,
            libc::EFBIG => EFBIG,
            libc::ENOSPC => ENOSPC,
            libc::ESPIPE => ESPIPE,
            libc::EROFS => EROFS,
            libc::EMLINK => EMLINK,
            libc::EPIPE => EPIPE,
            libc::EDOM => EDOM,
            libc::ERANGE => ERANGE,
            libc::EAGAIN => EAGAIN,
            libc::EINPROGRESS => EINPROGRESS,
            libc::EALREADY => EALREADY,
            libc::ENOTSOCK => ENOTSOCK,
            libc::EDESTADDRREQ => EDESTADDRREQ,
            libc::EMSGSIZE => EMSGSIZE,
            libc::EPROTOTYPE => EPROTOTYPE,
            libc::ENOPROTOOPT => ENOPROTOOPT,
            libc::EPROTONOSUPPORT => EPROTONOSUPPORT,
            libc::ESOCKTNOSUPPORT => ESOCKTNOSUPPORT,
            libc::EOPNOTSUPP => EOPNOTSUPP,
            libc::EPFNOSUPPORT => EPFNOSUPPORT,
            libc::EAFNOSUPPORT => EAFNOSUPPORT,
            libc::EADDRINUSE => EADDRINUSE,
            libc::EADDRNOTAVAIL => EADDRNOTAVAIL,
            libc::ENETDOWN => ENETDOWN,
            libc::ENETUNREACH => ENETUNREACH,
            libc::ENETRESET => ENETRESET,
            libc::ECONNABORTED => ECONNABORTED,
            libc::ECONNRESET => ECONNRESET,
            libc::ENOBUFS => ENOBUFS,
            libc::EISCONN => EISCONN,
            libc::ENOTCONN => ENOTCONN,
            libc::ESHUTDOWN => ESHUTDOWN,
            libc::ETOOMANYREFS => ETOOMANYREFS,
            libc::ETIMEDOUT => ETIMEDOUT,
            libc::ECONNREFUSED => ECONNREFUSED,
            libc::ELOOP => ELOOP,
            libc::ENAMETOOLONG => ENAMETOOLONG,
            libc::EHOSTDOWN => EHOSTDOWN,
            libc::EHOSTUNREACH => EHOSTUNREACH,
            libc::ENOTEMPTY => ENOTEMPTY,
            libc::EUSERS => EUSERS,
            libc::EDQUOT => EDQUOT,
            libc::ESTALE => ESTALE,
            libc::EREMOTE => EREMOTE,
            libc::ENOLCK => ENOLCK,
            libc::ENOSYS => ENOSYS,
            libc::EIDRM => EIDRM,
            libc::ENOMSG => ENOMSG,
            libc::EOVERFLOW => EOVERFLOW,
            libc::EILSEQ => EILSEQ,
            libc::ECANCELED => ECANCELED,
            libc::EBADMSG => EBADMSG,
            libc::ENODATA => ENODATA,
            libc::ENOSR => ENOSR,
            libc::ENOSTR => ENOSTR,
            libc::ETIME => ETIME,
            libc::EMULTIHOP => EMULTIHOP,
            libc::ENOLINK => ENOLINK,
            libc::EPROTO => EPROTO,
            _ => UnknownErrno,
        }
    }
}

#[cfg(solarish)]
mod consts {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(i32)]
    #[non_exhaustive]
    pub enum Errno {
        UnknownErrno = 0,
        EPERM = libc::EPERM,
        ENOENT = libc::ENOENT,
        ESRCH = libc::ESRCH,
        EINTR = libc::EINTR,
        EIO = libc::EIO,
        ENXIO = libc::ENXIO,
        E2BIG = libc::E2BIG,
        ENOEXEC = libc::ENOEXEC,
        EBADF = libc::EBADF,
        ECHILD = libc::ECHILD,
        EAGAIN = libc::EAGAIN,
        ENOMEM = libc::ENOMEM,
        EACCES = libc::EACCES,
        EFAULT = libc::EFAULT,
        ENOTBLK = libc::ENOTBLK,
        EBUSY = libc::EBUSY,
        EEXIST = libc::EEXIST,
        EXDEV = libc::EXDEV,
        ENODEV = libc::ENODEV,
        ENOTDIR = libc::ENOTDIR,
        EISDIR = libc::EISDIR,
        EINVAL = libc::EINVAL,
        ENFILE = libc::ENFILE,
        EMFILE = libc::EMFILE,
        ENOTTY = libc::ENOTTY,
        ETXTBSY = libc::ETXTBSY,
        EFBIG = libc::EFBIG,
        ENOSPC = libc::ENOSPC,
        ESPIPE = libc::ESPIPE,
        EROFS = libc::EROFS,
        EMLINK = libc::EMLINK,
        EPIPE = libc::EPIPE,
        EDOM = libc::EDOM,
        ERANGE = libc::ERANGE,
        ENOMSG = libc::ENOMSG,
        EIDRM = libc::EIDRM,
        ECHRNG = libc::ECHRNG,
        EL2NSYNC = libc::EL2NSYNC,
        EL3HLT = libc::EL3HLT,
        EL3RST = libc::EL3RST,
        ELNRNG = libc::ELNRNG,
        EUNATCH = libc::EUNATCH,
        ENOCSI = libc::ENOCSI,
        EL2HLT = libc::EL2HLT,
        EDEADLK = libc::EDEADLK,
        ENOLCK = libc::ENOLCK,
        ECANCELED = libc::ECANCELED,
        ENOTSUP = libc::ENOTSUP,
        EDQUOT = libc::EDQUOT,
        EBADE = libc::EBADE,
        EBADR = libc::EBADR,
        EXFULL = libc::EXFULL,
        ENOANO = libc::ENOANO,
        EBADRQC = libc::EBADRQC,
        EBADSLT = libc::EBADSLT,
        EDEADLOCK = libc::EDEADLOCK,
        EBFONT = libc::EBFONT,
        EOWNERDEAD = libc::EOWNERDEAD,
        ENOTRECOVERABLE = libc::ENOTRECOVERABLE,
        ENOSTR = libc::ENOSTR,
        ENODATA = libc::ENODATA,
        ETIME = libc::ETIME,
        ENOSR = libc::ENOSR,
        ENONET = libc::ENONET,
        ENOPKG = libc::ENOPKG,
        EREMOTE = libc::EREMOTE,
        ENOLINK = libc::ENOLINK,
        EADV = libc::EADV,
        ESRMNT = libc::ESRMNT,
        ECOMM = libc::ECOMM,
        EPROTO = libc::EPROTO,
        ELOCKUNMAPPED = libc::ELOCKUNMAPPED,
        ENOTACTIVE = libc::ENOTACTIVE,
        EMULTIHOP = libc::EMULTIHOP,
        EBADMSG = libc::EBADMSG,
        ENAMETOOLONG = libc::ENAMETOOLONG,
        EOVERFLOW = libc::EOVERFLOW,
        ENOTUNIQ = libc::ENOTUNIQ,
        EBADFD = libc::EBADFD,
        EREMCHG = libc::EREMCHG,
        ELIBACC = libc::ELIBACC,
        ELIBBAD = libc::ELIBBAD,
        ELIBSCN = libc::ELIBSCN,
        ELIBMAX = libc::ELIBMAX,
        ELIBEXEC = libc::ELIBEXEC,
        EILSEQ = libc::EILSEQ,
        ENOSYS = libc::ENOSYS,
        ELOOP = libc::ELOOP,
        ERESTART = libc::ERESTART,
        ESTRPIPE = libc::ESTRPIPE,
        ENOTEMPTY = libc::ENOTEMPTY,
        EUSERS = libc::EUSERS,
        ENOTSOCK = libc::ENOTSOCK,
        EDESTADDRREQ = libc::EDESTADDRREQ,
        EMSGSIZE = libc::EMSGSIZE,
        EPROTOTYPE = libc::EPROTOTYPE,
        ENOPROTOOPT = libc::ENOPROTOOPT,
        EPROTONOSUPPORT = libc::EPROTONOSUPPORT,
        ESOCKTNOSUPPORT = libc::ESOCKTNOSUPPORT,
        EOPNOTSUPP = libc::EOPNOTSUPP,
        EPFNOSUPPORT = libc::EPFNOSUPPORT,
        EAFNOSUPPORT = libc::EAFNOSUPPORT,
        EADDRINUSE = libc::EADDRINUSE,
        EADDRNOTAVAIL = libc::EADDRNOTAVAIL,
        ENETDOWN = libc::ENETDOWN,
        ENETUNREACH = libc::ENETUNREACH,
        ENETRESET = libc::ENETRESET,
        ECONNABORTED = libc::ECONNABORTED,
        ECONNRESET = libc::ECONNRESET,
        ENOBUFS = libc::ENOBUFS,
        EISCONN = libc::EISCONN,
        ENOTCONN = libc::ENOTCONN,
        ESHUTDOWN = libc::ESHUTDOWN,
        ETOOMANYREFS = libc::ETOOMANYREFS,
        ETIMEDOUT = libc::ETIMEDOUT,
        ECONNREFUSED = libc::ECONNREFUSED,
        EHOSTDOWN = libc::EHOSTDOWN,
        EHOSTUNREACH = libc::EHOSTUNREACH,
        EALREADY = libc::EALREADY,
        EINPROGRESS = libc::EINPROGRESS,
        ESTALE = libc::ESTALE,
    }

    impl Errno {
        pub const ELAST: Errno = Errno::ESTALE;
        pub const EWOULDBLOCK: Errno = Errno::EAGAIN;
    }

    #[deprecated(
        since = "0.28.0",
        note = "please use `Errno::from_raw()` instead"
    )]
    pub const fn from_i32(e: i32) -> Errno {
        use self::Errno::*;

        match e {
            libc::EPERM => EPERM,
            libc::ENOENT => ENOENT,
            libc::ESRCH => ESRCH,
            libc::EINTR => EINTR,
            libc::EIO => EIO,
            libc::ENXIO => ENXIO,
            libc::E2BIG => E2BIG,
            libc::ENOEXEC => ENOEXEC,
            libc::EBADF => EBADF,
            libc::ECHILD => ECHILD,
            libc::EAGAIN => EAGAIN,
            libc::ENOMEM => ENOMEM,
            libc::EACCES => EACCES,
            libc::EFAULT => EFAULT,
            libc::ENOTBLK => ENOTBLK,
            libc::EBUSY => EBUSY,
            libc::EEXIST => EEXIST,
            libc::EXDEV => EXDEV,
            libc::ENODEV => ENODEV,
            libc::ENOTDIR => ENOTDIR,
            libc::EISDIR => EISDIR,
            libc::EINVAL => EINVAL,
            libc::ENFILE => ENFILE,
            libc::EMFILE => EMFILE,
            libc::ENOTTY => ENOTTY,
            libc::ETXTBSY => ETXTBSY,
            libc::EFBIG => EFBIG,
            libc::ENOSPC => ENOSPC,
            libc::ESPIPE => ESPIPE,
            libc::EROFS => EROFS,
            libc::EMLINK => EMLINK,
            libc::EPIPE => EPIPE,
            libc::EDOM => EDOM,
            libc::ERANGE => ERANGE,
            libc::ENOMSG => ENOMSG,
            libc::EIDRM => EIDRM,
            libc::ECHRNG => ECHRNG,
            libc::EL2NSYNC => EL2NSYNC,
            libc::EL3HLT => EL3HLT,
            libc::EL3RST => EL3RST,
            libc::ELNRNG => ELNRNG,
            libc::EUNATCH => EUNATCH,
            libc::ENOCSI => ENOCSI,
            libc::EL2HLT => EL2HLT,
            libc::EDEADLK => EDEADLK,
            libc::ENOLCK => ENOLCK,
            libc::ECANCELED => ECANCELED,
            libc::ENOTSUP => ENOTSUP,
            libc::EDQUOT => EDQUOT,
            libc::EBADE => EBADE,
            libc::EBADR => EBADR,
            libc::EXFULL => EXFULL,
            libc::ENOANO => ENOANO,
            libc::EBADRQC => EBADRQC,
            libc::EBADSLT => EBADSLT,
            libc::EDEADLOCK => EDEADLOCK,
            libc::EBFONT => EBFONT,
            libc::EOWNERDEAD => EOWNERDEAD,
            libc::ENOTRECOVERABLE => ENOTRECOVERABLE,
            libc::ENOSTR => ENOSTR,
            libc::ENODATA => ENODATA,
            libc::ETIME => ETIME,
            libc::ENOSR => ENOSR,
            libc::ENONET => ENONET,
            libc::ENOPKG => ENOPKG,
            libc::EREMOTE => EREMOTE,
            libc::ENOLINK => ENOLINK,
            libc::EADV => EADV,
            libc::ESRMNT => ESRMNT,
            libc::ECOMM => ECOMM,
            libc::EPROTO => EPROTO,
            libc::ELOCKUNMAPPED => ELOCKUNMAPPED,
            libc::ENOTACTIVE => ENOTACTIVE,
            libc::EMULTIHOP => EMULTIHOP,
            libc::EBADMSG => EBADMSG,
            libc::ENAMETOOLONG => ENAMETOOLONG,
            libc::EOVERFLOW => EOVERFLOW,
            libc::ENOTUNIQ => ENOTUNIQ,
            libc::EBADFD => EBADFD,
            libc::EREMCHG => EREMCHG,
            libc::ELIBACC => ELIBACC,
            libc::ELIBBAD => ELIBBAD,
            libc::ELIBSCN => ELIBSCN,
            libc::ELIBMAX => ELIBMAX,
            libc::ELIBEXEC => ELIBEXEC,
            libc::EILSEQ => EILSEQ,
            libc::ENOSYS => ENOSYS,
            libc::ELOOP => ELOOP,
            libc::ERESTART => ERESTART,
            libc::ESTRPIPE => ESTRPIPE,
            libc::ENOTEMPTY => ENOTEMPTY,
            libc::EUSERS => EUSERS,
            libc::ENOTSOCK => ENOTSOCK,
            libc::EDESTADDRREQ => EDESTADDRREQ,
            libc::EMSGSIZE => EMSGSIZE,
            libc::EPROTOTYPE => EPROTOTYPE,
            libc::ENOPROTOOPT => ENOPROTOOPT,
            libc::EPROTONOSUPPORT => EPROTONOSUPPORT,
            libc::ESOCKTNOSUPPORT => ESOCKTNOSUPPORT,
            libc::EOPNOTSUPP => EOPNOTSUPP,
            libc::EPFNOSUPPORT => EPFNOSUPPORT,
            libc::EAFNOSUPPORT => EAFNOSUPPORT,
            libc::EADDRINUSE => EADDRINUSE,
            libc::EADDRNOTAVAIL => EADDRNOTAVAIL,
            libc::ENETDOWN => ENETDOWN,
            libc::ENETUNREACH => ENETUNREACH,
            libc::ENETRESET => ENETRESET,
            libc::ECONNABORTED => ECONNABORTED,
            libc::ECONNRESET => ECONNRESET,
            libc::ENOBUFS => ENOBUFS,
            libc::EISCONN => EISCONN,
            libc::ENOTCONN => ENOTCONN,
            libc::ESHUTDOWN => ESHUTDOWN,
            libc::ETOOMANYREFS => ETOOMANYREFS,
            libc::ETIMEDOUT => ETIMEDOUT,
            libc::ECONNREFUSED => ECONNREFUSED,
            libc::EHOSTDOWN => EHOSTDOWN,
            libc::EHOSTUNREACH => EHOSTUNREACH,
            libc::EALREADY => EALREADY,
            libc::EINPROGRESS => EINPROGRESS,
            libc::ESTALE => ESTALE,
            _ => UnknownErrno,
        }
    }
}

#[cfg(target_os = "haiku")]
mod consts {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(i32)]
    #[non_exhaustive]
    pub enum Errno {
        UnknownErrno = 0,
        EPERM = libc::EPERM,
        ENOENT = libc::ENOENT,
        ESRCH = libc::ESRCH,
        EINTR = libc::EINTR,
        EIO = libc::EIO,
        ENXIO = libc::ENXIO,
        E2BIG = libc::E2BIG,
        ENOEXEC = libc::ENOEXEC,
        EBADF = libc::EBADF,
        ECHILD = libc::ECHILD,
        EDEADLK = libc::EDEADLK,
        ENOMEM = libc::ENOMEM,
        EACCES = libc::EACCES,
        EFAULT = libc::EFAULT,
        EBUSY = libc::EBUSY,
        EEXIST = libc::EEXIST,
        EXDEV = libc::EXDEV,
        ENODEV = libc::ENODEV,
        ENOTDIR = libc::ENOTDIR,
        EISDIR = libc::EISDIR,
        EINVAL = libc::EINVAL,
        ENFILE = libc::ENFILE,
        EMFILE = libc::EMFILE,
        ENOTTY = libc::ENOTTY,
        ETXTBSY = libc::ETXTBSY,
        EFBIG = libc::EFBIG,
        ENOSPC = libc::ENOSPC,
        ESPIPE = libc::ESPIPE,
        EROFS = libc::EROFS,
        EMLINK = libc::EMLINK,
        EPIPE = libc::EPIPE,
        EDOM = libc::EDOM,
        ERANGE = libc::ERANGE,
        EAGAIN = libc::EAGAIN,
        EINPROGRESS = libc::EINPROGRESS,
        EALREADY = libc::EALREADY,
        ENOTSOCK = libc::ENOTSOCK,
        EDESTADDRREQ = libc::EDESTADDRREQ,
        EMSGSIZE = libc::EMSGSIZE,
        EPROTOTYPE = libc::EPROTOTYPE,
        ENOPROTOOPT = libc::ENOPROTOOPT,
        EPROTONOSUPPORT = libc::EPROTONOSUPPORT,
        ENOTSUP = libc::ENOTSUP,
        EADDRINUSE = libc::EADDRINUSE,
        EADDRNOTAVAIL = libc::EADDRNOTAVAIL,
        ENETDOWN = libc::ENETDOWN,
        ENETUNREACH = libc::ENETUNREACH,
        ENETRESET = libc::ENETRESET,
        ECONNABORTED = libc::ECONNABORTED,
        ECONNRESET = libc::ECONNRESET,
        ENOBUFS = libc::ENOBUFS,
        EISCONN = libc::EISCONN,
        ENOTCONN = libc::ENOTCONN,
        ESHUTDOWN = libc::ESHUTDOWN,
        ETIMEDOUT = libc::ETIMEDOUT,
        ECONNREFUSED = libc::ECONNREFUSED,
        ELOOP = libc::ELOOP,
        ENAMETOOLONG = libc::ENAMETOOLONG,
        EHOSTDOWN = libc::EHOSTDOWN,
        EHOSTUNREACH = libc::EHOSTUNREACH,
        ENOTEMPTY = libc::ENOTEMPTY,
        EDQUOT = libc::EDQUOT,
        ESTALE = libc::ESTALE,
        ENOLCK = libc::ENOLCK,
        ENOSYS = libc::ENOSYS,
        EIDRM = libc::EIDRM,
        ENOMSG = libc::ENOMSG,
        EOVERFLOW = libc::EOVERFLOW,
        ECANCELED = libc::ECANCELED,
        EILSEQ = libc::EILSEQ,
        ENOATTR = libc::ENOATTR,
        EBADMSG = libc::EBADMSG,
        EMULTIHOP = libc::EMULTIHOP,
        ENOLINK = libc::ENOLINK,
        EPROTO = libc::EPROTO,
    }

    impl Errno {
        pub const EWOULDBLOCK: Errno = Errno::EAGAIN;
        pub const EDEADLOCK: Errno = Errno::EDEADLK;
        pub const EOPNOTSUPP: Errno = Errno::ENOTSUP;
    }

    #[deprecated(
        since = "0.28.0",
        note = "please use `Errno::from_raw()` instead"
    )]
    pub const fn from_i32(e: i32) -> Errno {
        use self::Errno::*;

        match e {
            libc::EPERM => EPERM,
            libc::ENOENT => ENOENT,
            libc::ESRCH => ESRCH,
            libc::EINTR => EINTR,
            libc::EIO => EIO,
            libc::ENXIO => ENXIO,
            libc::E2BIG => E2BIG,
            libc::ENOEXEC => ENOEXEC,
            libc::EBADF => EBADF,
            libc::ECHILD => ECHILD,
            libc::EDEADLK => EDEADLK,
            libc::ENOMEM => ENOMEM,
            libc::EACCES => EACCES,
            libc::EFAULT => EFAULT,
            libc::EBUSY => EBUSY,
            libc::EEXIST => EEXIST,
            libc::EXDEV => EXDEV,
            libc::ENODEV => ENODEV,
            libc::ENOTDIR => ENOTDIR,
            libc::EISDIR => EISDIR,
            libc::EINVAL => EINVAL,
            libc::ENFILE => ENFILE,
            libc::EMFILE => EMFILE,
            libc::ENOTTY => ENOTTY,
            libc::ETXTBSY => ETXTBSY,
            libc::EFBIG => EFBIG,
            libc::ENOSPC => ENOSPC,
            libc::ESPIPE => ESPIPE,
            libc::EROFS => EROFS,
            libc::EMLINK => EMLINK,
            libc::EPIPE => EPIPE,
            libc::EDOM => EDOM,
            libc::ERANGE => ERANGE,
            libc::EAGAIN => EAGAIN,
            libc::EINPROGRESS => EINPROGRESS,
            libc::EALREADY => EALREADY,
            libc::ENOTSOCK => ENOTSOCK,
            libc::EDESTADDRREQ => EDESTADDRREQ,
            libc::EMSGSIZE => EMSGSIZE,
            libc::EPROTOTYPE => EPROTOTYPE,
            libc::ENOPROTOOPT => ENOPROTOOPT,
            libc::EPROTONOSUPPORT => EPROTONOSUPPORT,
            libc::ENOTSUP => ENOTSUP,
            libc::EADDRINUSE => EADDRINUSE,
            libc::EADDRNOTAVAIL => EADDRNOTAVAIL,
            libc::ENETDOWN => ENETDOWN,
            libc::ENETUNREACH => ENETUNREACH,
            libc::ENETRESET => ENETRESET,
            libc::ECONNABORTED => ECONNABORTED,
            libc::ECONNRESET => ECONNRESET,
            libc::ENOBUFS => ENOBUFS,
            libc::EISCONN => EISCONN,
            libc::ENOTCONN => ENOTCONN,
            libc::ESHUTDOWN => ESHUTDOWN,
            libc::ETIMEDOUT => ETIMEDOUT,
            libc::ECONNREFUSED => ECONNREFUSED,
            libc::ELOOP => ELOOP,
            libc::ENAMETOOLONG => ENAMETOOLONG,
            libc::EHOSTDOWN => EHOSTDOWN,
            libc::EHOSTUNREACH => EHOSTUNREACH,
            libc::ENOTEMPTY => ENOTEMPTY,
            libc::EDQUOT => EDQUOT,
            libc::ESTALE => ESTALE,
            libc::ENOLCK => ENOLCK,
            libc::ENOSYS => ENOSYS,
            libc::EIDRM => EIDRM,
            libc::ENOMSG => ENOMSG,
            libc::EOVERFLOW => EOVERFLOW,
            libc::ECANCELED => ECANCELED,
            libc::EILSEQ => EILSEQ,
            libc::ENOATTR => ENOATTR,
            libc::EBADMSG => EBADMSG,
            libc::EMULTIHOP => EMULTIHOP,
            libc::ENOLINK => ENOLINK,
            libc::EPROTO => EPROTO,
            _ => UnknownErrno,
        }
    }
}

#[cfg(target_os = "aix")]
mod consts {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(i32)]
    #[non_exhaustive]
    pub enum Errno {
        UnknownErrno = 0,
        EPERM = libc::EPERM,
        ENOENT = libc::ENOENT,
        ESRCH = libc::ESRCH,
        EINTR = libc::EINTR,
        EIO = libc::EIO,
        ENXIO = libc::ENXIO,
        E2BIG = libc::E2BIG,
        ENOEXEC = libc::ENOEXEC,
        EBADF = libc::EBADF,
        ECHILD = libc::ECHILD,
        EAGAIN = libc::EAGAIN,
        ENOMEM = libc::ENOMEM,
        EACCES = libc::EACCES,
        EFAULT = libc::EFAULT,
        ENOTBLK = libc::ENOTBLK,
        EBUSY = libc::EBUSY,
        EEXIST = libc::EEXIST,
        EXDEV = libc::EXDEV,
        ENODEV = libc::ENODEV,
        ENOTDIR = libc::ENOTDIR,
        EISDIR = libc::EISDIR,
        EINVAL = libc::EINVAL,
        ENFILE = libc::ENFILE,
        EMFILE = libc::EMFILE,
        ENOTTY = libc::ENOTTY,
        ETXTBSY = libc::ETXTBSY,
        EFBIG = libc::EFBIG,
        ENOSPC = libc::ENOSPC,
        ESPIPE = libc::ESPIPE,
        EROFS = libc::EROFS,
        EMLINK = libc::EMLINK,
        EPIPE = libc::EPIPE,
        EDOM = libc::EDOM,
        ERANGE = libc::ERANGE,
        EDEADLK = libc::EDEADLK,
        ENAMETOOLONG = libc::ENAMETOOLONG,
        ENOLCK = libc::ENOLCK,
        ENOSYS = libc::ENOSYS,
        ENOTEMPTY = libc::ENOTEMPTY,
        ELOOP = libc::ELOOP,
        ENOMSG = libc::ENOMSG,
        EIDRM = libc::EIDRM,
        EINPROGRESS = libc::EINPROGRESS,
        EALREADY = libc::EALREADY,
        ENOTSOCK = libc::ENOTSOCK,
        EDESTADDRREQ = libc::EDESTADDRREQ,
        EMSGSIZE = libc::EMSGSIZE,
        EPROTOTYPE = libc::EPROTOTYPE,
        ENOPROTOOPT = libc::ENOPROTOOPT,
        EPROTONOSUPPORT = libc::EPROTONOSUPPORT,
        ESOCKTNOSUPPORT = libc::ESOCKTNOSUPPORT,
        EPFNOSUPPORT = libc::EPFNOSUPPORT,
        EAFNOSUPPORT = libc::EAFNOSUPPORT,
        EADDRINUSE = libc::EADDRINUSE,
        EADDRNOTAVAIL = libc::EADDRNOTAVAIL,
        ENETDOWN = libc::ENETDOWN,
        ENETUNREACH = libc::ENETUNREACH,
        ENETRESET = libc::ENETRESET,
        ECONNABORTED = libc::ECONNABORTED,
        ECONNRESET = libc::ECONNRESET,
        ENOBUFS = libc::ENOBUFS,
        EISCONN = libc::EISCONN,
        ENOTCONN = libc::ENOTCONN,
        ESHUTDOWN = libc::ESHUTDOWN,
        ETOOMANYREFS = libc::ETOOMANYREFS,
        ETIMEDOUT = libc::ETIMEDOUT,
        ECONNREFUSED = libc::ECONNREFUSED,
        EHOSTDOWN = libc::EHOSTDOWN,
        EHOSTUNREACH = libc::EHOSTUNREACH,
        ECHRNG = libc::ECHRNG,
        EL2NSYNC = libc::EL2NSYNC,
        EL3HLT = libc::EL3HLT,
        EL3RST = libc::EL3RST,
        ELNRNG = libc::ELNRNG,
        EUNATCH = libc::EUNATCH,
        ENOCSI = libc::ENOCSI,
        EL2HLT = libc::EL2HLT,
        ENOLINK = libc::ENOLINK,
        EPROTO = libc::EPROTO,
        EMULTIHOP = libc::EMULTIHOP,
        EBADMSG = libc::EBADMSG,
        EOVERFLOW = libc::EOVERFLOW,
        EILSEQ = libc::EILSEQ,
        ERESTART = libc::ERESTART,
        EOWNERDEAD = libc::EOWNERDEAD,
        ENOTRECOVERABLE = libc::ENOTRECOVERABLE,
        ENOTSUP = libc::ENOTSUP,
        EPROCLIM = libc::EPROCLIM,
        EUSERS = libc::EUSERS,
        EDQUOT = libc::EDQUOT,
        ESTALE = libc::ESTALE,
        EREMOTE = libc::EREMOTE,
        ECANCELED = libc::ECANCELED,
        ENODATA = libc::ENODATA,
        ENOSR = libc::ENOSR,
        ENOSTR = libc::ENOSTR,
        ETIME = libc::ETIME,
        EOPNOTSUPP = libc::EOPNOTSUPP,
    }

    #[deprecated(
        since = "0.28.0",
        note = "please use `Errno::from_raw()` instead"
    )]
    pub const fn from_i32(e: i32) -> Errno {
        use self::Errno::*;

        match e {
            libc::EPERM => EPERM,
            libc::ENOENT => ENOENT,
            libc::ESRCH => ESRCH,
            libc::EINTR => EINTR,
            libc::EIO => EIO,
            libc::ENXIO => ENXIO,
            libc::E2BIG => E2BIG,
            libc::ENOEXEC => ENOEXEC,
            libc::EBADF => EBADF,
            libc::ECHILD => ECHILD,
            libc::EAGAIN => EAGAIN,
            libc::ENOMEM => ENOMEM,
            libc::EACCES => EACCES,
            libc::EFAULT => EFAULT,
            libc::ENOTBLK => ENOTBLK,
            libc::EBUSY => EBUSY,
            libc::EEXIST => EEXIST,
            libc::EXDEV => EXDEV,
            libc::ENODEV => ENODEV,
            libc::ENOTDIR => ENOTDIR,
            libc::EISDIR => EISDIR,
            libc::EINVAL => EINVAL,
            libc::ENFILE => ENFILE,
            libc::EMFILE => EMFILE,
            libc::ENOTTY => ENOTTY,
            libc::ETXTBSY => ETXTBSY,
            libc::EFBIG => EFBIG,
            libc::ENOSPC => ENOSPC,
            libc::ESPIPE => ESPIPE,
            libc::EROFS => EROFS,
            libc::EMLINK => EMLINK,
            libc::EPIPE => EPIPE,
            libc::EDOM => EDOM,
            libc::ERANGE => ERANGE,
            libc::EDEADLK => EDEADLK,
            libc::ENAMETOOLONG => ENAMETOOLONG,
            libc::ENOLCK => ENOLCK,
            libc::ENOSYS => ENOSYS,
            libc::ENOTEMPTY => ENOTEMPTY,
            libc::ELOOP => ELOOP,
            libc::ENOMSG => ENOMSG,
            libc::EIDRM => EIDRM,
            libc::EINPROGRESS => EINPROGRESS,
            libc::EALREADY => EALREADY,
            libc::ENOTSOCK => ENOTSOCK,
            libc::EDESTADDRREQ => EDESTADDRREQ,
            libc::EMSGSIZE => EMSGSIZE,
            libc::EPROTOTYPE => EPROTOTYPE,
            libc::ENOPROTOOPT => ENOPROTOOPT,
            libc::EPROTONOSUPPORT => EPROTONOSUPPORT,
            libc::ESOCKTNOSUPPORT => ESOCKTNOSUPPORT,
            libc::EPFNOSUPPORT => EPFNOSUPPORT,
            libc::EAFNOSUPPORT => EAFNOSUPPORT,
            libc::EADDRINUSE => EADDRINUSE,
            libc::EADDRNOTAVAIL => EADDRNOTAVAIL,
            libc::ENETDOWN => ENETDOWN,
            libc::ENETUNREACH => ENETUNREACH,
            libc::ENETRESET => ENETRESET,
            libc::ECONNABORTED => ECONNABORTED,
            libc::ECONNRESET => ECONNRESET,
            libc::ENOBUFS => ENOBUFS,
            libc::EISCONN => EISCONN,
            libc::ENOTCONN => ENOTCONN,
            libc::ESHUTDOWN => ESHUTDOWN,
            libc::ETOOMANYREFS => ETOOMANYREFS,
            libc::ETIMEDOUT => ETIMEDOUT,
            libc::ECONNREFUSED => ECONNREFUSED,
            libc::EHOSTDOWN => EHOSTDOWN,
            libc::EHOSTUNREACH => EHOSTUNREACH,
            libc::ECHRNG => ECHRNG,
            libc::EL2NSYNC => EL2NSYNC,
            libc::EL3HLT => EL3HLT,
            libc::EL3RST => EL3RST,
            libc::ELNRNG => ELNRNG,
            libc::EUNATCH => EUNATCH,
            libc::ENOCSI => ENOCSI,
            libc::EL2HLT => EL2HLT,
            libc::ENOLINK => ENOLINK,
            libc::EPROTO => EPROTO,
            libc::EMULTIHOP => EMULTIHOP,
            libc::EBADMSG => EBADMSG,
            libc::EOVERFLOW => EOVERFLOW,
            libc::EILSEQ => EILSEQ,
            libc::ERESTART => ERESTART,
            libc::ENOTRECOVERABLE => ENOTRECOVERABLE,
            libc::EOWNERDEAD => EOWNERDEAD,
            libc::ENOTSUP => ENOTSUP,
            libc::EPROCLIM => EPROCLIM,
            libc::EUSERS => EUSERS,
            libc::EDQUOT => EDQUOT,
            libc::ESTALE => ESTALE,
            libc::EREMOTE => EREMOTE,
            libc::ECANCELED => ECANCELED,
            libc::ENODATA => ENODATA,
            libc::ENOSR => ENOSR,
            libc::ENOSTR => ENOSTR,
            libc::ETIME => ETIME,
            libc::EOPNOTSUPP => EOPNOTSUPP,
            _ => UnknownErrno,
        }
    }
}

#[cfg(target_os = "hurd")]
mod consts {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(i32)]
    #[non_exhaustive]
    pub enum Errno {
        UnknownErrno = 0,
        EPERM = libc::EPERM,
        ENOENT = libc::ENOENT,
        ESRCH = libc::ESRCH,
        EINTR = libc::EINTR,
        EIO = libc::EIO,
        ENXIO = libc::ENXIO,
        E2BIG = libc::E2BIG,
        ENOEXEC = libc::ENOEXEC,
        EBADF = libc::EBADF,
        ECHILD = libc::ECHILD,
        EDEADLK = libc::EDEADLK,
        ENOMEM = libc::ENOMEM,
        EACCES = libc::EACCES,
        EFAULT = libc::EFAULT,
        ENOTBLK = libc::ENOTBLK,
        EBUSY = libc::EBUSY,
        EEXIST = libc::EEXIST,
        EXDEV = libc::EXDEV,
        ENODEV = libc::ENODEV,
        ENOTDIR = libc::ENOTDIR,
        EISDIR = libc::EISDIR,
        EINVAL = libc::EINVAL,
        EMFILE = libc::EMFILE,
        ENFILE = libc::ENFILE,
        ENOTTY = libc::ENOTTY,
        ETXTBSY = libc::ETXTBSY,
        EFBIG = libc::EFBIG,
        ENOSPC = libc::ENOSPC,
        ESPIPE = libc::ESPIPE,
        EROFS = libc::EROFS,
        EMLINK = libc::EMLINK,
        EPIPE = libc::EPIPE,
        EDOM = libc::EDOM,
        ERANGE = libc::ERANGE,
        EAGAIN = libc::EAGAIN,
        EINPROGRESS = libc::EINPROGRESS,
        EALREADY = libc::EALREADY,
        ENOTSOCK = libc::ENOTSOCK,
        EMSGSIZE = libc::EMSGSIZE,
        EPROTOTYPE = libc::EPROTOTYPE,
        ENOPROTOOPT = libc::ENOPROTOOPT,
        EPROTONOSUPPORT = libc::EPROTONOSUPPORT,
        ESOCKTNOSUPPORT = libc::ESOCKTNOSUPPORT,
        EOPNOTSUPP = libc::EOPNOTSUPP,
        EPFNOSUPPORT = libc::EPFNOSUPPORT,
        EAFNOSUPPORT = libc::EAFNOSUPPORT,
        EADDRINUSE = libc::EADDRINUSE,
        EADDRNOTAVAIL = libc::EADDRNOTAVAIL,
        ENETDOWN = libc::ENETDOWN,
        ENETUNREACH = libc::ENETUNREACH,
        ENETRESET = libc::ENETRESET,
        ECONNABORTED = libc::ECONNABORTED,
        ECONNRESET = libc::ECONNRESET,
        ENOBUFS = libc::ENOBUFS,
        EISCONN = libc::EISCONN,
        ENOTCONN = libc::ENOTCONN,
        EDESTADDRREQ = libc::EDESTADDRREQ,
        ESHUTDOWN = libc::ESHUTDOWN,
        ETOOMANYREFS = libc::ETOOMANYREFS,
        ETIMEDOUT = libc::ETIMEDOUT,
        ECONNREFUSED = libc::ECONNREFUSED,
        ELOOP = libc::ELOOP,
        ENAMETOOLONG = libc::ENAMETOOLONG,
        EHOSTDOWN = libc::EHOSTDOWN,
        EHOSTUNREACH = libc::EHOSTUNREACH,
        ENOTEMPTY = libc::ENOTEMPTY,
        EPROCLIM = libc::EPROCLIM,
        EUSERS = libc::EUSERS,
        EDQUOT = libc::EDQUOT,
        ESTALE = libc::ESTALE,
        EREMOTE = libc::EREMOTE,
        EBADRPC = libc::EBADRPC,
        ERPCMISMATCH = libc::ERPCMISMATCH,
        EPROGUNAVAIL = libc::EPROGUNAVAIL,
        EPROGMISMATCH = libc::EPROGMISMATCH,
        EPROCUNAVAIL = libc::EPROCUNAVAIL,
        ENOLCK = libc::ENOLCK,
        EFTYPE = libc::EFTYPE,
        EAUTH = libc::EAUTH,
        ENEEDAUTH = libc::ENEEDAUTH,
        ENOSYS = libc::ENOSYS,
        ELIBEXEC = libc::ELIBEXEC,
        ENOTSUP = libc::ENOTSUP,
        EILSEQ = libc::EILSEQ,
        EBACKGROUND = libc::EBACKGROUND,
        EDIED = libc::EDIED,
        EGREGIOUS = libc::EGREGIOUS,
        EIEIO = libc::EIEIO,
        EGRATUITOUS = libc::EGRATUITOUS,
        EBADMSG = libc::EBADMSG,
        EIDRM = libc::EIDRM,
        EMULTIHOP = libc::EMULTIHOP,
        ENODATA = libc::ENODATA,
        ENOLINK = libc::ENOLINK,
        ENOMSG = libc::ENOMSG,
        ENOSR = libc::ENOSR,
        ENOSTR = libc::ENOSTR,
        EOVERFLOW = libc::EOVERFLOW,
        EPROTO = libc::EPROTO,
        ETIME = libc::ETIME,
        ECANCELED = libc::ECANCELED,
        EOWNERDEAD = libc::EOWNERDEAD,
        ENOTRECOVERABLE = libc::ENOTRECOVERABLE,
    }

    impl Errno {
        pub const EWOULDBLOCK: Errno = Errno::EAGAIN;
    }

    #[deprecated(
        since = "0.28.0",
        note = "please use `Errno::from_raw()` instead"
    )]
    pub const fn from_i32(e: i32) -> Errno {
        use self::Errno::*;

        match e {
            libc::EPERM => EPERM,
            libc::ENOENT => ENOENT,
            libc::ESRCH => ESRCH,
            libc::EINTR => EINTR,
            libc::EIO => EIO,
            libc::ENXIO => ENXIO,
            libc::E2BIG => E2BIG,
            libc::ENOEXEC => ENOEXEC,
            libc::EBADF => EBADF,
            libc::ECHILD => ECHILD,
            libc::EDEADLK => EDEADLK,
            libc::ENOMEM => ENOMEM,
            libc::EACCES => EACCES,
            libc::EFAULT => EFAULT,
            libc::ENOTBLK => ENOTBLK,
            libc::EBUSY => EBUSY,
            libc::EEXIST => EEXIST,
            libc::EXDEV => EXDEV,
            libc::ENODEV => ENODEV,
            libc::ENOTDIR => ENOTDIR,
            libc::EISDIR => EISDIR,
            libc::EINVAL => EINVAL,
            libc::EMFILE => EMFILE,
            libc::ENFILE => ENFILE,
            libc::ENOTTY => ENOTTY,
            libc::ETXTBSY => ETXTBSY,
            libc::EFBIG => EFBIG,
            libc::ENOSPC => ENOSPC,
            libc::ESPIPE => ESPIPE,
            libc::EROFS => EROFS,
            libc::EMLINK => EMLINK,
            libc::EPIPE => EPIPE,
            libc::EDOM => EDOM,
            libc::ERANGE => ERANGE,
            libc::EAGAIN => EAGAIN,
            libc::EINPROGRESS => EINPROGRESS,
            libc::EALREADY => EALREADY,
            libc::ENOTSOCK => ENOTSOCK,
            libc::EMSGSIZE => EMSGSIZE,
            libc::EPROTOTYPE => EPROTOTYPE,
            libc::ENOPROTOOPT => ENOPROTOOPT,
            libc::EPROTONOSUPPORT => EPROTONOSUPPORT,
            libc::ESOCKTNOSUPPORT => ESOCKTNOSUPPORT,
            libc::EOPNOTSUPP => EOPNOTSUPP,
            libc::EPFNOSUPPORT => EPFNOSUPPORT,
            libc::EAFNOSUPPORT => EAFNOSUPPORT,
            libc::EADDRINUSE => EADDRINUSE,
            libc::EADDRNOTAVAIL => EADDRNOTAVAIL,
            libc::ENETDOWN => ENETDOWN,
            libc::ENETUNREACH => ENETUNREACH,
            libc::ENETRESET => ENETRESET,
            libc::ECONNABORTED => ECONNABORTED,
            libc::ECONNRESET => ECONNRESET,
            libc::ENOBUFS => ENOBUFS,
            libc::EISCONN => EISCONN,
            libc::ENOTCONN => ENOTCONN,
            libc::EDESTADDRREQ => EDESTADDRREQ,
            libc::ESHUTDOWN => ESHUTDOWN,
            libc::ETOOMANYREFS => ETOOMANYREFS,
            libc::ETIMEDOUT => ETIMEDOUT,
            libc::ECONNREFUSED => ECONNREFUSED,
            libc::ELOOP => ELOOP,
            libc::ENAMETOOLONG => ENAMETOOLONG,
            libc::EHOSTDOWN => EHOSTDOWN,
            libc::EHOSTUNREACH => EHOSTUNREACH,
            libc::ENOTEMPTY => ENOTEMPTY,
            libc::EPROCLIM => EPROCLIM,
            libc::EUSERS => EUSERS,
            libc::EDQUOT => EDQUOT,
            libc::ESTALE => ESTALE,
            libc::EREMOTE => EREMOTE,
            libc::EBADRPC => EBADRPC,
            libc::ERPCMISMATCH => ERPCMISMATCH,
            libc::EPROGUNAVAIL => EPROGUNAVAIL,
            libc::EPROGMISMATCH => EPROGMISMATCH,
            libc::EPROCUNAVAIL => EPROCUNAVAIL,
            libc::ENOLCK => ENOLCK,
            libc::EFTYPE => EFTYPE,
            libc::EAUTH => EAUTH,
            libc::ENEEDAUTH => ENEEDAUTH,
            libc::ENOSYS => ENOSYS,
            libc::ELIBEXEC => ELIBEXEC,
            libc::ENOTSUP => ENOTSUP,
            libc::EILSEQ => EILSEQ,
            libc::EBACKGROUND => EBACKGROUND,
            libc::EDIED => EDIED,
            libc::EGREGIOUS => EGREGIOUS,
            libc::EIEIO => EIEIO,
            libc::EGRATUITOUS => EGRATUITOUS,
            libc::EBADMSG => EBADMSG,
            libc::EIDRM => EIDRM,
            libc::EMULTIHOP => EMULTIHOP,
            libc::ENODATA => ENODATA,
            libc::ENOLINK => ENOLINK,
            libc::ENOMSG => ENOMSG,
            libc::ENOSR => ENOSR,
            libc::ENOSTR => ENOSTR,
            libc::EOVERFLOW => EOVERFLOW,
            libc::EPROTO => EPROTO,
            libc::ETIME => ETIME,
            libc::ECANCELED => ECANCELED,
            libc::EOWNERDEAD => EOWNERDEAD,
            libc::ENOTRECOVERABLE => ENOTRECOVERABLE,
            _ => UnknownErrno,
        }
    }
}
