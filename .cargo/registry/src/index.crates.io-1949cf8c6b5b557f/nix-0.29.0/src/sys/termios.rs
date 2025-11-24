//! An interface for controlling asynchronous communication ports
//!
//! This interface provides a safe wrapper around the termios subsystem defined by POSIX. The
//! underlying types are all implemented in libc for most platforms and either wrapped in safer
//! types here or exported directly.
//!
//! If you are unfamiliar with the `termios` API, you should first read the
//! [API documentation](https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/termios.h.html) and
//! then come back to understand how `nix` safely wraps it.
//!
//! It should be noted that this API incurs some runtime overhead above the base `libc` definitions.
//! As this interface is not used with high-bandwidth information, this should be fine in most
//! cases. The primary cost when using this API is that the `Termios` datatype here duplicates the
//! standard fields of the underlying `termios` struct and uses safe type wrappers for those fields.
//! This means that when crossing the FFI interface to the underlying C library, data is first
//! copied into the underlying `termios` struct, then the operation is done, and the data is copied
//! back (with additional sanity checking) into the safe wrapper types. The `termios` struct is
//! relatively small across all platforms (on the order of 32-64 bytes).
//!
//! The following examples highlight some of the API use cases such that users coming from using C
//! or reading the standard documentation will understand how to use the safe API exposed here.
//!
//! Example disabling processing of the end-of-file control character:
//!
//! ```
//! # use self::nix::sys::termios::SpecialCharacterIndices::VEOF;
//! # use self::nix::sys::termios::{_POSIX_VDISABLE, Termios};
//! # let mut termios: Termios = unsafe { std::mem::zeroed() };
//! termios.control_chars[VEOF as usize] = _POSIX_VDISABLE;
//! ```
//!
//! The flags within `Termios` are defined as bitfields using the `bitflags` crate. This provides
//! an interface for working with bitfields that is similar to working with the raw unsigned
//! integer types but offers type safety because of the internal checking that values will always
//! be a valid combination of the defined flags.
//!
//! An example showing some of the basic operations for interacting with the control flags:
//!
//! ```
//! # use self::nix::sys::termios::{ControlFlags, Termios};
//! # let mut termios: Termios = unsafe { std::mem::zeroed() };
//! termios.control_flags & ControlFlags::CSIZE == ControlFlags::CS5;
//! termios.control_flags |= ControlFlags::CS5;
//! ```
//!
//! # Baud rates
//!
//! This API is not consistent across platforms when it comes to `BaudRate`: Android and Linux both
//! only support the rates specified by the `BaudRate` enum through their termios API while the BSDs
//! support arbitrary baud rates as the values of the `BaudRate` enum constants are the same integer
//! value of the constant (`B9600` == `9600`). Therefore the `nix::termios` API uses the following
//! conventions:
//!
//! * `cfgetispeed()` - Returns `u32` on BSDs, `BaudRate` on Android/Linux
//! * `cfgetospeed()` - Returns `u32` on BSDs, `BaudRate` on Android/Linux
//! * `cfsetispeed()` - Takes `u32` or `BaudRate` on BSDs, `BaudRate` on Android/Linux
//! * `cfsetospeed()` - Takes `u32` or `BaudRate` on BSDs, `BaudRate` on Android/Linux
//! * `cfsetspeed()` - Takes `u32` or `BaudRate` on BSDs, `BaudRate` on Android/Linux
//!
//! The most common use case of specifying a baud rate using the enum will work the same across
//! platforms:
//!
//! ```rust
//! # use nix::sys::termios::{BaudRate, cfsetispeed, cfsetospeed, cfsetspeed, Termios};
//! # fn main() {
//! # let mut t: Termios = unsafe { std::mem::zeroed() };
//! cfsetispeed(&mut t, BaudRate::B9600).unwrap();
//! cfsetospeed(&mut t, BaudRate::B9600).unwrap();
//! cfsetspeed(&mut t, BaudRate::B9600).unwrap();
//! # }
//! ```
//!
//! Additionally round-tripping baud rates is consistent across platforms:
//!
//! ```rust
//! # use nix::sys::termios::{BaudRate, cfgetispeed, cfgetospeed, cfsetispeed, cfsetspeed, Termios};
//! # fn main() {
//! # let mut t: Termios = unsafe { std::mem::zeroed() };
//! # cfsetspeed(&mut t, BaudRate::B9600).unwrap();
//! let speed = cfgetispeed(&t);
//! assert_eq!(speed, cfgetospeed(&t));
//! cfsetispeed(&mut t, speed).unwrap();
//! # }
//! ```
//!
//! On non-BSDs, `cfgetispeed()` and `cfgetospeed()` both return a `BaudRate`:
//!
#![cfg_attr(bsd, doc = " ```rust,ignore")]
#![cfg_attr(not(bsd), doc = " ```rust")]
//! # use nix::sys::termios::{BaudRate, cfgetispeed, cfgetospeed, cfsetspeed, Termios};
//! # fn main() {
//! # let mut t: Termios = unsafe { std::mem::zeroed() };
//! # cfsetspeed(&mut t, BaudRate::B9600);
//! assert_eq!(cfgetispeed(&t), BaudRate::B9600);
//! assert_eq!(cfgetospeed(&t), BaudRate::B9600);
//! # }
//! ```
//!
//! But on the BSDs, `cfgetispeed()` and `cfgetospeed()` both return `u32`s:
//!
#![cfg_attr(bsd, doc = " ```rust")]
#![cfg_attr(not(bsd), doc = " ```rust,ignore")]
//! # use nix::sys::termios::{BaudRate, cfgetispeed, cfgetospeed, cfsetspeed, Termios};
//! # fn main() {
//! # let mut t: Termios = unsafe { std::mem::zeroed() };
//! # cfsetspeed(&mut t, 9600u32);
//! assert_eq!(cfgetispeed(&t), 9600u32);
//! assert_eq!(cfgetospeed(&t), 9600u32);
//! # }
//! ```
//!
//! It's trivial to convert from a `BaudRate` to a `u32` on BSDs:
//!
#![cfg_attr(bsd, doc = " ```rust")]
#![cfg_attr(not(bsd), doc = " ```rust,ignore")]
//! # use nix::sys::termios::{BaudRate, cfgetispeed, cfsetspeed, Termios};
//! # fn main() {
//! # let mut t: Termios = unsafe { std::mem::zeroed() };
//! # cfsetspeed(&mut t, 9600u32);
//! assert_eq!(cfgetispeed(&t), BaudRate::B9600.into());
//! assert_eq!(u32::from(BaudRate::B9600), 9600u32);
//! # }
//! ```
//!
//! And on BSDs you can specify arbitrary baud rates (**note** this depends on hardware support)
//! by specifying baud rates directly using `u32`s:
//!
#![cfg_attr(bsd, doc = " ```rust")]
#![cfg_attr(not(bsd), doc = " ```rust,ignore")]
//! # use nix::sys::termios::{cfsetispeed, cfsetospeed, cfsetspeed, Termios};
//! # fn main() {
//! # let mut t: Termios = unsafe { std::mem::zeroed() };
//! cfsetispeed(&mut t, 9600u32);
//! cfsetospeed(&mut t, 9600u32);
//! cfsetspeed(&mut t, 9600u32);
//! # }
//! ```
use crate::errno::Errno;
use crate::Result;
use cfg_if::cfg_if;
use libc::{self, c_int, tcflag_t};
use std::cell::{Ref, RefCell};
use std::convert::From;
use std::mem;
use std::os::unix::io::{AsFd, AsRawFd};

