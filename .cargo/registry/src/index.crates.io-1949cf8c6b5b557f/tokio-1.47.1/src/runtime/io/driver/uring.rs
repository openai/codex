use io_uring::{squeue::Entry, IoUring};
use mio::unix::SourceFd;
use slab::Slab;

use crate::loom::sync::atomic::Ordering;
use crate::runtime::driver::op::{Cancellable, Lifecycle};
use crate::{io::Interest, loom::sync::Mutex};

use super::{Handle, TOKEN_WAKEUP};

use std::os::fd::{AsRawFd, RawFd};
use std::{io, mem, task::Waker};

const DEFAULT_RING_SIZE: u32 = 256;

#[repr(usize)]
#[derive(Debug, PartialEq, Eq)]
enum State {
    Uninitialized = 0,
    Initialized = 1,
    Unsupported = 2,
}

impl State {
    fn as_usize(self) -> usize {
        self as usize
    }

    fn from_usize(value: usize) -> Self {
        match value {
            0 => State::Uninitialized,
            1 => State::Initialized,
            2 => State::Unsupported,
            _ => unreachable!("invalid Uring state: {}", value),
        }
    }
}

pub(crate) struct UringContext {
    pub(crate) uring: Option<io_uring::IoUring>,
    pub(crate) ops: slab::Slab<Lifecycle>,
}

impl UringContext {
    pub(crate) fn new() -> Self {
        Self {
            ops: Slab::new(),
            uring: None,
        }
    }

    pub(crate) fn ring(&self) -> &io_uring::IoUring {
        self.uring.as_ref().expect("io_uring not initialized")
    }

    pub(crate) fn ring_mut(&mut self) -> &mut io_uring::IoUring {
        self.uring.as_mut().expect("io_uring not initialized")
    }

    /// Perform `io_uring_setup` system call, and Returns true if this
    /// actually initialized the io_uring.
    ///
    /// If the machine doesn't support io_uring, then this will return an
    /// `ENOSYS` error.
    pub(crate) fn try_init(&mut self) -> io::Result<bool> {
        if self.uring.is_some() {
            // Already initialized.
            return Ok(false);
        }

        self.uring.replace(IoUring::new(DEFAULT_RING_SIZE)?);

        Ok(true)
    }

    pub(crate) fn dispatch_completions(&mut self) {
        let ops = &mut self.ops;
        let Some(mut uring) = self.uring.take() else {
            // Uring is not initialized yet.
            return;
        };

        let cq = uring.completion();

        for cqe in cq {
            let idx = cqe.user_data() as usize;

            match ops.get_mut(idx) {
                Some(Lifecycle::Waiting(waker)) => {
                    waker.wake_by_ref();
                    *ops.get_mut(idx).unwrap() = Lifecycle::Completed(cqe);
                }
                Some(Lifecycle::Cancelled(_)) => {
                    // Op future was cancelled, so we discard the result.
                    // We just remove the entry from the slab.
                    ops.remove(idx);
                }
                Some(other) => {
                    panic!("unexpected lifecycle for slot {}: {:?}", idx, other);
                }
                None => {
                    panic!("no op at index {}", idx);
                }
            }
        }

        self.uring.replace(uring);

        // `cq`'s drop gets called here, updating the latest head pointer
    }

    pub(crate) fn submit(&mut self) -> io::Result<()> {
        loop {
            // Errors from io_uring_enter: https://man7.org/linux/man-pages/man2/io_uring_enter.2.html#ERRORS
            match self.ring().submit() {
                Ok(_) => {
                    return Ok(());
                }

                // If the submission queue is full, we dispatch completions and try again.
                Err(ref e) if e.raw_os_error() == Some(libc::EBUSY) => {
                    self.dispatch_completions();
                }
                // For other errors, we currently return the error as is.
                Err(e) => {
                    return Err(e);
                }
            }
        }
    }

    pub(crate) fn remove_op(&mut self, index: usize) -> Lifecycle {
        self.ops.remove(index)
    }
}

/// Drop the driver, cancelling any in-progress ops and waiting for them to terminate.
impl Drop for UringContext {
    fn drop(&mut self) {
        if self.uring.is_none() {
            // Uring is not initialized or not supported.
            return;
        }

        // Make sure we flush the submission queue before dropping the driver.
        while !self.ring_mut().submission().is_empty() {
            self.submit().expect("Internal error when dropping driver");
        }

        let mut cancel_ops = Slab::new();
        let mut keys_to_move = Vec::new();

        for (key, lifecycle) in self.ops.iter() {
            match lifecycle {
                Lifecycle::Waiting(_) | Lifecycle::Submitted | Lifecycle::Cancelled(_) => {
                    // these should be cancelled
                    keys_to_move.push(key);
                }
                // We don't wait for completed ops.
                Lifecycle::Completed(_) => {}
            }
        }

        for key in keys_to_move {
            let lifecycle = self.remove_op(key);
            cancel_ops.insert(lifecycle);
        }

        while !cancel_ops.is_empty() {
            // Wait until at least one completion is available.
            self.ring_mut()
                .submit_and_wait(1)
                .expect("Internal error when dropping driver");

            for cqe in self.ring_mut().completion() {
                let idx = cqe.user_data() as usize;
                cancel_ops.remove(idx);
            }
        }
    }
}

