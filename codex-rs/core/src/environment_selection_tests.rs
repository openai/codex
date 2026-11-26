#![cfg(test)]
//! Tests for environment selection, readiness, and configuration inheritance.

use std::time::Duration;

use crate::config::PermissionProfileSnapshot;
use codex_exec_server::Environment;
use codex_exec_server::ExecServerRuntimeOptions;
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_exec_server::REMOTE_ENVIRONMENT_ID;
use codex_exec_server_test_support::environment_manager_without_environments;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::ActivePermissionProfile;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemSandboxPolicyContext;
use codex_protocol::protocol::EnvironmentConnectionEvent;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::TurnEnvironmentRequest;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_protocol::sandbox::SandboxType;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::Level;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_test::internal::MockWriter;

use super::*;

pub(super) fn test_environment_config() -> EnvironmentConfig {
    EnvironmentConfig {
        environment_hooks: Default::default(),
        allow_login_shell: true,
        workspace_roots: Vec::new(),
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
        windows_sandbox_type: codex_protocol::sandbox::SandboxType::None,
        use_legacy_landlock: false,
        permission_profile: PermissionProfileSnapshot::legacy(PermissionProfile::read_only()),
        shell_environment_policy: Default::default(),
        exec_policy: None,
        plugins: Default::default(),
        mcp_policy: None,
        network_policy: None,
        selected_capability_roots: Vec::new(),
    }
}

async fn resolve_turn_environments(
    environment_manager: Arc<EnvironmentManager>,
    selections: &[TurnEnvironmentSelection],
) -> Arc<ThreadEnvironments> {
    let turn_environments = Arc::new(ThreadEnvironments::new(
        environment_manager,
        crate::shell::default_user_shell(),
        ThreadEnvironmentDefaults::new(test_environment_config(), SandboxType::None),
        ShellSnapshot::disabled(),
        TurnEnvironmentSnapshot::default(),
        /*non_blocking_snapshots*/ false,
        /*executor_discovery*/ None,
    ));
    turn_environments.update_selections(selections);
    turn_environments.snapshot().await;
    turn_environments
}

fn test_runtime_paths() -> ExecServerRuntimeOptions {
    ExecServerRuntimeOptions::new(
        std::env::current_exe().expect("current exe"),
        /*codex_linux_sandbox_exe*/ None,
    )
    .expect("runtime paths")
}

pub(super) async fn read_websocket_json(websocket: &mut WebSocketStream<TcpStream>) -> Value {
    loop {
        match timeout(std::time::Duration::from_secs(5), websocket.next())
            .await
            .expect("websocket read should not time out")
            .expect("websocket should stay open")
            .expect("websocket frame should read")
        {
            Message::Text(text) => {
                return serde_json::from_str(text.as_ref()).expect("valid JSON-RPC message");
            }
            Message::Binary(bytes) => {
                return serde_json::from_slice(bytes.as_ref()).expect("valid JSON-RPC message");
            }
            Message::Ping(_) | Message::Pong(_) => {}
            other => panic!("expected JSON-RPC message, got {other:?}"),
        }
    }
}

async fn serve_environment_info(listener: TcpListener) {
    let (stream, _) = listener.accept().await.expect("connection");
    let mut websocket = accept_async(stream).await.expect("websocket handshake");

    let initialize = read_websocket_json(&mut websocket).await;
    assert_eq!(initialize["method"], "initialize");
    websocket
        .send(Message::Text(
            serde_json::json!({
                "id": initialize["id"],
                "result": { "sessionId": "test-session" }
            })
            .to_string()
            .into(),
        ))
        .await
        .expect("initialize response");
    let initialized = read_websocket_json(&mut websocket).await;
    assert_eq!(initialized["method"], "initialized");

    let info = read_websocket_json(&mut websocket).await;
    assert_eq!(info["method"], "environment/info");
    websocket
        .send(Message::Text(
            serde_json::json!({
                "id": info["id"],
                "result": {
                    "shell": { "name": "zsh", "path": "/bin/zsh" },
                    "userHomeDir": "file:///home/remote",
                    "platformOs": "windows",
                    "temporaryDirectories": ["file:///tmp/remote"],
                    "capabilities": {"windowsMxc": true},
                }
            })
            .to_string()
            .into(),
        ))
        .await
        .expect("environment info response");
}

#[tokio::test]
async fn default_thread_environment_requests_use_manager_default_id() {
    let cwd = AbsolutePathBuf::current_dir().expect("cwd");
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let manager = EnvironmentManager::create_for_tests(
        Some("ws://127.0.0.1:8765".to_string()),
        Some(test_runtime_paths()),
    )
    .await;

    assert_eq!(
        default_thread_environment_requests(&manager, &cwd, std::slice::from_ref(&cwd)),
        vec![TurnEnvironmentRequest {
            environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
            cwd: cwd_uri.clone(),
            workspace_roots: vec![cwd_uri],
            config: EnvironmentConfigState::FromThread,
        }]
    );
}

#[tokio::test]
async fn toml_default_thread_environment_requests_include_local_and_remote() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        temp_dir.path().join("environments.toml"),
        r#"
