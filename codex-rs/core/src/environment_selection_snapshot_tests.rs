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

/// Roots stay attached through config changes and retain API ordering across environment states.
#[tokio::test]
async fn capability_roots_follow_attachments_through_configuration_changes() {
    let manager = Arc::new(EnvironmentManager::default_for_tests());
    manager
        .upsert_environment(
            "other".to_string(),
            "http://example.com".to_string(),
            /*connect_timeout*/ None,
        )
        .expect("register other environment");
    let cwd = PathUri::from_abs_path(&AbsolutePathBuf::current_dir().expect("cwd"));
    let mut local = TurnEnvironmentSelection {
        selected_capability_roots: Default::default(),
        environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
        cwd: cwd.clone(),
        workspace_roots: Vec::new(),
        config: EnvironmentConfigState::Pending,
    };
    let mut other = TurnEnvironmentSelection {
        environment_id: "other".to_string(),
        config: EnvironmentConfigState::Failed("offline".to_string()),
        ..local.clone()
    };
    let roots = [(&other, "first"), (&local, "second"), (&other, "third")]
        .into_iter()
        .map(|(selection, id)| SelectedCapabilityRoot {
            id: id.to_string(),
            location: CapabilityRootLocation::Environment {
                environment_id: selection.environment_id.clone(),
                path: cwd.clone(),
            },
        })
        .collect::<Vec<_>>();
    local.selected_capability_roots =
        EnvironmentCapabilityRoots::for_environment(&local.environment_id, &roots);
    other.selected_capability_roots =
        EnvironmentCapabilityRoots::for_environment(&other.environment_id, &roots);
    let parent = ThreadEnvironments::new(
        Arc::clone(&manager),
        crate::shell::default_user_shell(),
        ThreadEnvironmentDefaults::new(tests::test_environment_config(), SandboxType::None),
        ShellSnapshot::disabled(),
        TurnEnvironmentSnapshot::default(),
        /*non_blocking_snapshots*/ false,
    );
    parent.update_selections(&[local.clone(), other.clone()]);
    let snapshot = parent.snapshot().await;
    assert_eq!(snapshot.selected_capability_roots(), roots);
    assert!(matches!(
        &snapshot.environments[0],
        TurnEnvironmentState::Starting(_)
    ));
    assert!(matches!(
        &snapshot.environments[1],
        TurnEnvironmentState::Failed { .. }
    ));

    let child = ThreadEnvironments::new(
        Arc::clone(&manager),
        crate::shell::default_user_shell(),
        ThreadEnvironmentDefaults::new(tests::test_environment_config(), SandboxType::None),
        ShellSnapshot::disabled(),
        snapshot,
        /*non_blocking_snapshots*/ false,
    );
    assert_eq!(
        child.snapshot().await.selected_capability_roots(),
        vec![roots[1].clone()]
    );

    local.config = EnvironmentConfigState::Ready(tests::test_environment_config());
    parent.update_selections(&[local.clone(), other.clone()]);
    let ready = parent.snapshot().await;
    assert!(matches!(
        &ready.environments[0],
        TurnEnvironmentState::Ready(_)
    ));
    assert_eq!(ready.selected_capability_roots(), roots);

    local.cwd = cwd.join("changed-cwd").expect("changed cwd");
    parent.update_selections(&[other.clone(), local.clone()]);
    assert_eq!(parent.snapshot().await.selected_capability_roots(), roots);

    // A second thread selects different roots on the same executor without changing the parent.
    let independent_roots = vec![SelectedCapabilityRoot {
        id: "independent".to_string(),
        ..roots[1].clone()
    }];
    local.selected_capability_roots =
        EnvironmentCapabilityRoots::for_environment(&local.environment_id, &independent_roots);
    child.update_selections(std::slice::from_ref(&local));
    assert_eq!(
        child.snapshot().await.selected_capability_roots(),
        independent_roots
    );
    assert_eq!(parent.snapshot().await.selected_capability_roots(), roots);
    assert!(Arc::ptr_eq(
        &child
            .snapshot()
            .await
            .primary()
            .expect("child executor")
            .environment,
        &parent
            .snapshot()
            .await
            .turn_environments()
            .find(|environment| environment.selection.environment_id == LOCAL_ENVIRONMENT_ID)
            .expect("parent executor")
            .environment,
    ));
}

#[tokio::test]
async fn unpolled_snapshot_does_not_delay_canceling_a_removed_environment() {
    let cwd = PathUri::from_abs_path(&AbsolutePathBuf::current_dir().expect("cwd"));
    let selection = TurnEnvironmentSelection {
        selected_capability_roots: Default::default(),
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
async fn updating_another_environment_retries_the_executor_without_canceling_pending_config() {
    let manager = Arc::new(EnvironmentManager::default_for_tests());
    manager
        .upsert_environment(
            "failing".to_string(),
            "http://example.com".to_string(),
            /*connect_timeout*/ None,
        )
        .unwrap();
    let cwd = PathUri::from_abs_path(&AbsolutePathBuf::current_dir().unwrap());
    let mut local = TurnEnvironmentSelection {
        selected_capability_roots: Default::default(),
        environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
        cwd,
        workspace_roots: Vec::new(),
        config: EnvironmentConfigState::FromThread,
    };
    let mut pending = TurnEnvironmentSelection {
        environment_id: "failing".to_string(),
        config: EnvironmentConfigState::Pending,
        ..local.clone()
    };
    let environments = ThreadEnvironments::new(
        manager,
        crate::shell::default_user_shell(),
        ThreadEnvironmentDefaults::new(tests::test_environment_config(), SandboxType::None),
        ShellSnapshot::disabled(),
        TurnEnvironmentSnapshot::default(),
        /*non_blocking_snapshots*/ true,
    );
    let current = || {
        let state = environments.state.lock().unwrap();
        let failing = &state.environments[1];
        (
            failing.resolution.clone(),
            failing.pending_completion.as_ref().unwrap().subscribe(),
            failing.owner_config_result.clone().unwrap(),
        )
    };

    environments.update_selections(&[local.clone(), pending.clone()]);
    let (first_attempt, mut original_config, original_owner) = current();
    assert!(first_attempt.clone().await.is_err());

    local.config = EnvironmentConfigState::Ready(tests::test_environment_config());
    environments.update_selections(&[local.clone(), pending.clone()]);
    let (second_attempt, _, second_owner) = current();
    assert!(!first_attempt.ptr_eq(&second_attempt));
    assert!(original_owner.ptr_eq(&second_owner));
    assert!(second_attempt.await.is_err());
    assert!(original_config.borrow().is_none());

    let config = tests::test_environment_config();
    pending.config = EnvironmentConfigState::Ready(config.clone());
    environments.update_selections(&[local, pending]);
    let received = timeout(
        Duration::from_secs(/*secs*/ 5),
        original_config.wait_for(Option::is_some),
    )
    .await
    .unwrap()
    .unwrap()
    .clone();
    assert_eq!(received, Some(Ok(config.clone())));
    assert_eq!(original_owner.await, Ok(config));
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
        selected_capability_roots: Default::default(),
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
