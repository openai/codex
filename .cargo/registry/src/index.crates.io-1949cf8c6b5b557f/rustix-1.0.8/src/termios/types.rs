use crate::backend::c;
use crate::backend::termios::types;
#[cfg(target_os = "nto")]
use crate::ffi;
use crate::{backend, io};
use bitflags::bitflags;

/// `struct termios` for use with [`tcgetattr`] and [`tcsetattr`].
///
/// [`tcgetattr`]: crate::termios::tcgetattr
/// [`tcsetattr`]: crate::termios::tcsetattr
#[repr(C)]
#[derive(Clone)]
pub struct Termios {
    /// How is input interpreted?
    #[doc(alias = "c_iflag")]
    pub input_modes: InputModes,

    /// How is output translated?
    #[doc(alias = "c_oflag")]
    pub output_modes: OutputModes,

    /// Low-level configuration flags.
    #[doc(alias = "c_cflag")]
    pub control_modes: ControlModes,

    /// High-level configuration flags.
    #[doc(alias = "c_lflag")]
    pub local_modes: LocalModes,

    /// Line discipline.
    #[doc(alias = "c_line")]
    #[cfg(not(all(linux_raw, any(target_arch = "powerpc", target_arch = "powerpc64"))))]
    #[cfg(any(
        linux_like,
        target_env = "newlib",
        target_os = "fuchsia",
        target_os = "haiku",
        target_os = "redox"
    ))]
    pub line_discipline: u8,

    /// How are various special control codes handled?
    #[doc(alias = "c_cc")]
    #[cfg(not(target_os = "haiku"))]
    pub special_codes: SpecialCodes,

    #[cfg(target_os = "nto")]
    pub(crate) __reserved: [ffi::c_uint; 3],

    /// Line discipline.
    // On PowerPC, this field comes after `c_cc`.
    #[doc(alias = "c_line")]
    #[cfg(all(linux_raw, any(target_arch = "powerpc", target_arch = "powerpc64")))]
    pub line_discipline: c::cc_t,

    /// See the `input_speed` and `set_input_seed` functions.
    ///
    /// On Linux and BSDs, this is the arbitrary integer speed value. On all
    /// other platforms, this is the encoded speed value.
    #[cfg(not(any(solarish, all(libc, target_env = "newlib"), target_os = "aix")))]
    pub(crate) input_speed: c::speed_t,

    /// See the `output_speed` and `set_output_seed` functions.
    ///
    /// On Linux and BSDs, this is the integer speed value. On all other
    /// platforms, this is the encoded speed value.
    #[cfg(not(any(solarish, all(libc, target_env = "newlib"), target_os = "aix")))]
    pub(crate) output_speed: c::speed_t,

    /// How are various special control codes handled?
    #[doc(alias = "c_cc")]
    #[cfg(target_os = "haiku")]
    pub special_codes: SpecialCodes,
}

impl Termios {
    /// `cfmakeraw(self)`—Set a `Termios` value to the settings for “raw” mode.
    ///
    /// In raw mode, input is available a byte at a time, echoing is disabled,
    /// and special terminal input and output codes are disabled.
    #[cfg(not(target_os = "nto"))]
    #[doc(alias = "cfmakeraw")]
    #[inline]
    pub fn make_raw(&mut self) {
        backend::termios::syscalls::cfmakeraw(self)
    }

    /// Return the input communication speed.
    ///
    /// Unlike the `c_ispeed` field in glibc and others, this returns the
    /// integer value of the speed, rather than the `B*` encoded constant
    /// value.
    #[doc(alias = "c_ispeed")]
    #[doc(alias = "cfgetispeed")]
    #[doc(alias = "cfgetspeed")]
    #[inline]
    pub fn input_speed(&self) -> u32 {
        // On Linux and BSDs, `input_speed` is the arbitrary integer speed.
        #[cfg(any(linux_kernel, bsd))]
        {
            debug_assert!(u32::try_from(self.input_speed).is_ok());
            self.input_speed as u32
        }

        // On illumos, `input_speed` is not present.
        #[cfg(any(solarish, all(libc, target_env = "newlib"), target_os = "aix"))]
        unsafe {
            speed::decode(c::cfgetispeed(crate::utils::as_ptr(self).cast())).unwrap()
        }

        // On other platforms, it's the encoded speed.
        #[cfg(not(any(
            linux_kernel,
            bsd,
            solarish,
            all(libc, target_env = "newlib"),
            target_os = "aix"
        )))]
        {
            speed::decode(self.input_speed).unwrap()
        }
    }

    /// Return the output communication speed.
    ///
    /// Unlike the `c_ospeed` field in glibc and others, this returns the
    /// arbitrary integer value of the speed, rather than the `B*` encoded
    /// constant value.
    #[inline]
    pub fn output_speed(&self) -> u32 {
        // On Linux and BSDs, `output_speed` is the arbitrary integer speed.
        #[cfg(any(linux_kernel, bsd))]
        {
            debug_assert!(u32::try_from(self.output_speed).is_ok());
            self.output_speed as u32
        }

        // On illumos, `output_speed` is not present.
        #[cfg(any(solarish, all(libc, target_env = "newlib"), target_os = "aix"))]
        unsafe {
            speed::decode(c::cfgetospeed(crate::utils::as_ptr(self).cast())).unwrap()
        }

        // On other platforms, it's the encoded speed.
        #[cfg(not(any(
            linux_kernel,
            bsd,
            solarish,
            all(libc, target_env = "newlib"),
            target_os = "aix"
        )))]
        {
            speed::decode(self.output_speed).unwrap()
        }
    }

    /// Set the input and output communication speeds.
    ///
    /// Unlike the `c_ispeed` and `c_ospeed` fields in glibc and others, this
    /// takes the arbitrary integer value of the speed, rather than the `B*`
    /// encoded constant value. Not all implementations support all integer
    /// values; use the constants in the [`speed`] module for likely-supported
    /// speeds.
    #[cfg(not(target_os = "nto"))]
    #[doc(alias = "cfsetspeed")]
    #[doc(alias = "CBAUD")]
    #[doc(alias = "CBAUDEX")]
    #[doc(alias = "CIBAUD")]
    #[doc(alias = "CIBAUDEX")]
    #[inline]
    pub fn set_speed(&mut self, new_speed: u32) -> io::Result<()> {
        backend::termios::syscalls::set_speed(self, new_speed)
    }

    /// Set the input communication speed.
    ///
    /// Unlike the `c_ispeed` field in glibc and others, this takes the
    /// arbitrary integer value of the speed, rather than the `B*` encoded
    /// constant value. Not all implementations support all integer values; use
    /// the constants in the [`speed`] module for known-supported speeds.
    ///
    /// On some platforms, changing the input speed changes the output speed to
    /// the same speed.
    #[doc(alias = "c_ispeed")]
    #[doc(alias = "cfsetispeed")]
    #[doc(alias = "CIBAUD")]
    #[doc(alias = "CIBAUDEX")]
    #[inline]
    pub fn set_input_speed(&mut self, new_speed: u32) -> io::Result<()> {
        backend::termios::syscalls::set_input_speed(self, new_speed)
    }

    /// Set the output communication speed.
    ///
    /// Unlike the `c_ospeed` field in glibc and others, this takes the
    /// arbitrary integer value of the speed, rather than the `B*` encoded
    /// constant value. Not all implementations support all integer values; use
    /// the constants in the [`speed`] module for known-supported speeds.
    ///
    /// On some platforms, changing the output speed changes the input speed to
    /// the same speed.
    #[doc(alias = "c_ospeed")]
    #[doc(alias = "cfsetospeed")]
    #[doc(alias = "CBAUD")]
    #[doc(alias = "CBAUDEX")]
    #[inline]
    pub fn set_output_speed(&mut self, new_speed: u32) -> io::Result<()> {
        backend::termios::syscalls::set_output_speed(self, new_speed)
    }
}

