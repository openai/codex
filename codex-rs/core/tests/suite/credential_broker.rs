use anyhow::Result;
use codex_features::Feature;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::skip_if_sandbox;
use core_test_support::skip_if_target_windows;
use core_test_support::test_codex::TestCodexHarness;
use core_test_support::test_codex::local_selections;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use tempfile::TempDir;

const REAL_OPENAI_API_KEY: &str =
    "sk-proj-abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn configured_credential_broker_virtualizes_child_environment() -> Result<()> {
    skip_if_target_windows!(Ok(()), "uses a POSIX shell command");
    skip_if_sandbox!(Ok(()));

    let home = Arc::new(TempDir::new()?);
    fs::write(
        home.path().join("config.toml"),
        r#"
default_permissions = "workspace"

[features.network_proxy]
credential_broker = true

[permissions.workspace.filesystem]
":minimal" = "read"

[permissions.workspace.network]
enabled = true
mode = "full"
"#,
    )?;
    let builder = test_codex().with_home(home).with_config(|config| {
        config.use_experimental_unified_exec_tool = true;
        config
            .features
            .enable(Feature::UnifiedExec)
            .expect("test config should allow feature update");
        config.permissions.shell_environment_policy.r#set = HashMap::from([(
            "OPENAI_API_KEY".to_string(),
            REAL_OPENAI_API_KEY.to_string(),
        )]);
    });
    let harness = TestCodexHarness::with_auto_env_builder(builder).await?;
    let test = harness.test();

    assert!(test.config.features.enabled(Feature::NetworkProxy));
    assert!(
        test.config.permissions.network.is_some(),
        "credential broker config should start the managed network proxy"
    );

    let call_id = "credential-broker-child-env";
    let args = json!({
        "cmd": "printf '%s\\n%s' \"$OPENAI_API_KEY\" \"$CODEX_NETWORK_PROXY_CREDENTIAL_BROKER_ACTIVE\"",
        "yield_time_ms": 1_000,
    });
    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "exec_command", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_response_created("resp-2"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(harness.server(), responses).await;

    let session_model = test.session_configured.model.clone();
    let cwd = test.config.cwd.clone();
    let (sandbox_policy, permission_profile) = turn_permission_fields(
        test.config.permissions.permission_profile().clone(),
        cwd.as_path(),
    );
    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "show the brokered child credential".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
                environments: Some(local_selections(cwd)),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(codex_protocol::config_types::CollaborationMode {
                    mode: codex_protocol::config_types::ModeKind::Default,
                    settings: codex_protocol::config_types::Settings {
                        model: session_model,
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            },
        })
        .await?;

    let end = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::ExecCommandEnd(event) if event.call_id == call_id => Some(event.clone()),
        _ => None,
    })
    .await;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    assert_eq!(end.exit_code, 0);
    let output = end.aggregated_output.trim().lines().collect::<Vec<_>>();
    assert_eq!(output.len(), 2, "unexpected command output: {output:?}");
    let brokered_openai_api_key = output[0];
    assert_ne!(brokered_openai_api_key, REAL_OPENAI_API_KEY);
    assert!(brokered_openai_api_key.starts_with("sk-proj-"));
    assert_eq!(brokered_openai_api_key.len(), REAL_OPENAI_API_KEY.len());
    assert_eq!(output[1], "1");

    Ok(())
}
