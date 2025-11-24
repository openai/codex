use futures::future::{Future, FutureExt, FutureObj};
use futures::task::{Context, Poll};
use std::pin::Pin;

#[test]
fn dropping_does_not_segfault() {
    FutureObj::new(async { String::new() }.boxed());
}

#[test]
fn dropping_drops_the_future() {
    let mut times_dropped = 0;

    struct Inc<'a>(&'a mut u32);

    impl Future for Inc<'_> {
        type Output = ();

        fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
            unimplemented!()
        }
    }

    impl Drop for Inc<'_> {
        fn drop(&mut self) {
            *self.0 += 1;
        }
    }

    FutureObj::new(Inc(&mut times_dropped).boxed());

    assert_eq!(times_dropped, 1);
}
