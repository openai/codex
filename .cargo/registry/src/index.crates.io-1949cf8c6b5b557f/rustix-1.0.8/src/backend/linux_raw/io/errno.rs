//! The `rustix` `Errno` type.
//!
//! This type holds an OS error code, which conceptually corresponds to an
//! `errno` value.
//!
//! # Safety
//!
//! Linux uses error codes in `-4095..0`; we use rustc attributes to describe
//! this restricted range of values.
#![allow(unsafe_code)]
#![cfg_attr(not(rustc_attrs), allow(unused_unsafe))]

use crate::backend::c;
use crate::backend::fd::RawFd;
use crate::backend::reg::{RetNumber, RetReg};
use crate::io;
use linux_raw_sys::errno;

/// `errno`—An error code.
///
/// The error type for `rustix` APIs. This is similar to [`std::io::Error`],
/// but only holds an OS error code, and no extra error value.
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [Winsock]
///  - [FreeBSD]
///  - [NetBSD]
///  - [OpenBSD]
///  - [DragonFly BSD]
///  - [illumos]
///  - [glibc]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/errno.html
/// [Linux]: https://man7.org/linux/man-pages/man3/errno.3.html
/// [Winsock]: https://learn.microsoft.com/en-us/windows/win32/winsock/windows-sockets-error-codes-2
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?errno
/// [NetBSD]: https://man.netbsd.org/errno.2
/// [OpenBSD]: https://man.openbsd.org/errno.2
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=errno&section=2
/// [illumos]: https://illumos.org/man/3C/errno
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Error-Codes.html
#[repr(transparent)]
#[doc(alias = "errno")]
#[derive(Eq, PartialEq, Hash, Copy, Clone)]
// Linux returns negated error codes, and we leave them in negated form, so
// error codes are in `-4095..0`.
#[cfg_attr(rustc_attrs, rustc_layout_scalar_valid_range_start(0xf001))]
#[cfg_attr(rustc_attrs, rustc_layout_scalar_valid_range_end(0xffff))]
pub struct Errno(u16);

impl Errno {
    /// Extract an `Errno` value from a `std::io::Error`.
    ///
    /// This isn't a `From` conversion because it's expected to be relatively
    /// uncommon.
    #[cfg(feature = "std")]
    #[inline]
    pub fn from_io_error(io_err: &std::io::Error) -> Option<Self> {
        io_err.raw_os_error().and_then(|raw| {
            // `std::io::Error` could theoretically have arbitrary OS error
            // values, so check that they're in Linux's range.
            if (1..4096).contains(&raw) {
                Some(Self::from_errno(raw as u32))
            } else {
                None
            }
        })
    }

    /// Extract the raw OS error number from this error.
    #[inline]
    pub const fn raw_os_error(self) -> i32 {
        (self.0 as i16 as i32).wrapping_neg()
    }

    /// Construct an `Errno` from a raw OS error number.
    #[inline]
    pub const fn from_raw_os_error(raw: i32) -> Self {
        Self::from_errno(raw as u32)
    }

    /// Convert from a C `errno` value (which is positive) to an `Errno`.
    const fn from_errno(raw: u32) -> Self {
        // We store error values in negated form, so that we don't have to
        // negate them after every syscall.
        let encoded = raw.wrapping_neg() as u16;

        // TODO: Use Range::contains, once that's `const`.
        assert!(encoded >= 0xf001);

        // SAFETY: Linux syscalls return negated error values in the range
        // `-4095..0`, which we just asserted.
        unsafe { Self(encoded) }
    }
}

/// Check for an error from the result of a syscall which encodes a
/// `c::c_int` on success.
#[inline]
pub(in crate::backend) fn try_decode_c_int<Num: RetNumber>(
    raw: RetReg<Num>,
) -> io::Result<c::c_int> {
    if raw.is_in_range(-4095..0) {
        // SAFETY: `raw` must be in `-4095..0`, and we just checked that raw is
        // in that range.
        return Err(unsafe { Errno(raw.decode_error_code()) });
    }

    Ok(raw.decode_c_int())
}