[[environments]]
id = "remote"
url = "ws://127.0.0.1:8765"
"#,
    )
    .expect("write environments.toml");
    let cwd = AbsolutePathBuf::current_dir().expect("cwd");
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let manager = EnvironmentManager::from_codex_home(
        temp_dir.path(),
        Some(test_runtime_paths()),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    )
    .await
    .expect("environment manager");

    assert_eq!(
        default_thread_environment_requests(&manager, &cwd, std::slice::from_ref(&cwd)),
        vec![
            TurnEnvironmentRequest {
                environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
                cwd: cwd_uri.clone(),
                workspace_roots: vec![cwd_uri.clone()],
                config: EnvironmentConfigState::FromThread,
            },
            TurnEnvironmentRequest {
                environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                cwd: cwd_uri.clone(),
                workspace_roots: vec![cwd_uri],
                config: EnvironmentConfigState::FromThread,
            },
        ]
    );
}

#[tokio::test]
async fn default_thread_environment_requests_empty_when_default_disabled() {
    let cwd = AbsolutePathBuf::current_dir().expect("cwd");
    let manager = environment_manager_without_environments();

    assert_eq!(
        default_thread_environment_requests(&manager, &cwd, std::slice::from_ref(&cwd)),
        Vec::<TurnEnvironmentRequest>::new()
    );
}

#[tokio::test]
async fn local_environment_uses_configured_shell_and_login_policy() {
    let cwd = AbsolutePathBuf::current_dir().expect("cwd");
    let local_shell = Shell {
        shell_type: crate::shell::ShellType::Zsh,
        shell_path: std::path::PathBuf::from("/configured/zsh"),
    };
    let expected_config = EnvironmentConfig {
        environment_hooks: Default::default(),
        allow_login_shell: false,
        workspace_roots: Vec::new(),
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
        windows_sandbox_type: codex_protocol::sandbox::SandboxType::None,
        use_legacy_landlock: false,
        permission_profile: PermissionProfileSnapshot::active_with_profile_workspace_roots(
            PermissionProfile::read_only(),
            ActivePermissionProfile::read_only(),
            vec![cwd.join("profile-root").into()],
        ),
        shell_environment_policy: Default::default(),
        exec_policy: None,
        plugins: Default::default(),
        mcp_policy: None,
        network_policy: None,
        selected_capability_roots: Vec::new(),
    };
    let turn_environments = ThreadEnvironments::new(
        Arc::new(EnvironmentManager::default_for_tests()),
        local_shell.clone(),
        ThreadEnvironmentDefaults::new(
            expected_config.clone(),
            expected_config.windows_sandbox_type,
        ),
        ShellSnapshot::disabled(),
        TurnEnvironmentSnapshot::default(),
        /*non_blocking_snapshots*/ false,
        /*executor_discovery*/ None,
    );
    turn_environments.update_selections(&[TurnEnvironmentSelection {
        selected_capability_roots: Default::default(),
        environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
        cwd: PathUri::from_abs_path(&cwd),
        workspace_roots: Vec::new(),
        config: EnvironmentConfigState::FromThread,
    }]);

    let snapshot = turn_environments.snapshot().await;
    let environment = snapshot.primary().expect("local environment");
    let expected_user_home_dir = PathUri::from_host_native_path("~").ok();
    let expected_temporary_directories = EnvironmentInfo::local_temporary_directories();

    assert_eq!(environment.shell.as_ref(), Some(&local_shell));
    assert_eq!(environment.config(), &expected_config);
    assert_eq!(
        environment
            .sandbox_context(/*additional_permissions*/ None)
            .policy_context(),
        FileSystemSandboxPolicyContext {
            cwd: environment.cwd(),
            workspace_roots: &[],
            user_home_dir: expected_user_home_dir.as_ref(),
            temporary_directories: Some(expected_temporary_directories.as_slice()),
        }
    );
}

#[tokio::test]
async fn resolve_environment_selections_keeps_first_duplicate_id() {
    let cwd = AbsolutePathBuf::current_dir().expect("cwd");
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let manager = Arc::new(EnvironmentManager::default_for_tests());
    let first = TurnEnvironmentSelection {
        selected_capability_roots: Default::default(),
        environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
        cwd: cwd_uri.clone(),
        workspace_roots: Vec::new(),
        config: EnvironmentConfigState::FromThread,
    };

    let resolved = resolve_turn_environments(
        manager,
        &[
            first.clone(),
            TurnEnvironmentSelection {
                selected_capability_roots: Default::default(),
                environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
                cwd: cwd_uri.join("other").expect("other cwd URI"),
                workspace_roots: Vec::new(),
                config: EnvironmentConfigState::FromThread,
            },
        ],
    )
    .await;

    assert_eq!(resolved.snapshot().await.to_selections(), vec![first]);
}

