use futures::channel::{mpsc, oneshot};
use futures::executor::block_on;
use futures::future::{self, poll_fn, FutureExt};
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use futures::task::{Context, Poll};
use futures::{
    join, pending, pin_mut, poll, select, select_biased, stream, stream_select, try_join,
};
use std::mem;

#[test]
fn poll_and_pending() {
    let pending_once = async { pending!() };
    block_on(async {
        pin_mut!(pending_once);
        assert_eq!(Poll::Pending, poll!(&mut pending_once));
        assert_eq!(Poll::Ready(()), poll!(&mut pending_once));
    });
}

#[test]
fn join() {
    let (tx1, rx1) = oneshot::channel::<i32>();
    let (tx2, rx2) = oneshot::channel::<i32>();

    let fut = async {
        let res = join!(rx1, rx2);
        assert_eq!((Ok(1), Ok(2)), res);
    };

    block_on(async {
        pin_mut!(fut);
        assert_eq!(Poll::Pending, poll!(&mut fut));
        tx1.send(1).unwrap();
        assert_eq!(Poll::Pending, poll!(&mut fut));
        tx2.send(2).unwrap();
        assert_eq!(Poll::Ready(()), poll!(&mut fut));
    });
}

#[test]
fn select() {
    let (tx1, rx1) = oneshot::channel::<i32>();
    let (_tx2, rx2) = oneshot::channel::<i32>();
    tx1.send(1).unwrap();
    let mut ran = false;
    block_on(async {
        select! {
            res = rx1.fuse() => {
                assert_eq!(Ok(1), res);
                ran = true;
            },
            _ = rx2.fuse() => unreachable!(),
        }
    });
    assert!(ran);
}

#[test]
fn select_grammar() {
    // Parsing after `=>` using Expr::parse would parse `{}() = future::ready(())`
    // as one expression.
    block_on(async {
        select! {
            () = future::pending::<()>() => {}
            () = future::ready(()) => {}
        }
    });
}

#[test]
fn select_biased() {
    let (tx1, rx1) = oneshot::channel::<i32>();
    let (_tx2, rx2) = oneshot::channel::<i32>();
    tx1.send(1).unwrap();
    let mut ran = false;
    block_on(async {
        select_biased! {
            res = rx1.fuse() => {
                assert_eq!(Ok(1), res);
                ran = true;
            },
            _ = rx2.fuse() => unreachable!(),
        }
    });
    assert!(ran);
}

#[test]
fn select_streams() {
    let (mut tx1, rx1) = mpsc::channel::<i32>(1);
    let (mut tx2, rx2) = mpsc::channel::<i32>(1);
    let mut rx1 = rx1.fuse();
    let mut rx2 = rx2.fuse();
    let mut ran = false;
    let mut total = 0;
    block_on(async {
        let mut tx1_opt;
        let mut tx2_opt;
        select! {
            _ = rx1.next() => panic!(),
            _ = rx2.next() => panic!(),
            default => {
                tx1.send(2).await.unwrap();
                tx2.send(3).await.unwrap();
                tx1_opt = Some(tx1);
                tx2_opt = Some(tx2);
            }
            complete => panic!(),
        }
        loop {
            select! {
                // runs first and again after default
                x = rx1.next() => if let Some(x) = x { total += x; },
                // runs second and again after default
                x = rx2.next()  => if let Some(x) = x { total += x; },
                // runs third
                default => {
                    assert_eq!(total, 5);
                    ran = true;
                    drop(tx1_opt.take().unwrap());
                    drop(tx2_opt.take().unwrap());
                },
                // runs last
                complete => break,
            };
        }
    });
    assert!(ran);
}

#[test]
fn select_can_move_uncompleted_futures() {
    let (tx1, rx1) = oneshot::channel::<i32>();
    let (tx2, rx2) = oneshot::channel::<i32>();
    tx1.send(1).unwrap();
    tx2.send(2).unwrap();
    let mut ran = false;
    let mut rx1 = rx1.fuse();
    let mut rx2 = rx2.fuse();
    block_on(async {
        select! {
            res = rx1 => {
                assert_eq!(Ok(1), res);
                assert_eq!(Ok(2), rx2.await);
                ran = true;
            },
            res = rx2 => {
                assert_eq!(Ok(2), res);
                assert_eq!(Ok(1), rx1.await);
                ran = true;
            },
        }
    });
    assert!(ran);
}

#[test]
fn select_nested() {
    let mut outer_fut = future::ready(1);
    let mut inner_fut = future::ready(2);
    let res = block_on(async {
        select! {
            x = outer_fut => {
                select! {
                    y = inner_fut => x + y,
                }
            }
        }
    });
    assert_eq!(res, 3);
}

#[cfg_attr(not(target_pointer_width = "64"), ignore)]
#[test]
fn select_size() {
    let fut = async {
        let mut ready = future::ready(0i32);
        select! {
            _ = ready => {},
        }
    };
    assert_eq!(mem::size_of_val(&fut), 24);

    let fut = async {
        let mut ready1 = future::ready(0i32);
        let mut ready2 = future::ready(0i32);
        select! {
            _ = ready1 => {},
            _ = ready2 => {},
        }
    };
    assert_eq!(mem::size_of_val(&fut), 40);
}

#[test]
fn select_on_non_unpin_expressions() {
    // The returned Future is !Unpin
    let make_non_unpin_fut = || async { 5 };

    let res = block_on(async {
        let select_res;
        select! {
            value_1 = make_non_unpin_fut().fuse() => select_res = value_1,
            value_2 = make_non_unpin_fut().fuse() => select_res = value_2,
        };
        select_res
    });
    assert_eq!(res, 5);
}