#[cfg(feature = "process")]
use crate::unistd::Pid;

/// Stores settings for the termios API
///
/// This is a wrapper around the `libc::termios` struct that provides a safe interface for the
/// standard fields. The only safe way to obtain an instance of this struct is to extract it from
/// an open port using `tcgetattr()`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Termios {
    inner: RefCell<libc::termios>,
    /// Input mode flags (see `termios.c_iflag` documentation)
    pub input_flags: InputFlags,
    /// Output mode flags (see `termios.c_oflag` documentation)
    pub output_flags: OutputFlags,
    /// Control mode flags (see `termios.c_cflag` documentation)
    pub control_flags: ControlFlags,
    /// Local mode flags (see `termios.c_lflag` documentation)
    pub local_flags: LocalFlags,
    /// Control characters (see `termios.c_cc` documentation)
    pub control_chars: [libc::cc_t; NCCS],
    /// Line discipline (see `termios.c_line` documentation)
    #[cfg(linux_android)]
    pub line_discipline: libc::cc_t,
    /// Line discipline (see `termios.c_line` documentation)
    #[cfg(target_os = "haiku")]
    pub line_discipline: libc::c_char,
}

impl Termios {
    /// Exposes an immutable reference to the underlying `libc::termios` data structure.
    ///
    /// This is not part of `nix`'s public API because it requires additional work to maintain type
    /// safety.
    pub(crate) fn get_libc_termios(&self) -> Ref<libc::termios> {
        {
            let mut termios = self.inner.borrow_mut();
            termios.c_iflag = self.input_flags.bits();
            termios.c_oflag = self.output_flags.bits();
            termios.c_cflag = self.control_flags.bits();
            termios.c_lflag = self.local_flags.bits();
            termios.c_cc = self.control_chars;
            #[cfg(any(linux_android, target_os = "haiku"))]
            {
                termios.c_line = self.line_discipline;
            }
        }
        self.inner.borrow()
    }