#[tokio::test]
async fn resolved_environment_selections_use_first_selection_as_primary() {
    let cwd = AbsolutePathBuf::current_dir().expect("cwd");
    let selected_cwd = cwd.join("selected");
    let selected_cwd_uri = PathUri::from_abs_path(&selected_cwd);
    let manager = Arc::new(EnvironmentManager::default_for_tests());

    let resolved = resolve_turn_environments(
        Arc::clone(&manager),
        &[TurnEnvironmentSelection {
            selected_capability_roots: Default::default(),
            environment_id: "local".to_string(),
            cwd: selected_cwd_uri,
            workspace_roots: Vec::new(),
            config: EnvironmentConfigState::FromThread,
        }],
    )
    .await;

    let resolved = resolved.snapshot().await;
    assert_eq!(
        resolved
            .primary()
            .expect("primary environment")
            .selection
            .environment_id,
        "local"
    );
    assert_eq!(
        resolved.primary().expect("primary environment").shell,
        Some(
            Shell::from_environment_shell_info(
                manager
                    .get_environment("local")
                    .expect("local environment")
                    .info()
                    .await
                    .expect("local environment info")
                    .shell
            )
            .expect("resolved shell")
        )
    );
}

#[tokio::test]
async fn unresolved_environment_selections_are_skipped() {
    let cwd = AbsolutePathBuf::current_dir().expect("cwd");
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let manager = Arc::new(EnvironmentManager::default_for_tests());
    let local = TurnEnvironmentSelection {
        selected_capability_roots: Default::default(),
        environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
        cwd: cwd_uri.clone(),
        workspace_roots: Vec::new(),
        config: EnvironmentConfigState::FromThread,
    };

    let resolved = resolve_turn_environments(
        manager,
        &[
            TurnEnvironmentSelection {
                selected_capability_roots: Default::default(),
                environment_id: "missing".to_string(),
                cwd: cwd_uri,
                workspace_roots: Vec::new(),
                config: EnvironmentConfigState::FromThread,
            },
            local.clone(),
        ],
    )
    .await;

    assert_eq!(resolved.snapshot().await.to_selections(), vec![local]);
}

#[tokio::test]
async fn blocking_snapshot_waits_for_starting_environment() {
    let buffer: &'static std::sync::Mutex<Vec<u8>> =
        Box::leak(Box::new(std::sync::Mutex::new(Vec::new())));
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_max_level(Level::TRACE)
        .with_span_events(FmtSpan::NEW)
        .with_writer(MockWriter::new(buffer))
        .finish();
    // Avoid tracing-core's single-dispatch path caching no interest when another
    // test first reaches these shared callsites without a subscriber.
    let _interest_cache_guard =
        tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
    let _subscriber_guard = tracing::subscriber::set_default(subscriber);

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind websocket listener");
    let manager = Arc::new(
        EnvironmentManager::create_for_tests(
            Some(format!(
                "ws://{}",
                listener.local_addr().expect("listener address")
            )),
            Some(test_runtime_paths()),
        )
        .await,
    );
    let selection = TurnEnvironmentSelection {
        selected_capability_roots: Default::default(),
        environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
        cwd: PathUri::from_abs_path(&AbsolutePathBuf::current_dir().expect("cwd")),
        workspace_roots: Vec::new(),
        config: EnvironmentConfigState::FromThread,
    };
    let environments = Arc::new(ThreadEnvironments::new(
        manager,
        crate::shell::default_user_shell(),
        ThreadEnvironmentDefaults::new(test_environment_config(), SandboxType::None),
        ShellSnapshot::disabled(),
        TurnEnvironmentSnapshot::default(),
        /*non_blocking_snapshots*/ false,
        /*executor_discovery*/ None,
    ));
    environments.update_selections(std::slice::from_ref(&selection));
    // Exercise first-use callsite registration from a thread without a subscriber.
    std::thread::spawn({
        let environments = Arc::clone(&environments);
        move || assert!(environments.snapshot().now_or_never().is_none())
    })
    .join()
    .expect("unsubscribed snapshot");
    let snapshot_task = tokio::spawn({
        let environments = Arc::clone(&environments);
        async move { environments.snapshot().await }
    });
    tokio::task::yield_now().await;
    assert!(!snapshot_task.is_finished());

    let server = tokio::spawn(serve_environment_info(listener));
    let snapshot = timeout(Duration::from_secs(5), snapshot_task)
        .await
        .expect("snapshot should finish after the environment starts")
        .expect("snapshot task");

    assert!(snapshot.starting().next().is_none());
    assert_eq!(snapshot.to_selections(), vec![selection]);
    server.await.expect("server task");

    let logs = String::from_utf8(buffer.lock().expect("buffer lock").clone())
        .expect("UTF-8 tracing output");
    assert!(
        logs.contains(
            "environments.snapshot{environment_count=1 non_blocking=false}:environments.wait_until_ready{environment_id=remote}"
        ),
        "blocking snapshot should contain its environment wait span: {logs}"
    );
    assert!(
        logs.contains(
            "environments.resolve{environment_id=remote remote=true configuration_pending=false}:exec_server.environment.wait_until_ready{remote=true}"
        ),
        "environment resolution should contain the executor connection wait span: {logs}"
    );
    assert!(
        logs.contains(
            "environments.resolve{environment_id=remote remote=true configuration_pending=false}:exec_server.environment.info{remote=true}"
        ),
        "environment resolution should contain the remote environment-info span: {logs}"
    );
}

