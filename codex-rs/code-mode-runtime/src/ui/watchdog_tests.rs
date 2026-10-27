#![cfg(test)]
use super::*;
use pretty_assertions::assert_eq;
use std::sync::mpsc;

#[test]
fn disarm_waits_for_an_interrupt_in_progress() {
    crate::v8_init::ensure_v8_initialized().unwrap();
    let isolate = v8::Isolate::new(Default::default());
    let watchdog = IsolateExecutionWatchdog {
        observer: Observer(Arc::new((Mutex::new(Execution::default()), Condvar::new()))),
        thread: None,
    };
    watchdog.arm(/*instance*/ 1, isolate.thread_safe_handle());
    let (lock, _) = &*watchdog.observer.0;
    let mut interrupt = lock.lock().unwrap();
    std::thread::scope(|scope| {
        let (attempt_tx, attempt_rx) = mpsc::sync_channel(0);
        let watchdog = &watchdog;
        let disarming = scope.spawn(move || {
            watchdog.disarm_with(|lock| {
                assert!(matches!(
                    lock.try_lock(),
                    Err(std::sync::TryLockError::WouldBlock)
                ));
                attempt_tx.send(()).unwrap();
                lock.lock().unwrap()
            })
        });
        attempt_rx.recv().unwrap();
        interrupt.active.as_mut().unwrap().terminated = true;
        drop(interrupt);
        assert_eq!(
            disarming.join().unwrap().unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
    });
}
