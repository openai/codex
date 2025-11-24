//! Tests for the [unregister_signal] function.
//!
//! As a separate integration level test, so it doesn't clash with other tests on the signals.

// The unregister_signal itself is deprecated. But we still want to test it, so it's not deprecated
// and broken at the same time.
#![allow(deprecated)]

extern crate libc;
extern crate signal_hook_registry;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use libc::{SIGINT, SIGTERM}; // We'll use these here because SIGUSR1 is not available on Windows.
use signal_hook_registry::{register, unregister_signal};

#[test]
fn register_unregister() {
    let called = Arc::new(AtomicUsize::new(0));

    let hook = {
        let called = Arc::clone(&called);
        move || {
            called.fetch_add(1, Ordering::Relaxed);
        }
    };

    unsafe {
        register(SIGTERM, hook.clone()).unwrap();
        register(SIGTERM, hook.clone()).unwrap();
        register(SIGINT, hook.clone()).unwrap();

        libc::raise(SIGTERM);
    }

    // The closure is run twice.
    assert_eq!(2, called.load(Ordering::Relaxed));

    assert!(unregister_signal(SIGTERM));

    unsafe { libc::raise(SIGTERM) };
    // Second one unregisters nothing.
    assert!(!unregister_signal(SIGTERM));

    // After unregistering (both), it is no longer run at all.
    assert_eq!(2, called.load(Ordering::Relaxed));

    // The SIGINT one is not disturbed.
    unsafe { libc::raise(SIGINT) };
    assert_eq!(3, called.load(Ordering::Relaxed));

    // But it's possible to register it again just fine.
    unsafe {
        register(SIGTERM, hook).unwrap();
        libc::raise(SIGTERM);
    }
    assert_eq!(4, called.load(Ordering::Relaxed));
}
