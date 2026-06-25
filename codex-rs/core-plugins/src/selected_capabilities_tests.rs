use std::sync::Arc;
use std::time::Duration;

use codex_exec_server::EnvironmentManager;
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use tokio::sync::oneshot;
use tokio::time::timeout;

use super::*;

struct DropSignal(Option<oneshot::Sender<()>>);

impl Drop for DropSignal {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(());
        }
    }
}

#[tokio::test]
async fn snapshots_preserve_root_order_and_advance_on_terminal_transitions() {
    let roots = vec![
        selected_root("root-a", "local"),
        selected_root("root-b", "local"),
    ];
    let environment = EnvironmentManager::default_for_tests()
        .get_environment(LOCAL_ENVIRONMENT_ID)
        .expect("local environment");
    let (first_tx, first_rx) = oneshot::channel();
    let (second_tx, second_rx) = oneshot::channel();
    let bindings = SelectedCapabilityBindings::from_resolutions(
        roots.clone(),
        vec![
            Box::pin(async move { first_rx.await.expect("first resolution") }),
            Box::pin(async move { second_rx.await.expect("second resolution") }),
        ],
    );

    let initial = bindings.snapshot();
    let activation = SelectedCapabilityActivation::new(initial.clone());
    assert_eq!(initial.generation(), 0);
    assert_eq!(
        initial
            .entries()
            .iter()
            .map(|entry| entry.selected_root().id.as_str())
            .collect::<Vec<_>>(),
        vec!["root-a", "root-b"]
    );
    assert!(
        initial
            .entries()
            .iter()
            .all(|entry| matches!(entry.status(), SelectedCapabilityBindingStatus::Pending))
    );

    assert!(
        second_tx
            .send(Ok(ResolvedSelectedCapabilityRoot::new(
                /*selection_order*/ 1,
                roots[1].clone(),
                Arc::clone(&environment),
                None,
            )))
            .is_ok()
    );
    let after_second = timeout(
        Duration::from_secs(1),
        bindings.wait_for_change(/*generation*/ 0),
    )
    .await
    .expect("second root should resolve");
    assert_eq!(after_second.generation(), 1);
    assert!(matches!(
        after_second.entries()[0].status(),
        SelectedCapabilityBindingStatus::Pending
    ));
    let SelectedCapabilityBindingStatus::Ready(second) = after_second.entries()[1].status() else {
        panic!("second root should be ready");
    };
    assert_eq!(second.selection_order(), 1);
    assert_eq!(second.selected_root(), &roots[1]);
    assert!(Arc::ptr_eq(
        &second.file_system(),
        &environment.get_filesystem()
    ));
    assert!(activation.publish(after_second.clone()));
    assert_eq!(activation.snapshot().generation(), 1);
    assert!(!activation.publish(initial));

    assert!(
        first_tx
            .send(Err(SelectedCapabilityFailure {
                message: "root-a failed".to_string(),
            }))
            .is_ok()
    );
    let terminal = timeout(
        Duration::from_secs(1),
        bindings.wait_for_change(/*generation*/ 1),
    )
    .await
    .expect("first root should fail");
    assert_eq!(terminal.generation(), 2);
    let SelectedCapabilityBindingStatus::Failed(first) = terminal.entries()[0].status() else {
        panic!("first root should have failed");
    };
    assert_eq!(first.message(), "root-a failed");
    assert_eq!(
        bindings.resolve_all().await.generation(),
        terminal.generation()
    );
}

#[tokio::test]
async fn unavailable_environment_becomes_one_terminal_failure() {
    let bindings = SelectedCapabilityBindings::new(
        vec![selected_root("root-a", "missing")],
        Arc::new(EnvironmentManager::without_environments()),
    );

    let terminal = timeout(Duration::from_secs(1), bindings.resolve_all())
        .await
        .expect("missing environment should resolve as a failure");

    assert_eq!(terminal.generation(), 1);
    let SelectedCapabilityBindingStatus::Failed(failure) = terminal.entries()[0].status() else {
        panic!("missing environment should fail");
    };
    assert!(
        failure
            .message()
            .contains("unavailable environment `missing`")
    );
}

