//! libc syscalls supporting `rustix::termios`.
//!
//! # Safety
//!
//! See the `rustix::backend::syscalls` module documentation for details.

use crate::backend::c;
#[cfg(not(target_os = "wasi"))]
use crate::backend::conv::ret_pid_t;
use crate::backend::conv::{borrowed_fd, ret};
use crate::fd::BorrowedFd;
#[cfg(feature = "alloc")]
#[cfg(not(any(target_os = "fuchsia", target_os = "wasi")))]
use crate::ffi::CStr;
#[cfg(any(
    not(target_os = "espidf"),
    not(any(target_os = "fuchsia", target_os = "wasi"))
))]
use core::mem::MaybeUninit;
#[cfg(not(target_os = "wasi"))]
use {crate::io, crate::pid::Pid};
#[cfg(not(any(target_os = "espidf", target_os = "wasi")))]
use {
    crate::termios::{Action, OptionalActions, QueueSelector, Termios, Winsize},
    crate::utils::as_mut_ptr,
};

#[cfg(not(any(target_os = "espidf", target_os = "wasi")))]
pub(crate) fn tcgetattr(fd: BorrowedFd<'_>) -> io::Result<Termios> {
    // On Linux, use `TCGETS2`, and fall back to `TCGETS` if needed.
    #[cfg(linux_kernel)]
    {
        use crate::termios::{ControlModes, InputModes, LocalModes, OutputModes, SpecialCodes};

        let mut termios2 = MaybeUninit::<c::termios2>::uninit();
        let ptr = termios2.as_mut_ptr();

        // SAFETY: This invokes the `TCGETS2` ioctl, which initializes the full
        // `Termios` structure.
        let termios2 = unsafe {
            match ret(c::ioctl(borrowed_fd(fd), c::TCGETS2 as _, ptr)) {
                Ok(()) => {}

                // A `NOTTY` or `ACCESS` might mean the OS doesn't support
                // `TCGETS2`, for example a seccomp environment or WSL that
                // only knows about `TCGETS`. Fall back to the old `TCGETS`.
                #[cfg(not(any(target_arch = "powerpc", target_arch = "powerpc64")))]
                Err(io::Errno::NOTTY) | Err(io::Errno::ACCESS) => {
                    tcgetattr_fallback(fd, &mut termios2)?
                }

                Err(err) => return Err(err),
            }

            // Now all the fields are set.
            termios2.assume_init()
        };

        // Convert from the Linux `termios2` to our `Termios`.
        let mut result = Termios {
            input_modes: InputModes::from_bits_retain(termios2.c_iflag),
            output_modes: OutputModes::from_bits_retain(termios2.c_oflag),
            control_modes: ControlModes::from_bits_retain(termios2.c_cflag),
            local_modes: LocalModes::from_bits_retain(termios2.c_lflag),
            line_discipline: termios2.c_line,
            special_codes: SpecialCodes(Default::default()),

            // On PowerPC musl targets, `c_ispeed`/`c_ospeed` are named
            // `__c_ispeed`/`__c_ospeed`.
            #[cfg(not(all(
                target_env = "musl",
                any(target_arch = "powerpc", target_arch = "powerpc64")
            )))]
            input_speed: termios2.c_ispeed,
            #[cfg(not(all(
                target_env = "musl",
                any(target_arch = "powerpc", target_arch = "powerpc64")
            )))]
            output_speed: termios2.c_ospeed,
            #[cfg(all(
                target_env = "musl",
                any(target_arch = "powerpc", target_arch = "powerpc64")
            ))]
            input_speed: termios2.__c_ispeed,
            #[cfg(all(
                target_env = "musl",
                any(target_arch = "powerpc", target_arch = "powerpc64")
            ))]
            output_speed: termios2.__c_ospeed,
        };

        // Copy in the control codes, since libc's `c_cc` array may have a
        // different length from the ioctl's.
        let nccs = termios2.c_cc.len();
        result.special_codes.0[..nccs].copy_from_slice(&termios2.c_cc);

        Ok(result)
    }

    #[cfg(not(linux_kernel))]
    unsafe {
        let mut result = MaybeUninit::<Termios>::uninit();

        // `result` is a `Termios` which starts with the same layout as
        // `c::termios`, so we can cast the pointer.
        ret(c::tcgetattr(borrowed_fd(fd), result.as_mut_ptr().cast()))?;

        Ok(result.assume_init())
    }
}

