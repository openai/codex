//! One observer interrupts the active JS turn; disarming and interruption share a lock.

use std::io;
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::thread::JoinHandle;
use std::time::Duration;
use std::time::Instant;

struct Active {
    instance: u64,
    handle: v8::IsolateHandle,
    deadline: Instant,
    terminated: bool,
}

#[derive(Default)]
struct Execution {
    active: Option<Active>,
    stopped: bool,
}

type Shared = Arc<(Mutex<Execution>, Condvar)>;

#[derive(Clone)]
pub(super) struct Observer(Shared);

pub(super) struct IsolateExecutionWatchdog {
    pub(super) observer: Observer,
    thread: Option<JoinHandle<()>>,
}

impl IsolateExecutionWatchdog {
    pub(super) fn new() -> io::Result<Self> {
        let shared = Arc::new((Mutex::new(Execution::default()), Condvar::new()));
        let observer = Observer(Arc::clone(&shared));
        let thread = std::thread::Builder::new()
            .name("ui-plugin-watchdog".into())
            .spawn(move || {
                let (lock, changed) = &*shared;
                let mut state = lock
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                while !state.stopped {
                    if let Some(active) = &mut state.active
                        && !active.terminated
                    {
                        let remaining = active.deadline.saturating_duration_since(Instant::now());
                        if remaining.is_zero() {
                            active.handle.terminate_execution();
                            active.terminated = true;
                        } else {
                            (state, _) = changed
                                .wait_timeout(state, remaining)
                                .unwrap_or_else(std::sync::PoisonError::into_inner);
                            continue;
                        }
                    }
                    state = changed
                        .wait(state)
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                }
            })?;
        Ok(Self {
            observer,
            thread: Some(thread),
        })
    }

    pub(super) fn arm(&self, instance: u64, handle: v8::IsolateHandle) {
        let (lock, changed) = &*self.observer.0;
        let mut state = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert!(state.active.is_none());
        state.active = Some(Active {
            instance,
            handle,
            deadline: Instant::now() + Duration::from_millis(100),
            terminated: false,
        });
        changed.notify_one();
    }

    pub(super) fn disarm(&self) -> io::Result<()> {
        self.disarm_with(|lock| {
            lock.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        })
    }

    fn disarm_with<'a>(
        &'a self,
        acquire: impl FnOnce(&'a Mutex<Execution>) -> MutexGuard<'a, Execution>,
    ) -> io::Result<()> {
        let (lock, changed) = &*self.observer.0;
        let active = acquire(lock).active.take();
        changed.notify_one();
        if active.is_some_and(|active| active.terminated) {
            Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "plugin execution deadline exceeded",
            ))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "watchdog_tests.rs"]
mod tests;

impl Observer {
    pub(super) fn cancel(&self, instance: u64) {
        let (lock, changed) = &*self.0;
        let mut state = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(active) = &mut state.active
            && active.instance == instance
        {
            active.handle.terminate_execution();
            active.terminated = true;
        }
        changed.notify_one();
    }
}

impl Drop for IsolateExecutionWatchdog {
    fn drop(&mut self) {
        let (lock, changed) = &*self.observer.0;
        lock.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stopped = true;
        changed.notify_one();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
