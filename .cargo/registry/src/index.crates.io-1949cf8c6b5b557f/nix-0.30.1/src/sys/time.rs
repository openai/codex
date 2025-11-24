#[cfg_attr(
    any(target_env = "musl", target_env = "ohos"),
    allow(deprecated)
)]
// https://github.com/rust-lang/libc/issues/1848
pub use libc::{suseconds_t, time_t};
use libc::{timespec, timeval};
use std::time::Duration;
use std::{cmp, fmt, ops};

const fn zero_init_timespec() -> timespec {
    // `std::mem::MaybeUninit::zeroed()` is not yet a const fn
    // (https://github.com/rust-lang/rust/issues/91850) so we will instead initialize an array of
    // the appropriate size to zero and then transmute it to a timespec value.
    unsafe { std::mem::transmute([0u8; std::mem::size_of::<timespec>()]) }
}

#[cfg(any(
    all(feature = "time", any(target_os = "android", target_os = "linux")),
    all(
        any(
            target_os = "freebsd",
            solarish,
            target_os = "linux",
            target_os = "netbsd"
        ),
        feature = "time",
        feature = "signal"
    )
))]
pub(crate) mod timer {
    use crate::sys::time::{zero_init_timespec, TimeSpec};
    use bitflags::bitflags;

    #[derive(Debug, Clone, Copy)]
    pub(crate) struct TimerSpec(libc::itimerspec);

    impl TimerSpec {
        pub const fn none() -> Self {
            Self(libc::itimerspec {
                it_interval: zero_init_timespec(),
                it_value: zero_init_timespec(),
            })
        }
    }

    impl AsMut<libc::itimerspec> for TimerSpec {
        fn as_mut(&mut self) -> &mut libc::itimerspec {
            &mut self.0
        }
    }

    impl AsRef<libc::itimerspec> for TimerSpec {
        fn as_ref(&self) -> &libc::itimerspec {
            &self.0
        }
    }

    impl From<Expiration> for TimerSpec {
        fn from(expiration: Expiration) -> TimerSpec {
            match expiration {
                Expiration::OneShot(t) => TimerSpec(libc::itimerspec {
                    it_interval: zero_init_timespec(),
                    it_value: *t.as_ref(),
                }),
                Expiration::IntervalDelayed(start, interval) => {
                    TimerSpec(libc::itimerspec {
                        it_interval: *interval.as_ref(),
                        it_value: *start.as_ref(),
                    })
                }
                Expiration::Interval(t) => TimerSpec(libc::itimerspec {
                    it_interval: *t.as_ref(),
                    it_value: *t.as_ref(),
                }),
            }
        }
    }

    /// An enumeration allowing the definition of the expiration time of an alarm,
    /// recurring or not.
    #[derive(Debug, Clone, Copy, Eq, PartialEq)]
    pub enum Expiration {
        /// Alarm will trigger once after the time given in `TimeSpec`
        OneShot(TimeSpec),
        /// Alarm will trigger after a specified delay and then every interval of
        /// time.
        IntervalDelayed(TimeSpec, TimeSpec),
        /// Alarm will trigger every specified interval of time.
        Interval(TimeSpec),
    }

    #[cfg(linux_android)]
    bitflags! {
        /// Flags that are used for arming the timer.
        #[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct TimerSetTimeFlags: libc::c_int {
            const TFD_TIMER_ABSTIME = libc::TFD_TIMER_ABSTIME;
            const TFD_TIMER_CANCEL_ON_SET = libc::TFD_TIMER_CANCEL_ON_SET;
        }
    }
    #[cfg(any(freebsdlike, target_os = "netbsd", solarish))]
    bitflags! {
        /// Flags that are used for arming the timer.
        #[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct TimerSetTimeFlags: libc::c_int {
            const TFD_TIMER_ABSTIME = libc::TIMER_ABSTIME;
        }
    }

    impl From<TimerSpec> for Expiration {
        fn from(timerspec: TimerSpec) -> Expiration {
            match timerspec {
                TimerSpec(libc::itimerspec {
                    it_interval:
                        libc::timespec {
                            tv_sec: 0,
                            tv_nsec: 0,
                            ..
                        },
                    it_value: ts,
                }) => Expiration::OneShot(ts.into()),
                TimerSpec(libc::itimerspec {
                    it_interval: int_ts,
                    it_value: val_ts,
                }) => {
                    if (int_ts.tv_sec == val_ts.tv_sec)
                        && (int_ts.tv_nsec == val_ts.tv_nsec)
                    {
                        Expiration::Interval(int_ts.into())
                    } else {
                        Expiration::IntervalDelayed(
                            val_ts.into(),
                            int_ts.into(),
                        )
                    }
                }
            }
        }
    }
}

