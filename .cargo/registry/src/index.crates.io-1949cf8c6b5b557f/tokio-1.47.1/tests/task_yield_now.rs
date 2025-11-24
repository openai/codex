#![cfg(all(feature = "full", not(target_os = "wasi"), tokio_unstable))]

use tokio::task;
use tokio_test::task::spawn;

// `yield_now` is tested within the runtime in `rt_common`.
#[test]
fn yield_now_outside_of_runtime() {
    let mut task = spawn(async {
        task::yield_now().await;
    });

    assert!(task.poll().is_pending());
    assert!(task.is_woken());
    assert!(task.poll().is_ready());
}

#[tokio::test(flavor = "multi_thread")]
async fn yield_now_external_executor_and_block_in_place() {
    let j = tokio::spawn(async {
        task::block_in_place(|| futures::executor::block_on(task::yield_now()));
    });
    j.await.unwrap();
}
