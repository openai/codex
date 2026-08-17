use anyhow::Context;
use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_api::AuthProvider;
use codex_config::types::ApprovalsReviewer;
use codex_core::CodexThreadSettingsOverrides;
use codex_core::EnvironmentConfig;
use codex_core::EnvironmentNetworkPolicy;
use codex_core::StartThreadOptions;
use codex_core::TurnInputRequest;
use codex_core::WaitForEnvironmentToolConfig;
use codex_core::compact::SUMMARIZATION_PROMPT;
use codex_core::config::Config;
use codex_core::config::Constrained;
use codex_exec_server::CopyOptions;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::EnvironmentReadyInfo;
use codex_exec_server::ExecServerError;
use codex_exec_server::ExecServerRuntimePaths;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_exec_server::NoiseChannelPublicKey;
use codex_exec_server::NoiseRendezvousConnectBundle;
use codex_exec_server::NoiseRendezvousConnectProvider;
use codex_exec_server::REMOTE_ENVIRONMENT_ID;
use codex_exec_server::RemoteEnvironmentConfig;
use codex_exec_server::RemoveOptions;
use codex_extension_api::ContextContributor;
use codex_extension_api::ExtensionDataInit;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::RenderedWorldStateFragment;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::WorldStateContributionInput;
use codex_extension_api::WorldStateSectionContribution;
use codex_features::Feature;
use codex_history::RolloutItem;
use codex_history::RolloutLine;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_network_proxy::NetworkProxyConfig;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::PermissionProfileSnapshot;
use codex_protocol::models::SandboxPermissions;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::ApplyPatchApprovalRequestEvent;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ENVIRONMENTS_INSTRUCTIONS_OPEN_TAG;
use codex_protocol::protocol::EnvironmentConfigState;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_protocol::protocol::TurnEnvironmentSelections;
use codex_protocol::request_permissions::PermissionGrantScope;
use codex_protocol::request_permissions::RequestPermissionProfile;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use codex_protocol::request_user_input::RequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_protocol::user_input::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use core_test_support::PathBufExt;
use core_test_support::PathExt;
use core_test_support::TestTargetOs;
use core_test_support::responses::ResponseMock;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_apply_patch_custom_tool_call;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_completed_with_tokens;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_no_remote_env;
use core_test_support::skip_if_target_windows;
use core_test_support::submit_thread_settings;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::TestCodexBuilder;
use core_test_support::test_codex::local;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::test_env;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::test_docker_container_name;
use core_test_support::test_target_os;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use futures::SinkExt;
use futures::StreamExt;
use futures::future::BoxFuture;
use http::HeaderMap;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tempfile::TempDir;
use test_case::test_case;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

const WAIT_FOR_ENVIRONMENT_TEST_TOOL_DESCRIPTION: &str = "Test wait tool description";
const WAIT_FOR_ENVIRONMENT_TEST_ENVIRONMENT_ID_DESCRIPTION: &str =
    "Test environment ID description";

struct WaitForEnvironmentTestExtension;

impl ThreadLifecycleContributor<Config> for WaitForEnvironmentTestExtension {
    fn on_thread_start<'a>(
        &'a self,
        input: ThreadStartInput<'a, Config>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            input.thread_store.insert(WaitForEnvironmentToolConfig {
                tool_description: WAIT_FOR_ENVIRONMENT_TEST_TOOL_DESCRIPTION.to_string(),
                environment_id_description: WAIT_FOR_ENVIRONMENT_TEST_ENVIRONMENT_ID_DESCRIPTION
                    .to_string(),
            });
        })
    }
}

struct ReadyCapabilityRootsTestExtension;

impl ContextContributor for ReadyCapabilityRootsTestExtension {
    fn contribute_world_state<'a>(
        &'a self,
        input: WorldStateContributionInput<'a>,
    ) -> ExtensionFuture<'a, Vec<WorldStateSectionContribution>> {
        let root_ids = input
            .ready_selected_capability_roots
            .iter()
            .map(|root| root.id.clone())
            .collect::<Vec<_>>();
        Box::pin(async move {
            let body = root_ids.join(",");
            vec![WorldStateSectionContribution::new(
                "ready_capability_roots_test",
                json!(root_ids),
                move |_| {
                    Some(RenderedWorldStateFragment::new(
                        "user",
                        ("<ready_capability_roots>", "</ready_capability_roots>"),
                        body.clone(),
                    ))
                },
            )]
        })
    }
}

fn test_codex_with_wait_for_environment() -> TestCodexBuilder {
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.thread_lifecycle_contributor(Arc::new(WaitForEnvironmentTestExtension));
    test_codex().with_extensions(Arc::new(extensions.build()))
}

async fn unified_exec_test(server: &wiremock::MockServer) -> Result<TestCodex> {
    let mut builder = test_codex().with_config(|config| {
        config.use_experimental_unified_exec_tool = true;
        let result = config.features.enable(Feature::UnifiedExec);
        assert!(
            result.is_ok(),
            "unified exec should enable for test: {result:?}",
        );
    });
    builder.build_with_remote_and_local_env(server).await
}

async fn submit_turn_with_approval_and_environments(
    test: &TestCodex,
    prompt: &str,
    environments: Vec<TurnEnvironmentSelection>,
    approval_policy: AskForApproval,
) -> Result<()> {
    let turn_environment_selections = codex_protocol::protocol::TurnEnvironmentSelections::new(
        test.config.cwd.clone(),
        environments,
    );
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: prompt.into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(turn_environment_selections),
                approval_policy: Some(approval_policy),
                approvals_reviewer: Some(ApprovalsReviewer::User),
                sandbox_policy: Some(SandboxPolicy::new_read_only_policy()),
                collaboration_mode: Some(CollaborationMode {
                    mode: ModeKind::Default,
                    settings: Settings {
                        model: test.session_configured.model.clone(),
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            }),
        )
        .await?;

    Ok(())
}