pub trait TimeValLike: Sized {
    #[inline]
    fn zero() -> Self {
        Self::seconds(0)
    }

    #[inline]
    fn hours(hours: i64) -> Self {
        let secs = hours
            .checked_mul(SECS_PER_HOUR)
            .expect("TimeValLike::hours ouf of bounds");
        Self::seconds(secs)
    }

    #[inline]
    fn minutes(minutes: i64) -> Self {
        let secs = minutes
            .checked_mul(SECS_PER_MINUTE)
            .expect("TimeValLike::minutes out of bounds");
        Self::seconds(secs)
    }

    fn seconds(seconds: i64) -> Self;
    fn milliseconds(milliseconds: i64) -> Self;
    fn microseconds(microseconds: i64) -> Self;
    fn nanoseconds(nanoseconds: i64) -> Self;

    #[inline]
    fn num_hours(&self) -> i64 {
        self.num_seconds() / 3600
    }

    #[inline]
    fn num_minutes(&self) -> i64 {
        self.num_seconds() / 60
    }

    fn num_seconds(&self) -> i64;
    fn num_milliseconds(&self) -> i64;
    fn num_microseconds(&self) -> i64;
    fn num_nanoseconds(&self) -> i64;
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TimeSpec(timespec);

const NANOS_PER_SEC: i64 = 1_000_000_000;
const SECS_PER_MINUTE: i64 = 60;
const SECS_PER_HOUR: i64 = 3600;

#[cfg(target_pointer_width = "64")]
const TS_MAX_SECONDS: i64 = (i64::MAX / NANOS_PER_SEC) - 1;

#[cfg(target_pointer_width = "32")]
const TS_MAX_SECONDS: i64 = isize::MAX as i64;

const TS_MIN_SECONDS: i64 = -TS_MAX_SECONDS;

// x32 compatibility
// See https://sourceware.org/bugzilla/show_bug.cgi?id=16437
#[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
type timespec_tv_nsec_t = i64;
#[cfg(not(all(target_arch = "x86_64", target_pointer_width = "32")))]
type timespec_tv_nsec_t = libc::c_long;

impl From<timespec> for TimeSpec {
    fn from(ts: timespec) -> Self {
        Self(ts)
    }
}

impl From<Duration> for TimeSpec {
    fn from(duration: Duration) -> Self {
        Self::from_duration(duration)
    }
}

impl From<TimeSpec> for Duration {
    fn from(timespec: TimeSpec) -> Self {
        Duration::new(timespec.0.tv_sec as u64, timespec.0.tv_nsec as u32)
    }
}

impl AsRef<timespec> for TimeSpec {
    fn as_ref(&self) -> &timespec {
        &self.0
    }
}

impl AsMut<timespec> for TimeSpec {
    fn as_mut(&mut self) -> &mut timespec {
        &mut self.0
    }
}

impl Ord for TimeSpec {
    // The implementation of cmp is simplified by assuming that the struct is
    // normalized.  That is, tv_nsec must always be within [0, 1_000_000_000)
    fn cmp(&self, other: &TimeSpec) -> cmp::Ordering {
        if self.tv_sec() == other.tv_sec() {
            self.tv_nsec().cmp(&other.tv_nsec())
        } else {
            self.tv_sec().cmp(&other.tv_sec())
        }
    }
}

impl PartialOrd for TimeSpec {
    fn partial_cmp(&self, other: &TimeSpec) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl TimeValLike for TimeSpec {
    #[inline]
    #[cfg_attr(
        any(target_env = "musl", target_env = "ohos"),
        allow(deprecated)
    )]
    // https://github.com/rust-lang/libc/issues/1848
    fn seconds(seconds: i64) -> TimeSpec {
        assert!(
            (TS_MIN_SECONDS..=TS_MAX_SECONDS).contains(&seconds),
            "TimeSpec out of bounds; seconds={seconds}",
        );
        let mut ts = zero_init_timespec();
        ts.tv_sec = seconds as time_t;
        TimeSpec(ts)
    }

