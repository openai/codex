#![cfg(not(windows))]

extern crate signal_hook;

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use signal_hook::consts::{SIGUSR1, SIGUSR2};
use signal_hook::iterator::{Handle, Signals};
use signal_hook::low_level::raise;

use serial_test::serial;

fn send_sigusr1() {
    raise(SIGUSR1).unwrap();
}

fn send_sigusr2() {
    raise(SIGUSR2).unwrap();
}

fn setup_without_any_signals() -> (Signals, Handle) {
    let signals = Signals::new(&[]).unwrap();
    let controller = signals.handle();
    (signals, controller)
}

fn setup_for_sigusr2() -> (Signals, Handle) {
    let signals = Signals::new(&[SIGUSR2]).unwrap();
    let controller = signals.handle();
    (signals, controller)
}

macro_rules! assert_signals {
    ($actual:expr, $($expected:expr),+ $(,)?) => {
        let actual = $actual.collect::<HashSet<libc::c_int>>();
        let expected = vec!($($expected),+).into_iter().collect::<HashSet<libc::c_int>>();
        assert_eq!(actual, expected);
    };
}

macro_rules! assert_no_signals {
    ($signals:expr) => {
        assert_eq!($signals.next(), None);
    };
}

#[test]
#[serial]
fn forever_terminates_when_closed() {
    let (mut signals, controller) = setup_for_sigusr2();

    // Detect early terminations.
    let stopped = Arc::new(AtomicBool::new(false));

    let stopped_bg = Arc::clone(&stopped);
    let thread = thread::spawn(move || {
        // Eat all the signals there are (might come from a concurrent test, in theory).
        // Would wait forever, but it should be terminated by the close below.
        for _sig in &mut signals {}

        stopped_bg.store(true, Ordering::SeqCst);
    });

    // Wait a bit to see if the thread terminates by itself.
    thread::sleep(Duration::from_millis(100));
    assert!(!stopped.load(Ordering::SeqCst));

    controller.close();

    thread.join().unwrap();

    assert!(stopped.load(Ordering::SeqCst));
}

// A reproducer for #16: if we had the mio-support enabled (which is enabled also by the
// tokio-support feature), blocking no longer works. The .wait() would return immediately (an empty
// iterator, possibly), .forever() would do a busy loop.
// flag)
#[test]
#[serial]
fn signals_block_wait() {
    let mut signals = Signals::new(&[SIGUSR2]).unwrap();
    let (s, r) = mpsc::channel();
    let finish = Arc::new(AtomicBool::new(false));
    let thread_id = thread::spawn({
        let finish = Arc::clone(&finish);
        move || {
            // Technically, it may spuriously return early. But it shouldn't be doing it too much,
            // so we just try to wait multiple times ‒ if they *all* return right away, it is
            // broken.
            for _ in 0..10 {
                for _ in signals.wait() {
                    if finish.load(Ordering::SeqCst) {
                        // Asked to terminate at the end of the thread. Do so (but without
                        // signalling the receipt).
                        return;
                    } else {
                        panic!("Someone really did send us SIGUSR2, which breaks the test");
                    }
                }
            }
            let _ = s.send(());
        }
    });

    // A RAII guard to make sure we shut down the thread even if the test fails.
    struct ThreadGuard {
        thread: Option<JoinHandle<()>>,
        finish: Arc<AtomicBool>,
    }

    impl ThreadGuard {
        fn shutdown(&mut self) {
            // Tell it to shut down
            self.finish.store(true, Ordering::SeqCst);
            // Wake it up
            send_sigusr2();
            // Wait for it to actually terminate.
            if let Some(thread) = self.thread.take() {
                thread.join().unwrap(); // Propagate panics
            }
        }
    }

    impl Drop for ThreadGuard {
        fn drop(&mut self) {
            self.shutdown(); // OK if done twice, won't have the thread any more.
        }
    }

    let mut bg_thread = ThreadGuard {
        thread: Some(thread_id),
        finish,
    };

    let err = r
        .recv_timeout(Duration::from_millis(100))
        .expect_err("Wait didn't wait properly");
    assert_eq!(err, RecvTimeoutError::Timeout);

    bg_thread.shutdown();
}

#[test]
#[serial]
fn pending_doesnt_block() {
    let (mut signals, _) = setup_for_sigusr2();

    let mut recieved_signals = signals.pending();

    assert_no_signals!(recieved_signals);
}

#[test]
#[serial]
fn wait_returns_recieved_signals() {
    let (mut signals, _) = setup_for_sigusr2();
    send_sigusr2();

    let recieved_signals = signals.wait();

    assert_signals!(recieved_signals, SIGUSR2);
}

#[test]
#[serial]
fn forever_returns_recieved_signals() {
    let (mut signals, _) = setup_for_sigusr2();
    send_sigusr2();

    let signal = signals.forever().take(1);

    assert_signals!(signal, SIGUSR2);
}

#[test]
#[serial]
fn wait_doesnt_block_when_closed() {
    let (mut signals, controller) = setup_for_sigusr2();
    controller.close();

    let mut recieved_signals = signals.wait();

    assert_no_signals!(recieved_signals);
}

#[test]
#[serial]
fn wait_unblocks_when_closed() {
    let (mut signals, controller) = setup_without_any_signals();

    let thread = thread::spawn(move || {
        signals.wait();
    });

    controller.close();

    thread.join().unwrap();
}

#[test]
#[serial]
fn forever_doesnt_block_when_closed() {
    let (mut signals, controller) = setup_for_sigusr2();
    controller.close();

    let mut signal = signals.forever();

    assert_no_signals!(signal);
}

#[test]
#[serial]
fn add_signal_after_creation() {
    let (mut signals, _) = setup_without_any_signals();
    signals.add_signal(SIGUSR1).unwrap();

    send_sigusr1();

    assert_signals!(signals.pending(), SIGUSR1);
}

#[test]
#[serial]
fn delayed_signal_consumed() {
    let (mut signals, _) = setup_for_sigusr2();
    signals.add_signal(SIGUSR1).unwrap();

    send_sigusr1();
    let mut recieved_signals = signals.wait();
    send_sigusr2();

    assert_signals!(recieved_signals, SIGUSR1, SIGUSR2);

    // The pipe still contains the byte from the second
    // signal and so wait won't block but won't return
    // a signal.
    recieved_signals = signals.wait();
    assert_no_signals!(recieved_signals);
}

#[test]
#[serial]
fn is_closed_initially_returns_false() {
    let (_, controller) = setup_for_sigusr2();

    assert!(!controller.is_closed());
}

#[test]
#[serial]
fn is_closed_returns_true_when_closed() {
    let (_, controller) = setup_for_sigusr2();
    controller.close();

    assert!(controller.is_closed());
}