/// Check for an error from the result of a syscall which encodes a
/// `c::c_uint` on success.
#[inline]
pub(in crate::backend) fn try_decode_c_uint<Num: RetNumber>(
    raw: RetReg<Num>,
) -> io::Result<c::c_uint> {
    if raw.is_in_range(-4095..0) {
        // SAFETY: `raw` must be in `-4095..0`, and we just checked that raw is
        // in that range.
        return Err(unsafe { Errno(raw.decode_error_code()) });
    }

    Ok(raw.decode_c_uint())
}

/// Check for an error from the result of a syscall which encodes a `usize` on
/// success.
#[inline]
pub(in crate::backend) fn try_decode_usize<Num: RetNumber>(raw: RetReg<Num>) -> io::Result<usize> {
    if raw.is_in_range(-4095..0) {
        // SAFETY: `raw` must be in `-4095..0`, and we just checked that raw is
        // in that range.
        return Err(unsafe { Errno(raw.decode_error_code()) });
    }

    Ok(raw.decode_usize())
}

/// Check for an error from the result of a syscall which encodes a
/// `*mut c_void` on success.
#[inline]
pub(in crate::backend) fn try_decode_void_star<Num: RetNumber>(
    raw: RetReg<Num>,
) -> io::Result<*mut c::c_void> {
    if raw.is_in_range(-4095..0) {
        // SAFETY: `raw` must be in `-4095..0`, and we just checked that raw is
        // in that range.
        return Err(unsafe { Errno(raw.decode_error_code()) });
    }

    Ok(raw.decode_void_star())
}

/// Check for an error from the result of a syscall which encodes a
/// `u64` on success.
#[cfg(target_pointer_width = "64")]
#[inline]
pub(in crate::backend) fn try_decode_u64<Num: RetNumber>(raw: RetReg<Num>) -> io::Result<u64> {
    if raw.is_in_range(-4095..0) {
        // SAFETY: `raw` must be in `-4095..0`, and we just checked that raw is
        // in that range.
        return Err(unsafe { Errno(raw.decode_error_code()) });
    }

    Ok(raw.decode_u64())
}

/// Check for an error from the result of a syscall which encodes a file
/// descriptor on success.
///
/// # Safety
///
/// This must only be used with syscalls which return file descriptors on
/// success.
#[inline]
pub(in crate::backend) unsafe fn try_decode_raw_fd<Num: RetNumber>(
    raw: RetReg<Num>,
) -> io::Result<RawFd> {
    // Instead of using `check_result` here, we just check for negative, since
    // this function is only used for system calls which return file
    // descriptors, and this produces smaller code.
    if raw.is_negative() {
        debug_assert!(raw.is_in_range(-4095..0));

        // Tell the optimizer that we know the value is in the error range.
        // This helps it avoid unnecessary integer conversions.
        #[cfg(core_intrinsics)]
        {
            core::intrinsics::assume(raw.is_in_range(-4095..0));
        }

        return Err(Errno(raw.decode_error_code()));
    }

    Ok(raw.decode_raw_fd())
}

/// Check for an error from the result of a syscall which encodes no value on
/// success. On success, return the unconsumed `raw` value.
///
/// # Safety
///
/// This must only be used with syscalls which return no value on success.
#[inline]
pub(in crate::backend) unsafe fn try_decode_void<Num: RetNumber>(
    raw: RetReg<Num>,
) -> io::Result<()> {
    // Instead of using `check_result` here, we just check for zero, since this
    // function is only used for system calls which have no other return value,
    // and this produces smaller code.
    if raw.is_nonzero() {
        debug_assert!(raw.is_in_range(-4095..0));

        // Tell the optimizer that we know the value is in the error range.
        // This helps it avoid unnecessary integer conversions.
        #[cfg(core_intrinsics)]
        {
            core::intrinsics::assume(raw.is_in_range(-4095..0));
        }

        return Err(Errno(raw.decode_error_code()));
    }

    raw.decode_void();

    Ok(())
}

/// Check for an error from the result of a syscall which does not return on
/// success. On success, return the unconsumed `raw` value.
///
/// # Safety
///
/// This must only be used with syscalls which do not return on success.
#[cfg(any(feature = "event", feature = "runtime", feature = "system"))]
#[inline]
pub(in crate::backend) unsafe fn try_decode_error<Num: RetNumber>(raw: RetReg<Num>) -> io::Errno {
    debug_assert!(raw.is_in_range(-4095..0));

    // Tell the optimizer that we know the value is in the error range.
    // This helps it avoid unnecessary integer conversions.
    #[cfg(core_intrinsics)]
    {
        core::intrinsics::assume(raw.is_in_range(-4095..0));
    }

    Errno(raw.decode_error_code())
}