    #[inline]
    fn milliseconds(milliseconds: i64) -> TimeSpec {
        let nanoseconds = milliseconds
            .checked_mul(1_000_000)
            .expect("TimeSpec::milliseconds out of bounds");

        TimeSpec::nanoseconds(nanoseconds)
    }

    /// Makes a new `TimeSpec` with given number of microseconds.
    #[inline]
    fn microseconds(microseconds: i64) -> TimeSpec {
        let nanoseconds = microseconds
            .checked_mul(1_000)
            .expect("TimeSpec::milliseconds out of bounds");

        TimeSpec::nanoseconds(nanoseconds)
    }

    /// Makes a new `TimeSpec` with given number of nanoseconds.
    #[inline]
    #[cfg_attr(
        any(target_env = "musl", target_env = "ohos"),
        allow(deprecated)
    )]
    // https://github.com/rust-lang/libc/issues/1848
    fn nanoseconds(nanoseconds: i64) -> TimeSpec {
        let (secs, nanos) = div_mod_floor_64(nanoseconds, NANOS_PER_SEC);
        assert!(
            (TS_MIN_SECONDS..=TS_MAX_SECONDS).contains(&secs),
            "TimeSpec out of bounds"
        );
        let mut ts = zero_init_timespec();
        ts.tv_sec = secs as time_t;
        ts.tv_nsec = nanos as timespec_tv_nsec_t;
        TimeSpec(ts)
    }

    // The cast is not unnecessary on all platforms.
    #[allow(clippy::unnecessary_cast)]
    fn num_seconds(&self) -> i64 {
        if self.tv_sec() < 0 && self.tv_nsec() > 0 {
            (self.tv_sec() + 1) as i64
        } else {
            self.tv_sec() as i64
        }
    }

    fn num_milliseconds(&self) -> i64 {
        self.num_nanoseconds() / 1_000_000
    }

    fn num_microseconds(&self) -> i64 {
        self.num_nanoseconds() / 1_000
    }

    // The cast is not unnecessary on all platforms.
    #[allow(clippy::unnecessary_cast)]
    fn num_nanoseconds(&self) -> i64 {
        let secs = self.num_seconds() * 1_000_000_000;
        let nsec = self.nanos_mod_sec();
        secs + nsec as i64
    }
}

impl TimeSpec {
    /// Leave the timestamp unchanged.
    #[cfg(not(target_os = "redox"))]
    // At the time of writing this PR, redox does not support this feature
    pub const UTIME_OMIT: TimeSpec =
        TimeSpec::new(0, libc::UTIME_OMIT as timespec_tv_nsec_t);
    /// Update the timestamp to `Now`
    // At the time of writing this PR, redox does not support this feature
    #[cfg(not(target_os = "redox"))]
    pub const UTIME_NOW: TimeSpec =
        TimeSpec::new(0, libc::UTIME_NOW as timespec_tv_nsec_t);