impl Handle {
    #[allow(dead_code)]
    fn add_uring_source(&self, uringfd: RawFd) -> io::Result<()> {
        let mut source = SourceFd(&uringfd);
        self.registry
            .register(&mut source, TOKEN_WAKEUP, Interest::READABLE.to_mio())
    }

    pub(crate) fn get_uring(&self) -> &Mutex<UringContext> {
        &self.uring_context
    }

    fn set_uring_state(&self, state: State) {
        self.uring_state.store(state.as_usize(), Ordering::Release);
    }

    /// Check if the io_uring context is initialized. If not, it will try to initialize it.
    pub(crate) fn check_and_init(&self) -> io::Result<bool> {
        match State::from_usize(self.uring_state.load(Ordering::Acquire)) {
            State::Uninitialized => match self.try_init() {
                Ok(()) => {
                    self.set_uring_state(State::Initialized);
                    Ok(true)
                }
                // If the system doesn't support io_uring, we set the state to Unsupported.
                Err(e) if e.raw_os_error() == Some(libc::ENOSYS) => {
                    self.set_uring_state(State::Unsupported);
                    Ok(false)
                }
                // For other system errors, we just return it.
                Err(e) => Err(e),
            },
            State::Unsupported => Ok(false),
            State::Initialized => Ok(true),
        }
    }

    /// Initialize the io_uring context if it hasn't been initialized yet.
    fn try_init(&self) -> io::Result<()> {
        let mut guard = self.get_uring().lock();
        if guard.try_init()? {
            self.add_uring_source(guard.ring().as_raw_fd())?;
        }

        Ok(())
    }

    /// Register an operation with the io_uring.
    ///
    /// If this is the first io_uring operation, it will also initialize the io_uring context.
    /// If io_uring isn't supported, this function returns an `ENOSYS` error, so the caller can
    /// perform custom handling, such as falling back to an alternative mechanism.
    ///
    /// # Safety
    ///
    /// Callers must ensure that parameters of the entry (such as buffer) are valid and will
    /// be valid for the entire duration of the operation, otherwise it may cause memory problems.
    pub(crate) unsafe fn register_op(&self, entry: Entry, waker: Waker) -> io::Result<usize> {
        // Note: Maybe this check can be removed if upstream callers consistently use `check_and_init`.
        if !self.check_and_init()? {
            return Err(io::Error::from_raw_os_error(libc::ENOSYS));
        }

        // Uring is initialized.

        let mut guard = self.get_uring().lock();
        let ctx = &mut *guard;
        let index = ctx.ops.insert(Lifecycle::Waiting(waker));
        let entry = entry.user_data(index as u64);

        let submit_or_remove = |ctx: &mut UringContext| -> io::Result<()> {
            if let Err(e) = ctx.submit() {
                // Submission failed, remove the entry from the slab and return the error
                ctx.remove_op(index);
                return Err(e);
            }
            Ok(())
        };

        // SAFETY: entry is valid for the entire duration of the operation
        while unsafe { ctx.ring_mut().submission().push(&entry).is_err() } {
            // If the submission queue is full, flush it to the kernel
            submit_or_remove(ctx)?;
        }

        // Note: For now, we submit the entry immediately without utilizing batching.
        submit_or_remove(ctx)?;

        Ok(index)
    }

    // TODO: Remove this annotation when operations are actually supported
    #[allow(unused_variables, unreachable_code)]
    pub(crate) fn cancel_op<T: Cancellable>(&self, index: usize, data: Option<T>) {
        let mut guard = self.get_uring().lock();
        let ctx = &mut *guard;
        let ops = &mut ctx.ops;
        let Some(lifecycle) = ops.get_mut(index) else {
            // The corresponding index doesn't exist anymore, so this Op is already complete.
            return;
        };

        // This Op will be cancelled. Here, we don't remove the lifecycle from the slab to keep
        // uring data alive until the operation completes.

        let cancel_data = data.expect("Data should be present").cancel();
        match mem::replace(lifecycle, Lifecycle::Cancelled(cancel_data)) {
            Lifecycle::Submitted | Lifecycle::Waiting(_) => (),
            // The driver saw the completion, but it was never polled.
            Lifecycle::Completed(_) => {
                // We can safely remove the entry from the slab, as it has already been completed.
                ops.remove(index);
            }
            prev => panic!("Unexpected state: {:?}", prev),
        };
    }
}