impl core::fmt::Debug for Termios {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut d = f.debug_struct("Termios");
        d.field("input_modes", &self.input_modes);
        d.field("output_modes", &self.output_modes);

        // This includes any bits set in the `CBAUD` and `CIBAUD` ranges, which
        // is a little ugly, because we also decode those bits for the speeds
        // below. However, it seems better to print them here than to hide
        // them, because hiding them would make the `Termios` debug output
        // appear to disagree with the `ControlModes` debug output for the same
        // value, which could be confusing.
        d.field("control_modes", &self.control_modes);

        d.field("local_modes", &self.local_modes);
        #[cfg(any(
            linux_like,
            target_env = "newlib",
            target_os = "fuchsia",
            target_os = "haiku",
            target_os = "redox"
        ))]
        {
            d.field("line_discipline", &SpecialCode(self.line_discipline));
        }
        d.field("special_codes", &self.special_codes);
        d.field("input_speed", &self.input_speed());
        d.field("output_speed", &self.output_speed());
        d.finish()
    }
}

bitflags! {
    /// Flags controlling terminal input.
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct InputModes: types::tcflag_t {
        /// `IGNBRK`
        const IGNBRK = c::IGNBRK;

        /// `BRKINT`
        const BRKINT = c::BRKINT;

        /// `IGNPAR`
        const IGNPAR = c::IGNPAR;

        /// `PARMRK`
        const PARMRK = c::PARMRK;

        /// `INPCK`
        const INPCK = c::INPCK;

        /// `ISTRIP`
        const ISTRIP = c::ISTRIP;

        /// `INLCR`
        const INLCR = c::INLCR;

        /// `IGNCR`
        const IGNCR = c::IGNCR;

        /// `ICRNL`
        const ICRNL = c::ICRNL;

        /// `IUCLC`
        #[cfg(any(linux_kernel, solarish, target_os = "aix", target_os = "haiku", target_os = "nto"))]
        const IUCLC = c::IUCLC;

        /// `IXON`
        const IXON = c::IXON;

        /// `IXANY`
        #[cfg(not(target_os = "redox"))]
        const IXANY = c::IXANY;

        /// `IXOFF`
        const IXOFF = c::IXOFF;

        /// `IMAXBEL`
        #[cfg(not(any(target_os = "haiku", target_os = "redox")))]
        const IMAXBEL = c::IMAXBEL;

        /// `IUTF8`
        #[cfg(not(any(
            freebsdlike,
            netbsdlike,
            solarish,
            target_os = "aix",
            target_os = "emscripten",
            target_os = "haiku",
            target_os = "hurd",
            target_os = "redox",
        )))]
        const IUTF8 = c::IUTF8;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags! {
    /// Flags controlling terminal output.
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct OutputModes: types::tcflag_t {
        /// `OPOST`
        const OPOST = c::OPOST;

        /// `OLCUC`
        #[cfg(not(any(
            apple,
            freebsdlike,
            target_os = "aix",
            target_os = "netbsd",
            target_os = "redox",
        )))]
        const OLCUC = c::OLCUC;

        /// `ONLCR`
        const ONLCR = c::ONLCR;

        /// `OCRNL`
        const OCRNL = c::OCRNL;

        /// `ONOCR`
        const ONOCR = c::ONOCR;

        /// `ONLRET`
        const ONLRET = c::ONLRET;

        /// `OFILL`
        #[cfg(not(bsd))]
        const OFILL = c::OFILL;

        /// `OFDEL`
        #[cfg(not(bsd))]
        const OFDEL = c::OFDEL;

        /// `NLDLY`
        #[cfg(not(any(bsd, solarish, target_os = "redox")))]
        const NLDLY = c::NLDLY;

        /// `NL0`
        #[cfg(not(any(bsd, solarish, target_os = "fuchsia", target_os = "redox")))]
        const NL0 = c::NL0;

        /// `NL1`
        #[cfg(not(any(bsd, solarish, target_os = "fuchsia", target_os = "redox")))]
        const NL1 = c::NL1;

        /// `CRDLY`
        #[cfg(not(any(bsd, solarish, target_os = "redox")))]
        const CRDLY = c::CRDLY;

        /// `CR0`
        #[cfg(not(any(bsd, solarish, target_os = "fuchsia", target_os = "redox")))]
        const CR0 = c::CR0;

        /// `CR1`
        #[cfg(not(any(
            target_env = "musl",
            bsd,
            solarish,
            target_os = "emscripten",
            target_os = "fuchsia",
            target_os = "redox",
        )))]
        const CR1 = c::CR1;

        /// `CR2`
        #[cfg(not(any(
            target_env = "musl",
            bsd,
            solarish,
            target_os = "emscripten",
            target_os = "fuchsia",
            target_os = "redox",
        )))]
        const CR2 = c::CR2;

        /// `CR3`
        #[cfg(not(any(
            target_env = "musl",
            bsd,
            solarish,
            target_os = "emscripten",
            target_os = "fuchsia",
            target_os = "redox",
        )))]
        const CR3 = c::CR3;

        /// `TABDLY`
        #[cfg(not(any(
            netbsdlike,
            solarish,
            target_os = "dragonfly",
            target_os = "redox",
        )))]
        const TABDLY = c::TABDLY;

        /// `TAB0`
        #[cfg(not(any(
            netbsdlike,
            solarish,
            target_os = "dragonfly",
            target_os = "fuchsia",
            target_os = "redox",
        )))]
        const TAB0 = c::TAB0;

        /// `TAB1`
        #[cfg(not(any(
            target_env = "musl",
            bsd,
            solarish,
            target_os = "emscripten",
            target_os = "fuchsia",
            target_os = "redox",
        )))]
        const TAB1 = c::TAB1;

        /// `TAB2`
        #[cfg(not(any(
            target_env = "musl",
            bsd,
            solarish,
            target_os = "emscripten",
            target_os = "fuchsia",
            target_os = "redox",
        )))]
        const TAB2 = c::TAB2;

        /// `TAB3`
        #[cfg(not(any(
            target_env = "musl",
            bsd,
            solarish,
            target_os = "emscripten",
            target_os = "fuchsia",
            target_os = "redox",
        )))]
        const TAB3 = c::TAB3;

        /// `XTABS`
        #[cfg(not(any(
            bsd,
            solarish,
            target_os = "aix",
            target_os = "haiku",
            target_os = "redox",
        )))]
        const XTABS = c::XTABS;

        /// `BSDLY`
        #[cfg(not(any(bsd, solarish, target_os = "redox")))]
        const BSDLY = c::BSDLY;

        /// `BS0`
        #[cfg(not(any(bsd, solarish, target_os = "fuchsia", target_os = "redox")))]
        const BS0 = c::BS0;

        /// `BS1`
        #[cfg(not(any(
            target_env = "musl",
            bsd,
            solarish,
            target_os = "emscripten",
            target_os = "fuchsia",
            target_os = "redox",
        )))]
        const BS1 = c::BS1;

        /// `FFDLY`
        #[cfg(not(any(bsd, solarish, target_os = "redox")))]
        const FFDLY = c::FFDLY;

        /// `FF0`
        #[cfg(not(any(bsd, solarish, target_os = "fuchsia", target_os = "redox")))]
        const FF0 = c::FF0;

        /// `FF1`
        #[cfg(not(any(
            target_env = "musl",
            bsd,
            solarish,
            target_os = "emscripten",
            target_os = "fuchsia",
            target_os = "redox",
        )))]
        const FF1 = c::FF1;

        /// `VTDLY`
        #[cfg(not(any(bsd, solarish, target_os = "redox")))]
        const VTDLY = c::VTDLY;

        /// `VT0`
        #[cfg(not(any(bsd, solarish, target_os = "fuchsia", target_os = "redox")))]
        const VT0 = c::VT0;

        /// `VT1`
        #[cfg(not(any(
            target_env = "musl",
            bsd,
            solarish,
            target_os = "emscripten",
            target_os = "fuchsia",
            target_os = "redox",
        )))]
        const VT1 = c::VT1;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags! {
    /// Flags controlling special terminal modes.
    ///
    /// `CBAUD`, `CBAUDEX`, `CIBAUD`, `CIBAUDEX`, and various `B*` speed
    /// constants are often included in the control modes, however rustix
    /// handles them separately, in [`Termios::set_speed`] and related
    /// functions. If you see extra bits in the `Debug` output, they're
    /// probably these flags.
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct ControlModes: types::tcflag_t {
        /// `CSIZE`
        const CSIZE = c::CSIZE;

        /// `CS5`
        const CS5 = c::CS5;

        /// `CS6`
        const CS6 = c::CS6;

        /// `CS7`
        const CS7 = c::CS7;

        /// `CS8`
        const CS8 = c::CS8;

        /// `CSTOPB`
        const CSTOPB = c::CSTOPB;

        /// `CREAD`
        const CREAD = c::CREAD;

        /// `PARENB`
        const PARENB = c::PARENB;

        /// `PARODD`
        const PARODD = c::PARODD;

        /// `HUPCL`
        const HUPCL = c::HUPCL;

        /// `CLOCAL`
        const CLOCAL = c::CLOCAL;

        /// `CRTSCTS`
        #[cfg(not(any(target_os = "aix", target_os = "nto", target_os = "redox")))]
        const CRTSCTS = c::CRTSCTS;

        /// `CMSPAR`
        #[cfg(not(any(
            bsd,
            solarish,
            target_os = "aix",
            target_os = "emscripten",
            target_os = "haiku",
            target_os = "hurd",
            target_os = "nto",
            target_os = "redox",
        )))]
        const CMSPAR = c::CMSPAR;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

