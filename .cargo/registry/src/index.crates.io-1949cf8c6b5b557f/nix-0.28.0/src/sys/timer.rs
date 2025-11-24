//! Timer API via signals.
//!
//! Timer is a POSIX API to create timers and get expiration notifications
//! through queued Unix signals, for the current process. This is similar to
//! Linux's timerfd mechanism, except that API is specific to Linux and makes
//! use of file polling.
//!
//! For more documentation, please read [timer_create](https://pubs.opengroup.org/onlinepubs/9699919799/functions/timer_create.html).
//!
//! # Examples
//!
//! Create an interval timer that signals SIGALARM every 250 milliseconds.
//!
//! ```no_run
//! use nix::sys::signal::{self, SigEvent, SigHandler, SigevNotify, Signal};
//! use nix::sys::timer::{Expiration, Timer, TimerSetTimeFlags};
//! use nix::time::ClockId;
//! use std::convert::TryFrom;
//! use std::sync::atomic::{AtomicU64, Ordering};
//! use std::thread::yield_now;
//! use std::time::Duration;
//!
//! const SIG: Signal = Signal::SIGALRM;
//! static ALARMS: AtomicU64 = AtomicU64::new(0);
//!
//! extern "C" fn handle_alarm(signal: libc::c_int) {
//!     let signal = Signal::try_from(signal).unwrap();
//!     if signal == SIG {
//!         ALARMS.fetch_add(1, Ordering::Relaxed);
//!     }
//! }
//!
//! fn main() {
//!     let clockid = ClockId::CLOCK_MONOTONIC;
//!     let sigevent = SigEvent::new(SigevNotify::SigevSignal {
//!         signal: SIG,
//!         si_value: 0,
//!     });
//!
//!     let mut timer = Timer::new(clockid, sigevent).unwrap();
//!     let expiration = Expiration::Interval(Duration::from_millis(250).into());
//!     let flags = TimerSetTimeFlags::empty();
//!     timer.set(expiration, flags).expect("could not set timer");
//!
//!     let handler = SigHandler::Handler(handle_alarm);
//!     unsafe { signal::signal(SIG, handler) }.unwrap();
//!
//!     loop {
//!         let alarms = ALARMS.load(Ordering::Relaxed);
//!         if alarms >= 10 {
//!             println!("total alarms handled: {}", alarms);
//!             break;
//!         }
//!         yield_now()
//!     }
//! }
//! ```
use crate::sys::signal::SigEvent;
use crate::sys::time::timer::TimerSpec;
pub use crate::sys::time::timer::{Expiration, TimerSetTimeFlags};
use crate::time::ClockId;
use crate::{errno::Errno, Result};
use core::mem;

/// A Unix signal per-process timer.
#[derive(Debug)]
#[repr(transparent)]
pub struct Timer(libc::timer_t);

impl Timer {
    /// Creates a new timer based on the clock defined by `clockid`. The details
    /// of the signal and its handler are defined by the passed `sigevent`.
    #[doc(alias("timer_create"))]
    pub fn new(clockid: ClockId, mut sigevent: SigEvent) -> Result<Self> {
        let mut timer_id: mem::MaybeUninit<libc::timer_t> =
            mem::MaybeUninit::uninit();
        Errno::result(unsafe {
            libc::timer_create(
                clockid.as_raw(),
                sigevent.as_mut_ptr(),
                timer_id.as_mut_ptr(),
            )
        })
        .map(|_| {
            // SAFETY: libc::timer_create is responsible for initializing
            // timer_id.
            unsafe { Self(timer_id.assume_init()) }
        })
    }

    /// Set a new alarm on the timer.
    ///
    /// # Types of alarm
    ///
    /// There are 3 types of alarms you can set:
    ///
    ///   - one shot: the alarm will trigger once after the specified amount of
    /// time.
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
    /// # Disabling alarms
    ///
    /// Note: Only one alarm can be set for any given timer. Setting a new alarm
    /// actually removes the previous one.
    ///
    /// Note: Setting a one shot alarm with a 0s TimeSpec disable the alarm
    /// altogether.
    #[doc(alias("timer_settime"))]
    pub fn set(
        &mut self,
        expiration: Expiration,
        flags: TimerSetTimeFlags,
    ) -> Result<()> {
        let timerspec: TimerSpec = expiration.into();
        Errno::result(unsafe {
            libc::timer_settime(
                self.0,
                flags.bits(),
                timerspec.as_ref(),
                core::ptr::null_mut(),
            )
        })
        .map(drop)
    }

    /// Get the parameters for the alarm currently set, if any.
    #[doc(alias("timer_gettime"))]
    pub fn get(&self) -> Result<Option<Expiration>> {
        let mut timerspec = TimerSpec::none();
        Errno::result(unsafe {
            libc::timer_gettime(self.0, timerspec.as_mut())
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

    /// Return the number of timers that have overrun
    ///
    /// Each timer is able to queue one signal to the process at a time, meaning
    /// if the signal is not handled before the next expiration the timer has
    /// 'overrun'. This function returns how many times that has happened to
    /// this timer, up to `libc::DELAYTIMER_MAX`. If more than the maximum
    /// number of overruns have happened the return is capped to the maximum.
    #[doc(alias("timer_getoverrun"))]
    pub fn overruns(&self) -> i32 {
        unsafe { libc::timer_getoverrun(self.0) }
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            let result = Errno::result(unsafe { libc::timer_delete(self.0) });
            if let Err(Errno::EINVAL) = result {
                panic!("close of Timer encountered EINVAL");
            }
        }
    }
}
