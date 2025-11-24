//! Tests for the shutdown.
//!
//! The tests work like this:
//!
//! * The register an alarm, to fail if anything takes too long (which is very much possible here).
//! * A fork is done, with the child registering a signal with a NOP and cleanup operation (one or
//!   the other).
//! * The child puts some kind of infinite loop or sleep inside itself, so it never actually
//!   terminates on the first, but would terminate after the signal.

#![cfg(not(windows))] // Forks don't work on Windows, but windows has the same implementation.

use std::io::Error;
use std::ptr;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use signal_hook::consts::signal::*;
use signal_hook::flag;
use signal_hook::low_level;

fn do_test<C: FnOnce()>(child: C) {
    unsafe {
        libc::alarm(10); // Time out the test after 10 seconds and get it killed.
        match libc::fork() {
            -1 => panic!("Fork failed: {}", Error::last_os_error()),
            0 => {
                child();
                loop {
                    thread::sleep(Duration::from_secs(1));
                }
            }
            pid => {
                // Give the child some time to register signals and stuff
                // We could actually signal that the child is ready by it eg. closing STDOUT, but
                // this is just a test so we don't really bother.
                thread::sleep(Duration::from_millis(250));
                libc::kill(pid, libc::SIGTERM);
                // Wait a small bit to make sure the signal got delivered.
                thread::sleep(Duration::from_millis(50));
                // The child is still running, because the first signal got "handled" by being
                // ignored.
                let terminated = libc::waitpid(pid, ptr::null_mut(), libc::WNOHANG);
                assert_eq!(0, terminated, "Process {} terminated prematurely", pid);
                // But it terminates on the second attempt (we do block on wait here).
                libc::kill(pid, libc::SIGTERM);
                let terminated = libc::waitpid(pid, ptr::null_mut(), 0);
                assert_eq!(pid, terminated);
            }
        }
    }
}

/// Use automatic cleanup inside the signal handler to get rid of old signals, the aggressive way.
#[test]
fn cleanup_inside_signal() {
    fn hook() {
        // Make sure we have some signal handler, not the default.
        unsafe { low_level::register(SIGTERM, || ()).unwrap() };
        let shutdown_cond = Arc::new(AtomicBool::new(false));
        // „disarmed“ shutdown
        flag::register_conditional_shutdown(SIGTERM, 0, Arc::clone(&shutdown_cond)).unwrap();
        // But arm at the first SIGTERM
        flag::register(SIGTERM, shutdown_cond).unwrap();
    }
    do_test(hook);
}

/// Manually remove the signal handler just after receiving the signal but before going into an
/// infinite loop.
#[test]
fn cleanup_after_signal() {
    fn hook() {
        let mut signals = signal_hook::iterator::Signals::new(&[libc::SIGTERM]).unwrap();
        assert_eq!(Some(SIGTERM), signals.into_iter().next());
        flag::register_conditional_shutdown(SIGTERM, 0, Arc::new(AtomicBool::new(true))).unwrap();
    }
    do_test(hook);
}
