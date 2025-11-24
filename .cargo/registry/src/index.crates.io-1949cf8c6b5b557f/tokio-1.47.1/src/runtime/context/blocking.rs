use super::{EnterRuntime, CONTEXT};

use crate::loom::thread::AccessError;
use crate::util::markers::NotSendOrSync;

use std::marker::PhantomData;
use std::time::Duration;

/// Guard tracking that a caller has entered a blocking region.
#[must_use]
pub(crate) struct BlockingRegionGuard {
    _p: PhantomData<NotSendOrSync>,
}

pub(crate) struct DisallowBlockInPlaceGuard(bool);

pub(crate) fn try_enter_blocking_region() -> Option<BlockingRegionGuard> {
    CONTEXT
        .try_with(|c| {
            if c.runtime.get().is_entered() {
                None
            } else {
                Some(BlockingRegionGuard::new())
            }
            // If accessing the thread-local fails, the thread is terminating
            // and thread-locals are being destroyed. Because we don't know if
            // we are currently in a runtime or not, we default to being
            // permissive.
        })
        .unwrap_or_else(|_| Some(BlockingRegionGuard::new()))
}

/// Disallows blocking in the current runtime context until the guard is dropped.
pub(crate) fn disallow_block_in_place() -> DisallowBlockInPlaceGuard {
    let reset = CONTEXT.try_with(|c| {
        if let EnterRuntime::Entered {
            allow_block_in_place: true,
        } = c.runtime.get()
        {
            c.runtime.set(EnterRuntime::Entered {
                allow_block_in_place: false,
            });
            true
        } else {
            false
        }
    });

    DisallowBlockInPlaceGuard(reset.unwrap_or(false))
}

impl BlockingRegionGuard {
    pub(super) fn new() -> BlockingRegionGuard {
        BlockingRegionGuard { _p: PhantomData }
    }

    /// Blocks the thread on the specified future, returning the value with
    /// which that future completes.
    pub(crate) fn block_on<F>(&mut self, f: F) -> Result<F::Output, AccessError>
    where
        F: std::future::Future,
    {
        use crate::runtime::park::CachedParkThread;

        let mut park = CachedParkThread::new();
        park.block_on(f)
    }

    /// Blocks the thread on the specified future for **at most** `timeout`
    ///
    /// If the future completes before `timeout`, the result is returned. If
    /// `timeout` elapses, then `Err` is returned.
    pub(crate) fn block_on_timeout<F>(&mut self, f: F, timeout: Duration) -> Result<F::Output, ()>
    where
        F: std::future::Future,
    {
        use crate::runtime::park::CachedParkThread;
        use std::task::Context;
        use std::task::Poll::Ready;
        use std::time::Instant;

        let mut park = CachedParkThread::new();
        let waker = park.waker().map_err(|_| ())?;
        let mut cx = Context::from_waker(&waker);

        pin!(f);
        let when = Instant::now() + timeout;

        loop {
            if let Ready(v) = crate::task::coop::budget(|| f.as_mut().poll(&mut cx)) {
                return Ok(v);
            }

            let now = Instant::now();

            if now >= when {
                return Err(());
            }

            park.park_timeout(when - now);
        }
    }
}

impl Drop for DisallowBlockInPlaceGuard {
    fn drop(&mut self) {
        if self.0 {
            // XXX: Do we want some kind of assertion here, or is "best effort" okay?
            CONTEXT.with(|c| {
                if let EnterRuntime::Entered {
                    allow_block_in_place: false,
                } = c.runtime.get()
                {
                    c.runtime.set(EnterRuntime::Entered {
                        allow_block_in_place: true,
                    });
                }
            });
        }
    }
}