async fn expect_patch_approval(
    test: &TestCodex,
    expected_call_id: &str,
) -> ApplyPatchApprovalRequestEvent {
    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ApplyPatchApprovalRequest(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;

    match event {
        EventMsg::ApplyPatchApprovalRequest(approval) => {
            assert_eq!(approval.call_id, expected_call_id);
            approval
        }
        EventMsg::TurnComplete(_) => panic!("expected patch approval request before completion"),
        other => panic!("unexpected event: {other:?}"),
    }
}

async fn wait_for_completion_without_patch_approval(test: &TestCodex) {
    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ApplyPatchApprovalRequest(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;

    match event {
        EventMsg::TurnComplete(_) => {}
        EventMsg::ApplyPatchApprovalRequest(event) => {
            panic!("unexpected patch approval request: {:?}", event.call_id)
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_test_env_can_connect_and_use_filesystem() -> Result<()> {
    skip_if_no_remote_env!(Ok(()));

    let test_env = test_env().await?;
    let file_system = test_env.environment().get_filesystem();

    let file_path_uri = test_env.selection().cwd.join("remote-test-env-ok")?;
    let payload = b"remote-test-env-ok".to_vec();

    file_system
        .write_file(&file_path_uri, payload.clone(), /*sandbox*/ None)
        .await?;
    let actual = file_system
        .read_file(&file_path_uri, /*sandbox*/ None)
        .await?;
    assert_eq!(actual, payload);

    file_system
        .remove(
            &file_path_uri,
            RemoveOptions {
                recursive: false,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_test_env_exposes_target_shell_to_model() -> Result<()> {
    skip_if_no_remote_env!(Ok(()));

    let server = start_mock_server().await;
    let response_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .disable(Feature::UnifiedExec)
            .expect("test config should allow feature update");
    });
    let test = builder.build_with_auto_env(&server).await?;

    test.submit_turn("report remote environment").await?;

    let request = response_mock.single_request();
    let tools = tool_names(&request.body_json());
    assert!(!tools.contains(&"shell_command".to_string()));
    let environment_context = request
        .message_input_texts("user")
        .into_iter()
        .find(|text| text.starts_with("<environment_context>"))
        .context("environment context should be model visible")?;
    // TODO(anp): Assert Wine-exec exposes a `C:\\...` cwd after model-visible paths preserve
    // target-native spelling instead of the Linux orchestrator's `/C:/...` representation.
    let expected_shell = match test_target_os() {
        TestTargetOs::Linux => "<shell>bash</shell>",
        TestTargetOs::Windows => "<shell>powershell</shell>",
        TestTargetOs::MacOs => unreachable!("remote test targets do not run macOS"),
    };
    assert_eq!(
        environment_context
            .lines()
            .find(|line| line.trim_start().starts_with("<shell>"))
            .map(str::trim),
        Some(expected_shell),
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_remote_shell_runs_in_remote_cwd() -> Result<()> {
    const CALL_ID: &str = "remote-explicit-shell";

    skip_if_no_remote_env!(Ok(()));

    let (shell, command) = match test_target_os() {
        TestTargetOs::Linux => (
            "bash",
            r#"case "$PWD" in /tmp/codex-core-test-cwd-*) ;; *) echo "unexpected cwd: $PWD" >&2; exit 1 ;; esac"#,
        ),
        TestTargetOs::Windows => (
            "powershell",
            r#"$cwd = (Get-Location).Path; if ($cwd -notlike 'C:\codex-core-test-cwd-*') { Write-Error "unexpected cwd: $cwd"; exit 1 }"#,
        ),
        TestTargetOs::MacOs => unreachable!("remote test targets do not run macOS"),
    };

    let server = start_mock_server().await;
    let arguments = serde_json::to_string(&json!({
        "cmd": command,
        "shell": shell,
        "login": false,
        "yield_time_ms": 10_000,
    }))?;
    let mut builder = test_codex().with_config(|config| {
        config.use_experimental_unified_exec_tool = true;
        config
            .features
            .enable(Feature::UnifiedExec)
            .expect("test config should allow feature update");
    });
    let test = builder.build_with_auto_env(&server).await?;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(CALL_ID, "exec_command", &arguments),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    test.submit_turn_with_environments(
        "run the remote shell in the remote cwd",
        Some(vec![TurnEnvironmentSelection {
            environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
            cwd: PathUri::from_abs_path(&test.config.cwd),
            workspace_roots: vec![PathUri::from_abs_path(&test.config.cwd)],
            config: EnvironmentConfigState::FromThread,
        }]),
    )
    .await?;
    let request = response_mock
        .last_request()
        .context("model should receive the command output")?;
    let (output, success) = request
        .function_call_output_content_and_success(CALL_ID)
        .context("remote shell tool result should be present")?;
    assert_ne!(success, Some(false));
    assert!(
        output.is_some_and(|output| output.contains("Process exited with code 0")),
        "remote shell command should exit successfully",
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn environment_permissions_follow_configuration_ownership() -> Result<()> {
    const THREAD_CONFIG_CALL_ID: &str = "thread-config-permissions";
    const OWNER_CONFIG_CALL_ID: &str = "owner-config-permissions";
    const FILE_NAME: &str = "attachment-read-only-marker.txt";

    skip_if_target_windows!(
        Ok(()),
        "Windows sandbox enforcement is covered by the platform-specific suite"
    );

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.use_experimental_unified_exec_tool = true;
        config
            .features
            .enable(Feature::UnifiedExec)
            .expect("test config should allow feature update");
        config
            .permissions
            .set_permission_profile(PermissionProfile::workspace_write())
            .expect("thread should allow workspace writes");
    });
    let test = builder.build_with_auto_env(&server).await?;
    let selection = test.executor_environment().selection().clone();
    let marker = selection.cwd.join(FILE_NAME)?;

    let (shell, command) = match test_target_os() {
        TestTargetOs::Linux => (
            "bash",
            format!(
                "if printf blocked > {FILE_NAME}; then echo WRITE_SUCCEEDED; else echo WRITE_DENIED; fi"
            ),
        ),
        TestTargetOs::MacOs => (
            "zsh",
            format!(
                "if printf blocked > {FILE_NAME}; then echo WRITE_SUCCEEDED; else echo WRITE_DENIED; fi"
            ),
        ),
        TestTargetOs::Windows => (
            "powershell",
            format!(
                "try {{ Set-Content -Path '{FILE_NAME}' -Value blocked -ErrorAction Stop; Write-Output WRITE_SUCCEEDED }} catch {{ Write-Output WRITE_DENIED }}"
            ),
        ),
    };
    let arguments = serde_json::to_string(&json!({
        "cmd": command,
        "shell": shell,
        "login": false,
        "yield_time_ms": 10_000,
        "sandbox_permissions": SandboxPermissions::UseDefault,
    }))?;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(THREAD_CONFIG_CALL_ID, "exec_command", &arguments),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
            sse(vec![
                ev_response_created("resp-3"),
                ev_function_call(OWNER_CONFIG_CALL_ID, "exec_command", &arguments),
                ev_completed("resp-3"),
            ]),
            sse(vec![
                ev_response_created("resp-4"),
                ev_assistant_message("msg-2", "done"),
                ev_completed("resp-4"),
            ]),
        ],
    )
    .await;

    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            permission_profile: Some(PermissionProfile::read_only()),
            ..Default::default()
        },
    )
    .await?;
    test.submit_text_turn("try to write a file with thread-owned permissions")
        .await?;

    let output = response_mock
        .last_request()
        .context("model should receive the command output")?
        .function_call_output_text(THREAD_CONFIG_CALL_ID)
        .context("shell tool result should be present")?;
    assert!(
        output.contains("WRITE_DENIED"),
        "unexpected output: {output}"
    );
    assert!(!output.contains("WRITE_SUCCEEDED"));

    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            environments: Some(TurnEnvironmentSelections::new(
                test.config.cwd.clone(),
                vec![TurnEnvironmentSelection {
                    config: EnvironmentConfigState::Ready(EnvironmentConfig {
                        allow_login_shell: test.config.permissions.allow_login_shell,
                        permission_profile: PermissionProfileSnapshot::legacy(
                            PermissionProfile::read_only(),
                        ),
                        shell_environment_policy: Default::default(),
                        exec_policy: None,
                        network_policy: None,
                        selected_capability_roots: Vec::new(),
                    }),
                    ..selection
                }],
            )),
            ..Default::default()
        },
    )
    .await?;
    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            permission_profile: Some(PermissionProfile::workspace_write()),
            ..Default::default()
        },
    )
    .await?;
    test.submit_text_turn("try to write a file with owner-provided permissions")
        .await?;

    let output = response_mock
        .last_request()
        .context("model should receive the command output")?
        .function_call_output_text(OWNER_CONFIG_CALL_ID)
        .context("shell tool result should be present")?;
    assert!(
        output.contains("WRITE_DENIED"),
        "unexpected output: {output}"
    );
    assert!(!output.contains("WRITE_SUCCEEDED"));
    assert!(
        test.fs()
            .read_file_text(&marker, /*sandbox*/ None)
            .await
            .is_err(),
        "read-only attachment unexpectedly wrote {FILE_NAME}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn step_world_state_gates_deferred_prompt_independently_of_host_config() -> Result<()> {
    for deferred_executor_enabled in [false, true] {
        for host_config_present in [false, true] {
            let server = start_mock_server().await;
            let response_mock = mount_sse_once(
                &server,
                sse(vec![
                    ev_response_created("resp-1"),
                    ev_assistant_message("msg-1", "done"),
                    ev_completed("resp-1"),
                ]),
            )
            .await;
            let builder = if host_config_present {
                test_codex_with_wait_for_environment()
            } else {
                test_codex()
            };
            let mut builder = builder.with_config(move |config| {
                if deferred_executor_enabled {
                    assert!(config.features.enable(Feature::DeferredExecutor).is_ok());
                }
            });
            let test = builder.build(&server).await?;

            test.submit_turn("report the environment").await?;

            let request = response_mock.single_request();
            let user_context = request.message_input_texts("user");
            assert_eq!(
                user_context
                    .iter()
                    .filter(|text| text.contains("<environment_context>"))
                    .count(),
                1,
                "deferred executor enabled: {deferred_executor_enabled}; host config present: {host_config_present}",
            );
            assert_eq!(
                environment_instructions_occurrences(&request),
                usize::from(deferred_executor_enabled),
            );
            assert_eq!(
                tool_names(&request.body_json()).contains(&"wait_for_environment".to_string()),
                deferred_executor_enabled,
            );
        }
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn settings_update_does_not_retarget_active_turn_environment() -> Result<()> {
    let server = start_mock_server().await;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    "pause-turn",
                    "request_user_input",
                    &json!({
                        "questions": [{
                            "id": "continue",
                            "header": "Continue",
                            "question": "Continue after settings update?",
                            "options": [{
                                "label": "Yes (Recommended)",
                                "description": "Continue the test."
                            }, {
                                "label": "No",
                                "description": "Stop the test."
                            }]
                        }]
                    })
                    .to_string(),
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "first turn done"),
                ev_completed("resp-2"),
            ]),
            sse(vec![
                ev_response_created("resp-3"),
                ev_assistant_message("msg-3", "second turn done"),
                ev_completed("resp-3"),
            ]),
        ],
    )
    .await;
    let mut builder = test_codex().with_config(|config| {
        assert!(config.features.enable(Feature::DeferredExecutor).is_ok());
        assert!(
            config
                .features
                .enable(Feature::DefaultModeRequestUserInput)
                .is_ok()
        );
    });
    let test = builder.build(&server).await?;
    let initial_cwd = test.config.cwd.clone();
    let initial_environments = test.codex.environment_selections().await;
    let next_workspace = TempDir::new()?;
    let next_cwd = next_workspace.path().abs();
    let next_environments =
        TurnEnvironmentSelections::new(next_cwd.clone(), vec![local(next_cwd.clone())]);

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "pause before continuing".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let request = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::RequestUserInput(request) => Some(request.clone()),
        _ => None,
    })
    .await;

    let preview = test
        .codex
        .preview_thread_settings_overrides(CodexThreadSettingsOverrides {
            environments: Some(next_environments.clone()),
            ..Default::default()
        })
        .await?;
    assert_eq!(
        preview.environment_selections(),
        &next_environments.environments
    );
    assert_eq!(preview.cwd(), &next_cwd);
    assert_eq!(preview.workspace_roots, vec![next_cwd.clone()]);
    assert_eq!(
        test.codex.environment_selections().await,
        initial_environments
    );

    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            environments: Some(next_environments.clone()),
            ..Default::default()
        },
    )
    .await?;
    assert_eq!(
        test.codex.environment_selections().await,
        next_environments.environments
    );
    let snapshot = test.codex.config_snapshot().await;
    assert_eq!(
        snapshot.environment_selections(),
        next_environments.environments
    );
    assert_eq!(snapshot.cwd(), &next_cwd);
    assert_eq!(snapshot.workspace_roots, vec![next_cwd.clone()]);
    test.codex
        .submit(Op::UserInputAnswer {
            id: request.turn_id,
            response: RequestUserInputResponse {
                answers: HashMap::from([(
                    "continue".to_string(),
                    RequestUserInputAnswer {
                        answers: vec!["Yes (Recommended)".to_string()],
                    },
                )]),
            },
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.submit_turn("start the next turn").await?;

    let request_texts = response_mock
        .requests()
        .iter()
        .map(|request| request.message_input_texts("user").join("\n"))
        .collect::<Vec<_>>();
    let initial_cwd = format!("<cwd>{}</cwd>", initial_cwd.as_path().display());
    let next_cwd = format!("<cwd>{}</cwd>", next_cwd.as_path().display());
    assert_eq!(
        request_texts
            .iter()
            .map(|text| text.contains(&next_cwd))
            .collect::<Vec<_>>(),
        vec![false, false, true]
    );
    assert!(request_texts[0].contains(&initial_cwd));
    assert!(request_texts[1].contains(&initial_cwd));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deferred_executor_promotes_primary_environment_when_startup_completes() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let server = start_mock_server().await;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("warmup"),
                ev_assistant_message("warmup-message", "ready"),
                ev_completed("warmup"),
            ]),
            sse(vec![
                ev_response_created("before-promotion"),
                ev_function_call(
                    "pause-for-environment",
                    "request_user_input",
                    &json!({
                        "questions": [{
                            "id": "continue",
                            "header": "Continue",
                            "question": "Continue after the environment starts?",
                            "options": [{
                                "label": "Yes (Recommended)",
                                "description": "Continue the test."
                            }, {
                                "label": "No",
                                "description": "Stop the test."
                            }]
                        }]
                    })
                    .to_string(),
                ),
                ev_completed("before-promotion"),
            ]),
            sse(vec![
                ev_response_created("after-promotion"),
                ev_assistant_message("after-promotion-message", "done"),
                ev_completed("after-promotion"),
            ]),
        ],
    )
    .await;
    let mut builder = test_codex()
        .with_exec_server_url(format!("ws://{}", listener.local_addr()?))
        .with_config(|config| {
            config.project_doc_max_bytes = 0;
            assert!(config.features.enable(Feature::DeferredExecutor).is_ok());
            assert!(
                config
                    .features
                    .enable(Feature::DefaultModeRequestUserInput)
                    .is_ok()
            );
        });
    let test = builder.build_with_remote_and_local_env(&server).await?;
    let local_selection = local(test.config.cwd.clone());
    let remote_selection = TurnEnvironmentSelection {
        environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
        cwd: PathUri::from_abs_path(&test.config.cwd),
        workspace_roots: vec![PathUri::from_abs_path(&test.config.cwd)],
        config: EnvironmentConfigState::FromThread,
    };

    test.submit_turn_with_environments(
        "warm the local environment",
        Some(vec![local_selection.clone(), remote_selection.clone()]),
    )
    .await?;
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "wait for the primary environment".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(TurnEnvironmentSelections::new(
                    test.config.cwd.clone(),
                    vec![remote_selection, local_selection],
                )),
                ..Default::default()
            }),
        )
        .await?;
    let request = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::RequestUserInput(request) => Some(request.clone()),
        _ => None,
    })
    .await;

    let requests = response_mock.requests();
    let initial_context = requests[1]
        .message_input_texts("user")
        .into_iter()
        .rfind(|text| text.contains("<environment_context>"))
        .context("starting environment context")?;
    assert!(initial_context.contains("<environment id=\"local\" primary=\"true\">"));
    assert!(initial_context.contains("<environment id=\"remote\" primary=\"false\">"));
    assert!(initial_context.contains("<status>starting</status>"));

    serve_environment_info(listener).await;
    test.codex
        .submit(Op::UserInputAnswer {
            id: request.turn_id,
            response: RequestUserInputResponse {
                answers: HashMap::from([(
                    "continue".to_string(),
                    RequestUserInputAnswer {
                        answers: vec!["Yes (Recommended)".to_string()],
                    },
                )]),
            },
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = response_mock.requests();
    let updated_context = requests[2]
        .message_input_texts("user")
        .into_iter()
        .rfind(|text| text.contains("<environment_context>"))
        .context("updated primary environment context")?;
    assert!(updated_context.contains("<environment id=\"local\" primary=\"false\">"));
    assert!(updated_context.contains("<environment id=\"remote\" primary=\"true\">"));
    assert!(updated_context.contains("<shell>zsh</shell>"));

    test.codex.ensure_rollout_materialized().await;
    test.codex.flush_rollout().await?;
    let rollout = fs::read_to_string(test.codex.rollout_path().context("rollout path")?)?;
    let world_state_patch = rollout
        .lines()
        .map(serde_json::from_str::<RolloutLine>)
        .collect::<serde_json::Result<Vec<_>>>()?
        .into_iter()
        .filter_map(|line| match line.item {
            RolloutItem::WorldState(item) if !item.full => Some(Value::Object(item.state)),
            _ => None,
        })
        .find(|patch| {
            patch.pointer("/environments/environments/remote/is_primary") == Some(&json!(true))
        })
        .context("primary environment World State patch")?;
    assert_eq!(
        world_state_patch.pointer("/environments/environments/local/is_primary"),
        Some(&Value::Null)
    );

    Ok(())
}

async fn read_exec_server_json(websocket: &mut WebSocketStream<TcpStream>) -> Value {
    loop {
        match timeout(Duration::from_secs(5), websocket.next())
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

async fn accept_initialized_exec_server(listener: TcpListener) -> WebSocketStream<TcpStream> {
    let (stream, _) = listener.accept().await.expect("connection");
    let mut websocket = accept_async(stream).await.expect("websocket handshake");

    let initialize = read_exec_server_json(&mut websocket).await;
    assert_eq!(initialize["method"], "initialize");
    websocket
        .send(Message::Text(
            json!({
                "id": initialize["id"],
                "result": { "sessionId": "test-session" }
            })
            .to_string()
            .into(),
        ))
        .await
        .expect("initialize response");
    let initialized = read_exec_server_json(&mut websocket).await;
    assert_eq!(initialized["method"], "initialized");

    websocket
}

async fn send_environment_info(websocket: &mut WebSocketStream<TcpStream>) {
    let info = read_exec_server_json(websocket).await;
    assert_eq!(info["method"], "environment/info");
    websocket
        .send(Message::Text(
            json!({
                "id": info["id"],
                "result": { "shell": { "name": "zsh", "path": "/bin/zsh" } }
            })
            .to_string()
            .into(),
        ))
        .await
        .expect("environment info response");
}

async fn serve_environment_info(listener: TcpListener) {
    let mut websocket = accept_initialized_exec_server(listener).await;
    send_environment_info(&mut websocket).await;
}

async fn serve_environment_with_agents_md(
    listener: TcpListener,
    contents: &str,
    attach: tokio::sync::oneshot::Receiver<()>,
    mut shutdown: tokio::sync::oneshot::Receiver<()>,
) -> usize {
    let mut websocket = accept_initialized_exec_server(listener).await;
    attach.await.expect("attach signal");
    send_environment_info(&mut websocket).await;

    let mut agents_md_reads = 0;
    loop {
        let request = tokio::select! {
            request = read_exec_server_json(&mut websocket) => request,
            _ = &mut shutdown => return agents_md_reads,
        };
        let is_agents_md = request["params"]["path"]
            .as_str()
            .is_some_and(|path| path.ends_with("/AGENTS.md"));
        let response = match request["method"].as_str() {
            Some("environment/info") => json!({
                "id": request["id"],
                "result": { "shell": { "name": "zsh", "path": "/bin/zsh" } }
            }),
            Some("fs/canonicalize") => json!({
                "id": request["id"],
                "result": { "path": request["params"]["path"] }
            }),
            Some("fs/walk") => json!({
                "id": request["id"],
                "result": { "entries": [], "errors": [], "truncated": false }
            }),
            Some("fs/getMetadata") if is_agents_md => {
                json!({
                    "id": request["id"],
                    "result": {
                        "isDirectory": false,
                        "isFile": true,
                        "isSymlink": false,
                        "size": contents.len(),
                        "createdAtMs": 0,
                        "modifiedAtMs": 0,
                    }
                })
            }
            Some("fs/getMetadata") => json!({
                "id": request["id"],
                "error": { "code": -32004, "message": "not found" }
            }),
            Some("fs/readFile") if is_agents_md => {
                agents_md_reads += 1;
                json!({
                    "id": request["id"],
                    "result": { "dataBase64": BASE64_STANDARD.encode(contents) }
                })
            }
            method => panic!("unexpected exec-server request: {method:?}"),
        };
        websocket
            .send(Message::Text(response.to_string().into()))
            .await
            .expect("filesystem response");
    }
}

fn tool_names(body: &Value) -> Vec<String> {
    body["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str).map(str::to_owned))
        .collect()
}

#[derive(Default)]
struct FailingNoiseConnectProvider {
    calls: AtomicUsize,
}

impl NoiseRendezvousConnectProvider for FailingNoiseConnectProvider {
    fn connect_bundle(
        &self,
        _: NoiseChannelPublicKey,
    ) -> BoxFuture<'_, std::result::Result<NoiseRendezvousConnectBundle, ExecServerError>> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Box::pin(async {
            Err(ExecServerError::Protocol(
                "test Noise connection failed".to_string(),
            ))
        })
    }
}

struct ReadyNoiseConnectProvider {
    websocket_url: String,
    executor_public_key: NoiseChannelPublicKey,
}

impl NoiseRendezvousConnectProvider for ReadyNoiseConnectProvider {
    fn connect_bundle(
        &self,
        _: NoiseChannelPublicKey,
    ) -> BoxFuture<'_, std::result::Result<NoiseRendezvousConnectBundle, ExecServerError>> {
        let bundle = NoiseRendezvousConnectBundle {
            websocket_url: self.websocket_url.clone(),
            environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
            executor_registration_id: "ready-first-registration".to_string(),
            executor_public_key: self.executor_public_key.clone(),
            harness_key_authorization: "ready-first-authorization".to_string(),
        };
        Box::pin(async move { Ok(bundle) })
    }
}

