#![cfg(feature = "macros")]
#![allow(clippy::disallowed_names)]

#[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
use wasm_bindgen_test::wasm_bindgen_test as maybe_tokio_test;

#[cfg(not(all(target_family = "wasm", not(target_os = "wasi"))))]
use tokio::test as maybe_tokio_test;

use tokio::sync::oneshot;
use tokio_test::{assert_ok, assert_pending, assert_ready};

use std::future::poll_fn;
use std::task::Poll::Ready;

#[maybe_tokio_test]
async fn sync_one_lit_expr_comma() {
    let foo = tokio::select! {
        foo = async { 1 } => foo,
    };

    assert_eq!(foo, 1);
}

#[maybe_tokio_test]
async fn no_branch_else_only() {
    let foo = tokio::select! {
        else => 1,
    };

    assert_eq!(foo, 1);
}

#[maybe_tokio_test]
async fn no_branch_else_only_biased() {
    let foo = tokio::select! {
        biased;
        else => 1,
    };

    assert_eq!(foo, 1);
}

#[maybe_tokio_test]
async fn nested_one() {
    let foo = tokio::select! {
        foo = async { 1 } => tokio::select! {
            bar = async { foo } => bar,
        },
    };

    assert_eq!(foo, 1);
}

#[maybe_tokio_test]
async fn sync_one_lit_expr_no_comma() {
    let foo = tokio::select! {
        foo = async { 1 } => foo
    };

    assert_eq!(foo, 1);
}

#[maybe_tokio_test]
async fn sync_one_lit_expr_block() {
    let foo = tokio::select! {
        foo = async { 1 } => { foo }
    };

    assert_eq!(foo, 1);
}

#[maybe_tokio_test]
async fn sync_one_await() {
    let foo = tokio::select! {
        foo = one() => foo,
    };

    assert_eq!(foo, 1);
}

#[maybe_tokio_test]
async fn sync_one_ident() {
    let one = one();

    let foo = tokio::select! {
        foo = one => foo,
    };

    assert_eq!(foo, 1);
}

#[maybe_tokio_test]
async fn sync_two() {
    use std::cell::Cell;

    let cnt = Cell::new(0);

    let res = tokio::select! {
        foo = async {
            cnt.set(cnt.get() + 1);
            1
        } => foo,
        bar = async {
            cnt.set(cnt.get() + 1);
            2
        } => bar,
    };

    assert_eq!(1, cnt.get());
    assert!(res == 1 || res == 2);
}

#[maybe_tokio_test]
async fn drop_in_fut() {
    let s = "hello".to_string();

    let res = tokio::select! {
        foo = async {
            let v = one().await;
            drop(s);
            v
        } => foo
    };

    assert_eq!(res, 1);
}

#[maybe_tokio_test]
#[cfg(feature = "full")]
async fn one_ready() {
    let (tx1, rx1) = oneshot::channel::<i32>();
    let (_tx2, rx2) = oneshot::channel::<i32>();

    tx1.send(1).unwrap();

    let v = tokio::select! {
        res = rx1 => {
            assert_ok!(res)
        },
        _ = rx2 => unreachable!(),
    };

    assert_eq!(1, v);
}

#[maybe_tokio_test]
#[cfg(feature = "full")]
async fn select_streams() {
    use tokio::sync::mpsc;

    let (tx1, mut rx1) = mpsc::unbounded_channel::<i32>();
    let (tx2, mut rx2) = mpsc::unbounded_channel::<i32>();

    tokio::spawn(async move {
        assert_ok!(tx2.send(1));
        tokio::task::yield_now().await;

        assert_ok!(tx1.send(2));
        tokio::task::yield_now().await;

        assert_ok!(tx2.send(3));
        tokio::task::yield_now().await;

        drop((tx1, tx2));
    });

    let mut rem = true;
    let mut msgs = vec![];

    while rem {
        tokio::select! {
            Some(x) = rx1.recv() => {
                msgs.push(x);
            }
            Some(y) = rx2.recv() => {
                msgs.push(y);
            }
            else => {
                rem = false;
            }
        }
    }

    msgs.sort_unstable();
    assert_eq!(&msgs[..], &[1, 2, 3]);
}