    /// Exposes the inner `libc::termios` datastore within `Termios`.
    ///
    /// This is unsafe because if this is used to modify the inner `libc::termios` struct, it will
    /// not automatically update the safe wrapper type around it. In this case it should also be
    /// paired with a call to `update_wrapper()` so that the wrapper-type and internal
    /// representation stay consistent.
    pub(crate) unsafe fn get_libc_termios_mut(&mut self) -> *mut libc::termios {
        {
            let mut termios = self.inner.borrow_mut();
            termios.c_iflag = self.input_flags.bits();
            termios.c_oflag = self.output_flags.bits();
            termios.c_cflag = self.control_flags.bits();
            termios.c_lflag = self.local_flags.bits();
            termios.c_cc = self.control_chars;
            #[cfg(any(linux_android, target_os = "haiku"))]
            {
                termios.c_line = self.line_discipline;
            }
        }
        self.inner.as_ptr()
    }

    /// Updates the wrapper values from the internal `libc::termios` data structure.
    pub(crate) fn update_wrapper(&mut self) {
        let termios = *self.inner.borrow_mut();
        self.input_flags = InputFlags::from_bits_truncate(termios.c_iflag);
        self.output_flags = OutputFlags::from_bits_truncate(termios.c_oflag);
        self.control_flags = ControlFlags::from_bits_retain(termios.c_cflag);
        self.local_flags = LocalFlags::from_bits_truncate(termios.c_lflag);
        self.control_chars = termios.c_cc;
        #[cfg(any(linux_android, target_os = "haiku"))]
        {
            self.line_discipline = termios.c_line;
        }
    }
}

impl From<libc::termios> for Termios {
    fn from(termios: libc::termios) -> Self {
        Termios {
            inner: RefCell::new(termios),
            input_flags: InputFlags::from_bits_truncate(termios.c_iflag),
            output_flags: OutputFlags::from_bits_truncate(termios.c_oflag),
            control_flags: ControlFlags::from_bits_truncate(termios.c_cflag),
            local_flags: LocalFlags::from_bits_truncate(termios.c_lflag),
            control_chars: termios.c_cc,
            #[cfg(any(linux_android, target_os = "haiku"))]
            line_discipline: termios.c_line,
        }
    }
}

impl From<Termios> for libc::termios {
    fn from(termios: Termios) -> Self {
        termios.inner.into_inner()
    }
}

libc_enum! {
    /// Baud rates supported by the system.
    ///
    /// For the BSDs, arbitrary baud rates can be specified by using `u32`s directly instead of this
    /// enum.
    ///
    /// B0 is special and will disable the port.
    #[cfg_attr(target_os = "haiku", repr(u8))]
    #[cfg_attr(target_os = "hurd", repr(i32))]
    #[cfg_attr(all(apple_targets, target_pointer_width = "64"), repr(u64))]
    #[cfg_attr(all(
        not(all(apple_targets, target_pointer_width = "64")),
        not(target_os = "haiku"),
        not(target_os = "hurd")
        ), repr(u32))]
    #[non_exhaustive]
    pub enum BaudRate {
        B0,
        B50,
        B75,
        B110,
        B134,
        B150,
        B200,
        B300,
        B600,
        B1200,
        B1800,
        B2400,
        B4800,
        #[cfg(bsd)]
        B7200,
        B9600,
        #[cfg(bsd)]
        B14400,
        B19200,
        #[cfg(bsd)]
        B28800,
        B38400,
        #[cfg(not(target_os = "aix"))]
        B57600,
        #[cfg(bsd)]
        B76800,
        #[cfg(not(target_os = "aix"))]
        B115200,
        #[cfg(solarish)]
        B153600,
        #[cfg(not(target_os = "aix"))]
        B230400,
        #[cfg(solarish)]
        B307200,
        #[cfg(any(linux_android,
                  solarish,
                  target_os = "freebsd",
                  target_os = "netbsd"))]
        B460800,
        #[cfg(linux_android)]
        B500000,
        #[cfg(linux_android)]
        B576000,
        #[cfg(any(linux_android,
                  solarish,
                  target_os = "freebsd",
                  target_os = "netbsd"))]
        B921600,
        #[cfg(linux_android)]
        B1000000,
        #[cfg(linux_android)]
        B1152000,
        #[cfg(linux_android)]
        B1500000,
        #[cfg(linux_android)]
        B2000000,
        #[cfg(any(target_os = "android", all(target_os = "linux", not(target_arch = "sparc64"))))]
        B2500000,
        #[cfg(any(target_os = "android", all(target_os = "linux", not(target_arch = "sparc64"))))]
        B3000000,
        #[cfg(any(target_os = "android", all(target_os = "linux", not(target_arch = "sparc64"))))]
        B3500000,
        #[cfg(any(target_os = "android", all(target_os = "linux", not(target_arch = "sparc64"))))]
        B4000000,
    }
    impl TryFrom<libc::speed_t>
}

