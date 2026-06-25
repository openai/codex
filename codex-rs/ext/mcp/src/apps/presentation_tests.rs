use std::sync::Arc;

use codex_core::config::ConfigBuilder;
use codex_extension_api::ContextContributor;
use codex_extension_api::ThreadDataInitializer;
use codex_extension_api::TurnItemContributor;
use codex_protocol::items::AgentMessageItem;
use codex_protocol::items::McpToolCallItem;
use codex_protocol::items::McpToolCallStatus;
use codex_protocol::items::TurnItem;
use pretty_assertions::assert_eq;

use super::AppsThreadState;
use crate::apps::CodexAppsMcpExtension;
use crate::apps::config::apps_mcp_product_sku;
use crate::apps::config::include_apps_instructions;
use crate::apps::test_support::gmail_tool;
use crate::apps::test_support::test_apps;

#[tokio::test]
async fn pinned_snapshot_enriches_turn_items_and_honors_instruction_config() {
    let codex_home = tempfile::tempdir().expect("temp codex home");
    std::fs::write(
        codex_home.path().join("config.toml"),
        "include_apps_instructions = false\napps_mcp_product_sku = \"test-sku\"\n",
    )
    .expect("disable Apps instructions");
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .build()
        .await
        .expect("load config");
    assert!(!include_apps_instructions(&config));
    assert_eq!(apps_mcp_product_sku(&config).as_deref(), Some("test-sku"));

    let mut tool = gmail_tool("GmailSearch", /*destructive*/ false);
    let meta = tool.meta.as_mut().expect("connector metadata");
    meta.insert("link_id".to_string(), serde_json::json!("link_gmail"));
    meta.insert(
        "ui".to_string(),
        serde_json::json!({"resourceUri": "ui://gmail/search.html"}),
    );
    meta.insert("template_id".to_string(), serde_json::json!("spoofed"));
    meta.insert(
        "_codex_apps".to_string(),
        serde_json::json!({
            "template_id": "gmail-template",
            "resource_uri": "/gmail/link/search_messages",
        }),
    );
    let apps = test_apps(vec![tool]).await;
    let thread_store = codex_extension_api::ExtensionData::new("thread");
    let state = AppsThreadState::default();
    state.replace(Some(Arc::clone(&apps)), &config);
    thread_store.insert(state);
    let pinned_state = thread_store
        .get::<AppsThreadState>()
        .expect("pinned Apps thread state");
    assert!(
        pinned_state.snapshot().is_some(),
        "the thread store retains its pinned runtime snapshot"
    );
    let turn_store = codex_extension_api::ExtensionData::new("turn");
    let session_store = codex_extension_api::ExtensionData::new("session");
    let service =
        CodexAppsMcpExtension::new_for_tests(codex_login::AuthManager::from_auth_for_testing(
            codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        ));
    let mut initialized = codex_extension_api::ExtensionDataInit::new();
    ThreadDataInitializer::initialize(&service, &mut initialized);
    let initialized_state = initialized
        .get::<AppsThreadState>()
        .expect("Apps initializer state");
    ThreadDataInitializer::initialize(&service, &mut initialized);
    assert!(Arc::ptr_eq(
        &initialized_state,
        &initialized
            .get::<AppsThreadState>()
            .expect("preserved Apps initializer state")
    ));

    assert!(
        ContextContributor::contribute_thread_context(&service, &session_store, &thread_store,)
            .await
            .is_empty(),
        "include_apps_instructions=false must suppress the Apps fragment"
    );
    let gmail_registration = "codex_apps__gmail".to_string();

    let mut started = TurnItem::McpToolCall(McpToolCallItem::new(
        "call-1".to_string(),
        gmail_registration.clone(),
        "search".to_string(),
        serde_json::json!({"query": "rust"}),
        McpToolCallStatus::InProgress,
    ));
    assert!(TurnItemContributor::applies_to(&service, &started));
    assert!(!TurnItemContributor::applies_to(
        &service,
        &TurnItem::AgentMessage(AgentMessageItem::new(&[])),
    ));
    TurnItemContributor::contribute(&service, &thread_store, &turn_store, &mut started)
        .await
        .expect("enrich started item");
    let TurnItem::McpToolCall(started) = &started else {
        panic!("expected MCP tool item")
    };
    assert_eq!(started.server, "codex_apps__gmail");
    assert_eq!(started.connector_id.as_deref(), Some("gmail"));
    assert_eq!(started.link_id.as_deref(), Some("link_gmail"));
    assert_eq!(started.app_name.as_deref(), Some("Gmail"));
    assert_eq!(started.template_id.as_deref(), Some("gmail-template"));
    assert_eq!(started.action_name.as_deref(), Some("search_messages"));
    assert_eq!(
        started.mcp_app_resource_uri.as_deref(),
        Some("ui://gmail/search.html")
    );

    let mut direct_namespace = TurnItem::McpToolCall(McpToolCallItem::new(
        "call-direct-namespace".to_string(),
        "codex_apps__gmail".to_string(),
        "search".to_string(),
        serde_json::Value::Null,
        McpToolCallStatus::InProgress,
    ));
    TurnItemContributor::contribute(&service, &thread_store, &turn_store, &mut direct_namespace)
        .await
        .expect("enrich direct Apps namespace");
    let TurnItem::McpToolCall(direct_namespace) = direct_namespace else {
        panic!("expected MCP tool item")
    };
    assert_eq!(direct_namespace.server, "codex_apps__gmail");
    assert_eq!(direct_namespace.connector_id.as_deref(), Some("gmail"));
    assert_eq!(direct_namespace.app_name.as_deref(), Some("Gmail"));

    let mut prepopulated = TurnItem::McpToolCall(
        McpToolCallItem::new(
            "call-prepopulated".to_string(),
            gmail_registration.clone(),
            "search".to_string(),
            serde_json::Value::Null,
            McpToolCallStatus::InProgress,
        )
        .with_presentation(
            Some("ui://generic/already-set.html".to_string()),
            Some("existing-link".to_string()),
            /*plugin_id*/ None,
        ),
    );
    let TurnItem::McpToolCall(prepopulated_item) = &mut prepopulated else {
        unreachable!("constructed an MCP tool item")
    };
    prepopulated_item.app_name = Some("spoofed app".to_string());
    prepopulated_item.template_id = Some("spoofed template".to_string());
    prepopulated_item.action_name = Some("spoofed action".to_string());
    TurnItemContributor::contribute(&service, &thread_store, &turn_store, &mut prepopulated)
        .await
        .expect("preserve generic presentation");
    let TurnItem::McpToolCall(prepopulated) = prepopulated else {
        panic!("expected MCP tool item")
    };
    assert_eq!(prepopulated.connector_id.as_deref(), Some("gmail"));
    assert_eq!(prepopulated.app_name.as_deref(), Some("Gmail"));
    assert_eq!(prepopulated.template_id.as_deref(), Some("gmail-template"));
    assert_eq!(prepopulated.action_name.as_deref(), Some("search_messages"));
    assert_eq!(
        prepopulated.link_id.as_deref(),
        Some("link_gmail"),
        "Apps-owned link identity must replace earlier contributor data"
    );
    assert_eq!(
        prepopulated.mcp_app_resource_uri.as_deref(),
        Some("ui://generic/already-set.html")
    );

    thread_store
        .get::<AppsThreadState>()
        .expect("Apps thread state")
        .replace(/*apps*/ None, &config);
    let mut colliding_completion = TurnItem::McpToolCall(McpToolCallItem::new(
        "call-1".to_string(),
        "custom-server".to_string(),
        "search".to_string(),
        serde_json::Value::Null,
        McpToolCallStatus::Completed,
    ));
    TurnItemContributor::contribute(
        &service,
        &thread_store,
        &turn_store,
        &mut colliding_completion,
    )
    .await
    .expect("ignore colliding non-Apps completion");
    let TurnItem::McpToolCall(colliding_completion) = colliding_completion else {
        panic!("expected MCP tool item")
    };
    assert_eq!(colliding_completion.server, "custom-server");
    assert_eq!(colliding_completion.connector_id, None);
    assert_eq!(colliding_completion.app_name, None);
    assert_eq!(colliding_completion.template_id, None);
    assert_eq!(colliding_completion.action_name, None);

    let mut completed = TurnItem::McpToolCall(McpToolCallItem::new(
        "call-1".to_string(),
        gmail_registration.clone(),
        "search".to_string(),
        serde_json::json!({"query": "rust"}),
        McpToolCallStatus::Completed,
    ));
    TurnItemContributor::contribute(&service, &thread_store, &turn_store, &mut completed)
        .await
        .expect("enrich completed item");
    let TurnItem::McpToolCall(completed) = completed else {
        panic!("expected MCP tool item")
    };
    assert_eq!(completed.server, "codex_apps__gmail");
    assert_eq!(completed.connector_id.as_deref(), Some("gmail"));
    assert_eq!(completed.link_id.as_deref(), Some("link_gmail"));
    assert_eq!(completed.app_name.as_deref(), Some("Gmail"));
    assert_eq!(completed.template_id.as_deref(), Some("gmail-template"));
    assert_eq!(completed.action_name.as_deref(), Some("search_messages"));
    assert_eq!(
        completed.mcp_app_resource_uri.as_deref(),
        Some("ui://gmail/search.html")
    );

    let mut lookalike = TurnItem::McpToolCall(McpToolCallItem::new(
        "call-2".to_string(),
        format!("{gmail_registration}-lookalike"),
        "search".to_string(),
        serde_json::Value::Null,
        McpToolCallStatus::InProgress,
    ));
    TurnItemContributor::contribute(&service, &thread_store, &turn_store, &mut lookalike)
        .await
        .expect("ignore lookalike item");
    let TurnItem::McpToolCall(lookalike) = lookalike else {
        panic!("expected MCP tool item")
    };
    assert_eq!(lookalike.server, format!("{gmail_registration}-lookalike"));
    assert_eq!(lookalike.connector_id, None);
    assert_eq!(lookalike.app_name, None);
    assert_eq!(lookalike.template_id, None);
    assert_eq!(lookalike.action_name, None);

    let unseeded_thread_store = codex_extension_api::ExtensionData::new("unseeded-thread");
    let mut unseeded_item = TurnItem::McpToolCall(McpToolCallItem::new(
        "call-unseeded".to_string(),
        gmail_registration,
        "search".to_string(),
        serde_json::Value::Null,
        McpToolCallStatus::InProgress,
    ));
    TurnItemContributor::contribute(
        &service,
        &unseeded_thread_store,
        &turn_store,
        &mut unseeded_item,
    )
    .await
    .expect("ignore unseeded thread");
    let TurnItem::McpToolCall(unseeded_item) = unseeded_item else {
        panic!("expected MCP tool item")
    };
    assert_eq!(unseeded_item.connector_id, None);
    assert_eq!(unseeded_item.app_name, None);
    assert_eq!(unseeded_item.template_id, None);
    assert_eq!(unseeded_item.action_name, None);

    std::fs::write(codex_home.path().join("config.toml"), "")
        .expect("restore default Apps instructions");
    let default_config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .build()
        .await
        .expect("load default config");
    assert!(include_apps_instructions(&default_config));
    let default_thread_store = codex_extension_api::ExtensionData::new("default-thread");
    let default_state = AppsThreadState::default();
    default_state.replace(Some(Arc::clone(&apps)), &default_config);
    default_thread_store.insert(default_state);
    assert_eq!(
        ContextContributor::contribute_thread_context(
            &service,
            &session_store,
            &default_thread_store,
        )
        .await
        .len(),
        1
    );

    apps.shutdown().await;
}