bitflags! {
    /// Flags controlling “local” terminal modes.
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct LocalModes: types::tcflag_t {
        /// `XCASE`
        #[cfg(any(linux_kernel, target_arch = "s390x", target_os = "haiku"))]
        const XCASE = c::XCASE;

        /// `ECHOCTL`
        #[cfg(not(target_os = "redox"))]
        const ECHOCTL = c::ECHOCTL;

        /// `ECHOPRT`
        #[cfg(not(any(target_os = "cygwin", target_os = "nto", target_os = "redox")))]
        const ECHOPRT = c::ECHOPRT;

        /// `ECHOKE`
        #[cfg(not(target_os = "redox"))]
        const ECHOKE = c::ECHOKE;

        /// `FLUSHO`
        #[cfg(not(any(target_os = "nto", target_os = "redox")))]
        const FLUSHO = c::FLUSHO;

        /// `PENDIN`
        #[cfg(not(any(target_os = "cygwin", target_os = "nto", target_os = "redox")))]
        const PENDIN = c::PENDIN;

        /// `EXTPROC`
        #[cfg(not(any(
            target_os = "aix",
            target_os = "cygwin",
            target_os = "haiku",
            target_os = "nto",
            target_os = "redox",
        )))]
        const EXTPROC = c::EXTPROC;

        /// `ISIG`
        const ISIG = c::ISIG;

        /// `ICANON`—A flag for the `c_lflag` field of [`Termios`] indicating
        /// canonical mode.
        const ICANON = c::ICANON;

        /// `ECHO`
        const ECHO = c::ECHO;

        /// `ECHOE`
        const ECHOE = c::ECHOE;

        /// `ECHOK`
        const ECHOK = c::ECHOK;

        /// `ECHONL`
        const ECHONL = c::ECHONL;

        /// `NOFLSH`
        const NOFLSH = c::NOFLSH;

        /// `TOSTOP`
        const TOSTOP = c::TOSTOP;

        /// `IEXTEN`
        const IEXTEN = c::IEXTEN;

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