#[cfg(bsd)]
impl From<BaudRate> for u32 {
    fn from(b: BaudRate) -> u32 {
        b as u32
    }
}

#[cfg(target_os = "haiku")]
impl From<BaudRate> for u8 {
    fn from(b: BaudRate) -> u8 {
        b as u8
    }
}

// TODO: Add TCSASOFT, which will require treating this as a bitfield.
libc_enum! {
    /// Specify when a port configuration change should occur.
    ///
    /// Used as an argument to `tcsetattr()`
    #[repr(i32)]
    #[non_exhaustive]
    pub enum SetArg {
        /// The change will occur immediately
        TCSANOW,
        /// The change occurs after all output has been written
        TCSADRAIN,
        /// Same as `TCSADRAIN`, but will also flush the input buffer
        TCSAFLUSH,
    }
}

libc_enum! {
    /// Specify a combination of the input and output buffers to flush
    ///
    /// Used as an argument to `tcflush()`.
    #[repr(i32)]
    #[non_exhaustive]
    pub enum FlushArg {
        /// Flush data that was received but not read
        TCIFLUSH,
        /// Flush data written but not transmitted
        TCOFLUSH,
        /// Flush both received data not read and written data not transmitted
        TCIOFLUSH,
    }
}

libc_enum! {
    /// Specify how transmission flow should be altered
    ///
    /// Used as an argument to `tcflow()`.
    #[repr(i32)]
    #[non_exhaustive]
    pub enum FlowArg {
        /// Suspend transmission
        TCOOFF,
        /// Resume transmission
        TCOON,
        /// Transmit a STOP character, which should disable a connected terminal device
        TCIOFF,
        /// Transmit a START character, which should re-enable a connected terminal device
        TCION,
    }
}

// TODO: Make this usable directly as a slice index.
libc_enum! {
    /// Indices into the `termios.c_cc` array for special characters.
    #[repr(usize)]
    #[non_exhaustive]
    pub enum SpecialCharacterIndices {
        #[cfg(not(any(target_os = "aix", target_os = "haiku")))]
        VDISCARD,
        #[cfg(any(bsd,
                solarish,
                target_os = "aix"))]
        VDSUSP,
        VEOF,
        VEOL,
        VEOL2,
        VERASE,
        #[cfg(any(freebsdlike, solarish))]
        VERASE2,
        VINTR,
        VKILL,
        #[cfg(not(target_os = "haiku"))]
        VLNEXT,
        #[cfg(not(any(all(target_os = "linux", target_arch = "sparc64"),
                solarish, target_os = "aix", target_os = "haiku")))]
        VMIN,
        VQUIT,
        #[cfg(not(target_os = "haiku"))]
        VREPRINT,
        VSTART,
        #[cfg(any(bsd, solarish))]
        VSTATUS,
        VSTOP,
        VSUSP,
        #[cfg(target_os = "linux")]
        VSWTC,
        #[cfg(any(solarish, target_os = "haiku"))]
        VSWTCH,
        #[cfg(not(any(all(target_os = "linux", target_arch = "sparc64"),
                solarish, target_os = "aix", target_os = "haiku")))]
        VTIME,
        #[cfg(not(any(target_os = "aix", target_os = "haiku")))]
        VWERASE,
        #[cfg(target_os = "dragonfly")]
        VCHECKPT,
    }
}

