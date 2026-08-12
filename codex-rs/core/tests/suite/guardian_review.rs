#![cfg(not(target_os = "windows"))]

use anyhow::Context;
use anyhow::Result;
use chrono::DateTime;
use chrono::Local;
use chrono::Utc;
use codex_core::SleepFuture;
use codex_core::TimeFuture;
use codex_core::TimeProvider;
use codex_core::TurnInputRequest;
use codex_core::config::Constrained;
use codex_core::config::CurrentTimeReminderConfig;
use codex_core::sandboxing::SandboxPermissions;
use codex_features::CurrentTimeSource;
use codex_features::Feature;
use codex_history::RolloutItem;
use codex_history::RolloutLine;
use codex_login::CodexAuth;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::openai_models::MODEL_SPECIALTY_CYBER;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::protocol::TurnAbortReason;
use codex_protocol::user_input::UserInput;
use core_test_support::fs_wait;
use core_test_support::responses::assert_parent_turn;
use core_test_support::responses::assert_root_turn;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_custom_tool_call;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_response_once_match;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::sse_response;
use core_test_support::responses::start_mock_server;
use core_test_support::responses::start_websocket_server;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_sandbox;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::local_selections;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tempfile::TempDir;
use test_case::test_case;

const CURRENT_TIME_AT: i64 = 1_781_717_655;

struct RecordingTimeProvider {
    thread_ids: Mutex<Vec<ThreadId>>,
}

impl TimeProvider for RecordingTimeProvider {
    fn current_time(&self, thread_id: ThreadId) -> TimeFuture<'_> {
        self.thread_ids
            .lock()
            .expect("time-provider thread ids lock should not be poisoned")
            .push(thread_id);
        Box::pin(async {
            Ok(DateTime::<Utc>::from_timestamp(CURRENT_TIME_AT, 0)
                .expect("test timestamp should be valid"))
        })
    }

    fn sleep(&self, _thread_id: ThreadId, _duration: Duration) -> SleepFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(CodexAuth::from_api_key("test-api-key"), "gpt-5.6-luna"; "api_key_uses_luna_with_responses_lite")]