#[maybe_tokio_test]
async fn move_uncompleted_futures() {
    let (tx1, mut rx1) = oneshot::channel::<i32>();
    let (tx2, mut rx2) = oneshot::channel::<i32>();

    tx1.send(1).unwrap();
    tx2.send(2).unwrap();

    let ran;

    tokio::select! {
        res = &mut rx1 => {
            assert_eq!(1, assert_ok!(res));
            assert_eq!(2, assert_ok!(rx2.await));
            ran = true;
        },
        res = &mut rx2 => {
            assert_eq!(2, assert_ok!(res));
            assert_eq!(1, assert_ok!(rx1.await));
            ran = true;
        },
    }

    assert!(ran);
}

#[maybe_tokio_test]
async fn nested() {
    let res = tokio::select! {
        x = async { 1 } => {
            tokio::select! {
                y = async { 2 } => x + y,
            }
        }
    };

    assert_eq!(res, 3);
}

#[cfg(target_pointer_width = "64")]
mod pointer_64_tests {
    use super::maybe_tokio_test;
    use futures::future;
    use std::mem;

    #[maybe_tokio_test]
    async fn struct_size_1() {
        let fut = async {
            let ready = future::ready(0i32);

            tokio::select! {
                _ = ready => {},
            }
        };

        assert_eq!(mem::size_of_val(&fut), 32);
    }

    #[maybe_tokio_test]
    async fn struct_size_2() {
        let fut = async {
            let ready1 = future::ready(0i32);
            let ready2 = future::ready(0i32);

            tokio::select! {
                _ = ready1 => {},
                _ = ready2 => {},
            }
        };

        assert_eq!(mem::size_of_val(&fut), 40);
    }

    #[maybe_tokio_test]
    async fn struct_size_3() {
        let fut = async {
            let ready1 = future::ready(0i32);
            let ready2 = future::ready(0i32);
            let ready3 = future::ready(0i32);

            tokio::select! {
                _ = ready1 => {},
                _ = ready2 => {},
                _ = ready3 => {},
            }
        };

        assert_eq!(mem::size_of_val(&fut), 48);
    }
}

#[maybe_tokio_test]
async fn mutable_borrowing_future_with_same_borrow_in_block() {
    let mut value = 234;

    tokio::select! {
        _ = require_mutable(&mut value) => { },
        _ = async_noop() => {
            value += 5;
        },
    }

    assert!(value >= 234);
}

#[maybe_tokio_test]
async fn mutable_borrowing_future_with_same_borrow_in_block_and_else() {
    let mut value = 234;

    tokio::select! {
        _ = require_mutable(&mut value) => { },
        _ = async_noop() => {
            value += 5;
        },
        else => {
            value += 27;
        },
    }

    assert!(value >= 234);
}

#[maybe_tokio_test]
async fn future_panics_after_poll() {
    use tokio_test::task;

    let (tx, rx) = oneshot::channel();

    let mut polled = false;

    let f = poll_fn(|_| {
        assert!(!polled);
        polled = true;
        Ready(None::<()>)
    });

    let mut f = task::spawn(async {
        tokio::select! {
            Some(_) = f => unreachable!(),
            ret = rx => ret.unwrap(),
        }
    });

    assert_pending!(f.poll());
    assert_pending!(f.poll());

    assert_ok!(tx.send(1));

    let res = assert_ready!(f.poll());
    assert_eq!(1, res);
}

#[maybe_tokio_test]
async fn disable_with_if() {
    use tokio_test::task;

    let f = poll_fn(|_| panic!());
    let (tx, rx) = oneshot::channel();

    let mut f = task::spawn(async {
        tokio::select! {
            _ = f, if false => unreachable!(),
            _ = rx => (),
        }
    });

    assert_pending!(f.poll());

    assert_ok!(tx.send(()));
    assert!(f.is_woken());

    assert_ready!(f.poll());
}