#[cfg(any(
    all(target_os = "linux", target_arch = "sparc64"),
    solarish,
    target_os = "aix",
    target_os = "haiku",
))]
impl SpecialCharacterIndices {
    pub const VMIN: SpecialCharacterIndices = SpecialCharacterIndices::VEOF;
    pub const VTIME: SpecialCharacterIndices = SpecialCharacterIndices::VEOL;
}

pub use libc::NCCS;
#[cfg(any(linux_android, target_os = "aix", bsd))]
pub use libc::_POSIX_VDISABLE;

libc_bitflags! {
    /// Flags for configuring the input mode of a terminal
    pub struct InputFlags: tcflag_t {
        IGNBRK;
        BRKINT;
        IGNPAR;
        PARMRK;
        INPCK;
        ISTRIP;
        INLCR;
        IGNCR;
        ICRNL;
        IXON;
        IXOFF;
        #[cfg(not(target_os = "redox"))]
        IXANY;
        #[cfg(not(any(target_os = "redox", target_os = "haiku")))]
        IMAXBEL;
        #[cfg(any(linux_android, apple_targets))]
        IUTF8;
    }
}

libc_bitflags! {
    /// Flags for configuring the output mode of a terminal
    pub struct OutputFlags: tcflag_t {
        OPOST;
        #[cfg(any(linux_android,
                  target_os = "haiku",
                  target_os = "openbsd"))]
        OLCUC;
        ONLCR;
        OCRNL as tcflag_t;
        ONOCR as tcflag_t;
        ONLRET as tcflag_t;
        #[cfg(any(linux_android,
                  target_os = "haiku",
                  apple_targets))]
        OFDEL as tcflag_t;
        #[cfg(any(linux_android,
                  target_os = "haiku",
                  apple_targets))]
        NL0 as tcflag_t;
        #[cfg(any(linux_android,
                  target_os = "haiku",
                  apple_targets))]
        NL1 as tcflag_t;
        #[cfg(any(linux_android,
                  target_os = "haiku",
                  apple_targets))]
        CR0 as tcflag_t;
        #[cfg(any(linux_android,
                  target_os = "haiku",
                  apple_targets))]
        CR1 as tcflag_t;
        #[cfg(any(linux_android,
                  target_os = "haiku",
                  apple_targets))]
        CR2 as tcflag_t;
        #[cfg(any(linux_android,
                  target_os = "haiku",
                  apple_targets))]
        CR3 as tcflag_t;
        #[cfg(any(linux_android,
                  target_os = "freebsd",
                  target_os = "haiku",
                  apple_targets))]
        TAB0 as tcflag_t;
        #[cfg(any(linux_android,
                  target_os = "haiku",
                  apple_targets))]
        TAB1 as tcflag_t;
        #[cfg(any(linux_android,
                  target_os = "haiku",
                  apple_targets))]
        TAB2 as tcflag_t;
        #[cfg(any(linux_android,
                  target_os = "freebsd",
                  target_os = "haiku",
                  apple_targets))]
        TAB3 as tcflag_t;
        #[cfg(linux_android)]
        XTABS;
        #[cfg(any(linux_android,
                  target_os = "haiku",
                  apple_targets))]
        BS0 as tcflag_t;
        #[cfg(any(linux_android,
                  target_os = "haiku",
                  apple_targets))]
        BS1 as tcflag_t;
        #[cfg(any(linux_android,
                  target_os = "haiku",
                  apple_targets))]
        VT0 as tcflag_t;
        #[cfg(any(linux_android,
                  target_os = "haiku",
                  apple_targets))]
        VT1 as tcflag_t;
        #[cfg(any(linux_android,
                  target_os = "haiku",
                  apple_targets))]
        FF0 as tcflag_t;
        #[cfg(any(linux_android,
                  target_os = "haiku",
                  apple_targets))]
        FF1 as tcflag_t;
        #[cfg(bsd)]
        OXTABS;
        #[cfg(bsd)]
        ONOEOT as tcflag_t;

        // Bitmasks for use with OutputFlags to select specific settings
        // These should be moved to be a mask once https://github.com/rust-lang-nursery/bitflags/issues/110
        // is resolved.

        #[cfg(any(linux_android,
                  target_os = "haiku",
                  apple_targets))]
        NLDLY as tcflag_t; // FIXME: Datatype needs to be corrected in libc for mac
        #[cfg(any(linux_android,
                  target_os = "haiku",
                  apple_targets))]
        CRDLY as tcflag_t;
        #[cfg(any(linux_android,
                  target_os = "freebsd",
                  target_os = "haiku",
                  apple_targets))]
        TABDLY as tcflag_t;
        #[cfg(any(linux_android,
                  target_os = "haiku",
                  apple_targets))]
        BSDLY as tcflag_t;
        #[cfg(any(linux_android,
                  target_os = "haiku",
                  apple_targets))]
        VTDLY as tcflag_t;
        #[cfg(any(linux_android,
                  target_os = "haiku",
                  apple_targets))]
        FFDLY as tcflag_t;
    }
}