#[test_case::test_case(SandboxType::None, SandboxType::None, WindowsSandboxLevel::Disabled; "legacy")]
#[test_case::test_case(SandboxType::WindowsRestrictedToken, SandboxType::WindowsMxc, WindowsSandboxLevel::Elevated; "local preference")]
#[test_case::test_case(SandboxType::WindowsMxc, SandboxType::WindowsMxc, WindowsSandboxLevel::Disabled; "explicit MXC")]
#[tokio::test]
async fn snapshot_refreshes_readiness_in_selection_order(
    configured_sandbox: SandboxType,
    local_sandbox: SandboxType,
    legacy_level: WindowsSandboxLevel,
) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind websocket listener");
    let manager = Arc::new(
        EnvironmentManager::create_for_tests_with_local(
            Some(format!(
                "ws://{}",
                listener.local_addr().expect("listener address")
            )),
            test_runtime_paths(),
        )
        .await,
    );
    let cwd = AbsolutePathBuf::current_dir().expect("cwd");
    let expected_config = EnvironmentConfig {
        environment_hooks: Default::default(),
        allow_login_shell: false,
        workspace_roots: Vec::new(),
        windows_sandbox_level: legacy_level,
        windows_sandbox_type: configured_sandbox,
        use_legacy_landlock: false,
        permission_profile: PermissionProfileSnapshot::active_with_profile_workspace_roots(
            PermissionProfile::read_only(),
            ActivePermissionProfile::read_only(),
            vec![cwd.join("profile-root").into()],
        ),
        shell_environment_policy: Default::default(),
        exec_policy: None,
        plugins: Default::default(),
        mcp_policy: None,
        network_policy: None,
        selected_capability_roots: Vec::new(),
    };
    let expected_local_config = EnvironmentConfig {
        windows_sandbox_type: local_sandbox,
        ..expected_config.clone()
    };
    let cwd = PathUri::from_abs_path(&cwd);
    let remote = TurnEnvironmentSelection {
        selected_capability_roots: Default::default(),
        environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
        cwd: cwd.clone(),
        workspace_roots: Vec::new(),
        config: EnvironmentConfigState::FromThread,
    };
    let local = TurnEnvironmentSelection {
        selected_capability_roots: Default::default(),
        environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
        cwd,
        workspace_roots: Vec::new(),
        config: EnvironmentConfigState::FromThread,
    };
    let turn_environments = ThreadEnvironments::new(
        Arc::clone(&manager),
        crate::shell::default_user_shell(),
        ThreadEnvironmentDefaults::new(test_environment_config(), local_sandbox),
        ShellSnapshot::disabled(),
        TurnEnvironmentSnapshot::default(),
        /*non_blocking_snapshots*/ true,
        /*executor_discovery*/ None,
    );
    turn_environments.update_selections(std::slice::from_ref(&local));
    let resolution = turn_environments.state.lock().unwrap().environments[0]
        .resolution
        .clone();
    resolution.await.expect("local environment should resolve");
    turn_environments.update_selections(&[remote.clone(), local.clone()]);
    let initial = turn_environments.snapshot().await;
    assert_eq!(
        initial.primary().expect("local environment").config(),
        &EnvironmentConfig {
            windows_sandbox_type: local_sandbox,
            ..test_environment_config()
        },
    );
    assert_eq!(
        initial.starting().next().unwrap().selection.config,
        EnvironmentConfigState::Ready(test_environment_config()),
    );
    turn_environments.set_active_thread_defaults(expected_config.clone());

    let starting = turn_environments.snapshot().await;
    let resolved_remote = TurnEnvironmentSelection {
        config: EnvironmentConfigState::Ready(expected_config.clone()),
        ..remote.clone()
    };
    assert_eq!(
        starting
            .turn_environments()
            .map(TurnEnvironment::selection)
            .collect::<Vec<_>>(),
        vec![local.clone()]
    );
    assert_eq!(
        starting
            .turn_environments()
            .map(|environment| environment.config().clone())
            .collect::<Vec<_>>(),
        vec![expected_local_config.clone()]
    );
    assert_eq!(
        starting
            .starting()
            .map(|environment| environment.selection.clone())
            .collect::<Vec<_>>(),
        vec![resolved_remote.clone()]
    );
    assert_eq!(starting.to_selections(), vec![local.clone()]);
    assert_eq!(
        starting.all_selections(),
        vec![remote.clone(), local.clone()]
    );
    assert!(starting.single_local_environment().is_none());

    let next_config = EnvironmentConfig {
        environment_hooks: Default::default(),
        permission_profile: PermissionProfileSnapshot::legacy(PermissionProfile::Disabled),
        ..test_environment_config()
    };
    turn_environments.set_active_thread_defaults(next_config.clone());
    let next_starting = turn_environments.snapshot().await;
    assert!(!next_starting.has_full_access(AskForApproval::Never, &PermissionProfile::Disabled));

    let server = tokio::spawn(serve_environment_info(listener));
    timeout(
        std::time::Duration::from_secs(5),
        starting
            .starting()
            .next()
            .expect("starting environment")
            .resolution
            .clone(),
    )
    .await
    .expect("environment resolution should finish")
    .expect("environment resolution should succeed");
    assert!(next_starting.has_full_access(AskForApproval::Never, &PermissionProfile::Disabled));
    assert!(!starting.has_full_access(AskForApproval::Never, &PermissionProfile::Disabled));
    let attached = starting.refresh_readiness();

    assert!(attached.starting().next().is_none());
    let child = ThreadEnvironments::new(
        manager,
        crate::shell::default_user_shell(),
        ThreadEnvironmentDefaults::new(next_config.clone(), local_sandbox),
        ShellSnapshot::disabled(),
        attached.clone(),
        /*non_blocking_snapshots*/ true,
        /*executor_discovery*/ None,
    );
    child.activate_configured(async_channel::unbounded().0);
    assert_eq!(
        child
            .snapshot()
            .await
            .turn_environments()
            .map(|environment| environment.config().clone())
            .collect::<Vec<_>>(),
        next_starting
            .refresh_readiness()
            .turn_environments()
            .map(|environment| environment.config().clone())
            .collect::<Vec<_>>(),
    );
    assert_eq!(
        attached
            .turn_environments()
            .map(TurnEnvironment::selection)
            .collect::<Vec<_>>(),
        vec![remote.clone(), local.clone()]
    );
    assert_eq!(
        attached
            .turn_environments()
            .map(|environment| environment.config().clone())
            .collect::<Vec<_>>(),
        vec![expected_config, expected_local_config]
    );
    assert_eq!(
        attached
            .turn_environments()
            .map(|environment| environment.executor_platform_os.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("windows"), Some(std::env::consts::OS)]
    );
    assert_eq!(attached.to_selections(), vec![remote, local]);
    let environment = attached.primary().expect("remote environment");
    let expected_user_home_dir = PathUri::parse("file:///home/remote").expect("remote home");
    let expected_temporary_directories =
        [PathUri::parse("file:///tmp/remote").expect("remote temporary directory")];
    assert_eq!(
        environment
            .sandbox_context(/*additional_permissions*/ None)
            .policy_context(),
        FileSystemSandboxPolicyContext {
            cwd: environment.cwd(),
            workspace_roots: &[],
            user_home_dir: Some(&expected_user_home_dir),
            temporary_directories: Some(expected_temporary_directories.as_slice()),
        }
    );
    assert_eq!(
        next_starting
            .refresh_readiness()
            .turn_environments()
            .map(|environment| environment.config().clone())
            .collect::<Vec<_>>(),
        vec![
            next_config.clone(),
            EnvironmentConfig {
                windows_sandbox_type: local_sandbox,
                ..next_config
            }
        ]
    );
    server.await.expect("server task");
}