    /// Construct a new `TimeSpec` from its components
    #[cfg_attr(
        any(target_env = "musl", target_env = "ohos"),
        allow(deprecated)
    )] // https://github.com/rust-lang/libc/issues/1848
    pub const fn new(seconds: time_t, nanoseconds: timespec_tv_nsec_t) -> Self {
        let mut ts = zero_init_timespec();
        ts.tv_sec = seconds;
        ts.tv_nsec = nanoseconds;
        Self(ts)
    }

    fn nanos_mod_sec(&self) -> timespec_tv_nsec_t {
        if self.tv_sec() < 0 && self.tv_nsec() > 0 {
            self.tv_nsec() - NANOS_PER_SEC as timespec_tv_nsec_t
        } else {
            self.tv_nsec()
        }
    }

    #[cfg_attr(
        any(target_env = "musl", target_env = "ohos"),
        allow(deprecated)
    )] // https://github.com/rust-lang/libc/issues/1848
    pub const fn tv_sec(&self) -> time_t {
        self.0.tv_sec
    }

    pub const fn tv_nsec(&self) -> timespec_tv_nsec_t {
        self.0.tv_nsec
    }

    #[cfg_attr(
        any(target_env = "musl", target_env = "ohos"),
        allow(deprecated)
    )]
    // https://github.com/rust-lang/libc/issues/1848
    pub const fn from_duration(duration: Duration) -> Self {
        let mut ts = zero_init_timespec();
        ts.tv_sec = duration.as_secs() as time_t;
        ts.tv_nsec = duration.subsec_nanos() as timespec_tv_nsec_t;
        TimeSpec(ts)
    }

    pub const fn from_timespec(timespec: timespec) -> Self {
        Self(timespec)
    }
}

impl ops::Neg for TimeSpec {
    type Output = TimeSpec;

    fn neg(self) -> TimeSpec {
        TimeSpec::nanoseconds(-self.num_nanoseconds())
    }
}

impl ops::Add for TimeSpec {
    type Output = TimeSpec;

    fn add(self, rhs: TimeSpec) -> TimeSpec {
        TimeSpec::nanoseconds(self.num_nanoseconds() + rhs.num_nanoseconds())
    }
}

impl ops::Sub for TimeSpec {
    type Output = TimeSpec;

    fn sub(self, rhs: TimeSpec) -> TimeSpec {
        TimeSpec::nanoseconds(self.num_nanoseconds() - rhs.num_nanoseconds())
    }
}

impl ops::Mul<i32> for TimeSpec {
    type Output = TimeSpec;

    fn mul(self, rhs: i32) -> TimeSpec {
        let usec = self
            .num_nanoseconds()
            .checked_mul(i64::from(rhs))
            .expect("TimeSpec multiply out of bounds");

        TimeSpec::nanoseconds(usec)
    }
}

impl ops::Div<i32> for TimeSpec {
    type Output = TimeSpec;

    fn div(self, rhs: i32) -> TimeSpec {
        let usec = self.num_nanoseconds() / i64::from(rhs);
        TimeSpec::nanoseconds(usec)
    }
}

impl fmt::Display for TimeSpec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (abs, sign) = if self.tv_sec() < 0 {
            (-*self, "-")
        } else {
            (*self, "")
        };

        let sec = abs.tv_sec();

        write!(f, "{sign}")?;

        if abs.tv_nsec() == 0 {
            if sec == 1 {
                write!(f, "1 second")?;
            } else {
                write!(f, "{sec} seconds")?;
            }
        } else if abs.tv_nsec() % 1_000_000 == 0 {
            write!(f, "{sec}.{:03} seconds", abs.tv_nsec() / 1_000_000)?;
        } else if abs.tv_nsec() % 1_000 == 0 {
            write!(f, "{sec}.{:06} seconds", abs.tv_nsec() / 1_000)?;
        } else {
            write!(f, "{sec}.{:09} seconds", abs.tv_nsec())?;
        }

        Ok(())
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TimeVal(timeval);

const MICROS_PER_SEC: i64 = 1_000_000;

#[cfg(target_pointer_width = "64")]
const TV_MAX_SECONDS: i64 = (i64::MAX / MICROS_PER_SEC) - 1;

#[cfg(target_pointer_width = "32")]
const TV_MAX_SECONDS: i64 = isize::MAX as i64;

