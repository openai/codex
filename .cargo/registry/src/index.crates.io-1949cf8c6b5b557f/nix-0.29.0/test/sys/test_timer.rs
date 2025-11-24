use nix::sys::signal::{
    sigaction, SaFlags, SigAction, SigEvent, SigHandler, SigSet, SigevNotify,
    Signal,
};
use nix::sys::timer::{Expiration, Timer, TimerSetTimeFlags};
use nix::time::ClockId;
use std::convert::TryFrom;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const SIG: Signal = Signal::SIGALRM;
static ALARM_CALLED: AtomicBool = AtomicBool::new(false);

pub extern "C" fn handle_sigalarm(raw_signal: libc::c_int) {
    let signal = Signal::try_from(raw_signal).unwrap();
    if signal == SIG {
        ALARM_CALLED.store(true, Ordering::Release);
    }
}

#[test]
fn alarm_fires() {
    // Avoid interfering with other signal using tests by taking a mutex shared
    // among other tests in this crate.
    let _m = crate::SIGNAL_MTX.lock();
    const TIMER_PERIOD: Duration = Duration::from_millis(100);

    //
    // Setup
    //

    // Create a handler for the test signal, `SIG`. The handler is responsible
    // for flipping `ALARM_CALLED`.
    let handler = SigHandler::Handler(handle_sigalarm);
    let signal_action =
        SigAction::new(handler, SaFlags::SA_RESTART, SigSet::empty());
    let old_handler = unsafe {
        sigaction(SIG, &signal_action)
            .expect("unable to set signal handler for alarm")
    };

    // Create the timer. We use the monotonic clock here, though any would do
    // really. The timer is set to fire every 250 milliseconds with no delay for
    // the initial firing.
    let clockid = ClockId::CLOCK_MONOTONIC;
    let sigevent = SigEvent::new(SigevNotify::SigevSignal {
        signal: SIG,
        si_value: 0,
    });
    let mut timer =
        Timer::new(clockid, sigevent).expect("failed to create timer");
    let expiration = Expiration::Interval(TIMER_PERIOD.into());
    let flags = TimerSetTimeFlags::empty();
    timer.set(expiration, flags).expect("could not set timer");

    //
    // Test
    //

    // Determine that there's still an expiration tracked by the
    // timer. Depending on when this runs either an `Expiration::Interval` or
    // `Expiration::IntervalDelayed` will be present. That is, if the timer has
    // not fired yet we'll get our original `expiration`, else the one that
    // represents a delay to the next expiration. We're only interested in the
    // timer still being extant.
    match timer.get() {
        Ok(Some(exp)) => assert!(matches!(
            exp,
            Expiration::Interval(..) | Expiration::IntervalDelayed(..)
        )),
        _ => panic!("timer lost its expiration"),
    }

    // Wait for 2 firings of the alarm before checking that it has fired and
    // been handled at least the once. If we wait for 3 seconds and the handler
    // is never called something has gone sideways and the test fails.
    let starttime = Instant::now();
    loop {
        thread::sleep(2 * TIMER_PERIOD);
        if ALARM_CALLED.load(Ordering::Acquire) {
            break;
        }
        if starttime.elapsed() > Duration::from_secs(3) {
            panic!("Timeout waiting for SIGALRM");
        }
    }

    // Cleanup:
    // 1) deregister the OS's timer.
    // 2) Wait for a full timer period, since POSIX does not require that
    //    disabling the timer will clear pending signals, and on NetBSD at least
    //    it does not.
    // 2) Replace the old signal handler now that we've completed the test. If
    //    the test fails this process panics, so the fact we might not get here
    //    is okay.
    drop(timer);
    thread::sleep(TIMER_PERIOD);
    unsafe {
        sigaction(SIG, &old_handler).expect("unable to reset signal handler");
    }
}