#[test_case(CodexAuth::create_dummy_chatgpt_auth_for_testing(), "codex-auto-review"; "chatgpt_uses_codex_auto_review")]
async fn guardian_session_prewarms_and_is_reused_for_first_review(
    auth: CodexAuth,
    expected_model: &str,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let bundled_models = codex_models_manager::bundled_models_response()?.models;
    let catalog_auto_review = bundled_models
        .iter()
        .find(|model| model.slug == "codex-auto-review")
        .and_then(|model| model.model_messages.as_ref())
        .and_then(|messages| messages.auto_review.as_ref())
        .expect("bundled auto-review model Guardian policy");
    let catalog_policy = catalog_auto_review
        .policy
        .as_deref()
        .expect("catalog Guardian policy");
    let catalog_template = catalog_auto_review
        .policy_template
        .as_deref()
        .expect("catalog Guardian policy template");
    let expected_guardian_policy =
        catalog_template.replace("{{ tenant_policy_config }}", catalog_policy.trim());
    let review_model = bundled_models
        .into_iter()
        .find(|model| model.slug == expected_model)
        .expect("bundled Guardian review model");
    let use_responses_lite = review_model.use_responses_lite;
    if expected_model == "gpt-5.6-luna" {
        assert!(use_responses_lite, "Luna must use Responses Lite");
        assert!(
            review_model
                .model_messages
                .as_ref()
                .and_then(|messages| messages.auto_review.as_ref())
                .is_none(),
            "Luna must exercise the bundled Guardian policy fallback"
        );
    }

    let tool_args = json!({
        "cmd": "true",
        "sandbox_permissions": SandboxPermissions::RequireEscalated,
        "justification": "Exercise Guardian approval routing.",
    })
    .to_string();
    let server = start_websocket_server(vec![
        vec![vec![ev_response_created("warm-1"), ev_completed("warm-1")]],
        vec![vec![ev_response_created("warm-2"), ev_completed("warm-2")]],
        vec![vec![
            ev_response_created("approval-request"),
            ev_function_call("approval-call", "exec_command", &tool_args),
            ev_completed("approval-request"),
        ]],
        vec![vec![
            ev_response_created("guardian-review"),
            ev_assistant_message(
                "guardian-assessment",
                &json!({
                    "risk_level": "low",
                    "user_authorization": "high",
                    "outcome": "allow",
                    "rationale": "The command is safe to execute.",
                })
                .to_string(),
            ),
            ev_completed("guardian-review"),
        ]],
    ])
    .await;
    let time_provider = Arc::new(RecordingTimeProvider {
        thread_ids: Mutex::new(Vec::new()),
    });
    let mut builder = test_codex()
        .with_auth(auth)
        .with_config(move |config| {
            let rules_dir = config.codex_home.join("rules");
            fs::create_dir_all(&rules_dir).expect("create execution policy directory");
            let policy_justification = format!(
                "Explicit policy approval required {} policy-justification-end",
                "x".repeat(10_000)
            );
            let policy_justification =
                serde_json::to_string(&policy_justification).expect("serialize policy justification");
            fs::write(
                rules_dir.join("default.rules"),
                format!(
                    r#"prefix_rule(pattern=["true"], decision="prompt", justification={policy_justification})"#
                ),
            )
            .expect("write execution policy rule");
            config.model_catalog = Some(ModelsResponse {
                models: vec![review_model],
            });
            config.model_context_window = Some(900_000);
            config.model_auto_compact_token_limit = Some(600_000);
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config
                .features
                .enable(Feature::CurrentTimeReminder)
                .expect("test config should allow current-time reminders");
            config.current_time_reminder = Some(CurrentTimeReminderConfig {
                clock_source: CurrentTimeSource::External,
                ..CurrentTimeReminderConfig::default()
            });
        })
        .with_external_time_provider(time_provider.clone());

    let test = builder.build_with_websocket_server(&server).await?;
    let root_thread_id = test.session_configured.thread_id;
    let (first, second) = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::join!(
            server.wait_for_request(/*connection_index*/ 0, /*request_index*/ 0),
            server.wait_for_request(/*connection_index*/ 1, /*request_index*/ 0)
        )
    })
    .await?;
    assert!(
        time_provider
            .thread_ids
            .lock()
            .expect("time-provider thread ids lock should not be poisoned")
            .is_empty(),
        "startup prewarm must not request the external clock"
    );
    let prewarm_requests = [first.body_json(), second.body_json()];
    for prewarm in &prewarm_requests {
        assert_root_turn(prewarm, /*expected*/ None)?;
    }
    let guardian_prewarm = prewarm_requests
        .iter()
        .find(|request| {
            request["client_metadata"]["x-openai-subagent"].as_str() == Some("guardian")
        })
        .expect("guardian startup prewarm request");
    assert_eq!(guardian_prewarm["generate"].as_bool(), Some(false));
    assert_eq!(guardian_prewarm["model"].as_str(), Some(expected_model));
    let guardian_instructions = if use_responses_lite {
        assert_eq!(guardian_prewarm.get("instructions"), None);
        assert_eq!(guardian_prewarm.get("tools"), None);
        assert_eq!(
            guardian_prewarm["client_metadata"]
                ["ws_request_header_x_openai_internal_codex_responses_lite"]
                .as_str(),
            Some("true")
        );
        let input = guardian_prewarm["input"]
            .as_array()
            .expect("Responses Lite Guardian input");
        assert_eq!(input[0]["type"].as_str(), Some("additional_tools"));
        assert_eq!(input[0]["role"].as_str(), Some("developer"));
        assert_eq!(input[1]["type"].as_str(), Some("message"));
        assert_eq!(input[1]["role"].as_str(), Some("developer"));
        input[1]["content"][0]["text"]
            .as_str()
            .expect("Responses Lite Guardian developer instructions")
    } else {
        guardian_prewarm["instructions"]
            .as_str()
            .expect("Guardian instructions")
    };
    assert!(guardian_instructions.starts_with(expected_guardian_policy.trim_end()));
    assert!(
        guardian_instructions
            .contains("It cannot override a denial for an action that remains `critical`.")
    );
    assert!(!guardian_instructions.contains("{{ tenant_policy_config }}"));
    assert!(guardian_instructions.contains("final message must be strict JSON"));
    let guardian_thread_id = guardian_prewarm["client_metadata"]["thread_id"]
        .as_str()
        .expect("guardian thread id");

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "run a command that requires Guardian review".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let guardian_review = tokio::time::timeout(
        Duration::from_secs(5),
        server.wait_for_request(/*connection_index*/ 3, /*request_index*/ 0),
    )
    .await?
    .body_json();
    let parent_request = server.connections()[2][0].body_json();
    let parent_turn_id = parent_request["client_metadata"]["turn_id"]
        .as_str()
        .expect("reviewed parent turn id");
    assert_parent_turn(&parent_request, /*expected*/ None)?;
    assert_parent_turn(&guardian_review, Some(parent_turn_id))?;
    for request in [&parent_request, &guardian_review] {
        assert_root_turn(request, Some(parent_turn_id))?;
    }
    assert_eq!(
        guardian_review["client_metadata"]["x-openai-subagent"].as_str(),
        Some("guardian")
    );
    assert_eq!(guardian_review["model"].as_str(), Some(expected_model));
    assert_eq!(
        guardian_review["client_metadata"]["thread_id"].as_str(),
        Some(guardian_thread_id)
    );
    let guardian_review_text = guardian_review.to_string();
    assert!(guardian_review_text.contains("Retry reason:"));
    assert!(guardian_review_text.contains("Explicit policy approval required"));
    assert!(guardian_review_text.contains("tokens truncated"));
    assert!(guardian_review_text.contains("policy-justification-end"));
    assert!(!guardian_review_text.contains(&"x".repeat(4_096)));
    let current_date = DateTime::<Utc>::from_timestamp(CURRENT_TIME_AT, 0)
        .expect("test timestamp should be valid")
        .with_timezone(&Local)
        .format("%Y-%m-%d")
        .to_string();
    assert!(
        guardian_review
            .to_string()
            .contains(&format!("<current_date>{current_date}</current_date>")),
        "guardian's environment context should use the simulated current date"
    );
    let guardian_thread_id = ThreadId::from_string(guardian_thread_id)?;
    {
        let thread_ids = time_provider
            .thread_ids
            .lock()
            .expect("time-provider thread ids lock should not be poisoned");
        assert!(thread_ids.contains(&root_thread_id));
        assert!(thread_ids.contains(&guardian_thread_id));
        assert!(
            thread_ids
                .iter()
                .all(|thread_id| thread_id == &root_thread_id || thread_id == &guardian_thread_id),
            "clock requests should use the corresponding agent's own thread id: {thread_ids:?}"
        );
    }
    assert_eq!(guardian_review.get("generate"), None);

    let guardian_rollout_path = test
        .codex
        .guardian_trunk_rollout_path()
        .await
        .expect("guardian trunk rollout path");
    test.codex.shutdown_and_wait().await?;
    let guardian_context_windows = fs::read_to_string(guardian_rollout_path)?
        .lines()
        .map(serde_json::from_str::<RolloutLine>)
        .collect::<serde_json::Result<Vec<_>>>()?
        .into_iter()
        .filter_map(|line| match line.item {
            RolloutItem::EventMsg(EventMsg::TurnStarted(event)) => Some(event.model_context_window),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(guardian_context_windows, vec![Some(258_400)]);
    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_session_is_reused_for_consecutive_tool_reviews_without_prewarm() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));
    skip_if_wine_exec!(
        Ok(()),
        "Guardian approval actions require host-native paths"
    );

    let server = start_mock_server().await;
    let approval_policy = AskForApproval::OnRequest;
    let sandbox_policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![],
        network_access: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    };
    let sandbox_policy_for_config = sandbox_policy.clone();

    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::allow_any(approval_policy);
        config.approvals_reviewer = ApprovalsReviewer::AutoReview;
        config
            .set_legacy_sandbox_policy(sandbox_policy_for_config)
            .expect("set sandbox policy");
    });
    let test = builder.build_with_auto_env(&server).await?;

    let first_output_file = test.cwd.path().join("guardian-first.txt");
    let second_output_file = test.cwd.path().join("guardian-second.txt");
    let first_command = format!("printf first > {}", first_output_file.display());
    let second_command = format!("printf second > {}", second_output_file.display());
    let first_tool_args = json!({
        "cmd": first_command,
        "yield_time_ms": 1_000_u64,
        "sandbox_permissions": SandboxPermissions::RequireEscalated,
        "justification": "Exercise the first Guardian approval.",
    });
    let second_tool_args = json!({
        "cmd": second_command,
        "yield_time_ms": 1_000_u64,
        "sandbox_permissions": SandboxPermissions::RequireEscalated,
        "justification": "Exercise the second Guardian approval.",
    });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-parent-first-tool"),
                ev_function_call(
                    "exec-call-first",
                    "exec_command",
                    &serde_json::to_string(&first_tool_args)?,
                ),
                ev_completed("resp-parent-first-tool"),
            ]),
            sse(vec![
                ev_response_created("resp-guardian-first-review"),
                ev_assistant_message(
                    "msg-guardian-first-review",
                    &json!({
                        "risk_level": "low",
                        "user_authorization": "high",
                        "outcome": "allow",
                        "rationale": "The first command writes a workspace marker.",
                    })
                    .to_string(),
                ),
                ev_completed("resp-guardian-first-review"),
            ]),
            sse(vec![
                ev_response_created("resp-parent-second-tool"),
                ev_function_call(
                    "exec-call-second",
                    "exec_command",
                    &serde_json::to_string(&second_tool_args)?,
                ),
                ev_completed("resp-parent-second-tool"),
            ]),
            sse(vec![
                ev_response_created("resp-guardian-second-review"),
                ev_assistant_message(
                    "msg-guardian-second-review",
                    &json!({
                        "risk_level": "low",
                        "user_authorization": "high",
                        "outcome": "allow",
                        "rationale": "The second command writes a workspace marker.",
                    })
                    .to_string(),
                ),
                ev_completed("resp-guardian-second-review"),
            ]),
            sse(vec![
                ev_response_created("resp-parent-done"),
                ev_assistant_message("msg-parent-done", "done"),
                ev_completed("resp-parent-done"),
            ]),
        ],
    )
    .await;

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "run two commands that require Guardian review".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(local_selections(test.config.cwd.clone())),
                approval_policy: Some(approval_policy),
                approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
                sandbox_policy: Some(sandbox_policy),
                ..Default::default()
            }),
        )
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let guardian_requests = responses
        .requests()
        .into_iter()
        .filter(|request| {
            request.body_json()["client_metadata"]["x-openai-subagent"].as_str() == Some("guardian")
        })
        .collect::<Vec<_>>();
    assert_eq!(guardian_requests.len(), 2);
    let first_guardian_request = guardian_requests[0].body_json();
    let second_guardian_request = guardian_requests[1].body_json();
    let parent_request = responses.requests()[0].body_json();
    let parent_turn_id = parent_request["client_metadata"]["turn_id"]
        .as_str()
        .expect("reviewed parent turn id");
    for request in [&first_guardian_request, &second_guardian_request] {
        assert_parent_turn(request, Some(parent_turn_id))?;
        assert_root_turn(request, Some(parent_turn_id))?;
    }
    let first_guardian_thread_id = first_guardian_request["client_metadata"]["thread_id"]
        .as_str()
        .expect("first Guardian review should have a thread id");
    let second_guardian_thread_id = second_guardian_request["client_metadata"]["thread_id"]
        .as_str()
        .expect("second Guardian review should have a thread id");
    assert_eq!(first_guardian_thread_id, second_guardian_thread_id);
    assert_eq!(fs::read_to_string(first_output_file)?, "first");
    assert_eq!(fs::read_to_string(second_output_file)?, "second");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_review_of_code_mode_exec_command_includes_outer_source() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));
    skip_if_wine_exec!(
        Ok(()),
        "Guardian approval actions require host-native paths"
    );

    let server = start_mock_server().await;
    let mut builder = test_codex()
        .with_model("test-gpt-5.1-codex")
        .with_config(|config| {
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            let _ = config.features.enable(Feature::CodeMode);
        });
    let test = builder.build_with_auto_env(&server).await?;

    let code = r#"const outerContext = "guardian-code-mode-outer-context";
