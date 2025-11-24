//! linux_raw syscalls supporting `rustix::termios`.
//!
//! # Safety
//!
//! See the `rustix::backend` module documentation for details.
#![allow(unsafe_code, clippy::undocumented_unsafe_blocks)]

use crate::backend::c;
use crate::backend::conv::{by_ref, c_uint, ret};
use crate::fd::BorrowedFd;
#[cfg(feature = "alloc")]
use crate::ffi::CStr;
use crate::io;
use crate::pid::Pid;
use crate::termios::{
    speed, Action, ControlModes, InputModes, LocalModes, OptionalActions, OutputModes,
    QueueSelector, SpecialCodeIndex, Termios, Winsize,
};
#[cfg(feature = "alloc")]
#[cfg(feature = "fs")]
use crate::{fs::FileType, path::DecInt};
use core::mem::MaybeUninit;

#[inline]
pub(crate) fn tcgetwinsize(fd: BorrowedFd<'_>) -> io::Result<Winsize> {
    unsafe {
        let mut result = MaybeUninit::<Winsize>::uninit();
        ret(syscall!(__NR_ioctl, fd, c_uint(c::TIOCGWINSZ), &mut result))?;
        Ok(result.assume_init())
    }
}

#[inline]
pub(crate) fn tcgetattr(fd: BorrowedFd<'_>) -> io::Result<Termios> {
    let mut result = MaybeUninit::<Termios>::uninit();

    // SAFETY: This invokes the `TCGETS2` ioctl, which initializes the full
    // `Termios` structure.
    unsafe {
        match ret(syscall!(__NR_ioctl, fd, c_uint(c::TCGETS2), &mut result)) {
            Ok(()) => Ok(result.assume_init()),

            // A `NOTTY` or `ACCESS` might mean the OS doesn't support
            // `TCGETS2`, for example a seccomp environment or WSL that only
            // knows about `TCGETS`. Fall back to the old `TCGETS`.
            #[cfg(not(any(target_arch = "powerpc", target_arch = "powerpc64")))]
            Err(io::Errno::NOTTY) | Err(io::Errno::ACCESS) => tcgetattr_fallback(fd),

            Err(err) => Err(err),
        }
    }
}

/// Implement `tcgetattr` using the old `TCGETS` ioctl.
#[cfg(not(any(target_arch = "powerpc", target_arch = "powerpc64")))]
#[cold]
fn tcgetattr_fallback(fd: BorrowedFd<'_>) -> io::Result<Termios> {
    use core::ptr::{addr_of, addr_of_mut};

    let mut result = MaybeUninit::<Termios>::uninit();

    // SAFETY: This invokes the `TCGETS` ioctl which initializes the `Termios`
    // structure except for the `input_speed` and `output_speed` fields, which
    // we manually initialize before forming a reference to the full `Termios`.
    unsafe {
        // Do the old `TCGETS` call.
        ret(syscall!(__NR_ioctl, fd, c_uint(c::TCGETS), &mut result))?;

        // Read the `control_modes` field without forming a reference to the
        // `Termios` because it isn't fully initialized yet.
        let ptr = result.as_mut_ptr();
        let control_modes = addr_of!((*ptr).control_modes).read();

        // Infer the output speed and set `output_speed`.
        let encoded_out = control_modes.bits() & c::CBAUD;
        let output_speed = match speed::decode(encoded_out) {
            Some(output_speed) => output_speed,
            None => return Err(io::Errno::RANGE),
        };
        addr_of_mut!((*ptr).output_speed).write(output_speed);

        // Infer the input speed and set `input_speed`. `B0` is a special-case
        // that means the input speed is the same as the output speed.
        let encoded_in = (control_modes.bits() & c::CIBAUD) >> c::IBSHIFT;
        let input_speed = if encoded_in == c::B0 {
            output_speed
        } else {
            match speed::decode(encoded_in) {
                Some(input_speed) => input_speed,
                None => return Err(io::Errno::RANGE),
            }
        };
        addr_of_mut!((*ptr).input_speed).write(input_speed);

        // Now all the fields are set.
        Ok(result.assume_init())
    }
}