/// Speeds for use with [`Termios::set_speed`], [`Termios::set_input_speed`],
/// and [`Termios::set_output_speed`].
///
/// Unlike in some platforms' libc APIs, these always have the same numerical
/// value as their names; for example, `B50` has the value `50`, and so on.
/// Consequently, it's not necessary to use them. They are provided here
/// because they help identify speeds which are likely to be supported, on
/// platforms and devices which don't support arbitrary speeds.
pub mod speed {
    #[cfg(not(bsd))]
    use crate::backend::c;

    /// `B0`
    pub const B0: u32 = 0;

    /// `B50`
    pub const B50: u32 = 50;

    /// `B75`
    pub const B75: u32 = 75;

    /// `B110`
    pub const B110: u32 = 110;

    /// `B134`
    pub const B134: u32 = 134;

    /// `B150`
    pub const B150: u32 = 150;

    /// `B200`
    pub const B200: u32 = 200;

    /// `B300`
    pub const B300: u32 = 300;

    /// `B600`
    pub const B600: u32 = 600;

    /// `B1200`
    pub const B1200: u32 = 1200;

    /// `B1800`
    pub const B1800: u32 = 1800;

    /// `B2400`
    pub const B2400: u32 = 2400;

    /// `B4800`
    pub const B4800: u32 = 4800;

    /// `B9600`
    pub const B9600: u32 = 9600;

    /// `B19200`
    #[doc(alias = "EXTA")]
    pub const B19200: u32 = 19200;

    /// `B38400`
    #[doc(alias = "EXTB")]
    pub const B38400: u32 = 38400;

    /// `B57600`
    #[cfg(not(target_os = "aix"))]
    pub const B57600: u32 = 57600;

    /// `B115200`
    #[cfg(not(target_os = "aix"))]
    pub const B115200: u32 = 115_200;

    /// `B230400`
    #[cfg(not(target_os = "aix"))]
    pub const B230400: u32 = 230_400;

