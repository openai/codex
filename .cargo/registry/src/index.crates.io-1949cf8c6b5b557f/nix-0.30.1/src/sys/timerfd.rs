//! Timer API via file descriptors.
//!
//! Timer FD is a Linux-only API to create timers and get expiration
//! notifications through file descriptors.
//!
//! For more documentation, please read [timerfd_create(2)](https://man7.org/linux/man-pages/man2/timerfd_create.2.html).
//!
//! # Examples
//!
//! Create a new one-shot timer that expires after 1 second.
//! ```
//! # use std::os::unix::io::AsRawFd;
//! # use nix::sys::timerfd::{TimerFd, ClockId, TimerFlags, TimerSetTimeFlags,
//! #    Expiration};
//! # use nix::sys::time::{TimeSpec, TimeValLike};
//! # use nix::unistd::read;
//! #
//! // We create a new monotonic timer.
//! let timer = TimerFd::new(ClockId::CLOCK_MONOTONIC, TimerFlags::empty())
//!     .unwrap();
//!
//! // We set a new one-shot timer in 1 seconds.
//! timer.set(
//!     Expiration::OneShot(TimeSpec::seconds(1)),
//!     TimerSetTimeFlags::empty()
//! ).unwrap();
//!
//! // We wait for the timer to expire.
//! timer.wait().unwrap();
//! ```
use crate::sys::time::timer::TimerSpec;
pub use crate::sys::time::timer::{Expiration, TimerSetTimeFlags};
use crate::unistd::read;
use crate::{errno::Errno, Result};
use libc::c_int;
use std::os::unix::io::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};

/// A timerfd instance. This is also a file descriptor, you can feed it to
/// other interfaces taking file descriptors as arguments, [`epoll`] for example.
///
/// [`epoll`]: crate::sys::epoll
#[derive(Debug)]
pub struct TimerFd {
    fd: OwnedFd,
}

impl AsFd for TimerFd {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

impl FromRawFd for TimerFd {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        TimerFd {
            fd: unsafe { OwnedFd::from_raw_fd(fd) },
        }
    }
}

impl From<TimerFd> for OwnedFd {
    fn from(value: TimerFd) -> Self {
        value.fd  
    }
}

libc_enum! {
    /// The type of the clock used to mark the progress of the timer. For more
    /// details on each kind of clock, please refer to [timerfd_create(2)](https://man7.org/linux/man-pages/man2/timerfd_create.2.html).
    #[repr(i32)]
    #[non_exhaustive]
    pub enum ClockId {
        /// A settable system-wide real-time clock.
        CLOCK_REALTIME,
        /// A non-settable monotonically increasing clock.
        ///
        /// Does not change after system startup.
        /// Does not measure time while the system is suspended.
        CLOCK_MONOTONIC,
        /// Like `CLOCK_MONOTONIC`, except that `CLOCK_BOOTTIME` includes the time
        /// that the system was suspended.
        CLOCK_BOOTTIME,
        /// Like `CLOCK_REALTIME`, but will wake the system if it is suspended.
        CLOCK_REALTIME_ALARM,
        /// Like `CLOCK_BOOTTIME`, but will wake the system if it is suspended.
        CLOCK_BOOTTIME_ALARM,
    }
}

libc_bitflags! {
    /// Additional flags to change the behaviour of the file descriptor at the
    /// time of creation.
    pub struct TimerFlags: c_int {
        /// Set the `O_NONBLOCK` flag on the open file description referred to by the new file descriptor.
        TFD_NONBLOCK;
        /// Set the `FD_CLOEXEC` flag on the file descriptor.
        TFD_CLOEXEC;
    }
}

impl TimerFd {
    /// Creates a new timer based on the clock defined by `clockid`. The
    /// underlying fd can be assigned specific flags with `flags` (CLOEXEC,
    /// NONBLOCK). The underlying fd will be closed on drop.
    #[doc(alias("timerfd_create"))]
    pub fn new(clockid: ClockId, flags: TimerFlags) -> Result<Self> {
        Errno::result(unsafe {
            libc::timerfd_create(clockid as i32, flags.bits())
        })
        .map(|fd| Self {
            fd: unsafe { OwnedFd::from_raw_fd(fd) },
        })
    }