#[inline]
pub(crate) fn tcgetpgrp(fd: BorrowedFd<'_>) -> io::Result<Pid> {
    unsafe {
        let mut result = MaybeUninit::<c::pid_t>::uninit();
        ret(syscall!(__NR_ioctl, fd, c_uint(c::TIOCGPGRP), &mut result))?;
        let pid = result.assume_init();

        // This doesn't appear to be documented, but it appears `tcsetpgrp` can
        // succeed and set the pid to 0 if we pass it a pseudo-terminal device
        // fd. For now, fail with `OPNOTSUPP`.
        if pid == 0 {
            return Err(io::Errno::OPNOTSUPP);
        }

        Ok(Pid::from_raw_unchecked(pid))
    }
}

#[inline]
pub(crate) fn tcsetattr(
    fd: BorrowedFd<'_>,
    optional_actions: OptionalActions,
    termios: &Termios,
) -> io::Result<()> {
    // Translate from `optional_actions` into a `TCSETS2` ioctl request code.
    // On MIPS, `optional_actions` has `TCSETS` added to it.
    let request = c::TCSETS2
        + if cfg!(any(
            target_arch = "mips",
            target_arch = "mips32r6",
            target_arch = "mips64",
            target_arch = "mips64r6"
        )) {
            optional_actions as u32 - c::TCSETS
        } else {
            optional_actions as u32
        };

    // SAFETY: This invokes the `TCSETS2` ioctl.
    unsafe {
        match ret(syscall_readonly!(
            __NR_ioctl,
            fd,
            c_uint(request),
            by_ref(termios)
        )) {
            Ok(()) => Ok(()),

            // Similar to `tcgetattr_fallback`, `NOTTY` or `ACCESS` might mean
            // the OS doesn't support `TCSETS2`. Fall back to the old `TCSETS`.
            #[cfg(not(any(target_arch = "powerpc", target_arch = "powerpc64")))]
            Err(io::Errno::NOTTY) | Err(io::Errno::ACCESS) => {
                tcsetattr_fallback(fd, optional_actions, termios)
            }

            Err(err) => Err(err),
        }
    }
}

/// Implement `tcsetattr` using the old `TCSETS` ioctl.
#[cfg(not(any(target_arch = "powerpc", target_arch = "powerpc64")))]
#[cold]
fn tcsetattr_fallback(
    fd: BorrowedFd<'_>,
    optional_actions: OptionalActions,
    termios: &Termios,
) -> io::Result<()> {
    // `TCSETS` silently accepts `BOTHER` in `c_cflag` even though it doesn't
    // read `c_ispeed`/`c_ospeed`, so detect this case and fail if needed.
    let control_modes_bits = termios.control_modes.bits();
    let encoded_out = control_modes_bits & c::CBAUD;
    let encoded_in = (control_modes_bits & c::CIBAUD) >> c::IBSHIFT;
    if encoded_out == c::BOTHER || encoded_in == c::BOTHER {
        return Err(io::Errno::RANGE);
    }

    // Translate from `optional_actions` into a `TCSETS` ioctl request code. On
    // MIPS, `optional_actions` already has `TCSETS` added to it.
    let request = if cfg!(any(
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "mips64",
        target_arch = "mips64r6"
    )) {
        optional_actions as u32
    } else {
        optional_actions as u32 + c::TCSETS
    };

    // SAFETY: This invokes the `TCSETS` ioctl.
    unsafe {
        ret(syscall_readonly!(
            __NR_ioctl,
            fd,
            c_uint(request),
            by_ref(termios)
        ))
    }
}

#[inline]
pub(crate) fn tcsendbreak(fd: BorrowedFd<'_>) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_ioctl,
            fd,
            c_uint(c::TCSBRK),
            c_uint(0)
        ))
    }
}