#[maybe_tokio_test]
async fn join_with_select() {
    use tokio_test::task;

    let (tx1, mut rx1) = oneshot::channel();
    let (tx2, mut rx2) = oneshot::channel();

    let mut f = task::spawn(async {
        let mut a = None;
        let mut b = None;

        while a.is_none() || b.is_none() {
            tokio::select! {
                v1 = &mut rx1, if a.is_none() => a = Some(assert_ok!(v1)),
                v2 = &mut rx2, if b.is_none() => b = Some(assert_ok!(v2))
            }
        }

        (a.unwrap(), b.unwrap())
    });

    assert_pending!(f.poll());

    assert_ok!(tx1.send(123));
    assert!(f.is_woken());
    assert_pending!(f.poll());

    assert_ok!(tx2.send(456));
    assert!(f.is_woken());
    let (a, b) = assert_ready!(f.poll());

    assert_eq!(a, 123);
    assert_eq!(b, 456);
}

#[tokio::test]
#[cfg(feature = "full")]
async fn use_future_in_if_condition() {
    use tokio::time::{self, Duration};

    tokio::select! {
        _ = time::sleep(Duration::from_millis(10)), if false => {
            panic!("if condition ignored")
        }
        _ = async { 1u32 } => {
        }
    }
}

#[tokio::test]
#[cfg(feature = "full")]
async fn use_future_in_if_condition_biased() {
    use tokio::time::{self, Duration};

    tokio::select! {
        biased;
        _ = time::sleep(Duration::from_millis(10)), if false => {
            panic!("if condition ignored")
        }
        _ = async { 1u32 } => {
        }
    }
}

#[maybe_tokio_test]
async fn many_branches() {
    let num = tokio::select! {
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
        x = async { 1 } => x,
    };

    assert_eq!(1, num);
}

#[maybe_tokio_test]
async fn never_branch_no_warnings() {
    let t = tokio::select! {
        _ = async_never() => 0,
        one_async_ready = one() => one_async_ready,
    };
    assert_eq!(t, 1);
}

async fn one() -> usize {
    1
}

async fn require_mutable(_: &mut i32) {}
async fn async_noop() {}

async fn async_never() -> ! {
    futures::future::pending().await
}

// From https://github.com/tokio-rs/tokio/issues/2857
#[maybe_tokio_test]
async fn mut_on_left_hand_side() {
    let v = async move {
        let ok = async { 1 };
        tokio::pin!(ok);
        tokio::select! {
            mut a = &mut ok => {
                a += 1;
                a
            }
        }
    }
    .await;
    assert_eq!(v, 2);
}

#[maybe_tokio_test]
async fn biased_one_not_ready() {
    let (_tx1, rx1) = oneshot::channel::<i32>();
    let (tx2, rx2) = oneshot::channel::<i32>();
    let (tx3, rx3) = oneshot::channel::<i32>();

    tx2.send(2).unwrap();
    tx3.send(3).unwrap();

    let v = tokio::select! {
        biased;

        _ = rx1 => unreachable!(),
        res = rx2 => {
            assert_ok!(res)
        },
        _ = rx3 => {
            panic!("This branch should never be activated because `rx2` should be polled before `rx3` due to `biased;`.")
        }
    };

    assert_eq!(2, v);
}

#[maybe_tokio_test]
#[cfg(feature = "full")]
async fn biased_eventually_ready() {
    use tokio::task::yield_now;

    let one = async {};
    let two = async { yield_now().await };
    let three = async { yield_now().await };

    let mut count = 0u8;

    tokio::pin!(one, two, three);

    loop {
        tokio::select! {
            biased;

            _ = &mut two, if count < 2 => {
                count += 1;
                assert_eq!(count, 2);
            }
            _ = &mut three, if count < 3 => {
                count += 1;
                assert_eq!(count, 3);
            }
            _ = &mut one, if count < 1 => {
                count += 1;
                assert_eq!(count, 1);
            }
            else => break,
        }
    }

    assert_eq!(count, 3);
}