struct NoopRegistryAuthProvider;

impl AuthProvider for NoopRegistryAuthProvider {
    fn add_auth_headers(&self, _: &mut HeaderMap) {}
}

async fn wait_for_response_request_count(response_mock: &ResponseMock, expected_count: usize) {
    timeout(Duration::from_secs(5), async {
        while response_mock.requests().len() < expected_count {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("timed out waiting for Responses API request");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shared_executor_keeps_ready_capability_roots_scoped_to_each_attachment() -> Result<()> {
    let server = start_mock_server().await;
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.prompt_contributor(Arc::new(ReadyCapabilityRootsTestExtension));
    let mut builder = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            config.use_experimental_unified_exec_tool = true;
            assert!(config.features.enable(Feature::UnifiedExec).is_ok());
        });
    let test = builder.build_with_auto_env(&server).await?;
    let selection = test
        .codex
        .environment_selections()
        .await
        .into_iter()
        .next()
        .context("thread should select its executor environment")?;
    let root = |id: &str| SelectedCapabilityRoot {
        id: id.to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: selection.environment_id.clone(),
            path: selection.cwd.clone(),
        },
    };
    let permission_profile =
        PermissionProfileSnapshot::legacy(test.config.permissions.permission_profile().clone());

    for config in [
        EnvironmentConfigState::Pending,
        EnvironmentConfigState::Ready(EnvironmentConfig {
            allow_login_shell: false,
            permission_profile: permission_profile.clone(),
            shell_environment_policy: Default::default(),
            exec_policy: None,
            network_policy: None,
            selected_capability_roots: vec![root("duplicate"), root("duplicate")],
        }),
    ] {
        let should_succeed = matches!(config, EnvironmentConfigState::Pending);
        let selection_override = TurnEnvironmentSelection {
            config,
            ..selection.clone()
        };
        let preview = test
            .codex
            .preview_thread_settings_overrides(CodexThreadSettingsOverrides {
                environments: Some(TurnEnvironmentSelections::new(
                    test.config.cwd.clone(),
                    vec![selection_override],
                )),
                ..Default::default()
            })
            .await;
        assert_eq!(preview.is_ok(), should_succeed);
        assert_eq!(
            test.codex.environment_selections().await,
            vec![selection.clone()]
        );
    }

    let mut second_thread_init = ExtensionDataInit::new();
    second_thread_init.insert(vec![root("startup-root")]);
    let second = test
        .thread_manager
        .start_thread(StartThreadOptions {
            environments: Some(vec![TurnEnvironmentSelection {
                config: EnvironmentConfigState::Ready(EnvironmentConfig {
                    allow_login_shell: true,
                    permission_profile: permission_profile.clone(),
                    shell_environment_policy: Default::default(),
                    exec_policy: None,
                    network_policy: None,
                    selected_capability_roots: vec![root("startup-root"), root("second-root")],
                }),
                ..selection.clone()
            }]),
            thread_extension_init: second_thread_init,
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?;

    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            environments: Some(TurnEnvironmentSelections::new(
                test.config.cwd.clone(),
                vec![TurnEnvironmentSelection {
                    config: EnvironmentConfigState::Ready(EnvironmentConfig {
                        allow_login_shell: false,
                        permission_profile: permission_profile.clone(),
                        shell_environment_policy: Default::default(),
                        exec_policy: None,
                        network_policy: None,
                        selected_capability_roots: vec![root("first-root")],
                    }),
                    ..selection.clone()
                }],
            )),
            ..Default::default()
        },
    )
    .await?;

    assert_eq!(
        test.codex.inspect_selected_capability_roots().ready_roots,
        vec![root("first-root")]
    );
    assert_eq!(
        second
            .thread
            .inspect_selected_capability_roots()
            .ready_roots,
        vec![root("startup-root"), root("second-root")]
    );

    let response_mock = mount_sse_sequence(
        &server,
        ["first", "second", "first-updated", "second-again"]
            .into_iter()
            .map(|response_id| {
                sse(vec![
                    ev_response_created(response_id),
                    ev_completed(response_id),
                ])
            })
            .collect(),
    )
    .await;

    for (index, (thread, prompt)) in [
        (&test.codex, "first"),
        (&second.thread, "second"),
        (&test.codex, "first-updated"),
        (&second.thread, "second-again"),
    ]
    .into_iter()
    .enumerate()
    {
        let mut request = TurnInputRequest::user_input(vec![UserInput::Text {
            text: prompt.to_string(),
            text_elements: Vec::new(),
        }]);
        if index == 2 {
            request = request.with_thread_settings(ThreadSettingsOverrides {
                environments: Some(TurnEnvironmentSelections::new(
                    test.config.cwd.clone(),
                    vec![TurnEnvironmentSelection {
                        config: EnvironmentConfigState::Ready(EnvironmentConfig {
                            allow_login_shell: false,
                            permission_profile: permission_profile.clone(),
                            shell_environment_policy: Default::default(),
                            exec_policy: None,
                            network_policy: None,
                            selected_capability_roots: vec![root("first-updated-root")],
                        }),
                        ..selection.clone()
                    }],
                )),
                ..Default::default()
            });
        }

        thread.start_or_steer_turn(request).await?;
        wait_for_event(thread, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    }

    let requests = response_mock.requests();
    let root_fragments = requests
        .iter()
        .map(|request| {
            request
                .message_input_texts("user")
                .into_iter()
                .rfind(|text| text.contains("<ready_capability_roots>"))
                .context("ready capability roots should be model-visible")
        })
        .collect::<Result<Vec<_>>>()?;
    assert_eq!(
        root_fragments,
        vec![
            "<ready_capability_roots>first-root</ready_capability_roots>",
            "<ready_capability_roots>startup-root,second-root</ready_capability_roots>",
            "<ready_capability_roots>first-updated-root</ready_capability_roots>",
            "<ready_capability_roots>startup-root,second-root</ready_capability_roots>",
        ]
    );

    let login_shells = requests
        .iter()
        .map(|request| {
            let body = request.body_json();
            let exec_command = body["tools"]
                .as_array()
                .context("tools should be an array")?
                .iter()
                .find(|tool| tool["name"] == "exec_command")
                .context("exec_command should be available")?;
            Ok(exec_command["parameters"]["properties"]
                .get("login")
                .is_some())
        })
        .collect::<Result<Vec<_>>>()?;
    assert_eq!(login_shells, vec![false, true, false, true]);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owner_network_policy_is_rejected_until_runtime_enforcement_exists() -> Result<()> {
    let server = start_mock_server().await;
    let test = test_codex().build_with_auto_env(&server).await?;
    let selections = test.codex.environment_selections().await;
    let selection = selections
        .first()
        .context("thread should select its executor environment")?;
    let owner_config = EnvironmentConfig {
        allow_login_shell: test.config.permissions.allow_login_shell,
        permission_profile: PermissionProfileSnapshot::legacy(
            test.config.permissions.permission_profile().clone(),
        ),
        shell_environment_policy: test.config.permissions.shell_environment_policy.clone(),
        exec_policy: None,
        network_policy: Some(EnvironmentNetworkPolicy::from_config(
            &NetworkProxyConfig::default(),
            /*managed_allowed_domains_only*/ true,
        )),
        selected_capability_roots: Vec::new(),
    };
    let preview_error = test
        .codex
        .preview_thread_settings_overrides(CodexThreadSettingsOverrides {
            environments: Some(TurnEnvironmentSelections::new(
                test.config.cwd.clone(),
                vec![TurnEnvironmentSelection {
                    config: EnvironmentConfigState::Ready(owner_config.clone()),
                    ..selection.clone()
                }],
            )),
            ..Default::default()
        })
        .await
        .err()
        .context("preview must not accept an unenforced policy")?;
    let ready_error = test
        .codex
        .environment_ready(selection, owner_config)
        .await
        .expect_err("readiness must not accept an unenforced policy");

    for error in [preview_error.to_string(), ready_error.to_string()] {
        assert!(error.contains("attachment-owned network policy is not supported yet"));
    }
    assert_eq!(test.codex.environment_selections().await, selections);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pending_attachment_installs_configuration_before_waiting_turn_resumes() -> Result<()> {
    const WAIT_CALL_ID: &str = "wait-for-owner-configuration";

    let server = start_mock_server().await;
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.thread_lifecycle_contributor(Arc::new(WaitForEnvironmentTestExtension));
    extensions.prompt_contributor(Arc::new(ReadyCapabilityRootsTestExtension));
    let mut builder = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            config.use_experimental_unified_exec_tool = true;
            assert!(config.features.enable(Feature::DeferredExecutor).is_ok());
            assert!(config.features.enable(Feature::UnifiedExec).is_ok());
            config
                .permissions
                .set_permission_profile(PermissionProfile::read_only())
                .expect("thread permissions should be configurable");
        });
    let test = builder.build_with_auto_env(&server).await?;
    let selection = test
        .codex
        .environment_selections()
        .await
        .into_iter()
        .next()
        .context("thread should select its executor environment")?;
    let pending_selection = TurnEnvironmentSelection {
        config: EnvironmentConfigState::Pending,
        ..selection.clone()
    };
    let root = |id: &str| SelectedCapabilityRoot {
        id: id.to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: selection.environment_id.clone(),
            path: selection.cwd.clone(),
        },
    };
    let owner_config = |id: &str, allow_login_shell: bool| EnvironmentConfig {
        allow_login_shell,
        permission_profile: PermissionProfileSnapshot::legacy(PermissionProfile::read_only()),
        shell_environment_policy: Default::default(),
        exec_policy: None,
        network_policy: None,
        selected_capability_roots: vec![root(id)],
    };
    let start_pending_thread = || {
        test.thread_manager.start_thread(StartThreadOptions {
            environments: Some(vec![pending_selection.clone()]),
            ..StartThreadOptions::new(test.config.clone())
        })
    };
    let waiting = timeout(Duration::from_secs(5), start_pending_thread())
        .await
        .context("pending thread startup should not block")??;
    let independent = start_pending_thread().await?;
    let failed = start_pending_thread().await?;

    submit_thread_settings(
        &waiting.thread,
        ThreadSettingsOverrides {
            permission_profile: Some(PermissionProfile::workspace_write()),
            ..Default::default()
        },
    )
    .await?;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("pending-configuration-wait"),
                ev_function_call(
                    WAIT_CALL_ID,
                    "wait_for_environment",
                    &json!({ "environment_id": selection.environment_id }).to_string(),
                ),
                ev_completed("pending-configuration-wait"),
            ]),
            sse(vec![
                ev_response_created("pending-configuration-ready"),
                ev_assistant_message("pending-configuration-message", "done"),
                ev_completed("pending-configuration-ready"),
            ]),
        ],
    )
    .await;
    waiting
        .thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "wait for environment configuration".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_response_request_count(&response_mock, /*expected_count*/ 1).await;
    let first_tool_names = tool_names(&response_mock.requests()[0].body_json());
    assert!(first_tool_names.contains(&"wait_for_environment".to_string()));
    assert!(!first_tool_names.contains(&"exec_command".to_string()));

    independent
        .thread
        .environment_ready(
            &pending_selection,
            owner_config("independent-root", /*allow_login_shell*/ true),
        )
        .await?;
    failed
        .thread
        .environment_failed(&pending_selection, "configuration unavailable".to_string())
        .await?;
    assert_eq!(
        failed.thread.environment_selections().await,
        vec![TurnEnvironmentSelection {
            config: EnvironmentConfigState::Failed("configuration unavailable".to_string()),
            ..pending_selection.clone()
        }]
    );
    assert!(
        waiting
            .thread
            .inspect_selected_capability_roots()
            .ready_roots
            .is_empty()
    );

    let waiting_config = owner_config("waiting-root", /*allow_login_shell*/ false);
    let ready_selection = TurnEnvironmentSelection {
        config: EnvironmentConfigState::Ready(waiting_config.clone()),
        ..pending_selection.clone()
    };
    submit_thread_settings(
        &waiting.thread,
        ThreadSettingsOverrides {
            environments: Some(TurnEnvironmentSelections::new(
                test.config.cwd.clone(),
                vec![ready_selection.clone()],
            )),
            ..Default::default()
        },
    )
    .await?;
    wait_for_response_request_count(&response_mock, /*expected_count*/ 2).await;
    assert_eq!(
        waiting.thread.environment_selections().await,
        vec![ready_selection]
    );

    let ready_request = response_mock
        .last_request()
        .context("waiting turn should resume")?;
    let (_, wait_succeeded) = ready_request
        .function_call_output_content_and_success(WAIT_CALL_ID)
        .context("wait_for_environment output should be model visible")?;
    assert_ne!(wait_succeeded, Some(false));
    let body = ready_request.body_json();
    let exec_command = body["tools"]
        .as_array()
        .context("tools should be an array")?
        .iter()
        .find(|tool| tool["name"] == "exec_command")
        .context("exec_command should become available")?;
    assert!(
        exec_command["parameters"]["properties"]
            .get("login")
            .is_none()
    );
    assert_eq!(
        ready_request
            .message_input_texts("user")
            .into_iter()
            .rfind(|text| text.contains("<ready_capability_roots>")),
        Some("<ready_capability_roots>waiting-root</ready_capability_roots>".to_string())
    );

    let recovered_selection = TurnEnvironmentSelection {
        config: EnvironmentConfigState::Ready(owner_config(
            "recovered-root",
            /*allow_login_shell*/ true,
        )),
        ..pending_selection
    };
    submit_thread_settings(
        &failed.thread,
        ThreadSettingsOverrides {
            environments: Some(TurnEnvironmentSelections::new(
                test.config.cwd.clone(),
                vec![recovered_selection.clone()],
            )),
            ..Default::default()
        },
    )
    .await?;
    assert_eq!(
        failed.thread.environment_selections().await,
        vec![recovered_selection]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ready_before_selection_exposes_remote_tools_and_capability_context_after_wait()
-> Result<()> {
    const WAIT_CALL_ID: &str = "wait-ready-before-selection";

    let rendezvous = TcpListener::bind("127.0.0.1:0").await?;
    let rendezvous_url = format!("ws://{}", rendezvous.local_addr()?);
    let registry = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!(
            "/cloud/environment/{REMOTE_ENVIRONMENT_ID}/register"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "environment_id": REMOTE_ENVIRONMENT_ID,
            "url": format!("{rendezvous_url}/relay?role=environment"),
            "security_profile": "noise_hybrid_ik_v1",
            "executor_registration_id": "ready-first-registration",
        })))
        .expect(1)
        .mount(&registry)
        .await;
    Mock::given(method("POST"))
        .and(path(format!(
            "/cloud/environment/{REMOTE_ENVIRONMENT_ID}/validate"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "valid": true })))
        .expect(1)
        .mount(&registry)
        .await;

    let runtime_paths = ExecServerRuntimePaths::new(
        std::env::current_exe()?,
        /*codex_linux_sandbox_exe*/ None,
    )?;
    let remote_config = RemoteEnvironmentConfig::new(
        registry.uri(),
        REMOTE_ENVIRONMENT_ID.to_string(),
        Arc::new(NoopRegistryAuthProvider),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    )?;
    let remote_environment = tokio::spawn(codex_exec_server::run_remote_environment(
        remote_config,
        runtime_paths,
    ));
    let (environment_socket, _) = timeout(Duration::from_secs(5), rendezvous.accept())
        .await
        .context("remote environment should reach rendezvous")??;
    let environment_websocket = timeout(Duration::from_secs(5), accept_async(environment_socket))
        .await
        .context("remote environment websocket handshake should complete")??;
    let executor_public_key = registry
        .received_requests()
        .await
        .context("wiremock should retain registration requests")?
        .iter()
        .find(|request| request.url.path().ends_with("/register"))
        .context("remote environment should register its public key")
        .and_then(|request| {
            serde_json::from_slice::<Value>(&request.body).context("registration request body")
        })
        .and_then(|body| {
            serde_json::from_value(body["executor_public_key"].clone())
                .context("registered executor public key")
        })?;

    let server = start_mock_server().await;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("ready-first-wait"),
                ev_function_call(
                    WAIT_CALL_ID,
                    "wait_for_environment",
                    &json!({ "environment_id": REMOTE_ENVIRONMENT_ID }).to_string(),
                ),
                ev_completed("ready-first-wait"),
            ]),
            sse(vec![
                ev_response_created("ready-first-done"),
                ev_assistant_message("ready-first-message", "done"),
                ev_completed("ready-first-done"),
            ]),
        ],
    )
    .await;
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.thread_lifecycle_contributor(Arc::new(WaitForEnvironmentTestExtension));
    extensions.prompt_contributor(Arc::new(ReadyCapabilityRootsTestExtension));
    let mut builder = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            config.project_doc_max_bytes = 0;
            config.use_experimental_unified_exec_tool = true;
            assert!(config.features.enable(Feature::DeferredExecutor).is_ok());
            assert!(config.features.enable(Feature::UnifiedExec).is_ok());
        });
    let test = builder.build(&server).await?;
    let ready_root = SelectedCapabilityRoot {
        id: "ready-first-root".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
            path: PathUri::parse("file:///ready-first-root")?,
        },
    };
    let environment = test
        .thread_manager
        .environment_manager()
        .report_environment_provisioning_status(
            REMOTE_ENVIRONMENT_ID.to_string(),
            Ok(EnvironmentReadyInfo {
                selected_capability_roots: Vec::new(),
            }),
            Arc::new(ReadyNoiseConnectProvider {
                websocket_url: format!("{rendezvous_url}/relay?role=harness"),
                executor_public_key,
            }),
        )?
        .context("Ready-first report should create the environment")?;

    assert!(!environment.startup_finished());
    let relay = tokio::spawn(async move {
        let (harness_socket, _) = timeout(Duration::from_secs(5), rendezvous.accept())
            .await
            .context("selecting the ready environment should start its Noise connection")??;
        let harness_websocket = timeout(Duration::from_secs(5), accept_async(harness_socket))
            .await
            .context("harness websocket handshake should complete")??;
        let mut environment_websocket = environment_websocket;
        let mut harness_websocket = harness_websocket;
        loop {
            tokio::select! {
                message = environment_websocket.next() => {
                    let Some(message) = message else {
                        break;
                    };
                    harness_websocket.send(message?).await?;
                }
                message = harness_websocket.next() => {
                    let Some(message) = message else {
                        break;
                    };
                    environment_websocket.send(message?).await?;
                }
            }
        }
        anyhow::Ok(())
    });

    test.submit_turn_with_environments(
        "use the ready environment",
        Some(vec![TurnEnvironmentSelection {
            environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
            cwd: PathUri::from_abs_path(&test.config.cwd),
            workspace_roots: vec![PathUri::from_abs_path(&test.config.cwd)],
            config: EnvironmentConfigState::Ready(EnvironmentConfig {
                allow_login_shell: true,
                permission_profile: PermissionProfileSnapshot::legacy(
                    test.config.permissions.permission_profile().clone(),
                ),
                shell_environment_policy: Default::default(),
                exec_policy: None,
                network_policy: None,
                selected_capability_roots: vec![ready_root],
            }),
        }]),
    )
    .await?;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2);
    // Provisioning was reported ready before selection, but selection materialization remains
    // nonblocking while the transport starts.
    // The first request may legally see either Starting or Ready; the wait makes step two ready.
    let first_tools = tool_names(&requests[0].body_json());
    assert!(first_tools.contains(&"wait_for_environment".to_string()));
    let first_user_context = requests[0].message_input_texts("user");
    let first_environment_context = first_user_context
        .iter()
        .rfind(|text| text.contains("<environment_context>"))
        .context("initial environment context should be model visible")?;
    let first_has_ready_root = first_user_context
        .iter()
        .any(|text| text.contains("<ready_capability_roots>ready-first-root"));
    if first_tools.contains(&"exec_command".to_string()) {
        assert!(!first_environment_context.contains("<status>starting</status>"));
        assert!(first_environment_context.contains("<shell>"));
        assert!(first_has_ready_root);
    } else {
        assert!(first_environment_context.contains("<status>starting</status>"));
        assert!(!first_has_ready_root);
    }

    let (_, wait_succeeded) = requests[1]
        .function_call_output_content_and_success(WAIT_CALL_ID)
        .context("wait_for_environment output should be model visible")?;
    assert_ne!(wait_succeeded, Some(false));
    assert!(tool_names(&requests[1].body_json()).contains(&"exec_command".to_string()));
    let user_context = requests[1].message_input_texts("user");
    let environment_context = user_context
        .iter()
        .rfind(|text| text.contains("<environment_context>"))
        .context("ready environment context should be model visible")?;
    assert!(!environment_context.contains("status=\"unavailable\""));
    assert!(!environment_context.contains("<status>starting</status>"));
    assert!(environment_context.contains("<shell>"));
    assert!(
        user_context
            .iter()
            .any(|text| text.contains("<ready_capability_roots>ready-first-root"))
    );

    relay.abort();
    remote_environment.abort();
    let _ = relay.await;
    let _ = remote_environment.await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deferred_executor_stays_pending_after_materialization() -> Result<()> {
    let server = start_mock_server().await;
    let wait_call_id = "wait-for-startup";
    let response_mock = mount_sse_sequence(
        &server,
        vec![sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(
                wait_call_id,
                "wait_for_environment",
                &json!({
                    "environment_id": REMOTE_ENVIRONMENT_ID,
                })
                .to_string(),
            ),
            ev_completed("resp-1"),
        ])],
    )
    .await;
    let mut builder = test_codex_with_wait_for_environment().with_config(|config| {
        config.use_experimental_unified_exec_tool = true;
        assert!(config.features.enable(Feature::DeferredExecutor).is_ok());
        assert!(config.features.enable(Feature::UnifiedExec).is_ok());
    });
    let test = timeout(Duration::from_secs(5), builder.build(&server))
        .await
        .context("thread startup should not wait for the remote environment")??;
    let environment_manager = test.thread_manager.environment_manager();
    let provider = Arc::new(FailingNoiseConnectProvider::default());
    environment_manager.materialize_pending_noise_environment(
        REMOTE_ENVIRONMENT_ID.to_string(),
        provider.clone(),
    )?;

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "wait for the environment".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(TurnEnvironmentSelections::new(
                    test.config.cwd.clone(),
                    vec![TurnEnvironmentSelection {
                        environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                        cwd: PathUri::from_abs_path(&test.config.cwd),
                        workspace_roots: vec![PathUri::from_abs_path(&test.config.cwd)],
                        config: EnvironmentConfigState::FromThread,
                    }],
                )),
                ..Default::default()
            }),
        )
        .await?;
    wait_for_response_request_count(&response_mock, /*expected_count*/ 1).await;
    assert_eq!(provider.calls.load(Ordering::Relaxed), 0);

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 1);
    let starting_request_body = requests[0].body_json();
    let starting_tools = tool_names(&starting_request_body);
    assert!(starting_tools.contains(&"wait_for_environment".to_string()));
    assert!(!starting_tools.contains(&"exec_command".to_string()));
    let wait_tool = starting_request_body["tools"]
        .as_array()
        .and_then(|tools| {
            tools
                .iter()
                .find(|tool| tool["name"] == "wait_for_environment")
        })
        .context("wait_for_environment tool schema should be present")?;
    assert_eq!(
        wait_tool["description"].as_str(),
        Some(WAIT_FOR_ENVIRONMENT_TEST_TOOL_DESCRIPTION)
    );
    assert_eq!(
        wait_tool["parameters"]["properties"]["environment_id"]["description"].as_str(),
        Some(WAIT_FOR_ENVIRONMENT_TEST_ENVIRONMENT_ID_DESCRIPTION)
    );

    test.codex.submit(Op::Interrupt).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnAborted(_))
    })
    .await;

    Ok(())
}