    /// `B460800`
    #[cfg(not(any(
        apple,
        target_os = "aix",
        target_os = "dragonfly",
        target_os = "haiku",
        target_os = "openbsd"
    )))]
    pub const B460800: u32 = 460_800;

    /// `B500000`
    #[cfg(not(any(bsd, solarish, target_os = "aix", target_os = "haiku")))]
    pub const B500000: u32 = 500_000;

    /// `B576000`
    #[cfg(not(any(bsd, solarish, target_os = "aix", target_os = "haiku")))]
    pub const B576000: u32 = 576_000;

    /// `B921600`
    #[cfg(not(any(
        apple,
        target_os = "aix",
        target_os = "dragonfly",
        target_os = "haiku",
        target_os = "openbsd"
    )))]
    pub const B921600: u32 = 921_600;

    /// `B1000000`
    #[cfg(not(any(bsd, target_os = "aix", target_os = "haiku", target_os = "solaris")))]
    pub const B1000000: u32 = 1_000_000;

    /// `B1152000`
    #[cfg(not(any(bsd, target_os = "aix", target_os = "haiku", target_os = "solaris")))]
    pub const B1152000: u32 = 1_152_000;

    /// `B1500000`
    #[cfg(not(any(bsd, target_os = "aix", target_os = "haiku", target_os = "solaris")))]
    pub const B1500000: u32 = 1_500_000;

    /// `B2000000`
    #[cfg(not(any(bsd, target_os = "aix", target_os = "haiku", target_os = "solaris")))]
    pub const B2000000: u32 = 2_000_000;

    /// `B2500000`
    #[cfg(not(any(
        target_arch = "sparc",
        target_arch = "sparc64",
        bsd,
        target_os = "aix",
        target_os = "haiku",
        target_os = "solaris",
    )))]
    pub const B2500000: u32 = 2_500_000;

    /// `B3000000`
    #[cfg(not(any(
        target_arch = "sparc",
        target_arch = "sparc64",
        bsd,
        target_os = "aix",
        target_os = "haiku",
        target_os = "solaris",
    )))]
    pub const B3000000: u32 = 3_000_000;

    /// `B3500000`
    #[cfg(not(any(
        target_arch = "sparc",
        target_arch = "sparc64",
        bsd,
        target_os = "aix",
        target_os = "haiku",
        target_os = "solaris",
    )))]
    pub const B3500000: u32 = 3_500_000;

    /// `B4000000`
    #[cfg(not(any(
        target_arch = "sparc",
        target_arch = "sparc64",
        bsd,
        target_os = "aix",
        target_os = "haiku",
        target_os = "solaris",
    )))]
    pub const B4000000: u32 = 4_000_000;

    /// Translate from a `c::speed_t` code to an arbitrary integer speed value
    /// `u32`.
    ///
    /// On BSD platforms, integer speed values are already the same as their
    /// encoded values.
    ///
    /// On Linux on PowerPC, `TCGETS`/`TCSETS` support the `c_ispeed` and
    /// `c_ospeed` fields.
    ///
    /// On Linux on architectures other than PowerPC, `TCGETS`/`TCSETS` don't
    /// support the `c_ispeed` and `c_ospeed` fields, so we have to fall back
    /// to `TCGETS2`/`TCSETS2` to support them.
    #[cfg(not(any(
        bsd,
        all(linux_kernel, any(target_arch = "powerpc", target_arch = "powerpc64"))
    )))]
    pub(crate) const fn decode(encoded_speed: c::speed_t) -> Option<u32> {
        match encoded_speed {
            c::B0 => Some(0),
            c::B50 => Some(50),
            c::B75 => Some(75),
            c::B110 => Some(110),
            c::B134 => Some(134),
            c::B150 => Some(150),
            c::B200 => Some(200),
            c::B300 => Some(300),
            c::B600 => Some(600),
            c::B1200 => Some(1200),
            c::B1800 => Some(1800),
            c::B2400 => Some(2400),
            c::B4800 => Some(4800),
            c::B9600 => Some(9600),
            c::B19200 => Some(19200),
            c::B38400 => Some(38400),
            #[cfg(not(target_os = "aix"))]
            c::B57600 => Some(57600),
            #[cfg(not(target_os = "aix"))]
            c::B115200 => Some(115_200),
            #[cfg(not(any(target_os = "aix", target_os = "nto")))]
            c::B230400 => Some(230_400),
            #[cfg(not(any(
                apple,
                target_os = "aix",
                target_os = "dragonfly",
                target_os = "haiku",
                target_os = "nto",
                target_os = "openbsd"
            )))]
            c::B460800 => Some(460_800),
            #[cfg(not(any(
                bsd,
                solarish,
                target_os = "aix",
                target_os = "haiku",
                target_os = "nto"
            )))]
            c::B500000 => Some(500_000),
            #[cfg(not(any(
                bsd,
                solarish,
                target_os = "aix",
                target_os = "haiku",
                target_os = "nto"
            )))]
            c::B576000 => Some(576_000),
            #[cfg(not(any(
                apple,
                target_os = "aix",
                target_os = "dragonfly",
                target_os = "haiku",
                target_os = "nto",
                target_os = "openbsd"
            )))]
            c::B921600 => Some(921_600),
            #[cfg(not(any(
                bsd,
                target_os = "aix",
                target_os = "haiku",
                target_os = "nto",
                target_os = "solaris"
            )))]
            c::B1000000 => Some(1_000_000),
            #[cfg(not(any(
                bsd,
                target_os = "aix",
                target_os = "haiku",
                target_os = "nto",
                target_os = "solaris"
            )))]
            c::B1152000 => Some(1_152_000),
            #[cfg(not(any(
                bsd,
                target_os = "aix",
                target_os = "haiku",
                target_os = "nto",
                target_os = "solaris"
            )))]
            c::B1500000 => Some(1_500_000),
            #[cfg(not(any(
                bsd,
                target_os = "aix",
                target_os = "haiku",
                target_os = "nto",
                target_os = "solaris"
            )))]
            c::B2000000 => Some(2_000_000),
            #[cfg(not(any(
                target_arch = "sparc",
                target_arch = "sparc64",
                bsd,
                target_os = "aix",
                target_os = "haiku",
                target_os = "nto",
                target_os = "solaris",
            )))]
            c::B2500000 => Some(2_500_000),
            #[cfg(not(any(
                target_arch = "sparc",
                target_arch = "sparc64",
                bsd,
                target_os = "aix",
                target_os = "haiku",
                target_os = "nto",
                target_os = "solaris",
            )))]
            c::B3000000 => Some(3_000_000),
            #[cfg(not(any(
                target_arch = "sparc",
                target_arch = "sparc64",
                bsd,
                target_os = "aix",
                target_os = "cygwin",
                target_os = "haiku",
                target_os = "nto",
                target_os = "solaris",
            )))]
            c::B3500000 => Some(3_500_000),
            #[cfg(not(any(
                target_arch = "sparc",
                target_arch = "sparc64",
                bsd,
                target_os = "aix",
                target_os = "cygwin",
                target_os = "haiku",
                target_os = "nto",
                target_os = "solaris",
            )))]
            c::B4000000 => Some(4_000_000),
            _ => None,
        }
    }

    /// Translate from an arbitrary `u32` arbitrary integer speed value to a
    /// `c::speed_t` code.
    #[cfg(not(bsd))]
    pub(crate) const fn encode(speed: u32) -> Option<c::speed_t> {
        match speed {
            0 => Some(c::B0),
            50 => Some(c::B50),
            75 => Some(c::B75),
            110 => Some(c::B110),
            134 => Some(c::B134),
            150 => Some(c::B150),
            200 => Some(c::B200),
            300 => Some(c::B300),
            600 => Some(c::B600),
            1200 => Some(c::B1200),
            1800 => Some(c::B1800),
            2400 => Some(c::B2400),
            4800 => Some(c::B4800),
            9600 => Some(c::B9600),
            19200 => Some(c::B19200),
            38400 => Some(c::B38400),
            #[cfg(not(target_os = "aix"))]
            57600 => Some(c::B57600),
            #[cfg(not(target_os = "aix"))]
            115_200 => Some(c::B115200),
            #[cfg(not(any(target_os = "aix", target_os = "nto")))]
            230_400 => Some(c::B230400),
            #[cfg(not(any(
                apple,
                target_os = "aix",
                target_os = "dragonfly",
                target_os = "haiku",
                target_os = "nto",
                target_os = "openbsd",
            )))]
            460_800 => Some(c::B460800),
            #[cfg(not(any(
                bsd,
                solarish,
                target_os = "aix",
                target_os = "haiku",
                target_os = "nto"
            )))]
            500_000 => Some(c::B500000),
            #[cfg(not(any(
                bsd,
                solarish,
                target_os = "aix",
                target_os = "haiku",
                target_os = "nto"
            )))]
            576_000 => Some(c::B576000),
            #[cfg(not(any(
                apple,
                target_os = "aix",
                target_os = "dragonfly",
                target_os = "haiku",
                target_os = "nto",
                target_os = "openbsd"
            )))]
            921_600 => Some(c::B921600),
            #[cfg(not(any(
                bsd,
                target_os = "aix",
                target_os = "haiku",
                target_os = "nto",
                target_os = "solaris"
            )))]
            1_000_000 => Some(c::B1000000),
            #[cfg(not(any(
                bsd,
                target_os = "aix",
                target_os = "haiku",
                target_os = "nto",
                target_os = "solaris"
            )))]
            1_152_000 => Some(c::B1152000),
            #[cfg(not(any(
                bsd,
                target_os = "aix",
                target_os = "haiku",
                target_os = "nto",
                target_os = "solaris"
            )))]
            1_500_000 => Some(c::B1500000),
            #[cfg(not(any(
                bsd,
                target_os = "aix",
                target_os = "haiku",
                target_os = "nto",
                target_os = "solaris"
            )))]
            2_000_000 => Some(c::B2000000),
            #[cfg(not(any(
                target_arch = "sparc",
                target_arch = "sparc64",
                bsd,
                target_os = "aix",
                target_os = "haiku",
                target_os = "nto",
                target_os = "solaris",
            )))]
            2_500_000 => Some(c::B2500000),
            #[cfg(not(any(
                target_arch = "sparc",
                target_arch = "sparc64",
                bsd,
                target_os = "aix",
                target_os = "haiku",
                target_os = "nto",
                target_os = "solaris",
            )))]
            3_000_000 => Some(c::B3000000),
            #[cfg(not(any(
                target_arch = "sparc",
                target_arch = "sparc64",
                bsd,
                target_os = "aix",
                target_os = "cygwin",
                target_os = "haiku",
                target_os = "nto",
                target_os = "solaris",
            )))]
            3_500_000 => Some(c::B3500000),
            #[cfg(not(any(
                target_arch = "sparc",
                target_arch = "sparc64",
                bsd,
                target_os = "aix",
                target_os = "cygwin",
                target_os = "haiku",
                target_os = "nto",
                target_os = "solaris",
            )))]
            4_000_000 => Some(c::B4000000),
            _ => None,
        }
    }
}