/// Implement `tcgetattr` using the old `TCGETS` ioctl.
#[cfg(all(
    linux_kernel,
    not(any(target_arch = "powerpc", target_arch = "powerpc64"))
))]
#[cold]
fn tcgetattr_fallback(
    fd: BorrowedFd<'_>,
    termios: &mut MaybeUninit<c::termios2>,
) -> io::Result<()> {
    use crate::termios::speed;
    use core::ptr::{addr_of, addr_of_mut};

    // SAFETY: This invokes the `TCGETS` ioctl, which, if it succeeds,
    // initializes the `Termios` structure except for the `input_speed` and
    // `output_speed` fields, which we manually initialize before forming a
    // reference to the full `Termios`.
    unsafe {
        let ptr = termios.as_mut_ptr();

        // Do the old `TCGETS` call, which doesn't initialize `input_speed` or
        // `output_speed`.
        ret(c::ioctl(borrowed_fd(fd), c::TCGETS as _, ptr))?;

        // Read the `control_modes` field without forming a reference to the
        // `Termios` because it isn't fully initialized yet.
        let control_modes = addr_of!((*ptr).c_cflag).read();

        // Infer `output_speed`.
        let encoded_out = control_modes & c::CBAUD;
        let output_speed = match speed::decode(encoded_out) {
            Some(output_speed) => output_speed,
            None => return Err(io::Errno::RANGE),
        };
        addr_of_mut!((*ptr).c_ospeed).write(output_speed);

        // Infer `input_speed`. For input speeds, `B0` is special-cased to mean
        // the input speed is the same as the output speed.
        let encoded_in = (control_modes & c::CIBAUD) >> c::IBSHIFT;
        let input_speed = if encoded_in == c::B0 {
            output_speed
        } else {
            match speed::decode(encoded_in) {
                Some(input_speed) => input_speed,
                None => return Err(io::Errno::RANGE),
            }
        };
        addr_of_mut!((*ptr).c_ispeed).write(input_speed);
    }

    Ok(())
}

#[cfg(not(target_os = "wasi"))]
pub(crate) fn tcgetpgrp(fd: BorrowedFd<'_>) -> io::Result<Pid> {
    unsafe {
        let pid = ret_pid_t(c::tcgetpgrp(borrowed_fd(fd)))?;

        // This doesn't appear to be documented, but on Linux, it appears
        // `tcsetpgrp` can succeed and set the pid to 0 if we pass it a
        // pseudo-terminal device fd. For now, translate it into `OPNOTSUPP`.
        #[cfg(linux_kernel)]
        if pid == 0 {
            return Err(io::Errno::OPNOTSUPP);
        }

        Ok(Pid::from_raw_unchecked(pid))
    }
}

#[cfg(not(target_os = "wasi"))]
pub(crate) fn tcsetpgrp(fd: BorrowedFd<'_>, pid: Pid) -> io::Result<()> {
    unsafe { ret(c::tcsetpgrp(borrowed_fd(fd), pid.as_raw_nonzero().get())) }
}