#[test_case(false, "multi_agent_v1"; "v1")]
#[test_case(true, "collaboration"; "v2")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deferred_executor_spawn_agent_inherits_ready_step_environments(
    multi_agent_v2: bool,
    namespace: &str,
) -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let server = start_mock_server().await;
    let wait_call_id = "wait-for-spawn-environment";
    let spawn_call_id = "spawn-in-ready-environment";
    let message = "inspect the ready step environment";
    let spawn_arguments = if multi_agent_v2 {
        json!({ "message": message, "task_name": "worker" })
    } else {
        json!({ "message": message })
    }
    .to_string();
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-wait"),
                ev_function_call(
                    wait_call_id,
                    "wait_for_environment",
                    &json!({ "environment_id": REMOTE_ENVIRONMENT_ID }).to_string(),
                ),
                ev_completed("resp-wait"),
            ]),
            sse(vec![
                ev_response_created("resp-spawn"),
                ev_function_call_with_namespace(
                    spawn_call_id,
                    namespace,
                    "spawn_agent",
                    &spawn_arguments,
                ),
                ev_completed("resp-spawn"),
            ]),
            sse(vec![
                ev_response_created("resp-done-1"),
                ev_assistant_message("msg-done-1", "done"),
                ev_completed("resp-done-1"),
            ]),
            sse(vec![
                ev_response_created("resp-done-2"),
                ev_assistant_message("msg-done-2", "done"),
                ev_completed("resp-done-2"),
            ]),
        ],
    )
    .await;
    let mut builder = test_codex()
        .with_exec_server_url(format!("ws://{}", listener.local_addr()?))
        .with_config(move |config| {
            config.project_doc_max_bytes = 0;
            assert!(config.features.enable(Feature::DeferredExecutor).is_ok());
            assert!(config.features.enable(Feature::Collab).is_ok());
            if multi_agent_v2 {
                assert!(config.features.enable(Feature::MultiAgentV2).is_ok());
            } else {
                assert!(config.features.disable(Feature::MultiAgentV2).is_ok());
            }
        });
    let (attach_tx, attach_rx) = tokio::sync::oneshot::channel();
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let exec_server = tokio::spawn(serve_environment_with_agents_md(
        listener,
        "",
        attach_rx,
        shutdown_rx,
    ));
    let test = timeout(
        Duration::from_secs(5),
        builder.build_with_remote_and_local_env(&server),
    )
    .await
    .context("thread startup should not wait for the remote environment")??;
    let remote_selection = TurnEnvironmentSelection {
        environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
        cwd: PathUri::from_abs_path(&test.config.cwd),
        workspace_roots: vec![PathUri::from_abs_path(&test.config.cwd)],
        config: EnvironmentConfigState::FromThread,
    };
    let expected_environments = vec![remote_selection, local(test.config.cwd.clone())];
    let mut created_threads = test.thread_manager.subscribe_thread_created();

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "spawn after the environment becomes ready".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(TurnEnvironmentSelections::new(
                    test.config.cwd.clone(),
                    expected_environments.clone(),
                )),
                ..Default::default()
            }),
        )
        .await?;
    wait_for_response_request_count(&response_mock, /*expected_count*/ 1).await;
    attach_tx.send(()).expect("attach remote environment");
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    wait_for_response_request_count(&response_mock, /*expected_count*/ 4).await;

    let child_thread_id = timeout(Duration::from_secs(5), created_threads.recv())
        .await
        .context("timed out waiting for the subagent thread")??;
    let child_thread = test.thread_manager.get_thread(child_thread_id).await?;
    assert_eq!(
        child_thread.environment_selections().await,
        expected_environments
    );
    assert!(
        response_mock.requests()[1]
            .function_call_output_content_and_success(wait_call_id)
            .is_some(),
        "the spawn request should follow the ready-environment step"
    );

    shutdown_tx
        .send(())
        .expect("stop remote environment server");
    exec_server.await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deferred_executor_guardian_uses_newly_ready_step_environment() -> Result<()> {
    const WAIT_CALL_ID: &str = "wait-for-guardian-environment";
    const EXEC_CALL_ID: &str = "guardian-ready-environment-command";
    const DENIAL_RATIONALE: &str = "The remote environment policy denies this action.";

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let server = start_mock_server().await;
    let completed_response =
        |id: &str, item| sse(vec![ev_response_created(id), item, ev_completed(id)]);
    let responses = mount_sse_sequence(
        &server,
        vec![
            completed_response(
                "resp-guardian-wait",
                ev_function_call(
                    WAIT_CALL_ID,
                    "wait_for_environment",
                    &json!({ "environment_id": REMOTE_ENVIRONMENT_ID }).to_string(),
                ),
            ),
            completed_response(
                "resp-guardian-command",
                ev_function_call(
                    EXEC_CALL_ID,
                    "exec_command",
                    &json!({
                        "cmd": "printf guardian-should-not-run",
                        "environment_id": REMOTE_ENVIRONMENT_ID,
                        "sandbox_permissions": SandboxPermissions::RequireEscalated,
                        "justification": "Review the newly ready remote environment.",
                    })
                    .to_string(),
                ),
            ),
            completed_response(
                "resp-guardian-review",
                ev_assistant_message(
                    "msg-guardian-review",
                    &json!({ "outcome": "deny", "rationale": DENIAL_RATIONALE }).to_string(),
                ),
            ),
            completed_response(
                "resp-guardian-done",
                ev_assistant_message("msg-guardian-done", "done"),
            ),
        ],
    )
    .await;
    let mut builder = test_codex()
        .with_exec_server_url(format!("ws://{}", listener.local_addr()?))
        .with_config(|config| {
            config.project_doc_max_bytes = 0;
            config.use_experimental_unified_exec_tool = true;
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            assert!(config.features.enable(Feature::DeferredExecutor).is_ok());
            assert!(config.features.enable(Feature::UnifiedExec).is_ok());
        });
    let (attach_tx, attach_rx) = tokio::sync::oneshot::channel();
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let exec_server = tokio::spawn(serve_environment_with_agents_md(
        listener,
        "",
        attach_rx,
        shutdown_rx,
    ));
    let test = timeout(
        Duration::from_secs(5),
        builder.build_with_remote_and_local_env(&server),
    )
    .await
    .context("thread startup should not wait for the remote environment")??;
    let remote_cwd = test.cwd.path().join("guardian-remote").abs();
    let local_cwd = test.cwd.path().abs();
    fs::create_dir_all(remote_cwd.as_path())?;
    let remote_denied_path = remote_cwd.canonicalize()?.join("private");
    let local_denied_path = local_cwd.canonicalize()?.join("private");
    let remote_selection = TurnEnvironmentSelection {
        environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
        cwd: PathUri::from_abs_path(&remote_cwd),
        workspace_roots: vec![PathUri::from_abs_path(&remote_cwd)],
        config: EnvironmentConfigState::FromThread,
    };
    let permission_profile = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                FileSystemAccessMode::Read,
            ),
            FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: FileSystemSpecialPath::project_roots(Some("private".to_string())),
                },
                FileSystemAccessMode::Deny,
            ),
        ]),
        NetworkSandboxPolicy::Restricted,
    );
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(permission_profile, test.config.cwd.as_path());

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "review a command after the remote environment becomes ready".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(TurnEnvironmentSelections::new(
                    test.config.cwd.clone(),
                    vec![remote_selection, local(local_cwd.clone())],
                )),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                ..Default::default()
            }),
        )
        .await?;
    wait_for_response_request_count(&responses, /*expected_count*/ 1).await;
    attach_tx.send(()).expect("attach remote environment");
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = responses.requests();
    assert!(
        requests[1]
            .function_call_output_content_and_success(WAIT_CALL_ID)
            .is_some(),
        "the reviewed command should follow the ready-environment step"
    );
    let guardian_request = requests
        .iter()
        .find(|request| {
            request.body_json()["client_metadata"]["x-openai-subagent"].as_str() == Some("guardian")
        })
        .context("expected Guardian review request")?;
    let guardian_context = guardian_request.message_input_texts("user").join("\n");
    for expected in [
        "<environment id=\"remote\" primary=\"true\">".to_string(),
        format!("<cwd>{}</cwd>", remote_cwd.display()),
        format!("- path `{}`", remote_denied_path.display()),
    ] {
        assert!(
            guardian_context.contains(&expected),
            "Guardian omitted `{expected}` from the ready environment context: {guardian_context}"
        );
    }
    assert!(
        !guardian_context.contains(&format!("- path `{}`", local_denied_path.display())),
        "Guardian used the stale local environment's denied-read policy: {guardian_context}"
    );
    let rejection = requests
        .iter()
        .find_map(|request| request.function_call_output_text(EXEC_CALL_ID))
        .context("Guardian denial should be returned to the parent model")?;
    assert!(rejection.contains(DENIAL_RATIONALE));

    shutdown_tx
        .send(())
        .expect("stop remote environment server");
    exec_server.await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deferred_executor_loads_agents_md_when_environment_becomes_ready() -> Result<()> {
    const AGENTS_CONTENT: &str = "REMOTE_AGENTS_INSTRUCTIONS";

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let server = start_mock_server().await;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    "wait-1",
                    "wait_for_environment",
                    &json!({ "environment_id": REMOTE_ENVIRONMENT_ID }).to_string(),
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_function_call(
                    "wait-2",
                    "wait_for_environment",
                    &json!({ "environment_id": REMOTE_ENVIRONMENT_ID }).to_string(),
                ),
                ev_completed("resp-2"),
            ]),
            sse(vec![
                ev_response_created("resp-3"),
                ev_assistant_message("msg-3", "done"),
                ev_completed("resp-3"),
            ]),
        ],
    )
    .await;
    let mut builder = test_codex_with_wait_for_environment()
        .with_exec_server_url(format!("ws://{}", listener.local_addr()?))
        .with_config(|config| {
            assert!(config.features.enable(Feature::DeferredExecutor).is_ok());
        });
    let (attach_tx, attach_rx) = tokio::sync::oneshot::channel();
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let exec_server = tokio::spawn(serve_environment_with_agents_md(
        listener,
        AGENTS_CONTENT,
        attach_rx,
        shutdown_rx,
    ));
    let test = timeout(Duration::from_secs(5), builder.build(&server))
        .await
        .context("thread startup should not wait for the remote environment")??;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "load the environment instructions".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_response_request_count(&response_mock, /*expected_count*/ 1).await;
    let agents_path = PathUri::from_abs_path(&test.config.cwd).join("AGENTS.md")?;
    attach_tx.send(()).expect("attach environment");
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    shutdown_tx.send(()).expect("stop exec server");
    let agents_md_reads = exec_server.await?;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 3);
    assert_eq!(agents_md_reads, 1);
    assert_eq!(agents_md_occurrences(&requests[0], AGENTS_CONTENT), 0);
    assert_eq!(agents_md_occurrences(&requests[1], AGENTS_CONTENT), 1);
    assert_eq!(agents_md_occurrences(&requests[2], AGENTS_CONTENT), 1);
    assert_eq!(environment_instructions_occurrences(&requests[0]), 1);
    assert_eq!(environment_instructions_occurrences(&requests[1]), 1);
    assert_eq!(environment_instructions_occurrences(&requests[2]), 1);
    assert_eq!(test.codex.instruction_sources().await, vec![agents_path]);

    Ok(())
}