#[inline]
pub(crate) fn tcdrain(fd: BorrowedFd<'_>) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_ioctl,
            fd,
            c_uint(c::TCSBRK),
            c_uint(1)
        ))
    }
}

#[inline]
pub(crate) fn tcflush(fd: BorrowedFd<'_>, queue_selector: QueueSelector) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_ioctl,
            fd,
            c_uint(c::TCFLSH),
            c_uint(queue_selector as u32)
        ))
    }
}

#[inline]
pub(crate) fn tcflow(fd: BorrowedFd<'_>, action: Action) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_ioctl,
            fd,
            c_uint(c::TCXONC),
            c_uint(action as u32)
        ))
    }
}

#[inline]
pub(crate) fn tcgetsid(fd: BorrowedFd<'_>) -> io::Result<Pid> {
    unsafe {
        let mut result = MaybeUninit::<c::pid_t>::uninit();
        ret(syscall!(__NR_ioctl, fd, c_uint(c::TIOCGSID), &mut result))?;
        let pid = result.assume_init();
        Ok(Pid::from_raw_unchecked(pid))
    }
}

#[inline]
pub(crate) fn tcsetwinsize(fd: BorrowedFd<'_>, winsize: Winsize) -> io::Result<()> {
    unsafe {
        ret(syscall_readonly!(
            __NR_ioctl,
            fd,
            c_uint(c::TIOCSWINSZ),
            by_ref(&winsize)
        ))
    }
}

#[inline]
pub(crate) fn tcsetpgrp(fd: BorrowedFd<'_>, pid: Pid) -> io::Result<()> {
    let raw_pid: c::c_int = pid.as_raw_nonzero().get();
    unsafe {
        ret(syscall_readonly!(
            __NR_ioctl,
            fd,
            c_uint(c::TIOCSPGRP),
            by_ref(&raw_pid)
        ))
    }
}

/// A wrapper around a conceptual `cfsetspeed` which handles an arbitrary
/// integer speed value.
#[inline]
pub(crate) fn set_speed(termios: &mut Termios, arbitrary_speed: u32) -> io::Result<()> {
    let encoded_speed = speed::encode(arbitrary_speed).unwrap_or(c::BOTHER);

    debug_assert_eq!(encoded_speed & !c::CBAUD, 0);

    termios.control_modes -= ControlModes::from_bits_retain(c::CBAUD | c::CIBAUD);
    termios.control_modes |=
        ControlModes::from_bits_retain(encoded_speed | (encoded_speed << c::IBSHIFT));

    termios.input_speed = arbitrary_speed;
    termios.output_speed = arbitrary_speed;

    Ok(())
}

/// A wrapper around a conceptual `cfsetospeed` which handles an arbitrary
/// integer speed value.
#[inline]
pub(crate) fn set_output_speed(termios: &mut Termios, arbitrary_speed: u32) -> io::Result<()> {
    let encoded_speed = speed::encode(arbitrary_speed).unwrap_or(c::BOTHER);

    debug_assert_eq!(encoded_speed & !c::CBAUD, 0);

    termios.control_modes -= ControlModes::from_bits_retain(c::CBAUD);
    termios.control_modes |= ControlModes::from_bits_retain(encoded_speed);

    termios.output_speed = arbitrary_speed;

    Ok(())
}

/// A wrapper around a conceptual `cfsetispeed` which handles an arbitrary
/// integer speed value.
#[inline]
pub(crate) fn set_input_speed(termios: &mut Termios, arbitrary_speed: u32) -> io::Result<()> {
    let encoded_speed = speed::encode(arbitrary_speed).unwrap_or(c::BOTHER);

    debug_assert_eq!(encoded_speed & !c::CBAUD, 0);

    termios.control_modes -= ControlModes::from_bits_retain(c::CIBAUD);
    termios.control_modes |= ControlModes::from_bits_retain(encoded_speed << c::IBSHIFT);

    termios.input_speed = arbitrary_speed;

    Ok(())
}