#[tokio::test]
async fn one_transition_wakes_all_generation_waiters() {
    let root = selected_root("root-a", "local");
    let (resolution_tx, resolution_rx) = oneshot::channel();
    let bindings = SelectedCapabilityBindings::from_resolutions(
        vec![root],
        vec![Box::pin(
            async move { resolution_rx.await.expect("resolution") },
        )],
    );
    let first = {
        let bindings = bindings.clone();
        tokio::spawn(async move {
            bindings
                .wait_for_change(/*generation*/ 0)
                .await
                .generation()
        })
    };
    let second = {
        let bindings = bindings.clone();
        tokio::spawn(async move {
            bindings
                .wait_for_change(/*generation*/ 0)
                .await
                .generation()
        })
    };
    tokio::task::yield_now().await;

    assert!(
        resolution_tx
            .send(Err(SelectedCapabilityFailure {
                message: "failed".to_string(),
            }))
            .is_ok()
    );

    assert_eq!(first.await.expect("first waiter"), 1);
    assert_eq!(second.await.expect("second waiter"), 1);
}

#[tokio::test]
async fn later_ready_root_is_not_blocked_by_pending_roots() {
    let roots = (0..5)
        .map(|index| selected_root(&format!("root-{index}"), "local"))
        .collect::<Vec<_>>();
    let environment = EnvironmentManager::default_for_tests()
        .get_environment(LOCAL_ENVIRONMENT_ID)
        .expect("local environment");
    let mut senders = Vec::new();
    let mut resolutions = Vec::new();
    for _ in &roots {
        let (sender, receiver) = oneshot::channel();
        senders.push(sender);
        resolutions
            .push(Box::pin(async move { receiver.await.expect("resolution") }) as ResolutionFuture);
    }
    let bindings = SelectedCapabilityBindings::from_resolutions(roots.clone(), resolutions);

    assert!(
        senders
            .pop()
            .expect("fifth sender")
            .send(Ok(ResolvedSelectedCapabilityRoot::new(
                /*selection_order*/ 4,
                roots[4].clone(),
                environment,
                None,
            )))
            .is_ok()
    );

    let snapshot = timeout(
        Duration::from_secs(1),
        bindings.wait_for_change(/*generation*/ 0),
    )
    .await
    .expect("fifth root should resolve independently");
    assert_eq!(snapshot.generation(), 1);
    assert!(
        snapshot.entries()[..4]
            .iter()
            .all(|entry| matches!(entry.status(), SelectedCapabilityBindingStatus::Pending))
    );
    assert!(matches!(
        snapshot.entries()[4].status(),
        SelectedCapabilityBindingStatus::Ready(_)
    ));
}

#[tokio::test]
async fn resolution_panic_becomes_terminal_failure() {
    let bindings = SelectedCapabilityBindings::from_resolutions(
        vec![selected_root("root-a", "local")],
        vec![Box::pin(async { panic!("resolution panic") })],
    );

    let terminal = timeout(Duration::from_secs(1), bindings.resolve_all())
        .await
        .expect("panicked resolution should become terminal");

    let SelectedCapabilityBindingStatus::Failed(failure) = terminal.entries()[0].status() else {
        panic!("panicked resolution should fail");
    };
    assert_eq!(failure.message(), "selected capability resolution panicked");
}

#[tokio::test]
async fn dropping_bindings_cancels_pending_resolution() {
    let (started_tx, started_rx) = oneshot::channel();
    let (dropped_tx, dropped_rx) = oneshot::channel();
    let bindings = SelectedCapabilityBindings::from_resolutions(
        vec![selected_root("root-a", "local")],
        vec![Box::pin(async move {
            let _drop_signal = DropSignal(Some(dropped_tx));
            let _ = started_tx.send(());
            std::future::pending().await
        })],
    );
    started_rx.await.expect("resolution should start");

    drop(bindings);

    timeout(Duration::from_secs(1), dropped_rx)
        .await
        .expect("resolution should be canceled")
        .expect("drop signal should be sent");
}

#[tokio::test]
async fn empty_bindings_are_immediately_terminal() {
    let bindings = SelectedCapabilityBindings::new(
        Vec::new(),
        Arc::new(EnvironmentManager::without_environments()),
    );

    let snapshot = bindings.resolve_all().await;

    assert_eq!(snapshot.generation(), 0);
    assert!(snapshot.entries().is_empty());
    assert!(snapshot.is_terminal());
}

fn selected_root(id: &str, environment_id: &str) -> SelectedCapabilityRoot {
    SelectedCapabilityRoot {
        id: id.to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: environment_id.to_string(),
            path: PathUri::parse(&format!("file:///plugins/{id}")).expect("plugin root URI"),
        },
    }
}