libc_bitflags! {
    /// Flags for setting the control mode of a terminal
    pub struct ControlFlags: tcflag_t {
        #[cfg(bsd)]
        CIGNORE;
        CS5;
        CS6;
        CS7;
        CS8;
        CSTOPB;
        CREAD;
        PARENB;
        PARODD;
        HUPCL;
        CLOCAL;
        #[cfg(not(any(target_os = "redox", target_os = "aix")))]
        CRTSCTS;
        #[cfg(linux_android)]
        CBAUD;
        #[cfg(any(target_os = "android", all(target_os = "linux", not(target_arch = "mips"))))]
        CMSPAR;
        #[cfg(any(target_os = "android",
                  all(target_os = "linux",
                      not(any(target_arch = "powerpc", target_arch = "powerpc64")))))]
        CIBAUD;
        #[cfg(linux_android)]
        CBAUDEX;
        #[cfg(bsd)]
        MDMBUF;
        #[cfg(netbsdlike)]
        CHWFLOW;
        #[cfg(any(freebsdlike, netbsdlike))]
        CCTS_OFLOW;
        #[cfg(any(freebsdlike, netbsdlike))]
        CRTS_IFLOW;
        #[cfg(freebsdlike)]
        CDTR_IFLOW;
        #[cfg(freebsdlike)]
        CDSR_OFLOW;
        #[cfg(freebsdlike)]
        CCAR_OFLOW;

        // Bitmasks for use with ControlFlags to select specific settings
        // These should be moved to be a mask once https://github.com/rust-lang-nursery/bitflags/issues/110
        // is resolved.

        CSIZE;
    }
}

libc_bitflags! {
    /// Flags for setting any local modes
    pub struct LocalFlags: tcflag_t {
        #[cfg(not(target_os = "redox"))]
        ECHOKE;
        ECHOE;
        ECHOK;
        ECHO;
        ECHONL;
        #[cfg(not(target_os = "redox"))]
        ECHOPRT;
        #[cfg(not(target_os = "redox"))]
        ECHOCTL;
        ISIG;
        ICANON;
        #[cfg(bsd)]
        ALTWERASE;
        IEXTEN;
        #[cfg(not(any(target_os = "redox", target_os = "haiku", target_os = "aix")))]
        EXTPROC;
        TOSTOP;
        #[cfg(not(target_os = "redox"))]
        FLUSHO;
        #[cfg(bsd)]
        NOKERNINFO;
        #[cfg(not(target_os = "redox"))]
        PENDIN;
        NOFLSH;
    }
}

