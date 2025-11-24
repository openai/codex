#![cfg(feature = "bilock")]

use futures::executor::block_on;
use futures::future;
use futures::stream;
use futures::task::{Context, Poll};
use futures::Future;
use futures::StreamExt;
use futures_test::task::noop_context;
use futures_util::lock::BiLock;
use std::pin::Pin;
use std::thread;

#[test]
fn smoke() {
    let future = future::lazy(|cx| {
        let (a, b) = BiLock::new(1);

        {
            let mut lock = match a.poll_lock(cx) {
                Poll::Ready(l) => l,
                Poll::Pending => panic!("poll not ready"),
            };
            assert_eq!(*lock, 1);
            *lock = 2;

            assert!(b.poll_lock(cx).is_pending());
            assert!(a.poll_lock(cx).is_pending());
        }

        assert!(b.poll_lock(cx).is_ready());
        assert!(a.poll_lock(cx).is_ready());

        {
            let lock = match b.poll_lock(cx) {
                Poll::Ready(l) => l,
                Poll::Pending => panic!("poll not ready"),
            };
            assert_eq!(*lock, 2);
        }

        assert_eq!(a.reunite(b).expect("bilock/smoke: reunite error"), 2);

        Ok::<(), ()>(())
    });

    assert_eq!(block_on(future), Ok(()));
}

#[test]
fn concurrent() {
    const N: usize = 10000;
    let mut cx = noop_context();
    let (a, b) = BiLock::new(0);

    let a = Increment { a: Some(a), remaining: N };
    let b = stream::iter(0..N).fold(b, |b, _n| async {
        let mut g = b.lock().await;
        *g += 1;
        drop(g);
        b
    });

    let t1 = thread::spawn(move || block_on(a));
    let b = block_on(b);
    let a = t1.join().unwrap();

    match a.poll_lock(&mut cx) {
        Poll::Ready(l) => assert_eq!(*l, 2 * N),
        Poll::Pending => panic!("poll not ready"),
    }
    match b.poll_lock(&mut cx) {
        Poll::Ready(l) => assert_eq!(*l, 2 * N),
        Poll::Pending => panic!("poll not ready"),
    }

    assert_eq!(a.reunite(b).expect("bilock/concurrent: reunite error"), 2 * N);

    struct Increment {
        remaining: usize,
        a: Option<BiLock<usize>>,
    }

    impl Future for Increment {
        type Output = BiLock<usize>;

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<BiLock<usize>> {
            loop {
                if self.remaining == 0 {
                    return self.a.take().unwrap().into();
                }

                let a = self.a.as_mut().unwrap();
                let mut a = match a.poll_lock(cx) {
                    Poll::Ready(l) => l,
                    Poll::Pending => return Poll::Pending,
                };
                *a += 1;
                drop(a);
                self.remaining -= 1;
            }
        }
    }
}
