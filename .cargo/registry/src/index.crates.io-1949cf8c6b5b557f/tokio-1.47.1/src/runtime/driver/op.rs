use crate::runtime::Handle;
use io_uring::cqueue;
use io_uring::squeue::Entry;
use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::task::Waker;
use std::{io, mem};

#[derive(Debug)]
pub(crate) enum CancelData {}

#[derive(Debug)]
pub(crate) enum Lifecycle {
    /// The operation has been submitted to uring and is currently in-flight
    Submitted,

    /// The submitter is waiting for the completion of the operation
    Waiting(Waker),

    /// The submitter no longer has interest in the operation result. The state
    /// must be passed to the driver and held until the operation completes.
    Cancelled(CancelData),

    /// The operation has completed with a single cqe result
    Completed(io_uring::cqueue::Entry),
}

pub(crate) enum State {
    #[allow(dead_code)]
    Initialize(Option<Entry>),
    Polled(usize),
    Complete,
}

pub(crate) struct Op<T: Cancellable> {
    // Handle to the runtime
    handle: Handle,
    // State of this Op
    state: State,
    // Per operation data.
    data: Option<T>,
}

impl<T: Cancellable> Op<T> {
    /// # Safety
    ///
    /// Callers must ensure that parameters of the entry (such as buffer) are valid and will
    /// be valid for the entire duration of the operation, otherwise it may cause memory problems.
    #[allow(dead_code)]
    pub(crate) unsafe fn new(entry: Entry, data: T) -> Self {
        let handle = Handle::current();
        Self {
            handle,
            data: Some(data),
            state: State::Initialize(Some(entry)),
        }
    }
    pub(crate) fn take_data(&mut self) -> Option<T> {
        self.data.take()
    }
}

impl<T: Cancellable> Drop for Op<T> {
    fn drop(&mut self) {
        match self.state {
            // We've already dropped this Op.
            State::Complete => (),
            // We will cancel this Op.
            State::Polled(index) => {
                let data = self.take_data();
                let handle = &mut self.handle;
                handle.inner.driver().io().cancel_op(index, data);
            }
            // This Op has not been polled yet.
            // We don't need to do anything here.
            State::Initialize(_) => (),
        }
    }
}

/// A single CQE result
pub(crate) struct CqeResult {
    #[allow(dead_code)]
    pub(crate) result: io::Result<u32>,
}

impl From<cqueue::Entry> for CqeResult {
    fn from(cqe: cqueue::Entry) -> Self {
        let res = cqe.result();
        let result = if res >= 0 {
            Ok(res as u32)
        } else {
            Err(io::Error::from_raw_os_error(-res))
        };
        CqeResult { result }
    }
}

/// A trait that converts a CQE result into a usable value for each operation.
pub(crate) trait Completable {
    type Output;
    fn complete(self, cqe: CqeResult) -> io::Result<Self::Output>;
}

/// Extracts the `CancelData` needed to safely cancel an in-flight io_uring operation.
pub(crate) trait Cancellable {
    fn cancel(self) -> CancelData;
}

impl<T: Cancellable> Unpin for Op<T> {}

impl<T: Cancellable + Completable + Send> Future for Op<T> {
    type Output = io::Result<T::Output>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let handle = &mut this.handle;
        let driver = handle.inner.driver().io();

        match &mut this.state {
            State::Initialize(entry_opt) => {
                let entry = entry_opt.take().expect("Entry must be present");
                let waker = cx.waker().clone();
                // SAFETY: entry is valid for the entire duration of the operation
                let idx = unsafe { driver.register_op(entry, waker)? };
                this.state = State::Polled(idx);
                Poll::Pending
            }

            State::Polled(idx) => {
                let mut ctx = driver.get_uring().lock();
                let lifecycle = ctx.ops.get_mut(*idx).expect("Lifecycle must be present");

                match mem::replace(lifecycle, Lifecycle::Submitted) {
                    // Only replace the stored waker if it wouldn't wake the new one
                    Lifecycle::Waiting(prev) if !prev.will_wake(cx.waker()) => {
                        let waker = cx.waker().clone();
                        *lifecycle = Lifecycle::Waiting(waker);
                        Poll::Pending
                    }

                    Lifecycle::Waiting(prev) => {
                        *lifecycle = Lifecycle::Waiting(prev);
                        Poll::Pending
                    }

                    Lifecycle::Completed(cqe) => {
                        // Clean up and complete the future
                        ctx.remove_op(*idx);

                        this.state = State::Complete;

                        drop(ctx);

                        let data = this
                            .take_data()
                            .expect("Data must be present on completion");
                        Poll::Ready(data.complete(cqe.into()))
                    }

                    Lifecycle::Submitted => {
                        unreachable!("Submitted lifecycle should never be seen here");
                    }

                    Lifecycle::Cancelled(_) => {
                        unreachable!("Cancelled lifecycle should never be seen here");
                    }
                }
            }

            State::Complete => {
                panic!("Future polled after completion");
            }
        }
    }
}
