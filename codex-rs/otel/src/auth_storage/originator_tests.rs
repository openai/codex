//! Verifies attribution isolation and explicit propagation across task boundaries.

use super::AuthStorageOriginator;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use tokio::sync::Barrier;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_clients_keep_their_originator_across_awaits_and_blocking_work() {
    let barrier = Arc::new(Barrier::new(/*n*/ 3));
    let mut tasks = Vec::new();
    for name in ["Codex Desktop", "codex_vscode", "private-client-name"] {
        let barrier = Arc::clone(&barrier);
        let originator = AuthStorageOriginator::from_client_name(name);
        tasks.push(tokio::spawn(originator.scope(async move {
            barrier.wait().await;
            tokio::task::yield_now().await;
            let before = AuthStorageOriginator::current();
            let nested = AuthStorageOriginator::from_client_name("codex_cli_rs")
                .scope(async {
                    tokio::task::yield_now().await;
                    AuthStorageOriginator::current().as_str()
                })
                .await;
            let blocking = tokio::task::spawn_blocking(move || {
                before.sync_scope(AuthStorageOriginator::current)
            })
            .await
            .unwrap();
            (
                before.as_str(),
                nested,
                blocking.as_str(),
                AuthStorageOriginator::current().as_str(),
            )
        })));
    }
    let mut actual = Vec::new();
    for task in tasks {
        actual.push(task.await.unwrap());
    }
    assert_eq!(
        actual,
        ["codex_desktop", "codex_vscode", "other"]
            .map(|originator| (originator, "codex_cli_rs", originator, originator))
            .to_vec()
    );
    assert_eq!(AuthStorageOriginator::current().as_str(), "none");
}
