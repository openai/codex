use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::Context;

use futures_lite::prelude::*;
use waker_fn::waker_fn;

pub fn check_yields_when_contended<G>(contending_guard: G, acquire_future: impl Future) {
    let was_woken = Arc::new(AtomicBool::new(false));
    let waker = {
        let was_woken = Arc::clone(&was_woken);
        waker_fn(move || was_woken.store(true, Ordering::SeqCst))
    };
    let mut cx = Context::from_waker(&waker);

    futures_lite::pin!(acquire_future);
    assert!(acquire_future.as_mut().poll(&mut cx).is_pending());
    drop(contending_guard);
    assert!(was_woken.load(Ordering::SeqCst));
    assert!(acquire_future.poll(&mut cx).is_ready());
}
