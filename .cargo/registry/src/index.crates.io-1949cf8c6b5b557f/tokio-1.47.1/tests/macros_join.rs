#![cfg(feature = "macros")]
#![allow(clippy::disallowed_names)]
use std::sync::Arc;

#[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
#[cfg(target_pointer_width = "64")]
use wasm_bindgen_test::wasm_bindgen_test as test;
#[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
use wasm_bindgen_test::wasm_bindgen_test as maybe_tokio_test;

#[cfg(not(all(target_family = "wasm", not(target_os = "wasi"))))]
use tokio::test as maybe_tokio_test;

use tokio::sync::{oneshot, Semaphore};
use tokio_test::{assert_pending, assert_ready, task};

#[maybe_tokio_test]
async fn sync_one_lit_expr_comma() {
    let foo = tokio::join!(async { 1 },);
    assert_eq!(foo, (1,));

    let foo = tokio::join!(biased; async { 1 },);
    assert_eq!(foo, (1,));
}

#[maybe_tokio_test]
async fn sync_one_lit_expr_no_comma() {
    let foo = tokio::join!(async { 1 });
    assert_eq!(foo, (1,));

    let foo = tokio::join!(biased; async { 1 });
    assert_eq!(foo, (1,));
}

#[maybe_tokio_test]
async fn sync_two_lit_expr_comma() {
    let foo = tokio::join!(async { 1 }, async { 2 },);
    assert_eq!(foo, (1, 2));

    let foo = tokio::join!(biased; async { 1 }, async { 2 },);
    assert_eq!(foo, (1, 2));
}

#[maybe_tokio_test]
async fn sync_two_lit_expr_no_comma() {
    let foo = tokio::join!(async { 1 }, async { 2 });
    assert_eq!(foo, (1, 2));

    let foo = tokio::join!(biased; async { 1 }, async { 2 });
    assert_eq!(foo, (1, 2));
}

#[maybe_tokio_test]
async fn two_await() {
    let (tx1, rx1) = oneshot::channel::<&str>();
    let (tx2, rx2) = oneshot::channel::<u32>();

    let mut join = task::spawn(async {
        tokio::join!(async { rx1.await.unwrap() }, async { rx2.await.unwrap() })
    });

    assert_pending!(join.poll());

    tx2.send(123).unwrap();
    assert!(join.is_woken());
    assert_pending!(join.poll());

    tx1.send("hello").unwrap();
    assert!(join.is_woken());
    let res = assert_ready!(join.poll());

    assert_eq!(("hello", 123), res);
}

#[test]
#[cfg(target_pointer_width = "64")]
fn join_size() {
    use futures::future;
    use std::mem;

    let fut = async {
        let ready = future::ready(0i32);
        tokio::join!(ready)
    };
    assert_eq!(mem::size_of_val(&fut), 32);

    let fut = async {
        let ready1 = future::ready(0i32);
        let ready2 = future::ready(0i32);
        tokio::join!(ready1, ready2)
    };
    assert_eq!(mem::size_of_val(&fut), 48);
}

async fn non_cooperative_task(permits: Arc<Semaphore>) -> usize {
    let mut exceeded_budget = 0;

    for _ in 0..5 {
        // Another task should run after this task uses its whole budget
        for _ in 0..128 {
            let _permit = permits.clone().acquire_owned().await.unwrap();
        }

        exceeded_budget += 1;
    }

    exceeded_budget
}

async fn poor_little_task(permits: Arc<Semaphore>) -> usize {
    let mut how_many_times_i_got_to_run = 0;

    for _ in 0..5 {
        let _permit = permits.clone().acquire_owned().await.unwrap();
        how_many_times_i_got_to_run += 1;
    }

    how_many_times_i_got_to_run
}

#[tokio::test]
async fn join_does_not_allow_tasks_to_starve() {
    let permits = Arc::new(Semaphore::new(1));

    // non_cooperative_task should yield after its budget is exceeded and then poor_little_task should run.
    let (non_cooperative_result, little_task_result) = tokio::join!(
        non_cooperative_task(Arc::clone(&permits)),
        poor_little_task(permits)
    );

    assert_eq!(5, non_cooperative_result);
    assert_eq!(5, little_task_result);
}

#[tokio::test]
async fn a_different_future_is_polled_first_every_time_poll_fn_is_polled() {
    let poll_order = Arc::new(std::sync::Mutex::new(vec![]));

    let fut = |x, poll_order: Arc<std::sync::Mutex<Vec<i32>>>| async move {
        for _ in 0..4 {
            {
                let mut guard = poll_order.lock().unwrap();

                guard.push(x);
            }

            tokio::task::yield_now().await;
        }
    };

    tokio::join!(
        fut(1, Arc::clone(&poll_order)),
        fut(2, Arc::clone(&poll_order)),
        fut(3, Arc::clone(&poll_order)),
    );

    // Each time the future created by join! is polled, it should start
    // by polling a different future first.
    assert_eq!(
        vec![1, 2, 3, 2, 3, 1, 3, 1, 2, 1, 2, 3],
        *poll_order.lock().unwrap()
    );
}

#[tokio::test]
async fn futures_are_polled_in_order_in_biased_mode() {
    let poll_order = Arc::new(std::sync::Mutex::new(vec![]));

    let fut = |x, poll_order: Arc<std::sync::Mutex<Vec<i32>>>| async move {
        for _ in 0..4 {
            {
                let mut guard = poll_order.lock().unwrap();

                guard.push(x);
            }

            tokio::task::yield_now().await;
        }
    };

    tokio::join!(
        biased;
        fut(1, Arc::clone(&poll_order)),
        fut(2, Arc::clone(&poll_order)),
        fut(3, Arc::clone(&poll_order)),
    );

    // Each time the future created by join! is polled, it should start
    // by polling in the order as declared in the macro inputs.
    assert_eq!(
        vec![1, 2, 3, 1, 2, 3, 1, 2, 3, 1, 2, 3],
        *poll_order.lock().unwrap()
    );
}

#[test]
#[cfg(target_pointer_width = "64")]
fn join_size_biased() {
    use futures::future;
    use std::mem;

    let fut = async {
        let ready = future::ready(0i32);
        tokio::join!(biased; ready)
    };
    assert_eq!(mem::size_of_val(&fut), 24);

    let fut = async {
        let ready1 = future::ready(0i32);
        let ready2 = future::ready(0i32);
        tokio::join!(biased; ready1, ready2)
    };
    assert_eq!(mem::size_of_val(&fut), 40);
}

#[tokio::test]
#[allow(clippy::unit_cmp)]
async fn empty_join() {
    assert_eq!(tokio::join!(), ());
    assert_eq!(tokio::join!(biased;), ());
}

#[tokio::test]
async fn join_into_future() {
    struct NotAFuture;
    impl std::future::IntoFuture for NotAFuture {
        type Output = ();
        type IntoFuture = std::future::Ready<()>;

        fn into_future(self) -> Self::IntoFuture {
            std::future::ready(())
        }
    }

    tokio::join!(NotAFuture);
}