/// Return the contained `usize` value.
#[cfg(not(debug_assertions))]
#[inline]
pub(in crate::backend) fn decode_usize_infallible<Num: RetNumber>(raw: RetReg<Num>) -> usize {
    raw.decode_usize()
}

/// Return the contained `c_int` value.
#[cfg(not(debug_assertions))]
#[inline]
pub(in crate::backend) fn decode_c_int_infallible<Num: RetNumber>(raw: RetReg<Num>) -> c::c_int {
    raw.decode_c_int()
}

/// Return the contained `c_uint` value.
#[cfg(not(debug_assertions))]
#[inline]
pub(in crate::backend) fn decode_c_uint_infallible<Num: RetNumber>(raw: RetReg<Num>) -> c::c_uint {
    raw.decode_c_uint()
}

impl Errno {
    /// `EACCES`
    #[doc(alias = "ACCES")]
    pub const ACCESS: Self = Self::from_errno(errno::EACCES);
    /// `EADDRINUSE`
    pub const ADDRINUSE: Self = Self::from_errno(errno::EADDRINUSE);
    /// `EADDRNOTAVAIL`
    pub const ADDRNOTAVAIL: Self = Self::from_errno(errno::EADDRNOTAVAIL);
    /// `EADV`
    pub const ADV: Self = Self::from_errno(errno::EADV);
    /// `EAFNOSUPPORT`
    pub const AFNOSUPPORT: Self = Self::from_errno(errno::EAFNOSUPPORT);
    /// `EAGAIN`
    pub const AGAIN: Self = Self::from_errno(errno::EAGAIN);
    /// `EALREADY`
    pub const ALREADY: Self = Self::from_errno(errno::EALREADY);
    /// `EBADE`
    pub const BADE: Self = Self::from_errno(errno::EBADE);
    /// `EBADF`
    pub const BADF: Self = Self::from_errno(errno::EBADF);
    /// `EBADFD`
    pub const BADFD: Self = Self::from_errno(errno::EBADFD);
    /// `EBADMSG`
    pub const BADMSG: Self = Self::from_errno(errno::EBADMSG);
    /// `EBADR`
    pub const BADR: Self = Self::from_errno(errno::EBADR);
    /// `EBADRQC`
    pub const BADRQC: Self = Self::from_errno(errno::EBADRQC);
    /// `EBADSLT`
    pub const BADSLT: Self = Self::from_errno(errno::EBADSLT);
    /// `EBFONT`
    pub const BFONT: Self = Self::from_errno(errno::EBFONT);
    /// `EBUSY`
    pub const BUSY: Self = Self::from_errno(errno::EBUSY);
    /// `ECANCELED`
    pub const CANCELED: Self = Self::from_errno(errno::ECANCELED);
    /// `ECHILD`
    pub const CHILD: Self = Self::from_errno(errno::ECHILD);
    /// `ECHRNG`
    pub const CHRNG: Self = Self::from_errno(errno::ECHRNG);
    /// `ECOMM`
    pub const COMM: Self = Self::from_errno(errno::ECOMM);
    /// `ECONNABORTED`
    pub const CONNABORTED: Self = Self::from_errno(errno::ECONNABORTED);
    /// `ECONNREFUSED`
    pub const CONNREFUSED: Self = Self::from_errno(errno::ECONNREFUSED);
    /// `ECONNRESET`
    pub const CONNRESET: Self = Self::from_errno(errno::ECONNRESET);
    /// `EDEADLK`
    pub const DEADLK: Self = Self::from_errno(errno::EDEADLK);
    /// `EDEADLOCK`
    pub const DEADLOCK: Self = Self::from_errno(errno::EDEADLOCK);
    /// `EDESTADDRREQ`
    pub const DESTADDRREQ: Self = Self::from_errno(errno::EDESTADDRREQ);
    /// `EDOM`
    pub const DOM: Self = Self::from_errno(errno::EDOM);
    /// `EDOTDOT`
    pub const DOTDOT: Self = Self::from_errno(errno::EDOTDOT);
    /// `EDQUOT`
    pub const DQUOT: Self = Self::from_errno(errno::EDQUOT);
    /// `EEXIST`
    pub const EXIST: Self = Self::from_errno(errno::EEXIST);
    /// `EFAULT`
    pub const FAULT: Self = Self::from_errno(errno::EFAULT);
    /// `EFBIG`
    pub const FBIG: Self = Self::from_errno(errno::EFBIG);
    /// `EHOSTDOWN`
    pub const HOSTDOWN: Self = Self::from_errno(errno::EHOSTDOWN);
    /// `EHOSTUNREACH`
    pub const HOSTUNREACH: Self = Self::from_errno(errno::EHOSTUNREACH);
    /// `EHWPOISON`
    pub const HWPOISON: Self = Self::from_errno(errno::EHWPOISON);
    /// `EIDRM`
    pub const IDRM: Self = Self::from_errno(errno::EIDRM);
    /// `EILSEQ`
    pub const ILSEQ: Self = Self::from_errno(errno::EILSEQ);
    /// `EINPROGRESS`
    pub const INPROGRESS: Self = Self::from_errno(errno::EINPROGRESS);
    /// `EINTR`
    ///
    /// For a convenient way to retry system calls that exit with `INTR`, use
    /// [`retry_on_intr`].
    ///
    /// [`retry_on_intr`]: io::retry_on_intr
    pub const INTR: Self = Self::from_errno(errno::EINTR);
    /// `EINVAL`
    pub const INVAL: Self = Self::from_errno(errno::EINVAL);
    /// `EIO`
    pub const IO: Self = Self::from_errno(errno::EIO);
    /// `EISCONN`
    pub const ISCONN: Self = Self::from_errno(errno::EISCONN);
    /// `EISDIR`
    pub const ISDIR: Self = Self::from_errno(errno::EISDIR);
    /// `EISNAM`
    pub const ISNAM: Self = Self::from_errno(errno::EISNAM);
    /// `EKEYEXPIRED`
    pub const KEYEXPIRED: Self = Self::from_errno(errno::EKEYEXPIRED);
    /// `EKEYREJECTED`
    pub const KEYREJECTED: Self = Self::from_errno(errno::EKEYREJECTED);
    /// `EKEYREVOKED`
    pub const KEYREVOKED: Self = Self::from_errno(errno::EKEYREVOKED);
    /// `EL2HLT`
    pub const L2HLT: Self = Self::from_errno(errno::EL2HLT);
    /// `EL2NSYNC`
    pub const L2NSYNC: Self = Self::from_errno(errno::EL2NSYNC);
    /// `EL3HLT`
    pub const L3HLT: Self = Self::from_errno(errno::EL3HLT);
    /// `EL3RST`
    pub const L3RST: Self = Self::from_errno(errno::EL3RST);
    /// `ELIBACC`
    pub const LIBACC: Self = Self::from_errno(errno::ELIBACC);
    /// `ELIBBAD`
    pub const LIBBAD: Self = Self::from_errno(errno::ELIBBAD);
    /// `ELIBEXEC`
    pub const LIBEXEC: Self = Self::from_errno(errno::ELIBEXEC);
    /// `ELIBMAX`
    pub const LIBMAX: Self = Self::from_errno(errno::ELIBMAX);
    /// `ELIBSCN`
    pub const LIBSCN: Self = Self::from_errno(errno::ELIBSCN);
    /// `ELNRNG`
    pub const LNRNG: Self = Self::from_errno(errno::ELNRNG);
    /// `ELOOP`
    pub const LOOP: Self = Self::from_errno(errno::ELOOP);
    /// `EMEDIUMTYPE`
    pub const MEDIUMTYPE: Self = Self::from_errno(errno::EMEDIUMTYPE);
    /// `EMFILE`
    pub const MFILE: Self = Self::from_errno(errno::EMFILE);
    /// `EMLINK`
    pub const MLINK: Self = Self::from_errno(errno::EMLINK);
    /// `EMSGSIZE`
    pub const MSGSIZE: Self = Self::from_errno(errno::EMSGSIZE);
    /// `EMULTIHOP`
    pub const MULTIHOP: Self = Self::from_errno(errno::EMULTIHOP);
    /// `ENAMETOOLONG`
    pub const NAMETOOLONG: Self = Self::from_errno(errno::ENAMETOOLONG);
    /// `ENAVAIL`
    pub const NAVAIL: Self = Self::from_errno(errno::ENAVAIL);
    /// `ENETDOWN`
    pub const NETDOWN: Self = Self::from_errno(errno::ENETDOWN);
    /// `ENETRESET`
    pub const NETRESET: Self = Self::from_errno(errno::ENETRESET);
    /// `ENETUNREACH`
    pub const NETUNREACH: Self = Self::from_errno(errno::ENETUNREACH);
    /// `ENFILE`
    pub const NFILE: Self = Self::from_errno(errno::ENFILE);
    /// `ENOANO`
    pub const NOANO: Self = Self::from_errno(errno::ENOANO);
    /// `ENOBUFS`
    pub const NOBUFS: Self = Self::from_errno(errno::ENOBUFS);
    /// `ENOCSI`
    pub const NOCSI: Self = Self::from_errno(errno::ENOCSI);
    /// `ENODATA`
    #[doc(alias = "NOATTR")]
    pub const NODATA: Self = Self::from_errno(errno::ENODATA);
    /// `ENODEV`
    pub const NODEV: Self = Self::from_errno(errno::ENODEV);
    /// `ENOENT`
    pub const NOENT: Self = Self::from_errno(errno::ENOENT);
    /// `ENOEXEC`
    pub const NOEXEC: Self = Self::from_errno(errno::ENOEXEC);
    /// `ENOKEY`
    pub const NOKEY: Self = Self::from_errno(errno::ENOKEY);
    /// `ENOLCK`
    pub const NOLCK: Self = Self::from_errno(errno::ENOLCK);
    /// `ENOLINK`
    pub const NOLINK: Self = Self::from_errno(errno::ENOLINK);
    /// `ENOMEDIUM`
    pub const NOMEDIUM: Self = Self::from_errno(errno::ENOMEDIUM);
    /// `ENOMEM`
    pub const NOMEM: Self = Self::from_errno(errno::ENOMEM);
    /// `ENOMSG`
    pub const NOMSG: Self = Self::from_errno(errno::ENOMSG);
    /// `ENONET`
    pub const NONET: Self = Self::from_errno(errno::ENONET);
    /// `ENOPKG`
    pub const NOPKG: Self = Self::from_errno(errno::ENOPKG);
    /// `ENOPROTOOPT`
    pub const NOPROTOOPT: Self = Self::from_errno(errno::ENOPROTOOPT);
    /// `ENOSPC`
    pub const NOSPC: Self = Self::from_errno(errno::ENOSPC);
    /// `ENOSR`
    pub const NOSR: Self = Self::from_errno(errno::ENOSR);
    /// `ENOSTR`
    pub const NOSTR: Self = Self::from_errno(errno::ENOSTR);
    /// `ENOSYS`
    pub const NOSYS: Self = Self::from_errno(errno::ENOSYS);
    /// `ENOTBLK`
    pub const NOTBLK: Self = Self::from_errno(errno::ENOTBLK);
    /// `ENOTCONN`
    pub const NOTCONN: Self = Self::from_errno(errno::ENOTCONN);
    /// `ENOTDIR`
    pub const NOTDIR: Self = Self::from_errno(errno::ENOTDIR);
    /// `ENOTEMPTY`
    pub const NOTEMPTY: Self = Self::from_errno(errno::ENOTEMPTY);
    /// `ENOTNAM`
    pub const NOTNAM: Self = Self::from_errno(errno::ENOTNAM);
    /// `ENOTRECOVERABLE`
    pub const NOTRECOVERABLE: Self = Self::from_errno(errno::ENOTRECOVERABLE);
    /// `ENOTSOCK`
    pub const NOTSOCK: Self = Self::from_errno(errno::ENOTSOCK);
    /// `ENOTSUP`
    // On Linux, `ENOTSUP` has the same value as `EOPNOTSUPP`.
    pub const NOTSUP: Self = Self::from_errno(errno::EOPNOTSUPP);
    /// `ENOTTY`
    pub const NOTTY: Self = Self::from_errno(errno::ENOTTY);
    /// `ENOTUNIQ`
    pub const NOTUNIQ: Self = Self::from_errno(errno::ENOTUNIQ);
    /// `ENXIO`
    pub const NXIO: Self = Self::from_errno(errno::ENXIO);
    /// `EOPNOTSUPP`
    pub const OPNOTSUPP: Self = Self::from_errno(errno::EOPNOTSUPP);
    /// `EOVERFLOW`
    pub const OVERFLOW: Self = Self::from_errno(errno::EOVERFLOW);
    /// `EOWNERDEAD`
    pub const OWNERDEAD: Self = Self::from_errno(errno::EOWNERDEAD);
    /// `EPERM`
    pub const PERM: Self = Self::from_errno(errno::EPERM);
    /// `EPFNOSUPPORT`
    pub const PFNOSUPPORT: Self = Self::from_errno(errno::EPFNOSUPPORT);
    /// `EPIPE`
    pub const PIPE: Self = Self::from_errno(errno::EPIPE);
    /// `EPROTO`
    pub const PROTO: Self = Self::from_errno(errno::EPROTO);
    /// `EPROTONOSUPPORT`
    pub const PROTONOSUPPORT: Self = Self::from_errno(errno::EPROTONOSUPPORT);
    /// `EPROTOTYPE`
    pub const PROTOTYPE: Self = Self::from_errno(errno::EPROTOTYPE);
    /// `ERANGE`
    pub const RANGE: Self = Self::from_errno(errno::ERANGE);
    /// `EREMCHG`
    pub const REMCHG: Self = Self::from_errno(errno::EREMCHG);
    /// `EREMOTE`
    pub const REMOTE: Self = Self::from_errno(errno::EREMOTE);
    /// `EREMOTEIO`
    pub const REMOTEIO: Self = Self::from_errno(errno::EREMOTEIO);
    /// `ERESTART`
    pub const RESTART: Self = Self::from_errno(errno::ERESTART);
    /// `ERFKILL`
    pub const RFKILL: Self = Self::from_errno(errno::ERFKILL);
    /// `EROFS`
    pub const ROFS: Self = Self::from_errno(errno::EROFS);
    /// `ESHUTDOWN`
    pub const SHUTDOWN: Self = Self::from_errno(errno::ESHUTDOWN);
    /// `ESOCKTNOSUPPORT`
    pub const SOCKTNOSUPPORT: Self = Self::from_errno(errno::ESOCKTNOSUPPORT);
    /// `ESPIPE`
    pub const SPIPE: Self = Self::from_errno(errno::ESPIPE);
    /// `ESRCH`
    pub const SRCH: Self = Self::from_errno(errno::ESRCH);
    /// `ESRMNT`
    pub const SRMNT: Self = Self::from_errno(errno::ESRMNT);
    /// `ESTALE`
    pub const STALE: Self = Self::from_errno(errno::ESTALE);
    /// `ESTRPIPE`
    pub const STRPIPE: Self = Self::from_errno(errno::ESTRPIPE);
    /// `ETIME`
    pub const TIME: Self = Self::from_errno(errno::ETIME);
    /// `ETIMEDOUT`
    pub const TIMEDOUT: Self = Self::from_errno(errno::ETIMEDOUT);
    /// `E2BIG`
    #[doc(alias = "2BIG")]
    pub const TOOBIG: Self = Self::from_errno(errno::E2BIG);
    /// `ETOOMANYREFS`
    pub const TOOMANYREFS: Self = Self::from_errno(errno::ETOOMANYREFS);
    /// `ETXTBSY`
    pub const TXTBSY: Self = Self::from_errno(errno::ETXTBSY);
    /// `EUCLEAN`
    pub const UCLEAN: Self = Self::from_errno(errno::EUCLEAN);
    /// `EUNATCH`
    pub const UNATCH: Self = Self::from_errno(errno::EUNATCH);
    /// `EUSERS`
    pub const USERS: Self = Self::from_errno(errno::EUSERS);
    /// `EWOULDBLOCK`
    pub const WOULDBLOCK: Self = Self::from_errno(errno::EWOULDBLOCK);
    /// `EXDEV`
    pub const XDEV: Self = Self::from_errno(errno::EXDEV);
    /// `EXFULL`
    pub const XFULL: Self = Self::from_errno(errno::EXFULL);
}