#[tokio::test]
async fn failed_resolution_is_replaced_from_the_environment_manager() {
    let manager = Arc::new(
        EnvironmentManager::create_for_tests(
            Some("http://example.com".to_string()),
            Some(test_runtime_paths()),
        )
        .await,
    );
    let selection = TurnEnvironmentSelection {
        selected_capability_roots: Default::default(),
        environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
        cwd: PathUri::from_abs_path(&AbsolutePathBuf::current_dir().expect("cwd")),
        workspace_roots: Vec::new(),
        config: EnvironmentConfigState::FromThread,
    };
    let environments = ThreadEnvironments::new(
        Arc::clone(&manager),
        crate::shell::default_user_shell(),
        ThreadEnvironmentDefaults::new(test_environment_config(), SandboxType::None),
        ShellSnapshot::disabled(),
        TurnEnvironmentSnapshot::default(),
        /*non_blocking_snapshots*/ true,
        /*executor_discovery*/ None,
    );
    environments.update_selections(std::slice::from_ref(&selection));
    let starting = environments.snapshot().await;
    let failed_resolution = environments.state.lock().unwrap().environments[0]
        .resolution
        .clone();
    let error = failed_resolution
        .clone()
        .await
        .err()
        .expect("environment should fail to start");
    // Failed selections must not turn an empty executable snapshot into Full Access.
    for snapshot in [starting.refresh_readiness(), environments.snapshot().await] {
        assert!(!snapshot.has_full_access(AskForApproval::Never, &PermissionProfile::Disabled));
        assert_eq!(snapshot.all_selections(), vec![selection.clone()]);
        assert!(snapshot.ready_environment_handles().is_empty());
    }
    let selected_root = SelectedCapabilityRoot {
        id: "failed-root".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: selection.environment_id.clone(),
            path: selection.cwd.clone(),
        },
    };
    assert_eq!(
        environments.inspect_selected_capability_roots(&[selected_root]),
        SelectedCapabilityRootsStatus {
            ready_roots: Vec::new(),
            warnings: vec![format!(
                "selected capability environment `{}` is unavailable: {error}",
                selection.environment_id
            )],
        }
    );

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind replacement listener");
    manager
        .upsert_environment(
            REMOTE_ENVIRONMENT_ID.to_string(),
            format!("ws://{}", listener.local_addr().expect("listener address")),
            /*connect_timeout*/ None,
        )
        .expect("replacement environment");
    let next_config = EnvironmentConfig {
        environment_hooks: Default::default(),
        allow_login_shell: false,
        ..test_environment_config()
    };
    environments.set_active_thread_defaults(next_config.clone());
    assert!(
        failed_resolution.ptr_eq(&environments.state.lock().unwrap().environments[0].resolution),
        "updating environment config must not retry a failed environment"
    );
    environments.update_selections(std::slice::from_ref(&selection));

    let replacement = environments.snapshot().await;
    let replacement = replacement
        .starting()
        .next()
        .expect("expected the replacement environment to be starting");
    assert_eq!(
        replacement.selection,
        TurnEnvironmentSelection {
            config: EnvironmentConfigState::Ready(next_config),
            ..selection.clone()
        }
    );
    assert_eq!(environments.selections(), vec![selection]);
    assert!(!failed_resolution.ptr_eq(&replacement.resolution));
}