cfg_if! {
    if #[cfg(bsd)] {
        /// Get input baud rate (see
        /// [cfgetispeed(3p)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/cfgetispeed.html)).
        ///
        /// `cfgetispeed()` extracts the input baud rate from the given `Termios` structure.
        // The cast is not unnecessary on all platforms.
        #[allow(clippy::unnecessary_cast)]
        pub fn cfgetispeed(termios: &Termios) -> u32 {
            let inner_termios = termios.get_libc_termios();
            unsafe { libc::cfgetispeed(&*inner_termios) as u32 }
        }

        /// Get output baud rate (see
        /// [cfgetospeed(3p)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/cfgetospeed.html)).
        ///
        /// `cfgetospeed()` extracts the output baud rate from the given `Termios` structure.
        // The cast is not unnecessary on all platforms.
        #[allow(clippy::unnecessary_cast)]
        pub fn cfgetospeed(termios: &Termios) -> u32 {
            let inner_termios = termios.get_libc_termios();
            unsafe { libc::cfgetospeed(&*inner_termios) as u32 }
        }

        /// Set input baud rate (see
        /// [cfsetispeed(3p)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/cfsetispeed.html)).
        ///
        /// `cfsetispeed()` sets the intput baud rate in the given `Termios` structure.
        pub fn cfsetispeed<T: Into<u32>>(termios: &mut Termios, baud: T) -> Result<()> {
            let inner_termios = unsafe { termios.get_libc_termios_mut() };
            let res = unsafe { libc::cfsetispeed(inner_termios, baud.into() as libc::speed_t) };
            termios.update_wrapper();
            Errno::result(res).map(drop)
        }

        /// Set output baud rate (see
        /// [cfsetospeed(3p)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/cfsetospeed.html)).
        ///
        /// `cfsetospeed()` sets the output baud rate in the given termios structure.
        pub fn cfsetospeed<T: Into<u32>>(termios: &mut Termios, baud: T) -> Result<()> {
            let inner_termios = unsafe { termios.get_libc_termios_mut() };
            let res = unsafe { libc::cfsetospeed(inner_termios, baud.into() as libc::speed_t) };
            termios.update_wrapper();
            Errno::result(res).map(drop)
        }

        /// Set both the input and output baud rates (see
        /// [termios(3)](https://www.freebsd.org/cgi/man.cgi?query=cfsetspeed)).
        ///
        /// `cfsetspeed()` sets the input and output baud rate in the given termios structure. Note that
        /// this is part of the 4.4BSD standard and not part of POSIX.
        pub fn cfsetspeed<T: Into<u32>>(termios: &mut Termios, baud: T) -> Result<()> {
            let inner_termios = unsafe { termios.get_libc_termios_mut() };
            let res = unsafe { libc::cfsetspeed(inner_termios, baud.into() as libc::speed_t) };
            termios.update_wrapper();
            Errno::result(res).map(drop)
        }
    } else {
        use std::convert::TryInto;

        /// Get input baud rate (see
        /// [cfgetispeed(3p)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/cfgetispeed.html)).
        ///
        /// `cfgetispeed()` extracts the input baud rate from the given `Termios` structure.
        pub fn cfgetispeed(termios: &Termios) -> BaudRate {
            let inner_termios = termios.get_libc_termios();
            unsafe { libc::cfgetispeed(&*inner_termios) }.try_into().unwrap()
        }

        /// Get output baud rate (see
        /// [cfgetospeed(3p)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/cfgetospeed.html)).
        ///
        /// `cfgetospeed()` extracts the output baud rate from the given `Termios` structure.
        pub fn cfgetospeed(termios: &Termios) -> BaudRate {
            let inner_termios = termios.get_libc_termios();
            unsafe { libc::cfgetospeed(&*inner_termios) }.try_into().unwrap()
        }

        /// Set input baud rate (see
        /// [cfsetispeed(3p)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/cfsetispeed.html)).
        ///
        /// `cfsetispeed()` sets the intput baud rate in the given `Termios` structure.
        pub fn cfsetispeed(termios: &mut Termios, baud: BaudRate) -> Result<()> {
            let inner_termios = unsafe { termios.get_libc_termios_mut() };
            let res = unsafe { libc::cfsetispeed(inner_termios, baud as libc::speed_t) };
            termios.update_wrapper();
            Errno::result(res).map(drop)
        }

        /// Set output baud rate (see
        /// [cfsetospeed(3p)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/cfsetospeed.html)).
        ///
        /// `cfsetospeed()` sets the output baud rate in the given `Termios` structure.
        pub fn cfsetospeed(termios: &mut Termios, baud: BaudRate) -> Result<()> {
            let inner_termios = unsafe { termios.get_libc_termios_mut() };
            let res = unsafe { libc::cfsetospeed(inner_termios, baud as libc::speed_t) };
            termios.update_wrapper();
            Errno::result(res).map(drop)
        }

        /// Set both the input and output baud rates (see
        /// [termios(3)](https://www.freebsd.org/cgi/man.cgi?query=cfsetspeed)).
        ///
        /// `cfsetspeed()` sets the input and output baud rate in the given `Termios` structure. Note that
        /// this is part of the 4.4BSD standard and not part of POSIX.
        #[cfg(not(target_os = "haiku"))]
        pub fn cfsetspeed(termios: &mut Termios, baud: BaudRate) -> Result<()> {
            let inner_termios = unsafe { termios.get_libc_termios_mut() };
            let res = unsafe { libc::cfsetspeed(inner_termios, baud as libc::speed_t) };
            termios.update_wrapper();
            Errno::result(res).map(drop)
        }
    }
}

/// Configures the port to something like the "raw" mode of the old Version 7 terminal driver (see
/// [termios(3)](https://man7.org/linux/man-pages/man3/termios.3.html)).
///
/// `cfmakeraw()` configures the termios structure such that input is available character-by-
/// character, echoing is disabled, and all special input and output processing is disabled. Note
/// that this is a non-standard function, but is available on Linux and BSDs.
pub fn cfmakeraw(termios: &mut Termios) {
    let inner_termios = unsafe { termios.get_libc_termios_mut() };
    unsafe {
        libc::cfmakeraw(inner_termios);
    }
    termios.update_wrapper();
}