/// An array indexed by [`SpecialCodeIndex`] indicating the current values of
/// various special control codes.
#[repr(transparent)]
#[derive(Clone)]
pub struct SpecialCodes(pub(crate) [c::cc_t; c::NCCS as usize]);

impl core::ops::Index<SpecialCodeIndex> for SpecialCodes {
    type Output = u8;

    fn index(&self, index: SpecialCodeIndex) -> &Self::Output {
        &self.0[index.0]
    }
}

impl core::ops::IndexMut<SpecialCodeIndex> for SpecialCodes {
    fn index_mut(&mut self, index: SpecialCodeIndex) -> &mut Self::Output {
        &mut self.0[index.0]
    }
}

impl core::fmt::Debug for SpecialCodes {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "SpecialCodes {{")?;
        let mut first = true;
        for i in 0..self.0.len() {
            if first {
                write!(f, " ")?;
            } else {
                write!(f, ", ")?;
            }
            first = false;
            let index = SpecialCodeIndex(i);
            write!(f, "{:?}: {:?}", index, SpecialCode(self[index]))?;
        }
        if !first {
            write!(f, " ")?;
        }
        write!(f, "}}")
    }
}

/// A newtype for pretty printing.
struct SpecialCode(u8);

impl core::fmt::Debug for SpecialCode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.0 == 0 {
            write!(f, "<undef>")
        } else if self.0 < 0x20 {
            write!(f, "^{}", (self.0 + 0x40) as char)
        } else if self.0 == 0x7f {
            write!(f, "^?")
        } else if self.0 >= 0x80 {
            write!(f, "M-")?;
            SpecialCode(self.0 - 0x80).fmt(f)
        } else {
            write!(f, "{}", (self.0 as char))
        }
    }
}

/// Indices for use with [`Termios::special_codes`].
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct SpecialCodeIndex(usize);

#[rustfmt::skip]
impl SpecialCodeIndex {
    /// `VINTR`
    pub const VINTR: Self = Self(c::VINTR as usize);

    /// `VQUIT`
    pub const VQUIT: Self = Self(c::VQUIT as usize);

    /// `VERASE`
    pub const VERASE: Self = Self(c::VERASE as usize);

    /// `VKILL`
    pub const VKILL: Self = Self(c::VKILL as usize);

    /// `VEOF`
    pub const VEOF: Self = Self(c::VEOF as usize);

    /// `VTIME`
    pub const VTIME: Self = Self(c::VTIME as usize);

    /// `VMIN`
    pub const VMIN: Self = Self(c::VMIN as usize);

    /// `VSWTC`
    #[cfg(not(any(
        bsd,
        solarish,
        target_os = "aix",
        target_os = "haiku",
        target_os = "hurd",
        target_os = "nto",
    )))]
    pub const VSWTC: Self = Self(c::VSWTC as usize);

    /// `VSTART`
    pub const VSTART: Self = Self(c::VSTART as usize);

    /// `VSTOP`
    pub const VSTOP: Self = Self(c::VSTOP as usize);

    /// `VSUSP`
    pub const VSUSP: Self = Self(c::VSUSP as usize);

    /// `VEOL`
    pub const VEOL: Self = Self(c::VEOL as usize);

    /// `VREPRINT`
    #[cfg(not(target_os = "haiku"))]
    pub const VREPRINT: Self = Self(c::VREPRINT as usize);

    /// `VDISCARD`
    #[cfg(not(any(target_os = "aix", target_os = "haiku")))]
    pub const VDISCARD: Self = Self(c::VDISCARD as usize);

    /// `VWERASE`
    #[cfg(not(any(target_os = "aix", target_os = "haiku")))]
    pub const VWERASE: Self = Self(c::VWERASE as usize);

    /// `VLNEXT`
    #[cfg(not(target_os = "haiku"))]
    pub const VLNEXT: Self = Self(c::VLNEXT as usize);

    /// `VEOL2`
    pub const VEOL2: Self = Self(c::VEOL2 as usize);

    /// `VSWTCH`
    #[cfg(any(solarish, target_os = "haiku", target_os = "nto"))]
    pub const VSWTCH: Self = Self(c::VSWTCH as usize);

    /// `VDSUSP`
    #[cfg(any(
        bsd,
        solarish,
        target_os = "aix",
        target_os = "hurd",
        target_os = "nto"
    ))]
    pub const VDSUSP: Self = Self(c::VDSUSP as usize);

    /// `VSTATUS`
    #[cfg(any(bsd, target_os = "hurd", target_os = "illumos"))]
    pub const VSTATUS: Self = Self(c::VSTATUS as usize);

    /// `VERASE2`
    #[cfg(any(freebsdlike, target_os = "illumos"))]
    pub const VERASE2: Self = Self(c::VERASE2 as usize);
}

