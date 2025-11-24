use crate::{backend, io};
use core::fmt;

pub use crate::timespec::{Nsecs, Secs, Timespec};

#[cfg(not(any(
    apple,
    target_os = "dragonfly",
    target_os = "espidf",
    target_os = "freebsd", // FreeBSD 12 has clock_nanosleep, but libc targets FreeBSD 11.
    target_os = "openbsd",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi",
)))]
pub use crate::clockid::ClockId;

/// `clock_nanosleep(id, 0, request, remain)`—Sleeps for a duration on a
/// given clock.
///
/// This is `clock_nanosleep` specialized for the case of a relative sleep
/// interval. See [`clock_nanosleep_absolute`] for absolute intervals.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/clock_nanosleep.html
/// [Linux]: https://man7.org/linux/man-pages/man2/clock_nanosleep.2.html
#[cfg(not(any(
    apple,
    target_os = "dragonfly",
    target_os = "emscripten",
    target_os = "espidf",
    target_os = "freebsd", // FreeBSD 12 has clock_nanosleep, but libc targets FreeBSD 11.
    target_os = "haiku",
    target_os = "openbsd",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi",
)))]
#[inline]
pub fn clock_nanosleep_relative(id: ClockId, request: &Timespec) -> NanosleepRelativeResult {
    backend::thread::syscalls::clock_nanosleep_relative(id, request)
}

/// `clock_nanosleep(id, TIMER_ABSTIME, request, NULL)`—Sleeps until an
/// absolute time on a given clock.
///
/// This is `clock_nanosleep` specialized for the case of an absolute sleep
/// interval. See [`clock_nanosleep_relative`] for relative intervals.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/clock_nanosleep.html
/// [Linux]: https://man7.org/linux/man-pages/man2/clock_nanosleep.2.html
#[cfg(not(any(
    apple,
    target_os = "dragonfly",
    target_os = "emscripten",
    target_os = "espidf",
    target_os = "freebsd", // FreeBSD 12 has clock_nanosleep, but libc targets FreeBSD 11.
    target_os = "haiku",
    target_os = "openbsd",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi",
)))]
#[inline]
pub fn clock_nanosleep_absolute(id: ClockId, request: &Timespec) -> io::Result<()> {
    backend::thread::syscalls::clock_nanosleep_absolute(id, request)
}

/// `nanosleep(request, remain)`—Sleeps for a duration.
///
/// This effectively uses the system monotonic clock.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/nanosleep.html
/// [Linux]: https://man7.org/linux/man-pages/man2/nanosleep.2.html
#[inline]
pub fn nanosleep(request: &Timespec) -> NanosleepRelativeResult {
    backend::thread::syscalls::nanosleep(request)
}

/// A return type for `nanosleep` and `clock_nanosleep_relative`.
#[derive(Clone)]
#[must_use]
pub enum NanosleepRelativeResult {
    /// The sleep completed normally.
    Ok,
    /// The sleep was interrupted, the remaining time is returned.
    Interrupted(Timespec),
    /// An invalid time value was provided.
    Err(io::Errno),
}

impl fmt::Debug for NanosleepRelativeResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ok => f.write_str("Ok"),
            Self::Interrupted(remaining) => write!(
                f,
                "Interrupted(Timespec {{ tv_sec: {:?}, tv_nsec: {:?} }})",
                remaining.tv_sec, remaining.tv_nsec
            ),
            Self::Err(err) => write!(f, "Err({:?})", err),
        }
    }
}