#[cfg(not(any(target_os = "espidf", target_os = "wasi")))]
pub(crate) fn tcsetattr(
    fd: BorrowedFd<'_>,
    optional_actions: OptionalActions,
    termios: &Termios,
) -> io::Result<()> {
    // On Linux, use `TCSETS2`, and fall back to `TCSETS` if needed.
    #[cfg(linux_kernel)]
    {
        use crate::termios::speed;

        let output_speed = termios.output_speed();
        let input_speed = termios.input_speed();

        let mut termios2 = c::termios2 {
            c_iflag: termios.input_modes.bits(),
            c_oflag: termios.output_modes.bits(),
            c_cflag: termios.control_modes.bits(),
            c_lflag: termios.local_modes.bits(),
            c_line: termios.line_discipline,
            c_cc: Default::default(),

            // On PowerPC musl targets, `c_ispeed`/`c_ospeed` are named
            // `__c_ispeed`/`__c_ospeed`.
            #[cfg(not(all(
                target_env = "musl",
                any(target_arch = "powerpc", target_arch = "powerpc64")
            )))]
            c_ispeed: input_speed,
            #[cfg(not(all(
                target_env = "musl",
                any(target_arch = "powerpc", target_arch = "powerpc64")
            )))]
            c_ospeed: output_speed,
            #[cfg(all(
                target_env = "musl",
                any(target_arch = "powerpc", target_arch = "powerpc64")
            ))]
            __c_ispeed: input_speed,
            #[cfg(all(
                target_env = "musl",
                any(target_arch = "powerpc", target_arch = "powerpc64")
            ))]
            __c_ospeed: output_speed,
        };

        // Ensure that our input and output speeds are set, as `libc`
        // routines don't always support setting these separately.
        termios2.c_cflag &= !c::CBAUD;
        termios2.c_cflag |= speed::encode(output_speed).unwrap_or(c::BOTHER);
        termios2.c_cflag &= !c::CIBAUD;
        termios2.c_cflag |= speed::encode(input_speed).unwrap_or(c::BOTHER) << c::IBSHIFT;

        // Copy in the control codes, since libc's `c_cc` array may have a
        // different length from the ioctl's.
        let nccs = termios2.c_cc.len();
        termios2
            .c_cc
            .copy_from_slice(&termios.special_codes.0[..nccs]);

        // Translate from `optional_actions` into a `TCSETS2` ioctl request
        // code. On MIPS, `optional_actions` has `TCSETS` added to it.
        let request = c::TCSETS2 as c::c_ulong
            + if cfg!(any(
                target_arch = "mips",
                target_arch = "mips32r6",
                target_arch = "mips64",
                target_arch = "mips64r6"
            )) {
                optional_actions as c::c_ulong - c::TCSETS as c::c_ulong
            } else {
                optional_actions as c::c_ulong
            };

        // SAFETY: This invokes the `TCSETS2` ioctl.
        unsafe {
            match ret(c::ioctl(borrowed_fd(fd), request as _, &termios2)) {
                Ok(()) => Ok(()),

                // Similar to `tcgetattr_fallback`, `NOTTY` or `ACCESS` might
                // mean the OS doesn't support `TCSETS2`. Fall back to the old
                // `TCSETS`.
                #[cfg(not(any(target_arch = "powerpc", target_arch = "powerpc64")))]
                Err(io::Errno::NOTTY) | Err(io::Errno::ACCESS) => {
                    tcsetattr_fallback(fd, optional_actions, &termios2)
                }

                Err(err) => Err(err),
            }
        }
    }

    #[cfg(not(linux_kernel))]
    unsafe {
        ret(c::tcsetattr(
            borrowed_fd(fd),
            optional_actions as _,
            crate::utils::as_ptr(termios).cast(),
        ))
    }
}

/// Implement `tcsetattr` using the old `TCSETS` ioctl.
#[cfg(all(
    linux_kernel,
    not(any(target_arch = "powerpc", target_arch = "powerpc64"))
))]
#[cold]
fn tcsetattr_fallback(
    fd: BorrowedFd<'_>,
    optional_actions: OptionalActions,
    termios2: &c::termios2,
) -> io::Result<()> {
    // `TCSETS` silently accepts `BOTHER` in `c_cflag` even though it doesn't
    // read `c_ispeed`/`c_ospeed`, so detect this case and fail if needed.
    let encoded_out = termios2.c_cflag & c::CBAUD;
    let encoded_in = (termios2.c_cflag & c::CIBAUD) >> c::IBSHIFT;
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
        optional_actions as c::c_ulong
    } else {
        optional_actions as c::c_ulong + c::TCSETS as c::c_ulong
    };

    // SAFETY: This invokes the `TCSETS` ioctl.
    unsafe { ret(c::ioctl(borrowed_fd(fd), request as _, termios2)) }
}