impl core::fmt::Debug for SpecialCodeIndex {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {
            Self::VINTR => write!(f, "VINTR"),
            Self::VQUIT => write!(f, "VQUIT"),
            Self::VERASE => write!(f, "VERASE"),
            Self::VKILL => write!(f, "VKILL"),
            #[cfg(not(any(
                solarish,
                all(linux_kernel, any(target_arch = "sparc", target_arch = "sparc64")),
                target_os = "aix",
                target_os = "haiku",
            )))]
            Self::VEOF => write!(f, "VEOF"),
            #[cfg(not(any(
                solarish,
                all(linux_kernel, any(target_arch = "sparc", target_arch = "sparc64")),
                target_os = "aix",
                target_os = "haiku",
            )))]
            Self::VTIME => write!(f, "VTIME"),
            #[cfg(not(any(
                solarish,
                all(linux_kernel, any(target_arch = "sparc", target_arch = "sparc64")),
                target_os = "aix",
                target_os = "haiku",
            )))]
            Self::VMIN => write!(f, "VMIN"),

            // On Solarish platforms, Linux on SPARC, AIX, and Haiku, `VMIN`
            // and `VTIME` have the same value as `VEOF` and `VEOL`.
            #[cfg(any(
                solarish,
                all(linux_kernel, any(target_arch = "sparc", target_arch = "sparc64")),
                target_os = "aix",
                target_os = "haiku",
            ))]
            Self::VMIN => write!(f, "VMIN/VEOF"),
            #[cfg(any(
                solarish,
                all(linux_kernel, any(target_arch = "sparc", target_arch = "sparc64")),
                target_os = "aix",
                target_os = "haiku",
            ))]
            Self::VTIME => write!(f, "VTIME/VEOL"),

            #[cfg(not(any(
                bsd,
                solarish,
                target_os = "aix",
                target_os = "haiku",
                target_os = "hurd",
                target_os = "nto",
            )))]
            Self::VSWTC => write!(f, "VSWTC"),
            Self::VSTART => write!(f, "VSTART"),
            Self::VSTOP => write!(f, "VSTOP"),
            Self::VSUSP => write!(f, "VSUSP"),
            #[cfg(not(any(
                solarish,
                all(linux_kernel, any(target_arch = "sparc", target_arch = "sparc64")),
                target_os = "aix",
                target_os = "haiku",
            )))]
            Self::VEOL => write!(f, "VEOL"),
            #[cfg(not(target_os = "haiku"))]
            Self::VREPRINT => write!(f, "VREPRINT"),
            #[cfg(not(any(target_os = "aix", target_os = "haiku")))]
            Self::VDISCARD => write!(f, "VDISCARD"),
            #[cfg(not(any(target_os = "aix", target_os = "haiku")))]
            Self::VWERASE => write!(f, "VWERASE"),
            #[cfg(not(target_os = "haiku"))]
            Self::VLNEXT => write!(f, "VLNEXT"),
            Self::VEOL2 => write!(f, "VEOL2"),
            #[cfg(any(solarish, target_os = "haiku", target_os = "nto"))]
            Self::VSWTCH => write!(f, "VSWTCH"),
            #[cfg(any(
                bsd,
                solarish,
                target_os = "aix",
                target_os = "hurd",
                target_os = "nto"
            ))]
            Self::VDSUSP => write!(f, "VDSUSP"),
            #[cfg(any(bsd, target_os = "hurd", target_os = "illumos"))]
            Self::VSTATUS => write!(f, "VSTATUS"),
            #[cfg(any(freebsdlike, target_os = "illumos"))]
            Self::VERASE2 => write!(f, "VERASE2"),

            _ => write!(f, "unknown"),
        }
    }
}

/// `TCSA*` values for use with [`tcsetattr`].
///
/// [`tcsetattr`]: crate::termios::tcsetattr
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[repr(u32)]
pub enum OptionalActions {
    /// `TCSANOW`—Make the change immediately.
    #[doc(alias = "TCSANOW")]
    Now = c::TCSANOW as u32,

    /// `TCSADRAIN`—Make the change after all output has been transmitted.
    #[doc(alias = "TCSADRAIN")]
    Drain = c::TCSADRAIN as u32,

    /// `TCSAFLUSH`—Discard any pending input and then make the change
    /// after all output has been transmitted.
    #[doc(alias = "TCSAFLUSH")]
    Flush = c::TCSAFLUSH as u32,
}

/// `TC*` values for use with [`tcflush`].
///
/// [`tcflush`]: crate::termios::tcflush
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[repr(u32)]
pub enum QueueSelector {
    /// `TCIFLUSH`—Flush data received but not read.
    #[doc(alias = "TCIFLUSH")]
    IFlush = c::TCIFLUSH as u32,

    /// `TCOFLUSH`—Flush data written but not transmitted.
    #[doc(alias = "TCOFLUSH")]
    OFlush = c::TCOFLUSH as u32,

    /// `TCIOFLUSH`—`IFlush` and `OFlush` combined.
    #[doc(alias = "TCIOFLUSH")]
    IOFlush = c::TCIOFLUSH as u32,
}

/// `TC*` values for use with [`tcflow`].
///
/// [`tcflow`]: crate::termios::tcflow
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[repr(u32)]
pub enum Action {
    /// `TCOOFF`—Suspend output.
    #[doc(alias = "TCOOFF")]
    OOff = c::TCOOFF as u32,

    /// `TCOON`—Restart suspended output.
    #[doc(alias = "TCOON")]
    OOn = c::TCOON as u32,

    /// `TCIOFF`—Transmits a STOP byte.
    #[doc(alias = "TCIOFF")]
    IOff = c::TCIOFF as u32,

    /// `TCION`—Transmits a START byte.
    #[doc(alias = "TCION")]
    IOn = c::TCION as u32,
}

/// `struct winsize` for use with [`tcgetwinsize`].
///
/// [`tcgetwinsize`]: crate::termios::tcgetwinsize
#[doc(alias = "winsize")]
#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, Hash, PartialEq)]
#[allow(missing_docs)]
pub struct Winsize {
    /// The number of rows the terminal has.
    pub ws_row: u16,
    /// The number of columns the terminal has.
    pub ws_col: u16,