fn agents_md_occurrences(request: &ResponsesRequest, contents: &str) -> usize {
    request
        .message_input_texts("user")
        .iter()
        .filter(|text| text.contains(contents))
        .count()
}

fn environment_instructions_occurrences(request: &ResponsesRequest) -> usize {
    request
        .message_input_texts("developer")
        .iter()
        .filter(|text| text.contains(ENVIRONMENTS_INSTRUCTIONS_OPEN_TAG))
        .count()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deferred_executor_compaction_preserves_then_updates_environment_once() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let server = start_mock_server().await;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    "wait-for-startup",
                    "request_user_input",
                    &json!({
                        "questions": [{
                            "id": "continue",
                            "header": "Continue",
                            "question": "Continue after startup?",
                            "options": [{
                                "label": "Yes (Recommended)",
                                "description": "Continue the test."
                            }, {
                                "label": "No",
                                "description": "Stop the test."
                            }]
                        }]
                    })
                    .to_string(),
                ),
                ev_completed_with_tokens("resp-1", /*total_tokens*/ 96),
            ]),
            sse(vec![
                ev_assistant_message("msg-compact", "AUTO_COMPACT_SUMMARY"),
                ev_completed_with_tokens("resp-compact", /*total_tokens*/ 10),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;
    let mut builder = test_codex_with_wait_for_environment()
        .with_exec_server_url(format!("ws://{}", listener.local_addr()?))
        .with_config(|config| {
            config.project_doc_max_bytes = 0;
            assert!(config.features.enable(Feature::DeferredExecutor).is_ok());
            assert!(
                config
                    .features
                    .enable(Feature::DefaultModeRequestUserInput)
                    .is_ok()
            );
            config.model_provider.name = "OpenAI (test)".to_string();
            config.compact_prompt = Some(SUMMARIZATION_PROMPT.to_string());
            config.model_context_window = Some(100);
            config.model_auto_compact_token_limit = Some(90);
        });
    let test = timeout(Duration::from_secs(5), builder.build(&server))
        .await
        .context("thread startup should not wait for the remote environment")??;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "wait for the environment".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let request = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::RequestUserInput(request) => Some(request.clone()),
        _ => None,
    })
    .await;

    serve_environment_info(listener).await;
    test.codex
        .submit(Op::UserInputAnswer {
            id: request.turn_id,
            response: RequestUserInputResponse {
                answers: HashMap::from([(
                    "continue".to_string(),
                    RequestUserInputAnswer {
                        answers: vec!["Yes (Recommended)".to_string()],
                    },
                )]),
            },
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 3);
    let initial_context = requests[0].message_input_texts("user");
    assert!(
        initial_context
            .iter()
            .any(|text| text.contains("<status>starting</status>"))
    );

    let post_compaction_context = requests[2].message_input_texts("user");
    assert_eq!(
        post_compaction_context
            .iter()
            .filter(|text| text.contains("<status>starting</status>"))
            .count(),
        1
    );
    assert_eq!(
        post_compaction_context
            .iter()
            .filter(|text| text.contains("<shell>zsh</shell>"))
            .count(),
        1
    );
    let starting_index = post_compaction_context
        .iter()
        .position(|text| text.contains("<status>starting</status>"))
        .expect("compaction should preserve the prior environment state");
    let ready_index = post_compaction_context
        .iter()
        .position(|text| text.contains("<shell>zsh</shell>"))
        .expect("the next sampling step should report that the environment is ready");
    assert!(starting_index < ready_index);

    test.codex.ensure_rollout_materialized().await;
    test.codex.flush_rollout().await?;
    let rollout_path = test.codex.rollout_path().context("rollout path")?;
    let rollout = fs::read_to_string(rollout_path)?;
    let world_state_items = rollout
        .lines()
        .map(serde_json::from_str::<RolloutLine>)
        .collect::<serde_json::Result<Vec<_>>>()?
        .into_iter()
        .filter_map(|line| match line.item {
            RolloutItem::WorldState(item) => Some(item),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        world_state_items
            .iter()
            .map(|item| item.full)
            .collect::<Vec<_>>(),
        vec![true, true, false]
    );
    assert_eq!(
        world_state_items[0].state["environments"].pointer("/environments/remote/status"),
        Some(&json!("starting"))
    );
    assert_eq!(
        world_state_items[2].state["environments"].pointer("/environments/remote/status"),
        Some(&json!("available"))
    );
    assert_eq!(
        world_state_items[2].state["environments"].pointer("/environments/remote/shell"),
        Some(&json!("zsh"))
    );

    Ok(())
}

fn absolute_path(path: PathBuf) -> AbsolutePathBuf {
    AbsolutePathBuf::try_from(path).expect("path should be absolute")
}

fn read_only_sandbox(readable_root: PathBuf) -> FileSystemSandboxContext {
    let readable_root = absolute_path(readable_root);
    FileSystemSandboxContext::from_permission_profile(PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: readable_root,
            },
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        }]),
        NetworkSandboxPolicy::Restricted,
    ))
}