#[test]
fn select_on_non_unpin_expressions_with_default() {
    // The returned Future is !Unpin
    let make_non_unpin_fut = || async { 5 };

    let res = block_on(async {
        let select_res;
        select! {
            value_1 = make_non_unpin_fut().fuse() => select_res = value_1,
            value_2 = make_non_unpin_fut().fuse() => select_res = value_2,
            default => select_res = 7,
        };
        select_res
    });
    assert_eq!(res, 5);
}

#[cfg_attr(not(target_pointer_width = "64"), ignore)]
#[test]
fn select_on_non_unpin_size() {
    // The returned Future is !Unpin
    let make_non_unpin_fut = || async { 5 };

    let fut = async {
        let select_res;
        select! {
            value_1 = make_non_unpin_fut().fuse() => select_res = value_1,
            value_2 = make_non_unpin_fut().fuse() => select_res = value_2,
        };
        select_res
    };

    assert_eq!(32, mem::size_of_val(&fut));
}

#[test]
fn select_can_be_used_as_expression() {
    block_on(async {
        let res = select! {
            x = future::ready(7) => x,
            y = future::ready(3) => y + 1,
        };
        assert!(res == 7 || res == 4);
    });
}

#[test]
fn select_with_default_can_be_used_as_expression() {
    fn poll_always_pending<T>(_cx: &mut Context<'_>) -> Poll<T> {
        Poll::Pending
    }

    block_on(async {
        let res = select! {
            x = poll_fn(poll_always_pending::<i32>).fuse() => x,
            y = poll_fn(poll_always_pending::<i32>).fuse() => y + 1,
            default => 99,
        };
        assert_eq!(res, 99);
    });
}

#[test]
fn select_with_complete_can_be_used_as_expression() {
    block_on(async {
        let res = select! {
            x = future::pending::<i32>() => x,
            y = future::pending::<i32>() => y + 1,
            default => 99,
            complete => 237,
        };
        assert_eq!(res, 237);
    });
}

#[test]
#[allow(unused_assignments)]
fn select_on_mutable_borrowing_future_with_same_borrow_in_block() {
    async fn require_mutable(_: &mut i32) {}
    async fn async_noop() {}

    block_on(async {
        let mut value = 234;
        select! {
            _ = require_mutable(&mut value).fuse() => { },
            _ = async_noop().fuse() => {
                value += 5;
            },
        }
    });
}

#[test]
#[allow(unused_assignments)]
fn select_on_mutable_borrowing_future_with_same_borrow_in_block_and_default() {
    async fn require_mutable(_: &mut i32) {}
    async fn async_noop() {}

    block_on(async {
        let mut value = 234;
        select! {
            _ = require_mutable(&mut value).fuse() => { },
            _ = async_noop().fuse() => {
                value += 5;
            },
            default => {
                value += 27;
            },
        }
    });
}

#[test]
#[allow(unused_assignments)]
fn stream_select() {
    // stream_select! macro
    block_on(async {
        let endless_ints = |i| stream::iter(vec![i].into_iter().cycle());

        let mut endless_ones = stream_select!(endless_ints(1i32), stream::pending());
        assert_eq!(endless_ones.next().await, Some(1));
        assert_eq!(endless_ones.next().await, Some(1));

        let mut finite_list =
            stream_select!(stream::iter(vec![1].into_iter()), stream::iter(vec![1].into_iter()));
        assert_eq!(finite_list.next().await, Some(1));
        assert_eq!(finite_list.next().await, Some(1));
        assert_eq!(finite_list.next().await, None);

        let endless_mixed = stream_select!(endless_ints(1i32), endless_ints(2), endless_ints(3));
        // Take 1000, and assert a somewhat even distribution of values.
        // The fairness is randomized, but over 1000 samples we should be pretty close to even.
        // This test may be a bit flaky. Feel free to adjust the margins as you see fit.
        let mut count = 0;
        let results = endless_mixed
            .take_while(move |_| {
                count += 1;
                let ret = count < 1000;
                async move { ret }
            })
            .collect::<Vec<_>>()
            .await;
        assert!(results.iter().filter(|x| **x == 1).count() >= 299);
        assert!(results.iter().filter(|x| **x == 2).count() >= 299);
        assert!(results.iter().filter(|x| **x == 3).count() >= 299);
    });
}

#[cfg_attr(not(target_pointer_width = "64"), ignore)]
#[test]
fn join_size() {
    let fut = async {
        let ready = future::ready(0i32);
        join!(ready)
    };
    assert_eq!(mem::size_of_val(&fut), 24);

    let fut = async {
        let ready1 = future::ready(0i32);
        let ready2 = future::ready(0i32);
        join!(ready1, ready2)
    };
    assert_eq!(mem::size_of_val(&fut), 40);
}

#[cfg_attr(not(target_pointer_width = "64"), ignore)]
#[test]
fn try_join_size() {
    let fut = async {
        let ready = future::ready(Ok::<i32, i32>(0));
        try_join!(ready)
    };
    assert_eq!(mem::size_of_val(&fut), 24);

    let fut = async {
        let ready1 = future::ready(Ok::<i32, i32>(0));
        let ready2 = future::ready(Ok::<i32, i32>(0));
        try_join!(ready1, ready2)
    };
    assert_eq!(mem::size_of_val(&fut), 48);
}

#[allow(clippy::let_underscore_future)]
#[test]
fn join_doesnt_require_unpin() {
    let _ = async { join!(async {}, async {}) };
}

#[allow(clippy::let_underscore_future)]
#[test]
fn try_join_doesnt_require_unpin() {
    let _ = async { try_join!(async { Ok::<(), ()>(()) }, async { Ok::<(), ()>(()) },) };
}
