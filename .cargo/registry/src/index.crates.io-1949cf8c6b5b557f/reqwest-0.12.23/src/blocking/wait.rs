use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{self, Thread};
use std::time::Duration;

use tokio::time::Instant;

pub(crate) fn timeout<F, I, E>(fut: F, timeout: Option<Duration>) -> Result<I, Waited<E>>
where
    F: Future<Output = Result<I, E>>,
{
    enter();

    let deadline = timeout.map(|d| {
        log::trace!("wait at most {d:?}");
        Instant::now() + d
    });

    let thread = ThreadWaker(thread::current());
    // Arc shouldn't be necessary, since `Thread` is reference counted internally,
    // but let's just stay safe for now.
    let waker = Waker::from(Arc::new(thread));
    let mut cx = Context::from_waker(&waker);

    futures_util::pin_mut!(fut);

    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(Ok(val)) => return Ok(val),
            Poll::Ready(Err(err)) => return Err(Waited::Inner(err)),
            Poll::Pending => (), // fallthrough
        }

        if let Some(deadline) = deadline {
            let now = Instant::now();
            if now >= deadline {
                log::trace!("wait timeout exceeded");
                return Err(Waited::TimedOut(crate::error::TimedOut));
            }

            log::trace!(
                "({:?}) park timeout {:?}",
                thread::current().id(),
                deadline - now
            );
            thread::park_timeout(deadline - now);
        } else {
            log::trace!("({:?}) park without timeout", thread::current().id());
            thread::park();
        }
    }
}

#[derive(Debug)]
pub(crate) enum Waited<E> {
    TimedOut(crate::error::TimedOut),
    Inner(E),
}

struct ThreadWaker(Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

fn enter() {
    // Check we aren't already in a runtime
    #[cfg(debug_assertions)]
    {
        let _enter = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("build shell runtime")
            .enter();
    }
}
