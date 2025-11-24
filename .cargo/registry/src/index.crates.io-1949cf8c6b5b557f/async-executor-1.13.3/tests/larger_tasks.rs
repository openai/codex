//! Test for larger tasks.

use async_executor::Executor;
use futures_lite::future::{self, block_on};
use futures_lite::prelude::*;

use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn do_run<Fut: Future<Output = ()>>(mut f: impl FnMut(Arc<Executor<'static>>) -> Fut) {
    // This should not run for longer than two minutes.
    #[cfg(not(miri))]
    let _stop_timeout = {
        let (stop_timeout, stopper) = async_channel::bounded::<()>(1);
        thread::spawn(move || {
            block_on(async move {
                let timeout = async {
                    async_io::Timer::after(Duration::from_secs(2 * 60)).await;
                    eprintln!("test timed out after 2m");
                    std::process::exit(1)
                };

                let _ = stopper.recv().or(timeout).await;
            })
        });
        stop_timeout
    };

    let ex = Arc::new(Executor::new());

    // Test 1: Use the `run` command.
    block_on(ex.run(f(ex.clone())));

    // Test 2: Loop on `tick`.
    block_on(async {
        let ticker = async {
            loop {
                ex.tick().await;
            }
        };

        f(ex.clone()).or(ticker).await
    });

    // Test 3: Run on many threads.
    thread::scope(|scope| {
        let (_signal, shutdown) = async_channel::bounded::<()>(1);

        for _ in 0..16 {
            let shutdown = shutdown.clone();
            let ex = &ex;
            scope.spawn(move || block_on(ex.run(shutdown.recv())));
        }

        block_on(f(ex.clone()));
    });

    // Test 4: Tick loop on many threads.
    thread::scope(|scope| {
        let (_signal, shutdown) = async_channel::bounded::<()>(1);

        for _ in 0..16 {
            let shutdown = shutdown.clone();
            let ex = &ex;
            scope.spawn(move || {
                block_on(async move {
                    let ticker = async {
                        loop {
                            ex.tick().await;
                        }
                    };

                    shutdown.recv().or(ticker).await
                })
            });
        }

        block_on(f(ex.clone()));
    });
}

#[test]
fn smoke() {
    do_run(|ex| async move { ex.spawn(async {}).await });
}

#[test]
fn yield_now() {
    do_run(|ex| async move { ex.spawn(future::yield_now()).await })
}

#[test]
fn timer() {
    do_run(|ex| async move {
        ex.spawn(async_io::Timer::after(Duration::from_millis(5)))
            .await;
    })
}
