use crate::loom::sync::{Mutex, MutexGuard};
use crate::runtime::signal::Handle as SignalHandle;
use crate::signal::unix::{signal_with_handle, SignalKind};
use crate::sync::watch;
use std::io;
use std::process::ExitStatus;

/// An interface for waiting on a process to exit.
pub(crate) trait Wait {
    /// Get the identifier for this process or diagnostics.
    #[allow(dead_code)]
    fn id(&self) -> u32;
    /// Try waiting for a process to exit in a non-blocking manner.
    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>>;
}

impl<T: Wait> Wait for &mut T {
    fn id(&self) -> u32 {
        (**self).id()
    }

    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        (**self).try_wait()
    }
}

/// An interface for queueing up an orphaned process so that it can be reaped.
pub(crate) trait OrphanQueue<T> {
    /// Adds an orphan to the queue.
    fn push_orphan(&self, orphan: T);
}

impl<T, O: OrphanQueue<T>> OrphanQueue<T> for &O {
    fn push_orphan(&self, orphan: T) {
        (**self).push_orphan(orphan);
    }
}

/// An implementation of `OrphanQueue`.
#[derive(Debug)]
pub(crate) struct OrphanQueueImpl<T> {
    sigchild: Mutex<Option<watch::Receiver<()>>>,
    queue: Mutex<Vec<T>>,
}

impl<T> OrphanQueueImpl<T> {
    cfg_not_has_const_mutex_new! {
        pub(crate) fn new() -> Self {
            Self {
                sigchild: Mutex::new(None),
                queue: Mutex::new(Vec::new()),
            }
        }
    }

