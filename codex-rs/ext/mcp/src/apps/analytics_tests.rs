use codex_core::config::ConfigBuilder;
use codex_extension_api::ExtensionData;
use codex_extension_api::TurnInputContext;
use codex_extension_api::TurnInputContributor;
use codex_extension_api::TurnItemContributor;
use codex_protocol::items::McpToolCallItem;
use codex_protocol::items::McpToolCallStatus;
use codex_protocol::items::TurnItem;
use pretty_assertions::assert_eq;
use std::time::Duration;

use super::*;
use crate::apps::test_support::gmail_tool;
use crate::apps::test_support::test_apps;

fn text_input(text: &str) -> UserInput {
    UserInput::Text {
        text: text.to_string(),
        text_elements: Vec::new(),
    }
}

async fn contribute_app_call(
    service: &CodexAppsMcpExtension,
    thread_store: &ExtensionData,
    turn_store: &ExtensionData,
    call_id: &str,
    status: McpToolCallStatus,
    duration: Option<Duration>,
) {
    let mut item = McpToolCallItem::new(
        call_id.to_string(),
        "codex_apps__gmail".to_string(),
        "search".to_string(),
        serde_json::Value::Null,
        status,
    );
    item.duration = duration;
    let mut item = TurnItem::McpToolCall(item);
    TurnItemContributor::contribute(service, thread_store, turn_store, &mut item)
        .await
        .expect("present Apps tool call");
}

#[test]
fn explicit_app_ids_include_structured_and_linked_mentions_only() {
    let inputs = vec![
        text_input("use [$gmail](app://gmail) and [$docs](mcp://docs)"),
        UserInput::Mention {
            name: "gmail".to_string(),
            path: "app://gmail".to_string(),
        },
        UserInput::Mention {
            name: "calendar".to_string(),
            path: "app://calendar".to_string(),
        },
        UserInput::Mention {
            name: "skill".to_string(),
            path: "skill://skill".to_string(),
        },
    ];

    assert_eq!(
        collect_explicit_app_ids(&inputs),
        HashSet::from(["calendar".to_string(), "gmail".to_string()])
    );
}

#[tokio::test]
async fn turn_state_unions_mentions_and_usage_comes_from_the_raw_turn_item() {
    let codex_home = tempfile::tempdir().expect("temp codex home");
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .build()
        .await
        .expect("load config");
    let apps = test_apps(vec![gmail_tool("GmailSearch", /*destructive*/ false)]).await;
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    let thread_state = AppsThreadState::default();
    thread_state.replace(Some(Arc::clone(&apps)), &config);
    thread_store.insert(thread_state);
    let turn_store = ExtensionData::new("turn");
    let service =
        CodexAppsMcpExtension::new_for_tests(codex_login::AuthManager::from_auth_for_testing(
            codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        ));

    for input in [
        text_input("use [$gmail](app://gmail)"),
        UserInput::Mention {
            name: "calendar".to_string(),
            path: "app://calendar".to_string(),
        },
    ] {
        TurnInputContributor::contribute(
            &service,
            TurnInputContext {
                turn_id: "turn".to_string(),
                model_slug: "test-model".to_string(),
                product_client_id: "test-client".to_string(),
                user_input: vec![input],
                environments: Vec::new(),
            },
            &session_store,
            &thread_store,
            &turn_store,
        )
        .await;
    }
    let analytics_state = turn_store
        .get::<AppsTurnAnalyticsState>()
        .expect("turn analytics state");
    assert_eq!(
        *analytics_state
            .explicit_app_ids
            .lock()
            .unwrap_or_else(PoisonError::into_inner),
        HashSet::from(["calendar".to_string(), "gmail".to_string()])
    );

    contribute_app_call(
        &service,
        &thread_store,
        &turn_store,
        "call-started",
        McpToolCallStatus::InProgress,
        /*duration*/ None,
    )
    .await;
    assert!(
        app_invocation_for_finished_call(&turn_store, "call-started").is_none(),
        "starting a call does not prove that the MCP operation was attempted"
    );
    for call_id in ["call-declined", "call-cancelled"] {
        contribute_app_call(
            &service,
            &thread_store,
            &turn_store,
            call_id,
            McpToolCallStatus::Failed,
            /*duration*/ None,
        )
        .await;
        assert!(
            app_invocation_for_finished_call(&turn_store, call_id).is_none(),
            "an approval skip was never attempted"
        );
    }

    contribute_app_call(
        &service,
        &thread_store,
        &turn_store,
        "call-succeeded",
        McpToolCallStatus::Completed,
        Some(Duration::from_millis(1)),
    )
    .await;
    let (_, invocation) = app_invocation_for_finished_call(&turn_store, "call-succeeded")
        .expect("an attempted successful Apps call should be tracked");
    assert_eq!(invocation.connector_id.as_deref(), Some("gmail"));
    assert_eq!(invocation.app_name.as_deref(), Some("Gmail"));
    assert!(matches!(
        invocation.invocation_type,
        Some(InvocationType::Explicit)
    ));
    assert!(
        app_invocation_for_finished_call(&turn_store, "call-succeeded").is_none(),
        "finishing a call consumes its usage attribution"
    );

    contribute_app_call(
        &service,
        &thread_store,
        &turn_store,
        "call-failed",
        McpToolCallStatus::Failed,
        Some(Duration::from_millis(1)),
    )
    .await;
    assert!(
        app_invocation_for_finished_call(&turn_store, "call-failed").is_some(),
        "an attempted Apps call still counts when its upstream returns an error"
    );

    apps.shutdown().await;
}

#[tokio::test]
async fn mentioned_apps_include_snapshot_display_names() {
    let mut synthetic_tool = gmail_tool("GmailSearch", /*destructive*/ false);
    synthetic_tool
        .meta
        .as_mut()
        .expect("connector metadata")
        .insert(
            "_codex_apps".to_string(),
            serde_json::json!({"synthetic_link": true}),
        );
    let apps = test_apps(vec![synthetic_tool]).await;
    let snapshot = apps.snapshot();
    assert!(snapshot.apps().is_empty());
    assert_eq!(snapshot.all_connectors().len(), 1);
    let ids = HashSet::from(["gmail".to_string(), "unknown".to_string()]);

    let mentions = mentioned_app_invocations(&ids, Some(&snapshot));

    assert_eq!(mentions.len(), 2);
    assert_eq!(mentions[0].connector_id.as_deref(), Some("gmail"));
    assert_eq!(mentions[0].app_name.as_deref(), Some("Gmail"));
    assert_eq!(mentions[1].connector_id.as_deref(), Some("unknown"));
    assert_eq!(mentions[1].app_name, None);
    assert!(
        mentions
            .iter()
            .all(|mention| matches!(mention.invocation_type, Some(InvocationType::Explicit)))
    );

    apps.shutdown().await;
}
use std::sync::Arc;
