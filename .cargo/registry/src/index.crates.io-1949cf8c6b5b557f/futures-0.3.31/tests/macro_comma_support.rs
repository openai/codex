use futures::{
    executor::block_on,
    future::{self, FutureExt},
    join, ready,
    task::Poll,
    try_join,
};

#[test]
fn ready() {
    block_on(future::poll_fn(|_| {
        ready!(Poll::Ready(()),);
        Poll::Ready(())
    }))
}

#[test]
fn poll() {
    use futures::poll;

    block_on(async {
        let _ = poll!(async {}.boxed(),);
    })
}

#[test]
fn join() {
    block_on(async {
        let future1 = async { 1 };
        let future2 = async { 2 };
        join!(future1, future2,);
    })
}

#[test]
fn try_join() {
    block_on(async {
        let future1 = async { 1 }.never_error();
        let future2 = async { 2 }.never_error();
        try_join!(future1, future2,)
    })
    .unwrap();
}