#[tokio::test]
async fn replacement_environment_events_follow_selected_environment() {
    let cwd = AbsolutePathBuf::current_dir().expect("cwd");
    let first_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind first listener");
    let manager = Arc::new(
        EnvironmentManager::create_for_tests(
            Some(format!(
                "ws://{}",
                first_listener.local_addr().expect("first listener address")
            )),
            Some(test_runtime_paths()),
        )
        .await,
    );
    let selection = TurnEnvironmentSelection {
        selected_capability_roots: Default::default(),
        environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
        cwd: PathUri::from_abs_path(&cwd),
        workspace_roots: Vec::new(),
        config: EnvironmentConfigState::FromThread,
    };
    let (tx_event, rx_event) = async_channel::unbounded();
    let environments = Arc::new(ThreadEnvironments::new(
        Arc::clone(&manager),
        crate::shell::default_user_shell(),
        ThreadEnvironmentDefaults::new(test_environment_config(), SandboxType::None),
        ShellSnapshot::disabled(),
        TurnEnvironmentSnapshot::default(),
        /*non_blocking_snapshots*/ true,
        /*executor_discovery*/ None,
    ));
    environments.activate_configured(tx_event);
    environments.update_selections(std::slice::from_ref(&selection));
    let initial_snapshot = environments.snapshot().await;
    let second_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind second listener");
    manager
        .upsert_environment(
            REMOTE_ENVIRONMENT_ID.to_string(),
            format!(
                "ws://{}",
                second_listener
                    .local_addr()
                    .expect("second listener address")
            ),
            /*connect_timeout*/ None,
        )
        .expect("replace environment");

    environments.update_selections(std::slice::from_ref(&selection));
    let reused_snapshot = environments.snapshot().await;
    environments.update_selections(&[TurnEnvironmentSelection {
        cwd: PathUri::from_abs_path(&cwd.join("changed")),
        ..selection
    }]);
    let changed_snapshot = environments.snapshot().await;

    let initial = initial_snapshot
        .starting()
        .next()
        .expect("initial environment");
    let reused = reused_snapshot
        .starting()
        .next()
        .expect("reused environment");
    let changed = changed_snapshot
        .starting()
        .next()
        .expect("changed environment");
    assert!(initial.resolution.ptr_eq(&reused.resolution));
    assert!(!reused.resolution.ptr_eq(&changed.resolution));
    assert!(Arc::ptr_eq(&initial.attachment, &reused.attachment));
    assert!(initial.attachment.detached.is_cancelled());
    assert!(!changed.attachment.detached.is_cancelled());

    serve_environment_info(first_listener).await;
    assert!(
        timeout(Duration::from_millis(250), rx_event.recv())
            .await
            .is_err(),
        "old environment event should not be forwarded"
    );

    serve_environment_info(second_listener).await;
    let event = timeout(Duration::from_secs(5), rx_event.recv())
        .await
        .expect("replacement environment event")
        .expect("event channel");
    let event = match event.msg {
        EventMsg::EnvironmentConnected(event) => event,
        other => panic!("expected connected event, got {other:?}"),
    };
    assert_eq!(
        event,
        EnvironmentConnectionEvent {
            environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
        }
    );
}