#[cfg(not(target_os = "wasi"))]
pub(crate) fn tcsendbreak(fd: BorrowedFd<'_>) -> io::Result<()> {
    unsafe { ret(c::tcsendbreak(borrowed_fd(fd), 0)) }
}

#[cfg(not(any(target_os = "espidf", target_os = "wasi")))]
pub(crate) fn tcdrain(fd: BorrowedFd<'_>) -> io::Result<()> {
    unsafe { ret(c::tcdrain(borrowed_fd(fd))) }
}

#[cfg(not(any(target_os = "espidf", target_os = "wasi")))]
pub(crate) fn tcflush(fd: BorrowedFd<'_>, queue_selector: QueueSelector) -> io::Result<()> {
    unsafe { ret(c::tcflush(borrowed_fd(fd), queue_selector as _)) }
}

#[cfg(not(any(target_os = "espidf", target_os = "wasi")))]
pub(crate) fn tcflow(fd: BorrowedFd<'_>, action: Action) -> io::Result<()> {
    unsafe { ret(c::tcflow(borrowed_fd(fd), action as _)) }
}

#[cfg(not(target_os = "wasi"))]
pub(crate) fn tcgetsid(fd: BorrowedFd<'_>) -> io::Result<Pid> {
    unsafe {
        let pid = ret_pid_t(c::tcgetsid(borrowed_fd(fd)))?;
        Ok(Pid::from_raw_unchecked(pid))
    }
}

#[cfg(not(any(target_os = "espidf", target_os = "horizon", target_os = "wasi")))]
pub(crate) fn tcsetwinsize(fd: BorrowedFd<'_>, winsize: Winsize) -> io::Result<()> {
    unsafe { ret(c::ioctl(borrowed_fd(fd), c::TIOCSWINSZ, &winsize)) }
}

#[cfg(not(any(target_os = "espidf", target_os = "horizon", target_os = "wasi")))]
pub(crate) fn tcgetwinsize(fd: BorrowedFd<'_>) -> io::Result<Winsize> {
    unsafe {
        let mut buf = MaybeUninit::<Winsize>::uninit();
        ret(c::ioctl(
            borrowed_fd(fd),
            c::TIOCGWINSZ.into(),
            buf.as_mut_ptr(),
        ))?;
        Ok(buf.assume_init())
    }
}

#[cfg(not(any(target_os = "espidf", target_os = "nto", target_os = "wasi")))]
#[inline]
pub(crate) fn set_speed(termios: &mut Termios, arbitrary_speed: u32) -> io::Result<()> {
    #[cfg(bsd)]
    let encoded_speed = arbitrary_speed;

    #[cfg(not(bsd))]
    let encoded_speed = match crate::termios::speed::encode(arbitrary_speed) {
        Some(encoded_speed) => encoded_speed,
        #[cfg(linux_kernel)]
        None => c::BOTHER,
        #[cfg(not(linux_kernel))]
        None => return Err(io::Errno::INVAL),
    };

    #[cfg(not(linux_kernel))]
    unsafe {
        ret(c::cfsetspeed(
            as_mut_ptr(termios).cast(),
            encoded_speed.into(),
        ))
    }

    // Linux libc implementations don't support arbitrary speeds, so we encode
    // the speed manually.
    #[cfg(linux_kernel)]
    {
        use crate::termios::ControlModes;

        debug_assert_eq!(encoded_speed & !c::CBAUD, 0);

        termios.control_modes -= ControlModes::from_bits_retain(c::CBAUD | c::CIBAUD);
        termios.control_modes |=
            ControlModes::from_bits_retain(encoded_speed | (encoded_speed << c::IBSHIFT));

        termios.input_speed = arbitrary_speed;
        termios.output_speed = arbitrary_speed;

        Ok(())
    }
}

