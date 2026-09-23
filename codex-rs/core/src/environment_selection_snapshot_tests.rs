//! Regression coverage for environment snapshots and pending configuration.

use super::*;
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_otel::SessionTelemetry;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use codex_protocol::sandbox::SandboxType;
use pretty_assertions::assert_eq;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::sync::watch;
use tokio::time::timeout;

#[tokio::test]
async fn unpolled_snapshot_does_not_delay_canceling_a_removed_environment() {
    let cwd = PathUri::from_abs_path(&AbsolutePathBuf::current_dir().expect("cwd"));
    let selection = TurnEnvironmentSelection {
        environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
        cwd: cwd.clone(),
        workspace_roots: vec![cwd],
        config: EnvironmentConfigState::Pending,
    };
    let environments = ThreadEnvironments::new(
        Arc::new(EnvironmentManager::default_for_tests()),
        crate::shell::default_user_shell(),
        ThreadEnvironmentDefaults::new(tests::test_environment_config(), SandboxType::None),
        ShellSnapshot::disabled(),
        TurnEnvironmentSnapshot::default(),
        /*non_blocking_snapshots*/ false,
    );
    environments.update_selections(&[selection]);
    let starting = environments
        .snapshot()
        .await
        .starting()
        .next()
        .unwrap()
        .clone();
    let held_snapshot = environments.snapshot();

    environments.update_selections(&[]);

    let error = timeout(Duration::from_secs(5), starting.wait_until_ready())
        .await
        .expect("removing the environment should cancel the wait before the old snapshot is polled")
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("environment configuration was canceled")
    );
    assert!(matches!(
        held_snapshot.await.environments.as_slice(),
        [TurnEnvironmentState::Failed { .. }]
    ));
}

#[tokio::test]
async fn credential_refresh_does_not_restore_a_removed_environment() {
    let home = tempfile::tempdir().expect("home");
    let cwd = AbsolutePathBuf::from_absolute_path(home.path()).expect("cwd");
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let session_id = ThreadId::new();
    let telemetry = SessionTelemetry::new(
        session_id,
        "test",
        "test",
        /*account_id*/ None,
        /*account_email*/ None,
        /*auth_mode*/ None,
        "test".to_string(),
        /*log_user_prompts*/ false,
        "test".to_string(),
        SessionSource::Cli,
    );
    let (broker, _) = watch::channel(SnapshotCredentialBrokerState::Starting);
    let shell_snapshot = ShellSnapshot::new(
        cwd,
        session_id,
        telemetry,
        /*state_db*/ None,
        Some(broker),
        /*prefer_executor_snapshots*/ false,
    );
    let environments = Arc::new(ThreadEnvironments::new(
        Arc::new(EnvironmentManager::default_for_tests()),
        crate::shell::default_user_shell(),
        ThreadEnvironmentDefaults::new(tests::test_environment_config(), SandboxType::None),
        shell_snapshot,
        TurnEnvironmentSnapshot::default(),
        /*non_blocking_snapshots*/ false,
    ));
    environments.update_selections(&[TurnEnvironmentSelection {
        environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
        cwd: cwd_uri.clone(),
        workspace_roots: vec![cwd_uri],
        config: EnvironmentConfigState::FromThread,
    }]);
    let resolution = environments.state.lock().unwrap().environments[0]
        .resolution
        .clone();
    let resolved = resolution.await.expect("local is ready");
    let (refresh_paused_tx, refresh_paused_rx) = oneshot::channel();
    let (resume_refresh_tx, resume_refresh_rx) = std::sync::mpsc::channel();
    // Pause the credential refresh while it is using the current list.
    environments.state.lock().unwrap().environments[0].resolution = async move {
        refresh_paused_tx.send(()).expect("signal refresh paused");
        resume_refresh_rx.recv().expect("resume refresh");
        Ok(resolved)
    }
    .boxed()
    .shared();

    let refreshing = Arc::clone(&environments);
    let refresh = tokio::task::spawn_blocking(move || {
        refreshing.set_snapshot_credential_broker(SnapshotCredentialBrokerState::Unavailable);
    });
    timeout(Duration::from_secs(/*secs*/ 5), refresh_paused_rx)
        .await
        .expect("credential refresh should reach the old list")
        .expect("refresh paused");
    let (removal_started_tx, removal_started_rx) = oneshot::channel();
    let removing = Arc::clone(&environments);
    let mut removal = tokio::task::spawn_blocking(move || {
        removal_started_tx.send(()).expect("signal removal started");
        removing.update_selections(&[]);
    });
    timeout(Duration::from_secs(/*secs*/ 5), removal_started_rx)
        .await
        .expect("removal should start")
        .expect("removal started");
    let early_removal = timeout(Duration::from_secs(/*secs*/ 1), &mut removal)
        .await
        .ok();
    let removal_finished_early = early_removal.is_some();

    resume_refresh_tx
        .send(())
        .expect("release credential refresh");
    refresh.await.expect("credential refresh finished");
    match early_removal {
        Some(result) => result.expect("removal finished"),
        None => removal.await.expect("removal finished"),
    }
    assert_eq!(environments.selections(), Vec::new());
    assert!(
        !removal_finished_early,
        "removal should wait until the credential refresh finishes"
    );
}
