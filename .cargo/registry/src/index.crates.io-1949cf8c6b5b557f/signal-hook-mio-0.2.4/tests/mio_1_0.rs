#![cfg(feature = "support-v1_0")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use mio_1_0::{Events, Interest, Poll, Token};

use signal_hook::consts::{SIGUSR1, SIGUSR2};
use signal_hook::low_level::raise;
use signal_hook_mio::v1_0::Signals;

use serial_test::serial;

use libc::c_int;

#[test]
#[serial]
fn mio_wakeup() {
    let mut signals = Signals::new(&[SIGUSR1]).unwrap();
    let mut poll = Poll::new().unwrap();
    let token = Token(0);
    poll.registry()
        .register(&mut signals, token, Interest::READABLE)
        .unwrap();

    let mut events = Events::with_capacity(10);

    // The self pipe shouldn't be readable yet
    poll.poll(&mut events, Some(Duration::from_secs(0)))
        .unwrap();
    assert!(events.is_empty());

    raise(SIGUSR1).unwrap();
    poll.poll(&mut events, Some(Duration::from_secs(10)))
        .unwrap();
    let event = events.iter().next().unwrap();

    assert!(event.is_readable());
    assert_eq!(token, event.token());
    let sig = signals.pending().next().unwrap();
    assert_eq!(SIGUSR1, sig);

    // The self pipe shouldn't be readable after consuming signals
    poll.poll(&mut events, Some(Duration::from_secs(0)))
        .unwrap();
    assert!(events.is_empty());
}

#[test]
#[serial]
fn mio_multiple_signals() {
    let mut signals = Signals::new(&[SIGUSR1, SIGUSR2]).unwrap();
    let mut poll = Poll::new().unwrap();
    let token = Token(0);
    poll.registry()
        .register(&mut signals, token, Interest::READABLE)
        .unwrap();

    let mut events = Events::with_capacity(10);

    raise(SIGUSR1).unwrap();
    raise(SIGUSR2).unwrap();

    poll.poll(&mut events, Some(Duration::from_secs(10)))
        .unwrap();

    let event = events.iter().next().unwrap();
    assert!(event.is_readable());

    let sigs: Vec<c_int> = signals.pending().collect();
    assert_eq!(2, sigs.len());
    assert!(sigs.contains(&SIGUSR1));
    assert!(sigs.contains(&SIGUSR2));

    // The self pipe shouldn't be completely empty after calling pending()
    poll.poll(&mut events, Some(Duration::from_secs(0)))
        .unwrap();
    assert!(events.is_empty());
}

#[test]
#[serial]
fn mio_parallel_multiple() {
    let mut signals = Signals::new(&[SIGUSR1]).unwrap();
    let mut poll = Poll::new().unwrap();
    let token = Token(0);
    poll.registry()
        .register(&mut signals, token, Interest::READABLE)
        .unwrap();

    let mut events = Events::with_capacity(10);

    let thread_done = Arc::new(AtomicBool::new(false));

    let done = Arc::clone(&thread_done);
    thread::spawn(move || {
        for _ in 0..10 {
            // Wait some time to allow main thread to poll
            thread::sleep(Duration::from_millis(25));
            raise(SIGUSR1).unwrap();
        }
        done.store(true, Ordering::SeqCst);

        // Raise a final signal so the main thread wakes up
        // if it already called poll.
        raise(SIGUSR1).unwrap();
    });

    while !thread_done.load(Ordering::SeqCst) {
        poll.poll(&mut events, Some(Duration::from_secs(10)))
            .unwrap();
        let event = events.iter().next().unwrap();
        assert!(event.is_readable());
        assert_eq!(SIGUSR1, signals.pending().next().unwrap());
    }
}