#[cfg(not(any(target_os = "espidf", target_os = "wasi")))]
#[inline]
pub(crate) fn set_output_speed(termios: &mut Termios, arbitrary_speed: u32) -> io::Result<()> {
    #[cfg(bsd)]
    let encoded_speed = arbitrary_speed;

    #[cfg(not(bsd))]
    let encoded_speed = match crate::termios::speed::encode(arbitrary_speed) {
        Some(encoded_speed) => encoded_speed,
        #[cfg(linux_kernel)]
        None => c::BOTHER,
        #[cfg(not(linux_kernel))]
        None => return Err(io::Errno::INVAL),
    };

    #[cfg(not(linux_kernel))]
    unsafe {
        ret(c::cfsetospeed(
            as_mut_ptr(termios).cast(),
            encoded_speed.into(),
        ))
    }

    // Linux libc implementations don't support arbitrary speeds or setting the
    // input and output speeds separately, so we encode the speed manually.
    #[cfg(linux_kernel)]
    {
        use crate::termios::ControlModes;

        debug_assert_eq!(encoded_speed & !c::CBAUD, 0);

        termios.control_modes -= ControlModes::from_bits_retain(c::CBAUD);
        termios.control_modes |= ControlModes::from_bits_retain(encoded_speed);

        termios.output_speed = arbitrary_speed;

        Ok(())
    }
}

#[cfg(not(any(target_os = "espidf", target_os = "wasi")))]
#[inline]
pub(crate) fn set_input_speed(termios: &mut Termios, arbitrary_speed: u32) -> io::Result<()> {
    #[cfg(bsd)]
    let encoded_speed = arbitrary_speed;

    #[cfg(not(bsd))]
    let encoded_speed = match crate::termios::speed::encode(arbitrary_speed) {
        Some(encoded_speed) => encoded_speed,
        #[cfg(linux_kernel)]
        None => c::BOTHER,
        #[cfg(not(linux_kernel))]
        None => return Err(io::Errno::INVAL),
    };

    #[cfg(not(linux_kernel))]
    unsafe {
        ret(c::cfsetispeed(
            as_mut_ptr(termios).cast(),
            encoded_speed.into(),
        ))
    }

    // Linux libc implementations don't support arbitrary speeds or setting the
    // input and output speeds separately, so we encode the speed manually.
    #[cfg(linux_kernel)]
    {
        use crate::termios::ControlModes;

        debug_assert_eq!(encoded_speed & !c::CBAUD, 0);

        termios.control_modes -= ControlModes::from_bits_retain(c::CIBAUD);
        termios.control_modes |= ControlModes::from_bits_retain(encoded_speed << c::IBSHIFT);

        termios.input_speed = arbitrary_speed;

        Ok(())
    }
}

#[cfg(not(any(target_os = "espidf", target_os = "nto", target_os = "wasi")))]
#[inline]
pub(crate) fn cfmakeraw(termios: &mut Termios) {
    unsafe { c::cfmakeraw(as_mut_ptr(termios).cast()) }
}

pub(crate) fn isatty(fd: BorrowedFd<'_>) -> bool {
    // Use the return value of `isatty` alone. We don't check `errno` because
    // we return `bool` rather than `io::Result<bool>`, because we assume
    // `BorrowedFd` protects us from `EBADF`, and any other reasonably
    // anticipated `errno` value would end up interpreted as “assume it's not a
    // terminal” anyway.
    unsafe { c::isatty(borrowed_fd(fd)) != 0 }
}

#[cfg(feature = "alloc")]
#[cfg(not(any(target_os = "fuchsia", target_os = "wasi")))]
pub(crate) fn ttyname(dirfd: BorrowedFd<'_>, buf: &mut [MaybeUninit<u8>]) -> io::Result<usize> {
    unsafe {
        // `ttyname_r` returns its error status rather than using `errno`.
        match c::ttyname_r(borrowed_fd(dirfd), buf.as_mut_ptr().cast(), buf.len()) {
            0 => Ok(CStr::from_ptr(buf.as_ptr().cast()).to_bytes().len()),
            err => Err(io::Errno::from_raw_os_error(err)),
        }
    }
}
