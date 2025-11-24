use std::{
    cell::Cell,
    future::Future,
    pin::Pin,
    ptr,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
    thread,
    time::Duration,
};

use crate::Error;

const NOOP_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
    // Cloning just returns a new no-op raw waker
    |_| NOOP_RAW_WAKER,
    // `wake` does nothing
    |_| {},
    // `wake_by_ref` does nothing
    |_| {},
    // Dropping does nothing as we don't allocate anything
    |_| {},
);
const NOOP_RAW_WAKER: RawWaker = RawWaker::new(ptr::null(), &NOOP_WAKER_VTABLE);

#[derive(Default)]
pub(crate) struct YieldOnce(bool);

impl Future for YieldOnce {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
        let flag = &mut std::pin::Pin::into_inner(self).0;
        if !*flag {
            *flag = true;
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}

/// Execute the futures and return when they are all done.
///
/// Here we use our own homebrew async executor since cc is used in the build
/// script of many popular projects, pulling in additional dependencies would
/// significantly slow down its compilation.
pub(crate) fn block_on<Fut1, Fut2>(
    mut fut1: Fut1,
    mut fut2: Fut2,
    has_made_progress: &Cell<bool>,
) -> Result<(), Error>
where
    Fut1: Future<Output = Result<(), Error>>,
    Fut2: Future<Output = Result<(), Error>>,
{
    // Shadows the future so that it can never be moved and is guaranteed
    // to be pinned.
    //
    // The same trick used in `pin!` macro.
    //
    // TODO: Once MSRV is bumped to 1.68, replace this with `std::pin::pin!`
    let mut fut1 = Some(unsafe { Pin::new_unchecked(&mut fut1) });
    let mut fut2 = Some(unsafe { Pin::new_unchecked(&mut fut2) });

    // TODO: Once `Waker::noop` stablised and our MSRV is bumped to the version
    // which it is stablised, replace this with `Waker::noop`.
    let waker = unsafe { Waker::from_raw(NOOP_RAW_WAKER) };
    let mut context = Context::from_waker(&waker);

    let mut backoff_cnt = 0;

    loop {
        has_made_progress.set(false);

        if let Some(fut) = fut2.as_mut() {
            if let Poll::Ready(res) = fut.as_mut().poll(&mut context) {
                fut2 = None;
                res?;
            }
        }

        if let Some(fut) = fut1.as_mut() {
            if let Poll::Ready(res) = fut.as_mut().poll(&mut context) {
                fut1 = None;
                res?;
            }
        }

        if fut1.is_none() && fut2.is_none() {
            return Ok(());
        }

        if !has_made_progress.get() {
            if backoff_cnt > 3 {
                // We have yielded at least three times without making'
                // any progress, so we will sleep for a while.
                let duration = Duration::from_millis(100 * (backoff_cnt - 3).min(10));
                thread::sleep(duration);
            } else {
                // Given that we spawned a lot of compilation tasks, it is unlikely
                // that OS cannot find other ready task to execute.
                //
                // If all of them are done, then we will yield them and spawn more,
                // or simply return.
                //
                // Thus this will not be turned into a busy-wait loop and it will not
                // waste CPU resource.
                thread::yield_now();
            }
        }

        backoff_cnt = if has_made_progress.get() {
            0
        } else {
            backoff_cnt + 1
        };
    }
}
