use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Converts a function to a future that completes on poll.
pub(crate) struct BlockingTask<T> {
    func: Option<T>,
}

impl<T> BlockingTask<T> {
    /// Initializes a new blocking task from the given function.
    pub(crate) fn new(func: T) -> BlockingTask<T> {
        BlockingTask { func: Some(func) }
    }
}

// The closure `F` is never pinned
impl<T> Unpin for BlockingTask<T> {}

impl<T, R> Future for BlockingTask<T>
where
    T: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    type Output = R;

    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<R> {
        let me = &mut *self;
        let func = me
            .func
            .take()
            .expect("[internal exception] blocking task ran twice.");

        // This is a little subtle:
        // For convenience, we'd like _every_ call tokio ever makes to Task::poll() to be budgeted
        // using coop. However, the way things are currently modeled, even running a blocking task
        // currently goes through Task::poll(), and so is subject to budgeting. That isn't really
        // what we want; a blocking task may itself want to run tasks (it might be a Worker!), so
        // we want it to start without any budgeting.
        crate::task::coop::stop();

        Poll::Ready(func())
    }
}