fn workspace_write_sandbox(writable_root: PathBuf) -> FileSystemSandboxContext {
    let writable_root = absolute_path(writable_root);
    FileSystemSandboxContext::from_permission_profile(PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: writable_root,
            },
            access: FileSystemAccessMode::Write,
            missing_path_behavior: None,
        }]),
        NetworkSandboxPolicy::Restricted,
    ))
}

fn assert_normalized_path_rejected(error: &std::io::Error) {
    match error.kind() {
        std::io::ErrorKind::NotFound => assert!(
            error.to_string().contains("No such file or directory"),
            "unexpected not-found message: {error}",
        ),
        std::io::ErrorKind::InvalidInput | std::io::ErrorKind::PermissionDenied => {
            let message = error.to_string();
            assert!(
                message.contains("is not permitted")
                    || message.contains("Operation not permitted")
                    || message.contains("Permission denied"),
                "unexpected rejection message: {message}",
            );
        }
        other => panic!("unexpected normalized-path error kind: {other:?}: {error:?}"),
    }
}

fn remote_exec(script: &str) -> Result<()> {
    let container_name = test_docker_container_name()
        .context("test requires direct access to the Docker container")?;
    let output = Command::new("docker")
        .args(["exec", container_name.as_str(), "sh", "-lc", script])
        .output()?;
    assert!(
        output.status.success(),
        "remote exec failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout).trim(),
        String::from_utf8_lossy(&output.stderr).trim(),
    );
    Ok(())
}