const TV_MIN_SECONDS: i64 = -TV_MAX_SECONDS;

impl AsRef<timeval> for TimeVal {
    fn as_ref(&self) -> &timeval {
        &self.0
    }
}

impl AsMut<timeval> for TimeVal {
    fn as_mut(&mut self) -> &mut timeval {
        &mut self.0
    }
}

impl Ord for TimeVal {
    // The implementation of cmp is simplified by assuming that the struct is
    // normalized.  That is, tv_usec must always be within [0, 1_000_000)
    fn cmp(&self, other: &TimeVal) -> cmp::Ordering {
        if self.tv_sec() == other.tv_sec() {
            self.tv_usec().cmp(&other.tv_usec())
        } else {
            self.tv_sec().cmp(&other.tv_sec())
        }
    }
}

impl PartialOrd for TimeVal {
    fn partial_cmp(&self, other: &TimeVal) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl TimeValLike for TimeVal {
    #[inline]
    fn seconds(seconds: i64) -> TimeVal {
        assert!(
            (TV_MIN_SECONDS..=TV_MAX_SECONDS).contains(&seconds),
            "TimeVal out of bounds; seconds={seconds}"
        );
        #[cfg_attr(
            any(target_env = "musl", target_env = "ohos"),
            allow(deprecated)
        )]
        // https://github.com/rust-lang/libc/issues/1848
        TimeVal(timeval {
            tv_sec: seconds as time_t,
            tv_usec: 0,
        })
    }

    #[inline]
    fn milliseconds(milliseconds: i64) -> TimeVal {
        let microseconds = milliseconds
            .checked_mul(1_000)
            .expect("TimeVal::milliseconds out of bounds");

        TimeVal::microseconds(microseconds)
    }

    /// Makes a new `TimeVal` with given number of microseconds.
    #[inline]
    fn microseconds(microseconds: i64) -> TimeVal {
        let (secs, micros) = div_mod_floor_64(microseconds, MICROS_PER_SEC);
        assert!(
            (TV_MIN_SECONDS..=TV_MAX_SECONDS).contains(&secs),
            "TimeVal out of bounds"
        );
        #[cfg_attr(
            any(target_env = "musl", target_env = "ohos"),
            allow(deprecated)
        )]
        // https://github.com/rust-lang/libc/issues/1848
        TimeVal(timeval {
            tv_sec: secs as time_t,
            tv_usec: micros as suseconds_t,
        })
    }

    /// Makes a new `TimeVal` with given number of nanoseconds.  Some precision
    /// will be lost
    #[inline]
    fn nanoseconds(nanoseconds: i64) -> TimeVal {
        let microseconds = nanoseconds / 1000;
        let (secs, micros) = div_mod_floor_64(microseconds, MICROS_PER_SEC);
        assert!(
            (TV_MIN_SECONDS..=TV_MAX_SECONDS).contains(&secs),
            "TimeVal out of bounds"
        );
        #[cfg_attr(
            any(target_env = "musl", target_env = "ohos"),
            allow(deprecated)
        )]
        // https://github.com/rust-lang/libc/issues/1848
        TimeVal(timeval {
            tv_sec: secs as time_t,
            tv_usec: micros as suseconds_t,
        })
    }

    // The cast is not unnecessary on all platforms.
    #[allow(clippy::unnecessary_cast)]
    fn num_seconds(&self) -> i64 {
        if self.tv_sec() < 0 && self.tv_usec() > 0 {
            (self.tv_sec() + 1) as i64
        } else {
            self.tv_sec() as i64
        }
    }

    fn num_milliseconds(&self) -> i64 {
        self.num_microseconds() / 1_000
    }

    // The cast is not unnecessary on all platforms.
    #[allow(clippy::unnecessary_cast)]
    fn num_microseconds(&self) -> i64 {
        let secs = self.num_seconds() * 1_000_000;
        let usec = self.micros_mod_sec();
        secs + usec as i64
    }

    fn num_nanoseconds(&self) -> i64 {
        self.num_microseconds() * 1_000
    }
}