#[inline]
pub(crate) fn cfmakeraw(termios: &mut Termios) {
    // From the Linux [`cfmakeraw` manual page]:
    //
    // [`cfmakeraw` manual page]: https://man7.org/linux/man-pages/man3/cfmakeraw.3.html
    termios.input_modes -= InputModes::IGNBRK
        | InputModes::BRKINT
        | InputModes::PARMRK
        | InputModes::ISTRIP
        | InputModes::INLCR
        | InputModes::IGNCR
        | InputModes::ICRNL
        | InputModes::IXON;
    termios.output_modes -= OutputModes::OPOST;
    termios.local_modes -= LocalModes::ECHO
        | LocalModes::ECHONL
        | LocalModes::ICANON
        | LocalModes::ISIG
        | LocalModes::IEXTEN;
    termios.control_modes -= ControlModes::CSIZE | ControlModes::PARENB;
    termios.control_modes |= ControlModes::CS8;

    // Musl and glibc also do these:
    termios.special_codes[SpecialCodeIndex::VMIN] = 1;
    termios.special_codes[SpecialCodeIndex::VTIME] = 0;
}

#[inline]
pub(crate) fn isatty(fd: BorrowedFd<'_>) -> bool {
    // On error, Linux will return either `EINVAL` (2.6.32) or `ENOTTY`
    // (otherwise), because we assume we're never passing an invalid
    // file descriptor (which would get `EBADF`). Either way, an error
    // means we don't have a tty.
    tcgetwinsize(fd).is_ok()
}

#[cfg(feature = "alloc")]
#[cfg(feature = "fs")]
pub(crate) fn ttyname(fd: BorrowedFd<'_>, buf: &mut [MaybeUninit<u8>]) -> io::Result<usize> {
    let fd_stat = crate::backend::fs::syscalls::fstat(fd)?;

    // Quick check: if `fd` isn't a character device, it's not a tty.
    if FileType::from_raw_mode(fd_stat.st_mode) != FileType::CharacterDevice {
        return Err(io::Errno::NOTTY);
    }

    // Check that `fd` is really a tty.
    tcgetwinsize(fd)?;

    // Create the "/proc/self/fd/<fd>" string.
    let mut proc_self_fd_buf: [u8; 25] = *b"/proc/self/fd/\0\0\0\0\0\0\0\0\0\0\0";
    let dec_int = DecInt::from_fd(fd);
    let bytes_with_nul = dec_int.as_bytes_with_nul();
    proc_self_fd_buf[b"/proc/self/fd/".len()..][..bytes_with_nul.len()]
        .copy_from_slice(bytes_with_nul);

    // SAFETY: We just wrote a valid C String.
    let proc_self_fd_path = unsafe { CStr::from_ptr(proc_self_fd_buf.as_ptr().cast()) };

    let ptr = buf.as_mut_ptr();
    let len = {
        // Gather the ttyname by reading the "fd" file inside `proc_self_fd`.
        let (init, uninit) = crate::fs::readlinkat_raw(crate::fs::CWD, proc_self_fd_path, buf)?;

        // If the number of bytes is equal to the buffer length, truncation may
        // have occurred. This check also ensures that we have enough space for
        // adding a NUL terminator.
        if uninit.is_empty() {
            return Err(io::Errno::RANGE);
        }

        // `readlinkat` returns the number of bytes placed in the buffer.
        // NUL-terminate the string at that offset.
        uninit[0].write(b'\0');

        init.len()
    };

    // Check that the path we read refers to the same file as `fd`.
    {
        // SAFETY: We just wrote the NUL byte above.
        let path = unsafe { CStr::from_ptr(ptr.cast()) };

        let path_stat = crate::backend::fs::syscalls::stat(path)?;
        if path_stat.st_dev != fd_stat.st_dev || path_stat.st_ino != fd_stat.st_ino {
            return Err(io::Errno::NODEV);
        }
    }

    // Return the length, excluding the NUL terminator.
    Ok(len)
}
