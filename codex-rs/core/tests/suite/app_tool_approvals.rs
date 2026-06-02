#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use anyhow::Result;
use codex_config::AppRequirementToml;
use codex_config::AppToolRequirementToml;
use codex_config::AppToolsRequirementsToml;
use codex_config::AppsRequirementsToml;
use codex_config::CloudRequirementsLoader;
use codex_config::ConfigRequirementsToml;
use codex_config::types::AppToolApproval;
use codex_features::Feature;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ElicitationAction;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::PathExt;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::SEARCH_CALENDAR_CREATE_TOOL;
use core_test_support::apps_test_server::SEARCH_CALENDAR_LIST_TOOL;
use core_test_support::apps_test_server::SEARCH_CALENDAR_NAMESPACE;
use core_test_support::apps_test_server::recorded_apps_tool_calls;
use core_test_support::apps_test_server::search_capable_apps_builder;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::collections::BTreeMap;

const CALENDAR_CONNECTOR_ID: &str = "calendar";
const CALENDAR_CREATE_EVENT_TOOL_NAME: &str = "calendar_create_event";
const CALENDAR_LIST_EVENTS_TOOL_NAME: &str = "calendar_list_events";

fn called_tool_names(calls: &[Value]) -> Vec<&str> {
    calls
        .iter()
        .filter_map(|call| call.pointer("/params/name").and_then(Value::as_str))
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn managed_prompt_writes_is_enforced_in_core_harness() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    let read_call_id = "calendar-list-events";
    let write_call_id = "calendar-create-event";
    let write_args = serde_json::to_string(&json!({
        "title": "Lunch",
        "starts_at": "2026-03-10T12:00:00Z"
    }))?;
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call_with_namespace(
                    read_call_id,
                    SEARCH_CALENDAR_NAMESPACE,
                    SEARCH_CALENDAR_LIST_TOOL,
                    r#"{"query":"Lunch"}"#,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_function_call_with_namespace(
                    write_call_id,
                    SEARCH_CALENDAR_NAMESPACE,
                    SEARCH_CALENDAR_CREATE_TOOL,
                    &write_args,
                ),
                ev_completed("resp-2"),
            ]),
            sse(vec![
                ev_response_created("resp-3"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-3"),
            ]),
        ],
    )
    .await;

    let managed_requirements = ConfigRequirementsToml {
        apps: Some(AppsRequirementsToml {
            apps: BTreeMap::from([(
                CALENDAR_CONNECTOR_ID.to_string(),
                AppRequirementToml {
                    enabled: None,
                    tools: Some(AppToolsRequirementsToml {
                        tools: BTreeMap::from([
                            (
                                CALENDAR_CREATE_EVENT_TOOL_NAME.to_string(),
                                AppToolRequirementToml {
                                    approval_mode: Some(AppToolApproval::PromptWrites),
                                },
                            ),
                            (
                                CALENDAR_LIST_EVENTS_TOOL_NAME.to_string(),
                                AppToolRequirementToml {
                                    approval_mode: Some(AppToolApproval::PromptWrites),
                                },
                            ),
                        ]),
                    }),
                },
            )]),
        }),
        ..Default::default()
    };
    let mut builder = search_capable_apps_builder(apps_server.chatgpt_base_url)
        .with_cloud_requirements(CloudRequirementsLoader::new(async move {
            Ok(Some(managed_requirements))
        }))
        .with_config(|config| {
            config
                .features
                .enable(Feature::ToolCallMcpElicitation)
                .expect("test config should allow feature update");
            let user_config_path = config.codex_home.join("config.toml").abs();
            let user_config = toml::from_str(
                r#"
[apps.calendar]
default_tools_approval_mode = "approve"
"#,
            )
            .expect("apps config should parse");
            config.config_layer_stack = config
                .config_layer_stack
                .with_user_config(&user_config_path, user_config);
        });
    let test = builder.build(&server).await?;

    let session_model = test.session_configured.model.clone();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, test.cwd.path());
    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "List events, then create one.".to_string(),
                text_elements: Vec::new(),
            }],
            environments: None,
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
                cwd: Some(test.cwd.path().to_path_buf()),
                approval_policy: Some(AskForApproval::OnRequest),
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

    let EventMsg::ElicitationRequest(request) = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::ElicitationRequest(_))
    })
    .await
    else {
        unreachable!("event guard guarantees ElicitationRequest");
    };

    let calls_before_approval = recorded_apps_tool_calls(&server).await;
    assert_eq!(
        called_tool_names(&calls_before_approval),
        vec![CALENDAR_LIST_EVENTS_TOOL_NAME]
    );

    test.codex
        .submit(Op::ResolveElicitation {
            server_name: request.server_name,
            request_id: request.id,
            decision: ElicitationAction::Accept,
            content: None,
            meta: None,
        })
        .await?;

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    assert_eq!(mock.requests().len(), 3);
    let calls_after_approval = recorded_apps_tool_calls(&server).await;
    assert_eq!(
        called_tool_names(&calls_after_approval),
        vec![
            CALENDAR_LIST_EVENTS_TOOL_NAME,
            CALENDAR_CREATE_EVENT_TOOL_NAME
        ]
    );

    Ok(())
}