await tools.exec_command({cmd: "true", sandbox_permissions: "require_escalated", justification: "test"});"#;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_custom_tool_call("code-mode-call", "exec", code),
                ev_completed("resp-parent"),
            ]),
            sse(vec![
                ev_assistant_message("guardian", r#"{"outcome":"allow"}"#),
                ev_completed("resp-guardian"),
            ]),
            sse(vec![ev_completed("resp-done")]),
        ],
    )
    .await;

    let prompt = "run a nested command that requires Guardian review";
    test.submit_text_turn(prompt).await?;

    let transcript = responses.requests()[1].message_input_texts("user")[2..6].concat();
    assert_eq!(
        transcript,
        format!(
            ">>> TRANSCRIPT START\n\
             [1] user: {prompt}\n\
             \n\
             [2] tool exec call: {code}\n\
             >>> TRANSCRIPT END\n"
        )
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupted_guardian_tool_review_aborts_without_executing_the_command() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));
    skip_if_wine_exec!(
        Ok(()),
        "Guardian approval actions require host-native paths"
    );

    let server = start_mock_server().await;
    let approval_policy = AskForApproval::OnRequest;
    let sandbox_policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![],
        network_access: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    };
    let sandbox_policy_for_config = sandbox_policy.clone();

    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::allow_any(approval_policy);
        config.approvals_reviewer = ApprovalsReviewer::AutoReview;
        config
            .set_legacy_sandbox_policy(sandbox_policy_for_config)
            .expect("set sandbox policy");
    });
    let test = builder.build_with_auto_env(&server).await?;

    let output_file = test.cwd.path().join("guardian-interrupted.txt");
    let command = format!("printf should-not-run > {}", output_file.display());
    let tool_args = json!({
        "cmd": command,
        "yield_time_ms": 1_000_u64,
        "sandbox_permissions": SandboxPermissions::RequireEscalated,
        "justification": "Exercise interrupted Guardian approval.",
    });
    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-parent-interrupted-tool"),
            ev_function_call(
                "exec-call-interrupted",
                "exec_command",
                &serde_json::to_string(&tool_args)?,
            ),
            ev_completed("resp-parent-interrupted-tool"),
        ]),
    )
    .await;
    let pending_guardian = mount_response_once_match(
        &server,
        |request: &wiremock::Request| {
            serde_json::from_slice::<Value>(&request.body)
                .ok()
                .and_then(|body| {
                    body.pointer("/client_metadata/x-openai-subagent")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
                .as_deref()
                == Some("guardian")
        },
        sse_response(sse(vec![
            ev_response_created("resp-guardian-interrupted-review"),
            ev_assistant_message(
                "msg-guardian-interrupted-review",
                &json!({
                    "risk_level": "low",
                    "user_authorization": "high",
                    "outcome": "allow",
                    "rationale": "This review should be interrupted before it completes.",
                })
                .to_string(),
            ),
            ev_completed("resp-guardian-interrupted-review"),
        ]))
        .set_delay(Duration::from_millis(200)),
    )
    .await;

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "interrupt a Guardian-reviewed command".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(local_selections(test.config.cwd.clone())),
                approval_policy: Some(approval_policy),
                approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
                sandbox_policy: Some(sandbox_policy),
                ..Default::default()
            }),
        )
        .await?;

    tokio::time::timeout(Duration::from_secs(5), async {
        while pending_guardian.requests().is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .context("timed out waiting for Guardian review request")?;

    test.codex.submit(Op::Interrupt).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnAborted(_))
    })
    .await;
    tokio::time::sleep(Duration::from_millis(350)).await;
    assert!(
        !output_file.exists(),
        "the interrupted Guardian-reviewed command executed after its delayed approval response"
    );

    let follow_up = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-parent-after-interrupted-review"),
            ev_assistant_message("msg-parent-after-interrupted-review", "next turn completed"),
            ev_completed("resp-parent-after-interrupted-review"),
        ]),
    )
    .await;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "verify Guardian cancellation left the next turn clean".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let follow_up_request = follow_up.single_request();
    assert!(
        follow_up_request
            .body_contains_text("verify Guardian cancellation left the next turn clean")
    );
    assert!(follow_up_request.has_function_call("exec-call-interrupted"));
    let interrupted_output = follow_up_request
        .function_call_output_text("exec-call-interrupted")
        .expect("next turn should contain the interrupted command's tool output");
    assert!(
        interrupted_output.contains("aborted"),
        "unexpected interrupted tool output: {interrupted_output}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_denial_rejects_tool_call_with_rationale() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));
    skip_if_wine_exec!(
        Ok(()),
        "Guardian approval actions require host-native paths"
    );

    let server = start_mock_server().await;
    let approval_policy = AskForApproval::OnRequest;
    let sandbox_policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![],
        network_access: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    };
    let sandbox_policy_for_config = sandbox_policy.clone();

    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::allow_any(approval_policy);
        config
            .set_legacy_sandbox_policy(sandbox_policy_for_config)
            .expect("set sandbox policy");
    });
    let test = builder.build_with_auto_env(&server).await?;

    let output_file = test.cwd.path().join("guardian-denied.txt");
    let command = format!("printf should-not-run > {}", output_file.display());
    let tool_args = json!({
        "cmd": command,
        "yield_time_ms": 1_000_u64,
        "sandbox_permissions": SandboxPermissions::RequireEscalated,
        "justification": "Exercise Guardian denial routing.",
    });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-parent-tool-denied"),
                ev_function_call(
                    "exec-call-denied",
                    "exec_command",
                    &serde_json::to_string(&tool_args)?,
                ),
                ev_completed("resp-parent-tool-denied"),
            ]),
            sse(vec![
                ev_response_created("resp-guardian-denied"),
                ev_assistant_message(
                    "msg-guardian-denied",
                    &json!({
                        "risk_level": "high",
                        "user_authorization": "low",
                        "outcome": "deny",
                        "rationale": "The requested write has unacceptable test risk.",
                    })
                    .to_string(),
                ),
                ev_completed("resp-guardian-denied"),
            ]),
            sse(vec![
                ev_response_created("resp-parent-after-denial"),
                ev_assistant_message("msg-parent-after-denial", "denied"),
                ev_completed("resp-parent-after-denial"),
            ]),
        ],
    )
    .await;

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "run a command that Guardian should deny".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                approval_policy: Some(approval_policy),
                approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
                sandbox_policy: Some(sandbox_policy),
                ..Default::default()
            }),
        )
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = responses.requests();
    let guardian_request = requests
        .iter()
        .find(|request| request.body_contains_text("Exercise Guardian denial routing."))
        .expect("expected Guardian review request");
    assert!(guardian_request.body_contains_text(&command));

    let tool_output = requests
        .iter()
        .find_map(|request| request.function_call_output_text("exec-call-denied"))
        .expect("expected rejected tool output to be returned to the parent model");
    assert!(
        tool_output.contains("The requested write has unacceptable test risk."),
        "Guardian rationale missing from rejected tool output: {tool_output}"
    );
    assert!(
        !output_file.exists(),
        "Guardian-denied command unexpectedly executed"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cyber_model_guardian_denial_interrupts_turn_immediately() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));
    skip_if_wine_exec!(
        Ok(()),
        "Guardian approval actions require host-native paths"
    );

    let server = start_mock_server().await;
    let approval_policy = AskForApproval::OnRequest;
    let sandbox_policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![],
        network_access: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    };
    let sandbox_policy_for_config = sandbox_policy.clone();

    let mut builder = test_codex()
        .with_model_info_override("gpt-5.4", |model| {
            model.model_specialty = Some(MODEL_SPECIALTY_CYBER.to_string());
        })
        .with_config(move |config| {
            config.permissions.approval_policy = Constrained::allow_any(approval_policy);
            config
                .set_legacy_sandbox_policy(sandbox_policy_for_config)
                .expect("set sandbox policy");
        });
    let test = builder.build_with_auto_env(&server).await?;

    let output_file = test.cwd.path().join("cyber-guardian-denied.txt");
    let command = format!("printf should-not-run > {}", output_file.display());
    let tool_args = json!({
        "cmd": command,
        "yield_time_ms": 1_000_u64,
        "sandbox_permissions": SandboxPermissions::RequireEscalated,
        "justification": "Exercise immediate Guardian interruption for cyber models.",
    });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-cyber-parent-tool-denied"),
                ev_function_call(
                    "exec-cyber-call-denied",
                    "exec_command",
                    &serde_json::to_string(&tool_args)?,
                ),
                ev_completed("resp-cyber-parent-tool-denied"),
            ]),
            sse(vec![
                ev_response_created("resp-cyber-guardian-denied"),
                ev_assistant_message(
                    "msg-cyber-guardian-denied",
                    &json!({
                        "risk_level": "high",
                        "user_authorization": "low",
                        "outcome": "deny",
                        "rationale": "The requested command has unacceptable test risk.",
                    })
                    .to_string(),
                ),
                ev_completed("resp-cyber-guardian-denied"),
            ]),
        ],
    )
    .await;

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "run a command that Guardian should deny for a cyber model".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                approval_policy: Some(approval_policy),
                approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
                sandbox_policy: Some(sandbox_policy),
                ..Default::default()
            }),
        )
        .await?;

    let warning = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::GuardianWarning(warning)
                if warning.message.contains("too many approval requests")
        )
    })
    .await;
    let EventMsg::GuardianWarning(warning) = warning else {
        unreachable!("wait_for_event returned a non-warning event")
    };
    assert!(
        warning
            .message
            .contains("1 consecutive, 1 in the last 50 reviews")
    );

    let aborted = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnAborted(_))
    })
    .await;
    let EventMsg::TurnAborted(aborted) = aborted else {
        unreachable!("wait_for_event returned a non-abort event")
    };
    assert_eq!(aborted.reason, TurnAbortReason::Interrupted);
    assert_eq!(responses.requests().len(), 2);
    assert!(
        !output_file.exists(),
        "Guardian-denied cyber-model command unexpectedly executed"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_review_session_does_not_inherit_legacy_notify() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let approval_policy = AskForApproval::OnRequest;
    let sandbox_policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![],
        network_access: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    };

    let notify_dir = TempDir::new()?;
    let notify_script = notify_dir.path().join("notify.sh");
    fs::write(
        &notify_script,
        r#"#!/bin/bash
set -e
payload_path="$(dirname "${0}")/notify.jsonl"
printf '%s\n' "${@: -1}" >> "${payload_path}""#,
    )?;
    fs::set_permissions(&notify_script, fs::Permissions::from_mode(0o755))?;
    let notify_file = notify_dir.path().join("notify.jsonl");
    let notify_script_str = notify_script.to_str().unwrap().to_string();
    let sandbox_policy_for_config = sandbox_policy.clone();

    let mut builder = test_codex().with_config(move |config| {
        config.notify = Some(vec![notify_script_str]);
        config.permissions.approval_policy = Constrained::allow_any(approval_policy);
        config
            .set_legacy_sandbox_policy(sandbox_policy_for_config)
            .expect("set sandbox policy");
    });
    let test = builder.build(&server).await?;

    let output_file = test.cwd.path().join("guardian-review-notify.txt");
    let command = format!("printf guardian-approved > {}", output_file.display());
    let tool_args = json!({
        "cmd": command,
        "yield_time_ms": 1_000_u64,
        "sandbox_permissions": SandboxPermissions::RequireEscalated,
        "justification": "Exercise Guardian approval routing.",
    });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-parent-tool"),
                ev_function_call(
                    "exec-call",
                    "exec_command",
                    &serde_json::to_string(&tool_args)?,
                ),
                ev_completed("resp-parent-tool"),
            ]),
            sse(vec![
                ev_response_created("resp-guardian-review"),
                ev_assistant_message(
                    "msg-guardian-review",
                    &json!({
                        "risk_level": "low",
                        "user_authorization": "high",
                        "outcome": "allow",
                        "rationale": "The command writes a marker file in the workspace.",
                    })
                    .to_string(),
                ),
                ev_completed("resp-guardian-review"),
            ]),
            sse(vec![
                ev_response_created("resp-parent-done"),
                ev_assistant_message("msg-parent-done", "done"),
                ev_completed("resp-parent-done"),
            ]),
        ],
    )
    .await;

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "run a command that requires Guardian review".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(local_selections(test.config.cwd.clone())),
                approval_policy: Some(approval_policy),
                approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
                sandbox_policy: Some(sandbox_policy),
                ..Default::default()
            }),
        )
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let guardian_request = responses
        .requests()
        .into_iter()
        .find(|request| request.body_contains_text("Exercise Guardian approval routing."))
        .expect("expected Guardian review request");
    assert!(guardian_request.body_contains_text(&command));

    fs_wait::wait_for_path_exists(&notify_file, Duration::from_secs(5)).await?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    let notify_payload_raw = tokio::fs::read_to_string(&notify_file).await?;
    let payloads: Vec<Value> = notify_payload_raw
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<std::result::Result<_, _>>()?;

    assert_eq!(
        payloads.len(),
        1,
        "unexpected notify payloads: {payloads:?}"
    );
    assert_eq!(
        payloads[0]["input-messages"],
        json!(["run a command that requires Guardian review"])
    );
    assert_eq!(payloads[0]["last-assistant-message"], json!("done"));
    assert!(
        !notify_payload_raw.contains(
            "The following is the Codex agent history whose request action you are assessing."
        ),
        "Guardian review transcript leaked into legacy notify payload: {notify_payload_raw}"
    );
    assert_eq!(fs::read_to_string(&output_file)?, "guardian-approved");

    Ok(())
}