// https://github.com/tokio-rs/tokio/issues/3830
// https://github.com/rust-lang/rust-clippy/issues/7304
#[warn(clippy::default_numeric_fallback)]
pub async fn default_numeric_fallback() {
    tokio::select! {
        _ = async {} => (),
        else => (),
    }
}

// https://github.com/tokio-rs/tokio/issues/4182
#[maybe_tokio_test]
async fn mut_ref_patterns() {
    tokio::select! {
        Some(mut foo) = async { Some("1".to_string()) } => {
            assert_eq!(foo, "1");
            foo = "2".to_string();
            assert_eq!(foo, "2");
        },
    };

    tokio::select! {
        Some(ref foo) = async { Some("1".to_string()) } => {
            assert_eq!(*foo, "1");
        },
    };

    tokio::select! {
        Some(ref mut foo) = async { Some("1".to_string()) } => {
            assert_eq!(*foo, "1");
            *foo = "2".to_string();
            assert_eq!(*foo, "2");
        },
    };
}

#[cfg(tokio_unstable)]
mod unstable {
    use tokio::runtime::RngSeed;

    #[test]
    fn deterministic_select_current_thread() {
        let seed = b"bytes used to generate seed";
        let rt1 = tokio::runtime::Builder::new_current_thread()
            .rng_seed(RngSeed::from_bytes(seed))
            .build()
            .unwrap();
        let rt1_values = rt1.block_on(async { (select_0_to_9().await, select_0_to_9().await) });

        let rt2 = tokio::runtime::Builder::new_current_thread()
            .rng_seed(RngSeed::from_bytes(seed))
            .build()
            .unwrap();
        let rt2_values = rt2.block_on(async { (select_0_to_9().await, select_0_to_9().await) });

        assert_eq!(rt1_values, rt2_values);
    }

    #[test]
    #[cfg(all(feature = "rt-multi-thread", not(target_os = "wasi")))]
    fn deterministic_select_multi_thread() {
        let seed = b"bytes used to generate seed";
        let rt1 = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .rng_seed(RngSeed::from_bytes(seed))
            .build()
            .unwrap();
        let rt1_values = rt1.block_on(async {
            let _ = tokio::spawn(async { (select_0_to_9().await, select_0_to_9().await) }).await;
        });

        let rt2 = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .rng_seed(RngSeed::from_bytes(seed))
            .build()
            .unwrap();
        let rt2_values = rt2.block_on(async {
            let _ = tokio::spawn(async { (select_0_to_9().await, select_0_to_9().await) }).await;
        });

        assert_eq!(rt1_values, rt2_values);
    }

    async fn select_0_to_9() -> u32 {
        tokio::select!(
            x = async { 0 } => x,
            x = async { 1 } => x,
            x = async { 2 } => x,
            x = async { 3 } => x,
            x = async { 4 } => x,
            x = async { 5 } => x,
            x = async { 6 } => x,
            x = async { 7 } => x,
            x = async { 8 } => x,
            x = async { 9 } => x,
        )
    }
}

#[tokio::test]
async fn select_into_future() {
    struct NotAFuture;
    impl std::future::IntoFuture for NotAFuture {
        type Output = ();
        type IntoFuture = std::future::Ready<()>;

        fn into_future(self) -> Self::IntoFuture {
            std::future::ready(())
        }
    }

    tokio::select! {
        () = NotAFuture => {},
    }
}

// regression test for https://github.com/tokio-rs/tokio/issues/6721
#[tokio::test]
async fn temporary_lifetime_extension() {
    tokio::select! {
        () = &mut std::future::ready(()) => {},
    }
}

#[tokio::test]
async fn select_is_budget_aware() {
    const BUDGET: usize = 128;

    let task = || {
        Box::pin(async move {
            tokio::select! {
                biased;

                () = tokio::task::coop::consume_budget() => {},
                () = std::future::ready(()) => {}
            }
        })
    };

    for _ in 0..BUDGET {
        let poll = futures::poll!(&mut task());
        assert!(poll.is_ready());
    }

    let poll = futures::poll!(&mut task());
    assert!(poll.is_pending());
}