    /// Sets a new alarm on the timer.
    ///
    /// # Types of alarm
    ///
    /// There are 3 types of alarms you can set:
    ///
    ///   - one shot: the alarm will trigger once after the specified amount of
    ///     time.
    ///     Example: I want an alarm to go off in 60s and then disable itself.
    ///
    ///   - interval: the alarm will trigger every specified interval of time.
    ///     Example: I want an alarm to go off every 60s. The alarm will first
    ///     go off 60s after I set it and every 60s after that. The alarm will
    ///     not disable itself.
    ///
    ///   - interval delayed: the alarm will trigger after a certain amount of
    ///     time and then trigger at a specified interval.
    ///     Example: I want an alarm to go off every 60s but only start in 1h.
    ///     The alarm will first trigger 1h after I set it and then every 60s
    ///     after that. The alarm will not disable itself.
    ///
    /// # Relative vs absolute alarm
    ///
    /// If you do not set any `TimerSetTimeFlags`, then the `TimeSpec` you pass
    /// to the `Expiration` you want is relative. If however you want an alarm
    /// to go off at a certain point in time, you can set `TFD_TIMER_ABSTIME`.
    /// Then the one shot TimeSpec and the delay TimeSpec of the delayed
    /// interval are going to be interpreted as absolute.
    ///
    /// # Cancel on a clock change
    ///
    /// If you set a `TFD_TIMER_CANCEL_ON_SET` alongside `TFD_TIMER_ABSTIME`
    /// and the clock for this timer is `CLOCK_REALTIME` or `CLOCK_REALTIME_ALARM`,
    /// then this timer is marked as cancelable if the real-time clock undergoes
    /// a discontinuous change.
    ///
    /// # Disabling alarms
    ///
    /// Note: Only one alarm can be set for any given timer. Setting a new alarm
    /// actually removes the previous one.
    ///
    /// Note: Setting a one shot alarm with a 0s TimeSpec disables the alarm
    /// altogether.
    #[doc(alias("timerfd_settime"))]
    pub fn set(
        &self,
        expiration: Expiration,
        flags: TimerSetTimeFlags,
    ) -> Result<()> {
        let timerspec: TimerSpec = expiration.into();
        Errno::result(unsafe {
            libc::timerfd_settime(
                self.fd.as_fd().as_raw_fd(),
                flags.bits(),
                timerspec.as_ref(),
                std::ptr::null_mut(),
            )
        })
        .map(drop)
    }

    /// Get the parameters for the alarm currently set, if any.
    #[doc(alias("timerfd_gettime"))]
    pub fn get(&self) -> Result<Option<Expiration>> {
        let mut timerspec = TimerSpec::none();
        Errno::result(unsafe {
            libc::timerfd_gettime(
                self.fd.as_fd().as_raw_fd(),
                timerspec.as_mut(),
            )
        })
        .map(|_| {
            if timerspec.as_ref().it_interval.tv_sec == 0
                && timerspec.as_ref().it_interval.tv_nsec == 0
                && timerspec.as_ref().it_value.tv_sec == 0
                && timerspec.as_ref().it_value.tv_nsec == 0
            {
                None
            } else {
                Some(timerspec.into())
            }
        })
    }

    /// Remove the alarm if any is set.
    #[doc(alias("timerfd_settime"))]
    pub fn unset(&self) -> Result<()> {
        Errno::result(unsafe {
            libc::timerfd_settime(
                self.fd.as_fd().as_raw_fd(),
                TimerSetTimeFlags::empty().bits(),
                TimerSpec::none().as_ref(),
                std::ptr::null_mut(),
            )
        })
        .map(drop)
    }

    /// Wait for the configured alarm to expire.
    ///
    /// Note: If the alarm is unset, then you will wait forever.
    pub fn wait(&self) -> Result<()> {
        while let Err(e) = read(&self.fd, &mut [0u8; 8]) {
            if e == Errno::ECANCELED {
                break;
            }
            if e != Errno::EINTR {
                return Err(e);
            }
        }

        Ok(())
    }


    /// Constructs a `TimerFd` wrapping an existing `OwnedFd`.
    ///
    /// # Safety
    ///
    /// `OwnedFd` is a valid `TimerFd`.
    pub unsafe fn from_owned_fd(fd: OwnedFd) -> Self {
        Self {
            fd
        }
    }
}