    cfg_has_const_mutex_new! {
        pub(crate) const fn new() -> Self {
            Self {
                sigchild: Mutex::const_new(None),
                queue: Mutex::const_new(Vec::new()),
            }
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.queue.lock().len()
    }

    pub(crate) fn push_orphan(&self, orphan: T)
    where
        T: Wait,
    {
        self.queue.lock().push(orphan);
    }

    /// Attempts to reap every process in the queue, ignoring any errors and
    /// enqueueing any orphans which have not yet exited.
    pub(crate) fn reap_orphans(&self, handle: &SignalHandle)
    where
        T: Wait,
    {
        // If someone else is holding the lock, they will be responsible for draining
        // the queue as necessary, so we can safely bail if that happens
        if let Some(mut sigchild_guard) = self.sigchild.try_lock() {
            match &mut *sigchild_guard {
                Some(sigchild) => {
                    if sigchild.try_has_changed().and_then(Result::ok).is_some() {
                        drain_orphan_queue(self.queue.lock());
                    }
                }
                None => {
                    let queue = self.queue.lock();

                    // Be lazy and only initialize the SIGCHLD listener if there
                    // are any orphaned processes in the queue.
                    if !queue.is_empty() {
                        // An errors shouldn't really happen here, but if it does it
                        // means that the signal driver isn't running, in
                        // which case there isn't anything we can
                        // register/initialize here, so we can try again later
                        if let Ok(sigchild) = signal_with_handle(SignalKind::child(), handle) {
                            *sigchild_guard = Some(sigchild);
                            drain_orphan_queue(queue);
                        }
                    }
                }
            }
        }
    }
}

fn drain_orphan_queue<T>(mut queue: MutexGuard<'_, Vec<T>>)
where
    T: Wait,
{
    for i in (0..queue.len()).rev() {
        match queue[i].try_wait() {
            Ok(None) => {}
            Ok(Some(_)) | Err(_) => {
                // The stdlib handles interruption errors (EINTR) when polling a child process.
                // All other errors represent invalid inputs or pids that have already been
                // reaped, so we can drop the orphan in case an error is raised.
                queue.swap_remove(i);
            }
        }
    }

    drop(queue);
}

#[cfg(all(test, not(loom)))]
pub(crate) mod test {
    use super::*;
    use crate::runtime::io::Driver as IoDriver;
    use crate::runtime::signal::{Driver as SignalDriver, Handle as SignalHandle};
    use crate::sync::watch;
    use std::cell::{Cell, RefCell};
    use std::io;
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;
    use std::rc::Rc;

    pub(crate) struct MockQueue<W> {
        pub(crate) all_enqueued: RefCell<Vec<W>>,
    }

    impl<W> MockQueue<W> {
        pub(crate) fn new() -> Self {
            Self {
                all_enqueued: RefCell::new(Vec::new()),
            }
        }
    }

    impl<W> OrphanQueue<W> for MockQueue<W> {
        fn push_orphan(&self, orphan: W) {
            self.all_enqueued.borrow_mut().push(orphan);
        }
    }

    struct MockWait {
        total_waits: Rc<Cell<usize>>,
        num_wait_until_status: usize,
        return_err: bool,
    }

    impl MockWait {
        fn new(num_wait_until_status: usize) -> Self {
            Self {
                total_waits: Rc::new(Cell::new(0)),
                num_wait_until_status,
                return_err: false,
            }
        }

        fn with_err() -> Self {
            Self {
                total_waits: Rc::new(Cell::new(0)),
                num_wait_until_status: 0,
                return_err: true,
            }
        }
    }

    impl Wait for MockWait {
        fn id(&self) -> u32 {
            42
        }

        fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
            let waits = self.total_waits.get();

            let ret = if self.num_wait_until_status == waits {
                if self.return_err {
                    Ok(Some(ExitStatus::from_raw(0)))
                } else {
                    Err(io::Error::new(io::ErrorKind::Other, "mock err"))
                }
            } else {
                Ok(None)
            };

            self.total_waits.set(waits + 1);
            ret
        }
    }

    #[test]
    fn drain_attempts_a_single_reap_of_all_queued_orphans() {
        let first_orphan = MockWait::new(0);
        let second_orphan = MockWait::new(1);
        let third_orphan = MockWait::new(2);
        let fourth_orphan = MockWait::with_err();

        let first_waits = first_orphan.total_waits.clone();
        let second_waits = second_orphan.total_waits.clone();
        let third_waits = third_orphan.total_waits.clone();
        let fourth_waits = fourth_orphan.total_waits.clone();

        let orphanage = OrphanQueueImpl::new();
        orphanage.push_orphan(first_orphan);
        orphanage.push_orphan(third_orphan);
        orphanage.push_orphan(second_orphan);
        orphanage.push_orphan(fourth_orphan);

        assert_eq!(orphanage.len(), 4);

        drain_orphan_queue(orphanage.queue.lock());
        assert_eq!(orphanage.len(), 2);
        assert_eq!(first_waits.get(), 1);
        assert_eq!(second_waits.get(), 1);
        assert_eq!(third_waits.get(), 1);
        assert_eq!(fourth_waits.get(), 1);

        drain_orphan_queue(orphanage.queue.lock());
        assert_eq!(orphanage.len(), 1);
        assert_eq!(first_waits.get(), 1);
        assert_eq!(second_waits.get(), 2);
        assert_eq!(third_waits.get(), 2);
        assert_eq!(fourth_waits.get(), 1);

        drain_orphan_queue(orphanage.queue.lock());
        assert_eq!(orphanage.len(), 0);
        assert_eq!(first_waits.get(), 1);
        assert_eq!(second_waits.get(), 2);
        assert_eq!(third_waits.get(), 3);
        assert_eq!(fourth_waits.get(), 1);

        // Safe to reap when empty
        drain_orphan_queue(orphanage.queue.lock());
    }

    #[test]
    fn no_reap_if_no_signal_received() {
        let (tx, rx) = watch::channel(());

        let handle = SignalHandle::default();

        let orphanage = OrphanQueueImpl::new();
        *orphanage.sigchild.lock() = Some(rx);

        let orphan = MockWait::new(2);
        let waits = orphan.total_waits.clone();
        orphanage.push_orphan(orphan);

        orphanage.reap_orphans(&handle);
        assert_eq!(waits.get(), 0);

        orphanage.reap_orphans(&handle);
        assert_eq!(waits.get(), 0);

        tx.send(()).unwrap();
        orphanage.reap_orphans(&handle);
        assert_eq!(waits.get(), 1);
    }

    #[test]
    fn no_reap_if_signal_lock_held() {
        let handle = SignalHandle::default();

        let orphanage = OrphanQueueImpl::new();
        let signal_guard = orphanage.sigchild.lock();

        let orphan = MockWait::new(2);
        let waits = orphan.total_waits.clone();
        orphanage.push_orphan(orphan);

        orphanage.reap_orphans(&handle);
        assert_eq!(waits.get(), 0);

        drop(signal_guard);
    }

    #[cfg_attr(miri, ignore)] // Miri does not support epoll.
    #[test]
    fn does_not_register_signal_if_queue_empty() {
        let (io_driver, io_handle) = IoDriver::new(1024).unwrap();
        let signal_driver = SignalDriver::new(io_driver, &io_handle).unwrap();
        let handle = signal_driver.handle();

        let orphanage = OrphanQueueImpl::new();
        assert!(orphanage.sigchild.lock().is_none()); // Sanity

        // No register when queue empty
        orphanage.reap_orphans(&handle);
        assert!(orphanage.sigchild.lock().is_none());

        let orphan = MockWait::new(2);
        let waits = orphan.total_waits.clone();
        orphanage.push_orphan(orphan);

        orphanage.reap_orphans(&handle);
        assert!(orphanage.sigchild.lock().is_some());
        assert_eq!(waits.get(), 1); // Eager reap when registering listener
    }

    #[test]
    fn does_nothing_if_signal_could_not_be_registered() {
        let handle = SignalHandle::default();

        let orphanage = OrphanQueueImpl::new();
        assert!(orphanage.sigchild.lock().is_none());

        let orphan = MockWait::new(2);
        let waits = orphan.total_waits.clone();
        orphanage.push_orphan(orphan);

        // Signal handler has "gone away", nothing to register or reap
        orphanage.reap_orphans(&handle);
        assert!(orphanage.sigchild.lock().is_none());
        assert_eq!(waits.get(), 0);
    }
}