    pub ws_xpixel: u16,
    pub ws_ypixel: u16,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn termios_layouts() {
        check_renamed_type!(InputModes, tcflag_t);
        check_renamed_type!(OutputModes, tcflag_t);
        check_renamed_type!(ControlModes, tcflag_t);
        check_renamed_type!(LocalModes, tcflag_t);
        assert_eq_size!(u8, libc::cc_t);
        assert_eq_size!(types::tcflag_t, libc::tcflag_t);

        check_renamed_struct!(Winsize, winsize, ws_row, ws_col, ws_xpixel, ws_ypixel);

        // On platforms with a termios/termios2 split, check `termios`.
        #[cfg(linux_raw)]
        {
            check_renamed_type!(Termios, termios2);
            check_renamed_struct_renamed_field!(Termios, termios2, input_modes, c_iflag);
            check_renamed_struct_renamed_field!(Termios, termios2, output_modes, c_oflag);
            check_renamed_struct_renamed_field!(Termios, termios2, control_modes, c_cflag);
            check_renamed_struct_renamed_field!(Termios, termios2, local_modes, c_lflag);
            check_renamed_struct_renamed_field!(Termios, termios2, line_discipline, c_line);
            check_renamed_struct_renamed_field!(Termios, termios2, special_codes, c_cc);
            check_renamed_struct_renamed_field!(Termios, termios2, input_speed, c_ispeed);
            check_renamed_struct_renamed_field!(Termios, termios2, output_speed, c_ospeed);

            // We assume that `termios` has the same layout as `termios2` minus the
            // `c_ispeed` and `c_ospeed` fields.
            check_renamed_struct_renamed_field!(Termios, termios, input_modes, c_iflag);
            check_renamed_struct_renamed_field!(Termios, termios, output_modes, c_oflag);
            check_renamed_struct_renamed_field!(Termios, termios, control_modes, c_cflag);
            check_renamed_struct_renamed_field!(Termios, termios, local_modes, c_lflag);
            check_renamed_struct_renamed_field!(Termios, termios, special_codes, c_cc);

            // On everything except PowerPC, `termios` matches `termios2` except
            // for the addition of `c_ispeed` and `c_ospeed`.
            #[cfg(not(any(target_arch = "powerpc", target_arch = "powerpc64")))]
            const_assert_eq!(
                memoffset::offset_of!(Termios, input_speed),
                core::mem::size_of::<c::termios>()
            );

            // On PowerPC, `termios2` is `termios`.
            #[cfg(any(target_arch = "powerpc", target_arch = "powerpc64"))]
            assert_eq_size!(c::termios2, c::termios);
        }

        #[cfg(not(linux_raw))]
        {
            // On MIPS, SPARC, and Android, the libc lacks the ospeed and ispeed
            // fields.
            #[cfg(all(
                not(all(
                    target_env = "gnu",
                    any(
                        target_arch = "mips",
                        target_arch = "mips32r6",
                        target_arch = "mips64",
                        target_arch = "mips64r6",
                        target_arch = "sparc",
                        target_arch = "sparc64"
                    )
                )),
                not(all(libc, target_os = "android"))
            ))]
            check_renamed_type!(Termios, termios);
            #[cfg(not(all(
                not(all(
                    target_env = "gnu",
                    any(
                        target_arch = "mips",
                        target_arch = "mips32r6",
                        target_arch = "mips64",
                        target_arch = "mips64r6",
                        target_arch = "sparc",
                        target_arch = "sparc64"
                    )
                )),
                not(all(libc, target_os = "android"))
            )))]
            const_assert!(core::mem::size_of::<Termios>() >= core::mem::size_of::<c::termios>());

            check_renamed_struct_renamed_field!(Termios, termios, input_modes, c_iflag);
            check_renamed_struct_renamed_field!(Termios, termios, output_modes, c_oflag);
            check_renamed_struct_renamed_field!(Termios, termios, control_modes, c_cflag);
            check_renamed_struct_renamed_field!(Termios, termios, local_modes, c_lflag);
            #[cfg(any(
                linux_like,
                target_env = "newlib",
                target_os = "fuchsia",
                target_os = "haiku",
                target_os = "redox"
            ))]
            check_renamed_struct_renamed_field!(Termios, termios, line_discipline, c_line);
            check_renamed_struct_renamed_field!(Termios, termios, special_codes, c_cc);
            #[cfg(not(any(
                linux_kernel,
                solarish,
                target_os = "emscripten",
                target_os = "fuchsia"
            )))]
            {
                check_renamed_struct_renamed_field!(Termios, termios, input_speed, c_ispeed);
                check_renamed_struct_renamed_field!(Termios, termios, output_speed, c_ospeed);
            }
            #[cfg(any(target_env = "musl", target_os = "fuchsia"))]
            {
                check_renamed_struct_renamed_field!(Termios, termios, input_speed, __c_ispeed);
                check_renamed_struct_renamed_field!(Termios, termios, output_speed, __c_ospeed);
            }
        }

        check_renamed_type!(OptionalActions, c_int);
        check_renamed_type!(QueueSelector, c_int);
        check_renamed_type!(Action, c_int);
    }

    #[test]
    #[cfg(not(any(
        solarish,
        target_os = "cygwin",
        target_os = "emscripten",
        target_os = "haiku",
        target_os = "redox",
    )))]
    fn termios_legacy() {
        // Check that our doc aliases above are correct.
        const_assert_eq!(c::EXTA, c::B19200);
        const_assert_eq!(c::EXTB, c::B38400);
    }

    #[cfg(bsd)]
    #[test]
    fn termios_bsd() {
        // On BSD platforms we can assume that the `B*` constants have their
        // arbitrary integer speed value. Confirm this.
        const_assert_eq!(c::B0, 0);
        const_assert_eq!(c::B50, 50);
        const_assert_eq!(c::B19200, 19200);
        const_assert_eq!(c::B38400, 38400);
    }

    #[test]
    #[cfg(not(bsd))]
    fn termios_speed_encoding() {
        assert_eq!(speed::encode(0), Some(c::B0));
        assert_eq!(speed::encode(50), Some(c::B50));
        assert_eq!(speed::encode(19200), Some(c::B19200));
        assert_eq!(speed::encode(38400), Some(c::B38400));
        assert_eq!(speed::encode(1), None);
        assert_eq!(speed::encode(!0), None);

        #[cfg(not(linux_kernel))]
        {
            assert_eq!(speed::decode(c::B0), Some(0));
            assert_eq!(speed::decode(c::B50), Some(50));
            assert_eq!(speed::decode(c::B19200), Some(19200));
            assert_eq!(speed::decode(c::B38400), Some(38400));
        }
    }

    #[cfg(linux_kernel)]
    #[test]
    fn termios_ioctl_contiguity() {
        // When using `termios2`, we assume that we can add the optional actions
        // value to the ioctl request code. Test this assumption.

        const_assert_eq!(c::TCSETS2, c::TCSETS2 + 0);
        const_assert_eq!(c::TCSETSW2, c::TCSETS2 + 1);
        const_assert_eq!(c::TCSETSF2, c::TCSETS2 + 2);

        const_assert_eq!(c::TCSANOW - c::TCSANOW, 0);
        const_assert_eq!(c::TCSADRAIN - c::TCSANOW, 1);
        const_assert_eq!(c::TCSAFLUSH - c::TCSANOW, 2);

        // MIPS is different here.
        #[cfg(any(
            target_arch = "mips",
            target_arch = "mips32r6",
            target_arch = "mips64",
            target_arch = "mips64r6"
        ))]
        {
            assert_eq!(i128::from(c::TCSANOW) - i128::from(c::TCSETS), 0);
            assert_eq!(i128::from(c::TCSADRAIN) - i128::from(c::TCSETS), 1);
            assert_eq!(i128::from(c::TCSAFLUSH) - i128::from(c::TCSETS), 2);
        }
        #[cfg(not(any(
            target_arch = "mips",
            target_arch = "mips32r6",
            target_arch = "mips64",
            target_arch = "mips64r6"
        )))]
        {
            const_assert_eq!(c::TCSANOW, 0);
            const_assert_eq!(c::TCSADRAIN, 1);
            const_assert_eq!(c::TCSAFLUSH, 2);
        }
    }

    #[cfg(linux_kernel)]
    #[test]
    fn termios_cibaud() {
        // Test an assumption.
        const_assert_eq!(c::CIBAUD, c::CBAUD << c::IBSHIFT);
    }
}
