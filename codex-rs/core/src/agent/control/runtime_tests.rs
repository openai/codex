use super::*;
use crate::thread_manager::default_thread_id_generator;
use futures::future;

#[tokio::test]
async fn aborted_teardown_fails_tree_shutdown() {
    let runtime = LocalAgentRuntime::new(
        Weak::default(),
        default_thread_id_generator(),
        /*rollout_budget*/ None,
    );
    let teardown = runtime
        .admit_start()
        .expect("teardown should be admitted")
        .into_teardown_guard();
    let task = tokio::spawn(async move {
        let _teardown = teardown;
        future::pending::<()>().await;
    });
    let shutdown = runtime.request_shutdown();

    task.abort();
    task.await.expect_err("teardown task should be aborted");

    shutdown
        .wait()
        .await
        .expect_err("aborted teardown must fail tree shutdown");
}
