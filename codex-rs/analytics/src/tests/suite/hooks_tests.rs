//! Hook execution event, source, and status tests.

use crate::events::CodexHookRunEventRequest;
use crate::events::TrackEventRequest;
use crate::events::codex_hook_run_metadata;
use crate::facts::AnalyticsFact;
use crate::facts::CustomAnalyticsFact;
use crate::facts::HookRunFact;
use crate::facts::HookRunInput;
use crate::reducer::AnalyticsReducer;
use crate::tests::support::TEST_PRODUCT_CLIENT_ID;
use crate::tests::support::test_tracking_context;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookExecutionMode;
use codex_protocol::protocol::HookHandlerType;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::HookSource;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn hook_run_event_serializes_expected_shape() {
    let tracking = test_tracking_context("thread-3", "turn-3");
    let event = TrackEventRequest::HookRun(CodexHookRunEventRequest {
        event_type: "codex_hook_run",
        event_params: codex_hook_run_metadata(
            &tracking,
            HookRunFact {
                event_name: HookEventName::PreToolUse,
                hook_source: HookSource::User,
                handler_type: HookHandlerType::McpTool,
                execution_mode: HookExecutionMode::Sync,
                status: HookRunStatus::Completed,
            },
        ),
    });

    let payload = serde_json::to_value(&event).expect("serialize hook run event");

    assert_eq!(
        payload,
        json!({
            "event_type": "codex_hook_run",
            "event_params": {
                "thread_id": "thread-3",
                "turn_id": "turn-3",
                "product_client_id": TEST_PRODUCT_CLIENT_ID,
                "model_slug": "gpt-5",
                "hook_name": "PreToolUse",
                "hook_source": "user",
                "handler_type": "mcp_tool",
                "execution_mode": "sync",
                "status": "completed"
            }
        })
    );
}

#[test]
fn hook_run_metadata_maps_sources_and_statuses() {
    let tracking = test_tracking_context("thread-1", "turn-1");

    let system = serde_json::to_value(codex_hook_run_metadata(
        &tracking,
        HookRunFact {
            event_name: HookEventName::SessionStart,
            hook_source: HookSource::System,
            handler_type: HookHandlerType::Command,
            execution_mode: HookExecutionMode::Sync,
            status: HookRunStatus::Completed,
        },
    ))
    .expect("serialize system hook");
    let project = serde_json::to_value(codex_hook_run_metadata(
        &tracking,
        HookRunFact {
            event_name: HookEventName::Stop,
            hook_source: HookSource::Project,
            handler_type: HookHandlerType::Prompt,
            execution_mode: HookExecutionMode::Async,
            status: HookRunStatus::Blocked,
        },
    ))
    .expect("serialize project hook");
    let cloud_requirements = serde_json::to_value(codex_hook_run_metadata(
        &tracking,
        HookRunFact {
            event_name: HookEventName::Stop,
            hook_source: HookSource::CloudRequirements,
            handler_type: HookHandlerType::Agent,
            execution_mode: HookExecutionMode::Sync,
            status: HookRunStatus::Blocked,
        },
    ))
    .expect("serialize cloud requirements hook");
    let unknown = serde_json::to_value(codex_hook_run_metadata(
        &tracking,
        HookRunFact {
            event_name: HookEventName::UserPromptSubmit,
            hook_source: HookSource::Unknown,
            handler_type: HookHandlerType::Command,
            execution_mode: HookExecutionMode::Async,
            status: HookRunStatus::Failed,
        },
    ))
    .expect("serialize unknown hook");

    assert_eq!(system["hook_source"], "system");
    assert_eq!(system["status"], "completed");
    assert_eq!(project["hook_source"], "project");
    assert_eq!(project["status"], "blocked");
    assert_eq!(cloud_requirements["hook_source"], "cloud_requirements");
    assert_eq!(cloud_requirements["status"], "blocked");
    assert_eq!(unknown["hook_source"], "unknown");
    assert_eq!(unknown["status"], "failed");
}

#[test]
fn hook_run_metadata_maps_stopped_status() {
    let tracking = test_tracking_context("thread-1", "turn-1");

    let stopped = serde_json::to_value(codex_hook_run_metadata(
        &tracking,
        HookRunFact {
            event_name: HookEventName::Stop,
            hook_source: HookSource::User,
            handler_type: HookHandlerType::Command,
            execution_mode: HookExecutionMode::Sync,
            status: HookRunStatus::Stopped,
        },
    ))
    .expect("serialize stopped hook");

    assert_eq!(stopped["hook_source"], "user");
    assert_eq!(stopped["status"], "stopped");
}

#[tokio::test]
async fn reducer_ingests_hook_run_fact() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();

    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::HookRun(HookRunInput {
                tracking: test_tracking_context("thread-1", "turn-1"),
                hook: HookRunFact {
                    event_name: HookEventName::PostToolUse,
                    hook_source: HookSource::Unknown,
                    handler_type: HookHandlerType::Agent,
                    execution_mode: HookExecutionMode::Async,
                    status: HookRunStatus::Failed,
                },
            })),
            &mut events,
        )
        .await;

    let payload = serde_json::to_value(&events).expect("serialize events");
    assert_eq!(payload.as_array().expect("events array").len(), 1);
    assert_eq!(payload[0]["event_type"], "codex_hook_run");
    assert_eq!(payload[0]["event_params"]["hook_name"], "PostToolUse");
    assert_eq!(payload[0]["event_params"]["hook_source"], "unknown");
    assert_eq!(payload[0]["event_params"]["handler_type"], "agent");
    assert_eq!(payload[0]["event_params"]["execution_mode"], "async");
    assert_eq!(payload[0]["event_params"]["status"], "failed");
}
