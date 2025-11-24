use futures::executor::block_on;
use futures::future::{self, FusedFuture, FutureExt};
use futures::select;
use futures::stream::{FuturesUnordered, StreamExt};
use futures::task::{Context, Poll};
use futures_test::future::FutureTestExt;
use futures_test::task::new_count_waker;

#[test]
fn is_terminated() {
    let (waker, counter) = new_count_waker();
    let mut cx = Context::from_waker(&waker);

    let mut tasks = FuturesUnordered::new();

    let mut select_next_some = tasks.select_next_some();
    assert_eq!(select_next_some.is_terminated(), false);
    assert_eq!(select_next_some.poll_unpin(&mut cx), Poll::Pending);
    assert_eq!(counter, 1);
    assert_eq!(select_next_some.is_terminated(), true);
    drop(select_next_some);

    tasks.push(future::ready(1));

    let mut select_next_some = tasks.select_next_some();
    assert_eq!(select_next_some.is_terminated(), false);
    assert_eq!(select_next_some.poll_unpin(&mut cx), Poll::Ready(1));
    assert_eq!(select_next_some.is_terminated(), false);
    assert_eq!(select_next_some.poll_unpin(&mut cx), Poll::Pending);
    assert_eq!(select_next_some.is_terminated(), true);
}

#[test]
fn select() {
    // Checks that even though `async_tasks` will yield a `None` and return
    // `is_terminated() == true` during the first poll, it manages to toggle
    // back to having items after a future is pushed into it during the second
    // poll (after pending_once completes).
    block_on(async {
        let mut fut = future::ready(1).pending_once();
        let mut async_tasks = FuturesUnordered::new();
        let mut total = 0;
        loop {
            select! {
                num = fut => {
                    total += num;
                    async_tasks.push(async { 5 });
                },
                num = async_tasks.select_next_some() => {
                    total += num;
                }
                complete => break,
            }
        }
        assert_eq!(total, 6);
    });
}

// Check that `select!` macro does not fail when importing from `futures_util`.
#[test]
fn futures_util_select() {
    use futures_util::select;

    // Checks that even though `async_tasks` will yield a `None` and return
    // `is_terminated() == true` during the first poll, it manages to toggle
    // back to having items after a future is pushed into it during the second
    // poll (after pending_once completes).
    block_on(async {
        let mut fut = future::ready(1).pending_once();
        let mut async_tasks = FuturesUnordered::new();
        let mut total = 0;
        loop {
            select! {
                num = fut => {
                    total += num;
                    async_tasks.push(async { 5 });
                },
                num = async_tasks.select_next_some() => {
                    total += num;
                }
                complete => break,
            }
        }
        assert_eq!(total, 6);
    });
}