/// Configures the port to "sane" mode (like the configuration of a newly created terminal) (see
/// [tcsetattr(3)](https://www.freebsd.org/cgi/man.cgi?query=tcsetattr)).
///
/// Note that this is a non-standard function, available on FreeBSD.
#[cfg(target_os = "freebsd")]
pub fn cfmakesane(termios: &mut Termios) {
    let inner_termios = unsafe { termios.get_libc_termios_mut() };
    unsafe {
        libc::cfmakesane(inner_termios);
    }
    termios.update_wrapper();
}

/// Return the configuration of a port
/// [tcgetattr(3p)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/tcgetattr.html)).
///
/// `tcgetattr()` returns a `Termios` structure with the current configuration for a port. Modifying
/// this structure *will not* reconfigure the port, instead the modifications should be done to
/// the `Termios` structure and then the port should be reconfigured using `tcsetattr()`.
pub fn tcgetattr<Fd: AsFd>(fd: Fd) -> Result<Termios> {
    let mut termios = mem::MaybeUninit::uninit();

    let res = unsafe {
        libc::tcgetattr(fd.as_fd().as_raw_fd(), termios.as_mut_ptr())
    };

    Errno::result(res)?;

    unsafe { Ok(termios.assume_init().into()) }
}

/// Set the configuration for a terminal (see
/// [tcsetattr(3p)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/tcsetattr.html)).
///
/// `tcsetattr()` reconfigures the given port based on a given `Termios` structure. This change
/// takes affect at a time specified by `actions`. Note that this function may return success if
/// *any* of the parameters were successfully set, not only if all were set successfully.
pub fn tcsetattr<Fd: AsFd>(
    fd: Fd,
    actions: SetArg,
    termios: &Termios,
) -> Result<()> {
    let inner_termios = termios.get_libc_termios();
    Errno::result(unsafe {
        libc::tcsetattr(
            fd.as_fd().as_raw_fd(),
            actions as c_int,
            &*inner_termios,
        )
    })
    .map(drop)
}

/// Block until all output data is written (see
/// [tcdrain(3p)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/tcdrain.html)).
pub fn tcdrain<Fd: AsFd>(fd: Fd) -> Result<()> {
    Errno::result(unsafe { libc::tcdrain(fd.as_fd().as_raw_fd()) }).map(drop)
}

/// Suspend or resume the transmission or reception of data (see
/// [tcflow(3p)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/tcflow.html)).
///
/// `tcflow()` suspends of resumes the transmission or reception of data for the given port
/// depending on the value of `action`.
pub fn tcflow<Fd: AsFd>(fd: Fd, action: FlowArg) -> Result<()> {
    Errno::result(unsafe {
        libc::tcflow(fd.as_fd().as_raw_fd(), action as c_int)
    })
    .map(drop)
}

/// Discard data in the output or input queue (see
/// [tcflush(3p)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/tcflush.html)).
///
/// `tcflush()` will discard data for a terminal port in the input queue, output queue, or both
/// depending on the value of `action`.
pub fn tcflush<Fd: AsFd>(fd: Fd, action: FlushArg) -> Result<()> {
    Errno::result(unsafe {
        libc::tcflush(fd.as_fd().as_raw_fd(), action as c_int)
    })
    .map(drop)
}

/// Send a break for a specific duration (see
/// [tcsendbreak(3p)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/tcsendbreak.html)).
///
/// When using asynchronous data transmission `tcsendbreak()` will transmit a continuous stream
/// of zero-valued bits for an implementation-defined duration.
pub fn tcsendbreak<Fd: AsFd>(fd: Fd, duration: c_int) -> Result<()> {
    Errno::result(unsafe {
        libc::tcsendbreak(fd.as_fd().as_raw_fd(), duration)
    })
    .map(drop)
}

feature! {
#![feature = "process"]
/// Get the session controlled by the given terminal (see
/// [tcgetsid(3)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/tcgetsid.html)).
pub fn tcgetsid<Fd: AsFd>(fd: Fd) -> Result<Pid> {
    let res = unsafe { libc::tcgetsid(fd.as_fd().as_raw_fd()) };

    Errno::result(res).map(Pid::from_raw)
}
}