async fn exec_command_routing_output(
    test: &TestCodex,
    server: &wiremock::MockServer,
    call_id: &str,
    arguments: Value,
    environments: Option<Vec<TurnEnvironmentSelection>>,
) -> Result<String> {
    let response_mock = mount_sse_sequence(
        server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(call_id, "exec_command", &serde_json::to_string(&arguments)?),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    test.submit_turn_with_environments("route exec command", environments)
        .await?;

    let output = response_mock
        .function_call_output_text(call_id)
        .with_context(|| format!("missing function_call_output for {call_id}"))?;
    let request = response_mock
        .requests()
        .into_iter()
        .next()
        .context("initial model request should be recorded")?;
    let tools = tool_names(&request.body_json());
    assert!(tools.contains(&"exec_command".to_string()));
    assert!(tools.contains(&"write_stdin".to_string()));
    assert!(!tools.contains(&"shell_command".to_string()));

    Ok(output)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_command_routes_to_selected_remote_environment() -> Result<()> {
    skip_if_no_network!(Ok(()));
    // TODO(anp): Remove after remote path fixtures use target-native paths.
    skip_if_target_windows!(Ok(()), "requires the Docker-backed POSIX executor");
    skip_if_no_remote_env!(Ok(()));

    let server = start_mock_server().await;
    let test = unified_exec_test(&server).await?;
    let local_cwd = TempDir::new()?;
    fs::write(local_cwd.path().join("marker.txt"), "local-routing")?;
    let local_selection = local(local_cwd.path().abs());
    let remote_cwd = PathBuf::from(format!(
        "/tmp/codex-remote-routing-{}",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
    ))
    .abs();
    let remote_marker_name = "marker.txt";
    let remote_cwd_uri = PathUri::from_host_native_path(&remote_cwd)?;
    let remote_marker_uri = PathUri::from_host_native_path(remote_cwd.join(remote_marker_name))?;
    test.fs()
        .create_directory(
            &remote_cwd_uri,
            CreateDirectoryOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await?;
    test.fs()
        .write_file(
            &remote_marker_uri,
            b"remote-routing".to_vec(),
            /*sandbox*/ None,
        )
        .await?;
    let remote_selection = TurnEnvironmentSelection {
        environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
        cwd: PathUri::from_abs_path(&remote_cwd),
        workspace_roots: vec![PathUri::from_abs_path(&remote_cwd)],
        config: EnvironmentConfigState::FromThread,
    };
    let multi_env_output = exec_command_routing_output(
        &test,
        &server,
        "call-multi-env",
        json!({
            "shell": "/bin/sh",
            "cmd": format!("cat {remote_marker_name}"),
            "login": false,
            "yield_time_ms": 1_000,
            "environment_id": REMOTE_ENVIRONMENT_ID,
        }),
        Some(vec![local_selection, remote_selection]),
    )
    .await?;
    assert!(
        multi_env_output.contains("remote-routing"),
        "unexpected multi-env output: {multi_env_output}",
    );
    assert!(
        !multi_env_output.contains("local-routing"),
        "multi-env command should not route to local: {multi_env_output}",
    );

    test.fs()
        .remove(
            &remote_cwd_uri,
            RemoveOptions {
                recursive: true,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_exec_materializes_target_roots_before_sandbox_selection() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_target_windows!(
        Ok(()),
        "sandboxed process launch is not supported by the exec-server Windows backend"
    );
    skip_if_no_remote_env!(Ok(()));

    const SECRET: &str = "target-root-secret";
    const SECRET_FILE: &str = "secret.txt";

    let server = start_mock_server().await;
    let test = unified_exec_test(&server).await?;
    let local_cwd = TempDir::new()?;
    let remote_cwd = PathBuf::from(format!(
        "/tmp/codex-remote-target-roots-{}",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
    ))
    .abs();
    let remote_cwd_uri = PathUri::from_abs_path(&remote_cwd);
    test.fs()
        .create_directory(
            &remote_cwd_uri,
            CreateDirectoryOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await?;
    test.fs()
        .write_file(
            &remote_cwd_uri.join(SECRET_FILE)?,
            SECRET.as_bytes().to_vec(),
            /*sandbox*/ None,
        )
        .await?;

    let call_id = "remote-target-root-sandbox";
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    call_id,
                    "exec_command",
                    &json!({
                        "shell": "bash",
                        "cmd": format!("cat {SECRET_FILE}"),
                        "login": false,
                        "yield_time_ms": 1_000,
                        "environment_id": REMOTE_ENVIRONMENT_ID,
                        "sandbox_permissions": SandboxPermissions::RequireEscalated,
                    })
                    .to_string(),
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;
    let permission_profile = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                access: FileSystemAccessMode::Read,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
                },
                access: FileSystemAccessMode::Deny,
                missing_path_behavior: None,
            },
        ]),
        NetworkSandboxPolicy::Restricted,
    );
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(permission_profile, test.config.cwd.as_path());
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "try to read the denied remote workspace root".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(TurnEnvironmentSelections::new(
                    test.config.cwd.clone(),
                    vec![
                        TurnEnvironmentSelection {
                            environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
                            cwd: PathUri::from_abs_path(&local_cwd.path().abs()),
                            workspace_roots: Vec::new(),
                            config: EnvironmentConfigState::FromThread,
                        },
                        TurnEnvironmentSelection {
                            environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                            cwd: remote_cwd_uri.clone(),
                            workspace_roots: vec![remote_cwd_uri.clone()],
                            config: EnvironmentConfigState::FromThread,
                        },
                    ],
                )),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(CollaborationMode {
                    mode: ModeKind::Default,
                    settings: Settings {
                        model: test.session_configured.model.clone(),
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            }),
        )
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let request = response_mock
        .last_request()
        .context("model should receive the denied remote command output")?;
    let (output, success) = request
        .function_call_output_content_and_success(call_id)
        .context("remote command output should be model visible")?;
    assert_ne!(success, Some(true));
    assert!(
        output.is_none_or(|output| !output.contains(SECRET)),
        "denied remote workspace contents should not be readable"
    );

    test.fs()
        .remove(
            &remote_cwd_uri,
            RemoveOptions {
                recursive: true,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_request_permissions_grant_unblocks_later_remote_exec() -> Result<()> {
    skip_if_no_network!(Ok(()));
    // TODO(anp): Remove after remote path fixtures use target-native paths.
    skip_if_target_windows!(Ok(()), "requires the Docker-backed POSIX executor");
    skip_if_no_remote_env!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.use_experimental_unified_exec_tool = true;
        config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
        config.approvals_reviewer = ApprovalsReviewer::User;
        config
            .features
            .enable(Feature::UnifiedExec)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::ExecPermissionApprovals)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::RequestPermissionsTool)
            .expect("test config should allow feature update");
    });
    let test = builder.build_with_remote_and_local_env(&server).await?;

    let local_cwd = TempDir::new()?;
    let remote_cwd = PathBuf::from(format!(
        "/tmp/codex-remote-request-permissions-{}",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
    ))
    .abs();
    let relative_write_root = "granted";
    let relative_target_path = "granted/request-permissions-output.txt";
    let remote_write_root = remote_cwd.join(relative_write_root);
    let remote_target_path = remote_cwd.join(relative_target_path);
    let local_write_root = local_cwd.path().join(relative_write_root);
    let local_target_path = local_cwd.path().join(relative_target_path);
    fs::create_dir(&local_write_root)?;
    let remote_write_root_uri = PathUri::from_host_native_path(&remote_write_root)?;
    test.fs()
        .create_directory(
            &remote_write_root_uri,
            CreateDirectoryOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await?;

    let expected_permissions = RequestPermissionProfile {
        file_system: Some(FileSystemPermissions::from_read_write_roots(
            Some(vec![]),
            Some(vec![remote_write_root.clone()]),
        )),
        ..RequestPermissionProfile::default()
    };
    let approved_response = RequestPermissionsResponse {
        permissions: expected_permissions.clone(),
        scope: PermissionGrantScope::Turn,
        strict_auto_review: false,
    };
    let command = format!(
        "printf 'remote-request-permissions-ok' > {relative_target_path} && cat {relative_target_path}"
    );
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-request-permissions-remote-1"),
                ev_function_call(
                    "permissions-call",
                    "request_permissions",
                    &json!({
                        "environment_id": REMOTE_ENVIRONMENT_ID,
                        "reason": "Allow writing inside the selected remote environment",
                        "permissions": {
                            "file_system": {
                                "write": [relative_write_root],
                            },
                        },
                    })
                    .to_string(),
                ),
                ev_completed("resp-request-permissions-remote-1"),
            ]),
            sse(vec![
                ev_response_created("resp-request-permissions-remote-2"),
                ev_function_call(
                    "exec-call",
                    "exec_command",
                    &json!({
                        "shell": "/bin/sh",
                        "cmd": command,
                        "login": false,
                        "yield_time_ms": 1_000,
                        "environment_id": REMOTE_ENVIRONMENT_ID,
                    })
                    .to_string(),
                ),
                ev_completed("resp-request-permissions-remote-2"),
            ]),
            sse(vec![
                ev_response_created("resp-request-permissions-remote-3"),
                ev_assistant_message("msg-request-permissions-remote-1", "done"),
                ev_completed("resp-request-permissions-remote-3"),
            ]),
        ],
    )
    .await;

    submit_turn_with_approval_and_environments(
        &test,
        "request permissions, then write in the remote environment",
        vec![
            local(local_cwd.path().abs()),
            TurnEnvironmentSelection {
                environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                cwd: PathUri::from_abs_path(&remote_cwd),
                workspace_roots: vec![PathUri::from_abs_path(&remote_cwd)],
                config: EnvironmentConfigState::FromThread,
            },
        ],
        AskForApproval::OnRequest,
    )
    .await?;

    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::RequestPermissions(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;
    let EventMsg::RequestPermissions(request) = event else {
        panic!("expected remote request_permissions before completion: {event:?}");
    };
    assert_eq!(request.call_id, "permissions-call");
    assert_eq!(
        request.environment_id.as_deref(),
        Some(REMOTE_ENVIRONMENT_ID)
    );
    assert_eq!(request.cwd.as_ref(), Some(&remote_cwd));
    assert_eq!(request.permissions, expected_permissions);

    test.codex
        .submit(Op::RequestPermissionsResponse {
            id: "permissions-call".to_string(),
            response: approved_response.clone(),
        })
        .await?;

    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ExecApprovalRequest(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;
    match event {
        EventMsg::TurnComplete(_) => {}
        EventMsg::ExecApprovalRequest(approval) => {
            panic!("remote request_permissions grant should preapprove exec: {approval:?}");
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let permissions_output: RequestPermissionsResponse = serde_json::from_str(
        &response_mock
            .function_call_output_text("permissions-call")
            .expect("expected request_permissions output"),
    )?;
    assert_eq!(permissions_output, approved_response);
    let exec_output = response_mock
        .function_call_output_text("exec-call")
        .expect("expected exec output");
    assert!(
        exec_output.contains("remote-request-permissions-ok"),
        "unexpected exec output: {exec_output}",
    );
    assert_eq!(
        test.fs()
            .read_file_text(
                &PathUri::from_host_native_path(&remote_target_path)?,
                /*sandbox*/ None,
            )
            .await?,
        "remote-request-permissions-ok"
    );
    assert!(
        !local_target_path.exists(),
        "remote exec should not write through the local environment"
    );

    test.fs()
        .remove(
            &PathUri::from_abs_path(&remote_cwd),
            RemoveOptions {
                recursive: true,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_freeform_routes_to_selected_remote_environment() -> Result<()> {
    skip_if_no_network!(Ok(()));
    // TODO(anp): Remove after remote path fixtures use target-native paths.
    skip_if_target_windows!(Ok(()), "requires the Docker-backed POSIX executor");
    skip_if_no_remote_env!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex();
    let test = builder.build_with_remote_and_local_env(&server).await?;
    let local_cwd = TempDir::new()?;
    let file_name = "apply_patch_remote_freeform.txt";
    let remote_cwd = PathBuf::from(format!(
        "/tmp/codex-remote-apply-patch-freeform-{}",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
    ))
    .abs();
    let remote_cwd_uri = PathUri::from_host_native_path(&remote_cwd)?;
    test.fs()
        .create_directory(
            &remote_cwd_uri,
            CreateDirectoryOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await?;

    let patch = format!(
        "*** Begin Patch\n*** Environment ID: {REMOTE_ENVIRONMENT_ID}\n*** Add File: {file_name}\n+patched remote freeform\n*** End Patch"
    );
    let call_id = "apply-patch-remote-freeform";
    mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_apply_patch_custom_tool_call(call_id, &patch),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    test.submit_turn_with_environments(
        "apply patch to remote environment",
        Some(vec![
            local(local_cwd.path().abs()),
            TurnEnvironmentSelection {
                environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                cwd: PathUri::from_abs_path(&remote_cwd),
                workspace_roots: vec![PathUri::from_abs_path(&remote_cwd)],
                config: EnvironmentConfigState::FromThread,
            },
        ]),
    )
    .await?;

    let remote_contents = test
        .fs()
        .read_file_text(
            &PathUri::from_host_native_path(remote_cwd.join(file_name))?,
            /*sandbox*/ None,
        )
        .await?;
    assert_eq!(remote_contents, "patched remote freeform\n");
    assert!(
        !local_cwd.path().join(file_name).exists(),
        "freeform apply_patch should not create the file in the local environment"
    );

    test.fs()
        .remove(
            &remote_cwd_uri,
            RemoveOptions {
                recursive: true,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_approvals_are_remembered_per_environment() -> Result<()> {
    skip_if_no_network!(Ok(()));
    // TODO(anp): Remove after remote path fixtures use target-native paths.
    skip_if_target_windows!(Ok(()), "requires the Docker-backed POSIX executor");
    skip_if_no_remote_env!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
        config.approvals_reviewer = ApprovalsReviewer::User;
    });
    let test = builder.build_with_remote_and_local_env(&server).await?;
    let local_cwd = TempDir::new()?;
    let remote_cwd = PathBuf::from(format!(
        "/tmp/codex-remote-apply-patch-approval-cwd-{}",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
    ))
    .abs();
    let remote_cwd_uri = PathUri::from_host_native_path(&remote_cwd)?;
    test.fs()
        .create_directory(
            &remote_cwd_uri,
            CreateDirectoryOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await?;

    let target_path = PathBuf::from(format!(
        "/tmp/codex-apply-patch-approval-scope-{}.txt",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
    ))
    .abs();
    let target_path_uri = PathUri::from_host_native_path(&target_path)?;
    let _ = fs::remove_file(&target_path);
    test.fs()
        .remove(
            &target_path_uri,
            RemoveOptions {
                recursive: false,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;

    let environments = vec![
        local(local_cwd.path().abs()),
        TurnEnvironmentSelection {
            environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
            cwd: PathUri::from_abs_path(&remote_cwd),
            workspace_roots: vec![PathUri::from_abs_path(&remote_cwd)],
            config: EnvironmentConfigState::FromThread,
        },
    ];
    let local_patch = format!(
        "*** Begin Patch\n*** Environment ID: {LOCAL_ENVIRONMENT_ID}\n*** Add File: {}\n+local\n*** End Patch",
        target_path.display()
    );
    let remote_patch = format!(
        "*** Begin Patch\n*** Environment ID: {REMOTE_ENVIRONMENT_ID}\n*** Add File: {}\n+remote\n*** End Patch",
        target_path.display()
    );
    let remote_update_patch = format!(
        "*** Begin Patch\n*** Environment ID: {REMOTE_ENVIRONMENT_ID}\n*** Update File: {}\n@@\n-remote\n+remote updated\n*** End Patch",
        target_path.display()
    );

    mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-local-1"),
                ev_apply_patch_custom_tool_call("call-local", &local_patch),
                ev_completed("resp-local-1"),
            ]),
            sse(vec![
                ev_response_created("resp-local-2"),
                ev_assistant_message("msg-local", "done"),
                ev_completed("resp-local-2"),
            ]),
            sse(vec![
                ev_response_created("resp-remote-1"),
                ev_apply_patch_custom_tool_call("call-remote", &remote_patch),
                ev_completed("resp-remote-1"),
            ]),
            sse(vec![
                ev_response_created("resp-remote-2"),
                ev_assistant_message("msg-remote", "done"),
                ev_completed("resp-remote-2"),
            ]),
            sse(vec![
                ev_response_created("resp-remote-3"),
                ev_apply_patch_custom_tool_call("call-remote-followup", &remote_update_patch),
                ev_completed("resp-remote-3"),
            ]),
            sse(vec![
                ev_response_created("resp-remote-4"),
                ev_assistant_message("msg-remote-followup", "done"),
                ev_completed("resp-remote-4"),
            ]),
        ],
    )
    .await;

    submit_turn_with_approval_and_environments(
        &test,
        "apply patch in local environment",
        environments.clone(),
        AskForApproval::OnRequest,
    )
    .await?;
    let approval = expect_patch_approval(&test, "call-local").await;
    test.codex
        .submit(Op::PatchApproval {
            id: approval.call_id,
            decision: ReviewDecision::ApprovedForSession,
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(fs::read_to_string(&target_path)?, "local\n");

    submit_turn_with_approval_and_environments(
        &test,
        "apply patch in remote environment",
        environments.clone(),
        AskForApproval::OnRequest,
    )
    .await?;
    let approval = expect_patch_approval(&test, "call-remote").await;
    test.codex
        .submit(Op::PatchApproval {
            id: approval.call_id,
            decision: ReviewDecision::ApprovedForSession,
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(
        test.fs()
            .read_file_text(&target_path_uri, /*sandbox*/ None)
            .await?,
        "remote\n"
    );

    submit_turn_with_approval_and_environments(
        &test,
        "apply patch again in remote environment",
        environments,
        AskForApproval::OnRequest,
    )
    .await?;
    wait_for_completion_without_patch_approval(&test).await;
    assert_eq!(
        test.fs()
            .read_file_text(&target_path_uri, /*sandbox*/ None)
            .await?,
        "remote updated\n"
    );

    let _ = fs::remove_file(&target_path);
    test.fs()
        .remove(
            &target_path_uri,
            RemoveOptions {
                recursive: false,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;
    test.fs()
        .remove(
            &remote_cwd_uri,
            RemoveOptions {
                recursive: true,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_intercepted_exec_command_routes_to_selected_remote_environment() -> Result<()>
{
    skip_if_no_network!(Ok(()));
    // TODO(anp): Remove after remote path fixtures use target-native paths.
    skip_if_target_windows!(Ok(()), "requires the Docker-backed POSIX executor");
    skip_if_no_remote_env!(Ok(()));

    let server = start_mock_server().await;
    let test = unified_exec_test(&server).await?;
    let local_cwd = TempDir::new()?;
    let file_name = "apply_patch_remote_exec.txt";
    let remote_cwd = PathBuf::from(format!(
        "/tmp/codex-remote-apply-patch-exec-{}",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
    ))
    .abs();
    let remote_cwd_uri = PathUri::from_host_native_path(&remote_cwd)?;
    test.fs()
        .create_directory(
            &remote_cwd_uri,
            CreateDirectoryOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await?;

    let patch =
        format!("*** Begin Patch\n*** Add File: {file_name}\n+patched remote exec\n*** End Patch");
    let command = format!("apply_patch <<'EOF'\n{patch}\nEOF\n");
    let call_id = "apply-patch-remote-exec";
    mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    call_id,
                    "exec_command",
                    &serde_json::to_string(&json!({
                        "shell": "/bin/sh",
                        "cmd": command,
                        "login": false,
                        "yield_time_ms": 5_000,
                        "environment_id": REMOTE_ENVIRONMENT_ID,
                    }))?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    test.submit_turn_with_environments(
        "apply patch through exec command to remote environment",
        Some(vec![
            local(local_cwd.path().abs()),
            TurnEnvironmentSelection {
                environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                cwd: PathUri::from_abs_path(&remote_cwd),
                workspace_roots: vec![PathUri::from_abs_path(&remote_cwd)],
                config: EnvironmentConfigState::FromThread,
            },
        ]),
    )
    .await?;

    let remote_contents = test
        .fs()
        .read_file_text(
            &PathUri::from_host_native_path(remote_cwd.join(file_name))?,
            /*sandbox*/ None,
        )
        .await?;
    assert_eq!(remote_contents, "patched remote exec\n");
    assert!(
        !local_cwd.path().join(file_name).exists(),
        "intercepted apply_patch should not create the file in the local environment"
    );

    test.fs()
        .remove(
            &remote_cwd_uri,
            RemoveOptions {
                recursive: true,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_test_env_sandboxed_read_allows_readable_root() -> Result<()> {
    // TODO(anp): Remove after remote sandbox fixtures use target-native paths.
    skip_if_target_windows!(Ok(()), "requires the Docker-backed POSIX executor");
    skip_if_no_network!(Ok(()));
    skip_if_no_remote_env!(Ok(()));

    let test_env = test_env().await?;
    let file_system = test_env.environment().get_filesystem();

    let allowed_dir = PathBuf::from(format!("/tmp/codex-remote-readable-{}", std::process::id()));
    let file_path = allowed_dir.join("note.txt");
    let allowed_dir_uri = PathUri::from_host_native_path(&allowed_dir)?;
    let file_path_uri = PathUri::from_host_native_path(&file_path)?;
    file_system
        .create_directory(
            &allowed_dir_uri,
            CreateDirectoryOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await?;
    file_system
        .write_file(
            &file_path_uri,
            b"sandboxed hello".to_vec(),
            /*sandbox*/ None,
        )
        .await?;

    let sandbox = read_only_sandbox(allowed_dir.clone());
    let contents = file_system
        .read_file(&file_path_uri, Some(&sandbox))
        .await?;
    assert_eq!(contents, b"sandboxed hello");

    file_system
        .remove(
            &allowed_dir_uri,
            RemoveOptions {
                recursive: true,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_test_env_sandboxed_read_rejects_symlink_parent_dotdot_escape() -> Result<()> {
    skip_if_target_windows!(Ok(()), "tests POSIX symlink and parent traversal semantics");
    skip_if_no_network!(Ok(()));
    skip_if_no_remote_env!(Ok(()));

    let test_env = test_env().await?;
    let file_system = test_env.environment().get_filesystem();

    let root = PathBuf::from(format!("/tmp/codex-remote-dotdot-{}", std::process::id()));
    let allowed_dir = root.join("allowed");
    let outside_dir = root.join("outside");
    let secret_path = root.join("secret.txt");
    remote_exec(&format!(
        "rm -rf {root}; mkdir -p {allowed} {outside}; printf nope > {secret}; ln -s {outside} {allowed}/link",
        root = root.display(),
        allowed = allowed_dir.display(),
        outside = outside_dir.display(),
        secret = secret_path.display(),
    ))?;

    let requested_path =
        PathUri::from_host_native_path(allowed_dir.join("link").join("..").join("secret.txt"))?;
    let sandbox = read_only_sandbox(allowed_dir.clone());
    let error = match file_system.read_file(&requested_path, Some(&sandbox)).await {
        Ok(_) => anyhow::bail!("read should fail after path normalization"),
        Err(error) => error,
    };
    assert_normalized_path_rejected(&error);

    remote_exec(&format!("rm -rf {}", root.display()))?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_test_env_remove_removes_symlink_not_target() -> Result<()> {
    skip_if_target_windows!(Ok(()), "tests POSIX symlink removal semantics");
    skip_if_no_network!(Ok(()));
    skip_if_no_remote_env!(Ok(()));

    let test_env = test_env().await?;
    let file_system = test_env.environment().get_filesystem();

    let root = PathBuf::from(format!(
        "/tmp/codex-remote-remove-link-{}",
        std::process::id()
    ));
    let allowed_dir = root.join("allowed");
    let outside_file = root.join("outside").join("keep.txt");
    let symlink_path = allowed_dir.join("link");
    remote_exec(&format!(
        "rm -rf {root}; mkdir -p {allowed} {outside_parent}; printf outside > {outside}; ln -s {outside} {symlink}",
        root = root.display(),
        allowed = allowed_dir.display(),
        outside_parent = absolute_path(
            outside_file
                .parent()
                .context("outside parent should exist")?
                .to_path_buf(),
        )
        .display(),
        outside = outside_file.display(),
        symlink = symlink_path.display(),
    ))?;

    let sandbox = workspace_write_sandbox(allowed_dir.clone());
    file_system
        .remove(
            &PathUri::from_host_native_path(&symlink_path)?,
            RemoveOptions {
                recursive: false,
                force: false,
            },
            Some(&sandbox),
        )
        .await?;

    let symlink_exists = file_system
        .get_metadata(
            &PathUri::from_abs_path(&absolute_path(symlink_path)),
            /*sandbox*/ None,
        )
        .await
        .is_ok();
    assert!(!symlink_exists);
    let outside = file_system
        .read_file_text(
            &PathUri::from_host_native_path(&outside_file)?,
            /*sandbox*/ None,
        )
        .await?;
    assert_eq!(outside, "outside");

    file_system
        .remove(
            &PathUri::from_host_native_path(&root)?,
            RemoveOptions {
                recursive: true,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_test_env_copy_preserves_symlink_source() -> Result<()> {
    skip_if_target_windows!(Ok(()), "tests POSIX symlink copy semantics");
    skip_if_no_network!(Ok(()));
    skip_if_no_remote_env!(Ok(()));

    let test_env = test_env().await?;
    let file_system = test_env.environment().get_filesystem();

    let root = PathBuf::from(format!(
        "/tmp/codex-remote-copy-link-{}",
        std::process::id()
    ));
    let allowed_dir = root.join("allowed");
    let outside_file = root.join("outside").join("outside.txt");
    let source_symlink = allowed_dir.join("link");
    let copied_symlink = allowed_dir.join("copied-link");
    remote_exec(&format!(
        "rm -rf {root}; mkdir -p {allowed} {outside_parent}; printf outside > {outside}; ln -s {outside} {source}",
        root = root.display(),
        allowed = allowed_dir.display(),
        outside_parent = outside_file.parent().expect("outside parent").display(),
        outside = outside_file.display(),
        source = source_symlink.display(),
    ))?;

    let sandbox = workspace_write_sandbox(allowed_dir.clone());
    file_system
        .copy(
            &PathUri::from_host_native_path(&source_symlink)?,
            &PathUri::from_host_native_path(&copied_symlink)?,
            CopyOptions { recursive: false },
            Some(&sandbox),
        )
        .await?;

    let container_name = test_docker_container_name()
        .context("test requires direct access to the Docker container")?;
    let link_target = Command::new("docker")
        .args([
            "exec",
            container_name.as_str(),
            "readlink",
            copied_symlink
                .to_str()
                .context("copied symlink path should be utf-8")?,
        ])
        .output()?;
    assert!(
        link_target.status.success(),
        "readlink failed: stdout={} stderr={}",
        String::from_utf8_lossy(&link_target.stdout).trim(),
        String::from_utf8_lossy(&link_target.stderr).trim(),
    );
    assert_eq!(
        String::from_utf8_lossy(&link_target.stdout).trim(),
        outside_file.to_string_lossy()
    );

    file_system
        .remove(
            &PathUri::from_host_native_path(&root)?,
            RemoveOptions {
                recursive: true,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;
    Ok(())
}
