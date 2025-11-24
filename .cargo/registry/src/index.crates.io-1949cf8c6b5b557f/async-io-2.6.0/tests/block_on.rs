use async_io::block_on;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
    time::{Duration, Instant},
};

#[test]
fn doesnt_poll_after_ready() {
    #[derive(Default)]
    struct Bomb {
        returned_ready: bool,
    }
    impl Future for Bomb {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
            if self.returned_ready {
                panic!("Future was polled again after returning Poll::Ready");
            } else {
                self.returned_ready = true;
                Poll::Ready(())
            }
        }
    }

    block_on(Bomb::default())
}

#[test]
fn recursive_wakers_are_different() {
    struct Outer;
    impl Future for Outer {
        type Output = ();

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let outer_waker = cx.waker();
            block_on(Inner { outer_waker });
            Poll::Ready(())
        }
    }

    struct Inner<'a> {
        pub outer_waker: &'a Waker,
    }
    impl Future for Inner<'_> {
        type Output = ();

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let inner_waker = cx.waker();
            assert!(!inner_waker.will_wake(self.outer_waker));
            Poll::Ready(())
        }
    }

    block_on(Outer);
}

#[test]
fn inner_cannot_wake_outer() {
    #[derive(Default)]
    struct Outer {
        elapsed: Option<Instant>,
    }
    impl Future for Outer {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            if let Some(elapsed) = self.elapsed {
                assert!(elapsed.elapsed() >= Duration::from_secs(1));
                Poll::Ready(())
            } else {
                let outer_waker = cx.waker().clone();
                block_on(Inner);
                std::thread::spawn(|| {
                    std::thread::sleep(Duration::from_secs(1));
                    outer_waker.wake();
                });
                self.elapsed = Some(Instant::now());
                Poll::Pending
            }
        }
    }

    struct Inner;
    impl Future for Inner {
        type Output = ();

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let inner_waker = cx.waker();
            inner_waker.wake_by_ref();
            Poll::Ready(())
        }
    }

    block_on(Outer::default());
}

#[test]
fn outer_cannot_wake_inner() {
    struct Outer;
    impl Future for Outer {
        type Output = ();

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let outer_waker = cx.waker();
            outer_waker.wake_by_ref();
            block_on(Inner::default());
            Poll::Ready(())
        }
    }

    #[derive(Default)]
    struct Inner {
        elapsed: Option<Instant>,
    }
    impl Future for Inner {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            if let Some(elapsed) = self.elapsed {
                assert!(elapsed.elapsed() >= Duration::from_secs(1));
                Poll::Ready(())
            } else {
                let inner_waker = cx.waker().clone();
                std::thread::spawn(|| {
                    std::thread::sleep(Duration::from_secs(1));
                    inner_waker.wake();
                });
                self.elapsed = Some(Instant::now());
                Poll::Pending
            }
        }
    }

    block_on(Outer);
}

#[test]
fn first_cannot_wake_second() {
    struct First;
    impl Future for First {
        type Output = ();

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let first_waker = cx.waker();
            first_waker.wake_by_ref();
            Poll::Ready(())
        }
    }

    #[derive(Default)]
    struct Second {
        elapsed: Option<Instant>,
    }
    impl Future for Second {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            if let Some(elapsed) = self.elapsed {
                assert!(elapsed.elapsed() >= Duration::from_secs(1));
                Poll::Ready(())
            } else {
                let second_waker = cx.waker().clone();
                std::thread::spawn(|| {
                    std::thread::sleep(Duration::from_secs(1));
                    second_waker.wake();
                });
                self.elapsed = Some(Instant::now());
                Poll::Pending
            }
        }
    }

    block_on(First);
    block_on(Second::default());
}