#[tokio::test]
async fn inherited_environment_reuses_parent_handle_and_uses_child_config() {
    let cwd = AbsolutePathBuf::current_dir().expect("cwd");
    let selection = TurnEnvironmentSelection {
        selected_capability_roots: Default::default(),
        environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
        cwd: PathUri::from_abs_path(&cwd),
        workspace_roots: Vec::new(),
        config: EnvironmentConfigState::FromThread,
    };
    let inherited_environment = Arc::new(
        Environment::create_for_tests(Some("ws://127.0.0.1:8765".to_string()))
            .expect("inherited environment"),
    );
    let mut inherited = TurnEnvironment::new(
        TurnEnvironmentSelection {
            config: EnvironmentConfigState::Ready(test_environment_config()),
            ..selection.clone()
        },
        EnvironmentConfigOrigin::Thread,
        Arc::clone(&inherited_environment),
        /*shell*/ None,
    );
    inherited.user_home_dir =
        Some(PathUri::parse("file:///home/inherited").expect("home directory"));
    inherited.temporary_directories = Some(vec![
        PathUri::parse("file:///tmp/inherited").expect("temporary directory"),
    ]);
    let manager = Arc::new(environment_manager_without_environments());
    manager
        .upsert_environment(
            REMOTE_ENVIRONMENT_ID.to_string(),
            "ws://127.0.0.1:9876".to_string(),
            /*connect_timeout*/ None,
        )
        .expect("replacement environment");
    let child_config = EnvironmentConfig {
        environment_hooks: Default::default(),
        allow_login_shell: false,
        workspace_roots: Vec::new(),
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
        windows_sandbox_type: codex_protocol::sandbox::SandboxType::None,
        use_legacy_landlock: false,
        permission_profile: PermissionProfileSnapshot::active_with_profile_workspace_roots(
            PermissionProfile::read_only(),
            ActivePermissionProfile::read_only(),
            vec![cwd.join("child-profile-root").into()],
        ),
        shell_environment_policy: Default::default(),
        exec_policy: None,
        plugins: Default::default(),
        mcp_policy: None,
        network_policy: None,
        selected_capability_roots: Vec::new(),
    };
    let environments = ThreadEnvironments::new(
        manager,
        crate::shell::default_user_shell(),
        ThreadEnvironmentDefaults::new(child_config.clone(), child_config.windows_sandbox_type),
        ShellSnapshot::disabled(),
        TurnEnvironmentSnapshot {
            environments: vec![TurnEnvironmentState::Ready(inherited)],
        },
        /*non_blocking_snapshots*/ false,
        /*executor_discovery*/ None,
    );

    environments.update_selections(std::slice::from_ref(&selection));
    let snapshot = environments.snapshot().await;

    let inherited = snapshot.primary().expect("inherited environment");
    assert!(Arc::ptr_eq(inherited.environment(), &inherited_environment));
    assert_eq!(inherited.config(), &child_config);
    assert_eq!(
        inherited
            .sandbox_context(/*additional_permissions*/ None)
            .user_home_dir,
        Some(PathUri::parse("file:///home/inherited").expect("home directory")),
    );
    assert_eq!(
        inherited
            .sandbox_context(/*additional_permissions*/ None)
            .temporary_directories
            .as_deref(),
        Some([PathUri::parse("file:///tmp/inherited").expect("temporary directory")].as_slice())
    );
}

#[tokio::test]
async fn owner_environment_config_is_inherited_and_reset_for_new_cwd() {
    let cwd = AbsolutePathBuf::current_dir().expect("cwd");
    let selection = TurnEnvironmentSelection {
        selected_capability_roots: Default::default(),
        environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
        cwd: PathUri::from_abs_path(&cwd),
        workspace_roots: Vec::new(),
        config: EnvironmentConfigState::FromThread,
    };
    let root = |id: &str| SelectedCapabilityRoot {
        id: id.to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: selection.environment_id.clone(),
            path: selection.cwd.clone(),
        },
    };
    let manager = Arc::new(EnvironmentManager::default_for_tests());
    let parent = ThreadEnvironments::new(
        Arc::clone(&manager),
        crate::shell::default_user_shell(),
        ThreadEnvironmentDefaults::new(test_environment_config(), SandboxType::WindowsMxc),
        ShellSnapshot::disabled(),
        TurnEnvironmentSnapshot::default(),
        /*non_blocking_snapshots*/ false,
        /*executor_discovery*/ None,
    );
    parent.update_selections(std::slice::from_ref(&selection));
    parent.snapshot().await;
    let parent_owner_config = EnvironmentConfig {
        environment_hooks: Default::default(),
        allow_login_shell: false,
        workspace_roots: selection.workspace_roots.clone(),
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
        windows_sandbox_type: codex_protocol::sandbox::SandboxType::None,
        use_legacy_landlock: false,
        permission_profile: PermissionProfileSnapshot::legacy(PermissionProfile::read_only()),
        shell_environment_policy: Default::default(),
        exec_policy: None,
        plugins: Default::default(),
        mcp_policy: None,
        network_policy: None,
        selected_capability_roots: vec![root("parent-root")],
    };
    let mut owner_selection = selection.clone();
    owner_selection.config = EnvironmentConfigState::Ready(parent_owner_config.clone());
    parent.update_selections(std::slice::from_ref(&owner_selection));

    let child_thread_config = EnvironmentConfig {
        windows_sandbox_type: SandboxType::WindowsMxc,
        permission_profile: PermissionProfileSnapshot::legacy(PermissionProfile::workspace_write()),
        ..test_environment_config()
    };
    parent.set_active_thread_defaults(child_thread_config.clone());
    assert_eq!(
        parent
            .snapshot()
            .await
            .primary()
            .expect("owner-configured environment")
            .config(),
        &parent_owner_config
    );
    let child = ThreadEnvironments::new(
        manager,
        crate::shell::default_user_shell(),
        ThreadEnvironmentDefaults::new(child_thread_config.clone(), SandboxType::WindowsMxc),
        ShellSnapshot::disabled(),
        parent.snapshot().await,
        /*non_blocking_snapshots*/ false,
        /*executor_discovery*/ None,
    );
    child.activate_configured(async_channel::unbounded().0);
    let child_snapshot = child.snapshot().await;
    let child_environment = child_snapshot.primary().expect("child environment");

    let cleared_parent_owner_config = EnvironmentConfig {
        environment_hooks: Default::default(),
        selected_capability_roots: Vec::new(),
        ..parent_owner_config.clone()
    };
    owner_selection.config = EnvironmentConfigState::Ready(cleared_parent_owner_config.clone());
    parent.update_selections(std::slice::from_ref(&owner_selection));
    let cleared_snapshot = parent.snapshot().await;
    assert_eq!(
        cleared_snapshot
            .primary()
            .expect("environment with cleared roots")
            .config(),
        &cleared_parent_owner_config
    );
    assert_eq!(child_environment.config(), &parent_owner_config);

    let changed_selection = TurnEnvironmentSelection {
        cwd: PathUri::from_abs_path(&cwd.join("changed")),
        ..selection
    };
    parent.update_selections(std::slice::from_ref(&changed_selection));
    let changed_snapshot = parent.snapshot().await;
    let changed_environment = changed_snapshot.primary().expect("changed environment");
    assert_eq!(changed_environment.config(), &child_thread_config);
}

