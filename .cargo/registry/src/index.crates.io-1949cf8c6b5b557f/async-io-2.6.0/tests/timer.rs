use std::future::{poll_fn, Future};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use async_io::Timer;
use futures_lite::{future, FutureExt, StreamExt};

fn spawn<T: Send + 'static>(
    f: impl Future<Output = T> + Send + 'static,
) -> impl Future<Output = T> + Send + 'static {
    let (s, r) = async_channel::bounded(1);

    thread::spawn(move || {
        future::block_on(async {
            s.send(f.await).await.ok();
        })
    });

    Box::pin(async move { r.recv().await.unwrap() })
}

#[test]
fn smoke() {
    future::block_on(async {
        let start = Instant::now();
        Timer::after(Duration::from_secs(1)).await;
        assert!(start.elapsed() >= Duration::from_secs(1));
    });
}

#[test]
fn interval() {
    future::block_on(async {
        let period = Duration::from_secs(1);
        let jitter = Duration::from_millis(500);
        let start = Instant::now();
        let mut timer = Timer::interval(period);
        timer.next().await;
        let elapsed = start.elapsed();
        assert!(elapsed >= period && elapsed - period < jitter);
        timer.next().await;
        let elapsed = start.elapsed();
        assert!(elapsed >= period * 2 && elapsed - period * 2 < jitter);
    });
}

#[test]
fn poll_across_tasks() {
    future::block_on(async {
        let start = Instant::now();
        let (sender, receiver) = async_channel::bounded(1);

        let task1 = spawn(async move {
            let mut timer = Timer::after(Duration::from_secs(1));

            async {
                (&mut timer).await;
                panic!("timer should not be ready")
            }
            .or(async {})
            .await;

            sender.send(timer).await.ok();
        });

        let task2 = spawn(async move {
            let timer = receiver.recv().await.unwrap();
            timer.await;
        });

        task1.await;
        task2.await;

        assert!(start.elapsed() >= Duration::from_secs(1));
    });
}

#[test]
fn set() {
    future::block_on(async {
        let start = Instant::now();
        let timer = Arc::new(Mutex::new(Timer::after(Duration::from_secs(10))));

        thread::spawn({
            let timer = timer.clone();
            move || {
                thread::sleep(Duration::from_secs(1));
                timer.lock().unwrap().set_after(Duration::from_secs(2));
            }
        });

        poll_fn(|cx| Pin::new(&mut *timer.lock().unwrap()).poll(cx)).await;

        assert!(start.elapsed() >= Duration::from_secs(2));
        assert!(start.elapsed() < Duration::from_secs(10));
    });
}