impl TimeVal {
    /// Construct a new `TimeVal` from its components
    #[cfg_attr(
        any(target_env = "musl", target_env = "ohos"),
        allow(deprecated)
    )] // https://github.com/rust-lang/libc/issues/1848
    pub const fn new(seconds: time_t, microseconds: suseconds_t) -> Self {
        Self(timeval {
            tv_sec: seconds,
            tv_usec: microseconds,
        })
    }

    fn micros_mod_sec(&self) -> suseconds_t {
        if self.tv_sec() < 0 && self.tv_usec() > 0 {
            self.tv_usec() - MICROS_PER_SEC as suseconds_t
        } else {
            self.tv_usec()
        }
    }

    #[cfg_attr(
        any(target_env = "musl", target_env = "ohos"),
        allow(deprecated)
    )] // https://github.com/rust-lang/libc/issues/1848
    pub const fn tv_sec(&self) -> time_t {
        self.0.tv_sec
    }

    pub const fn tv_usec(&self) -> suseconds_t {
        self.0.tv_usec
    }
}

impl ops::Neg for TimeVal {
    type Output = TimeVal;

    fn neg(self) -> TimeVal {
        TimeVal::microseconds(-self.num_microseconds())
    }
}

impl ops::Add for TimeVal {
    type Output = TimeVal;

    fn add(self, rhs: TimeVal) -> TimeVal {
        TimeVal::microseconds(self.num_microseconds() + rhs.num_microseconds())
    }
}

impl ops::Sub for TimeVal {
    type Output = TimeVal;

    fn sub(self, rhs: TimeVal) -> TimeVal {
        TimeVal::microseconds(self.num_microseconds() - rhs.num_microseconds())
    }
}

impl ops::Mul<i32> for TimeVal {
    type Output = TimeVal;

    fn mul(self, rhs: i32) -> TimeVal {
        let usec = self
            .num_microseconds()
            .checked_mul(i64::from(rhs))
            .expect("TimeVal multiply out of bounds");

        TimeVal::microseconds(usec)
    }
}

impl ops::Div<i32> for TimeVal {
    type Output = TimeVal;

    fn div(self, rhs: i32) -> TimeVal {
        let usec = self.num_microseconds() / i64::from(rhs);
        TimeVal::microseconds(usec)
    }
}

impl fmt::Display for TimeVal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (abs, sign) = if self.tv_sec() < 0 {
            (-*self, "-")
        } else {
            (*self, "")
        };

        let sec = abs.tv_sec();

        write!(f, "{sign}")?;

        if abs.tv_usec() == 0 {
            if sec == 1 {
                write!(f, "1 second")?;
            } else {
                write!(f, "{sec} seconds")?;
            }
        } else if abs.tv_usec() % 1000 == 0 {
            write!(f, "{sec}.{:03} seconds", abs.tv_usec() / 1000)?;
        } else {
            write!(f, "{sec}.{:06} seconds", abs.tv_usec())?;
        }

        Ok(())
    }
}

impl From<timeval> for TimeVal {
    fn from(tv: timeval) -> Self {
        TimeVal(tv)
    }
}

#[inline]
fn div_mod_floor_64(this: i64, other: i64) -> (i64, i64) {
    (div_floor_64(this, other), mod_floor_64(this, other))
}

#[inline]
fn div_floor_64(this: i64, other: i64) -> i64 {
    match div_rem_64(this, other) {
        (d, r) if (r > 0 && other < 0) || (r < 0 && other > 0) => d - 1,
        (d, _) => d,
    }
}

#[inline]
fn mod_floor_64(this: i64, other: i64) -> i64 {
    match this % other {
        r if (r > 0 && other < 0) || (r < 0 && other > 0) => r + other,
        r => r,
    }
}

#[inline]
fn div_rem_64(this: i64, other: i64) -> (i64, i64) {
    (this / other, this % other)
}