#[tokio::test]
async fn single_local_environment_cwd_requires_exactly_one_local_environment() {
    let cwd = AbsolutePathBuf::current_dir().expect("cwd");
    let cwd_uri = PathUri::from_abs_path(&cwd);
    let remote_cwd_uri = PathUri::from_abs_path(&cwd.join("remote-cwd"));
    let local_manager = Arc::new(EnvironmentManager::default_for_tests());
    let local = resolve_turn_environments(
        Arc::clone(&local_manager),
        &[TurnEnvironmentSelection {
            selected_capability_roots: Default::default(),
            environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
            cwd: cwd_uri.clone(),
            workspace_roots: Vec::new(),
            config: EnvironmentConfigState::FromThread,
        }],
    )
    .await;
    let local = local.snapshot().await;
    let remote_environment = Arc::new(
        Environment::create_for_tests(Some("ws://127.0.0.1:8765".to_string()))
            .expect("remote environment"),
    );
    let remote = TurnEnvironmentSnapshot {
        environments: vec![TurnEnvironmentState::Ready(TurnEnvironment::new(
            TurnEnvironmentSelection {
                selected_capability_roots: Default::default(),
                environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                cwd: remote_cwd_uri.clone(),
                workspace_roots: Vec::new(),
                config: EnvironmentConfigState::Ready(test_environment_config()),
            },
            EnvironmentConfigOrigin::Thread,
            remote_environment.clone(),
            /*shell*/ None,
        ))],
    };
    let multiple = TurnEnvironmentSnapshot {
        environments: vec![
            TurnEnvironmentState::Ready(TurnEnvironment::new(
                TurnEnvironmentSelection {
                    selected_capability_roots: Default::default(),
                    environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                    cwd: remote_cwd_uri,
                    workspace_roots: Vec::new(),
                    config: EnvironmentConfigState::Ready(test_environment_config()),
                },
                EnvironmentConfigOrigin::Thread,
                remote_environment,
                /*shell*/ None,
            )),
            TurnEnvironmentState::Ready(local.primary().expect("local environment").clone()),
        ],
    };

    assert_eq!(local.single_local_environment_cwd(), Some(cwd.clone()));
    assert_eq!(remote.single_local_environment_cwd(), None);
    assert_eq!(multiple.single_local_environment_cwd(), None);
    assert_eq!(multiple.local_environment_cwd(), Some(cwd));
}

#[tokio::test]
async fn local_environment_cwd_uses_starting_local_selection() {
    let cwd = AbsolutePathBuf::current_dir()
        .expect("cwd")
        .join("starting-local");
    let (attachment, _guard, _) = EnvironmentAttachment::new(
        LOCAL_ENVIRONMENT_ID,
        Arc::new(
            Environment::create_for_tests(/*exec_server_url*/ None).expect("local environment"),
        ),
    );
    let snapshot = TurnEnvironmentSnapshot {
        environments: vec![TurnEnvironmentState::Starting(StartingTurnEnvironment {
            selection: TurnEnvironmentSelection {
                selected_capability_roots: Default::default(),
                environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
                cwd: PathUri::from_abs_path(&cwd),
                workspace_roots: Vec::new(),
                config: EnvironmentConfigState::Ready(test_environment_config()),
            },
            config_origin: EnvironmentConfigOrigin::Thread,
            resolution: futures::future::pending().boxed().shared(),
            attachment,
            owner_config_result: None,
        })],
    };

    assert_eq!(snapshot.local_environment_cwd(), Some(cwd));
}
