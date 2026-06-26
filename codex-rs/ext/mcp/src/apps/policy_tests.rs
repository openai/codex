use codex_config::Constrained;
use codex_config::types::AppToolApproval;
use codex_config::types::ApprovalsReviewer;
use codex_connectors::ConnectorSnapshot;
use codex_core::config::ConfigBuilder;
use pretty_assertions::assert_eq;

use super::apply_apps_server_policy;
use crate::apps::test_support::connector_tool;
use crate::apps::test_support::gmail_tool;
use crate::apps::test_support::test_apps;

#[tokio::test]
async fn app_policy_becomes_ordinary_mcp_tool_policy() {
    let codex_home = tempfile::tempdir().expect("temp codex home");
    std::fs::write(
        codex_home.path().join("config.toml"),
        r#"
approvals_reviewer = "auto_review"

[apps._default]
approvals_reviewer = "auto_review"

[apps.gmail]
approvals_reviewer = "user"
destructive_enabled = false
default_tools_approval_mode = "prompt"

[apps.gmail.tools.GmailSearch]
approval_mode = "approve"
"#,
    )
    .expect("write config");
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .build()
        .await
        .expect("load config");
    let apps = test_apps(vec![
        gmail_tool("GmailSearch", /*destructive*/ false),
        gmail_tool("GmailList", /*destructive*/ false),
        gmail_tool("GmailDelete", /*destructive*/ true),
        connector_tool(
            "calendar",
            "Calendar",
            "CalendarList",
            /*destructive*/ false,
        ),
    ])
    .await;
    let snapshot = apps.snapshot();
    let servers = apply_apps_server_policy(
        &config,
        &snapshot,
        &ConnectorSnapshot::default(),
        snapshot
            .effective_mcp_servers()
            .into_iter()
            .collect::<Vec<_>>(),
    );

    let gmail_server = servers
        .iter()
        .find(|(name, _)| name == "codex_apps__gmail")
        .map(|(_, server)| server)
        .expect("Gmail server");
    assert_eq!(
        gmail_server.runtime_metadata().approvals_reviewer(),
        Some(ApprovalsReviewer::User)
    );
    let server = gmail_server.config();
    assert_eq!(
        server.enabled_tools,
        Some(vec!["list".to_string(), "search".to_string()])
    );
    assert!(!server.tools.contains_key("delete"));
    assert_eq!(
        server
            .tools
            .get("search")
            .and_then(|tool| tool.approval_mode),
        Some(AppToolApproval::Approve)
    );
    assert_eq!(
        server.tools.get("list").and_then(|tool| tool.approval_mode),
        Some(AppToolApproval::Prompt)
    );
    gmail_server
        .runtime_metadata()
        .tool("list")
        .and_then(codex_mcp::McpToolRuntimeMetadata::approval_persistence)
        .expect("enabled Apps tool should own durable approval persistence")
        .persist()
        .await
        .expect("persist Apps tool approval");
    let persisted = std::fs::read_to_string(codex_home.path().join("config.toml"))
        .expect("read persisted config");
    let persisted = persisted
        .parse::<toml_edit::DocumentMut>()
        .expect("parse persisted config");
    assert_eq!(
        persisted["apps"]["gmail"]["tools"]["GmailList"]["approval_mode"].as_str(),
        Some("approve")
    );
    let calendar_server = servers
        .iter()
        .find(|(name, _)| name == "codex_apps__calendar")
        .map(|(_, server)| server)
        .expect("Calendar server");
    assert_eq!(
        calendar_server.runtime_metadata().approvals_reviewer(),
        Some(ApprovalsReviewer::AutoReview)
    );

    let mut managed_config = config.clone();
    let layers = managed_config
        .config_layer_stack
        .get_layers(
            codex_config::ConfigLayerStackOrdering::LowestPrecedenceFirst,
            /*include_disabled*/ true,
        )
        .into_iter()
        .cloned()
        .collect();
    let mut requirements = managed_config.config_layer_stack.requirements().clone();
    requirements.approvals_reviewer = codex_config::ConstrainedWithSource::new(
        Constrained::allow_only(ApprovalsReviewer::AutoReview),
        /*source*/ None,
    );
    let mut requirements_toml = managed_config
        .config_layer_stack
        .requirements_toml()
        .clone();
    requirements_toml.allowed_approvals_reviewers = Some(vec![ApprovalsReviewer::AutoReview]);
    managed_config.config_layer_stack =
        codex_config::ConfigLayerStack::new(layers, requirements, requirements_toml)
            .expect("managed reviewer requirements");
    let managed_servers = apply_apps_server_policy(
        &managed_config,
        &snapshot,
        &ConnectorSnapshot::default(),
        snapshot
            .effective_mcp_servers()
            .into_iter()
            .collect::<Vec<_>>(),
    );
    assert_eq!(
        managed_servers
            .iter()
            .find(|(name, _)| name == "codex_apps__gmail")
            .and_then(|(_, server)| server.runtime_metadata().approvals_reviewer()),
        None,
        "a managed rejection must leave dynamic turn fallback intact"
    );
    assert_eq!(
        managed_servers
            .iter()
            .find(|(name, _)| name == "codex_apps__calendar")
            .and_then(|(_, server)| server.runtime_metadata().approvals_reviewer()),
        Some(ApprovalsReviewer::AutoReview)
    );

    std::fs::write(
        codex_home.path().join("config.toml"),
        "approvals_reviewer = \"auto_review\"\n",
    )
    .expect("remove Apps reviewer overrides");
    let no_override_config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .build()
        .await
        .expect("reload config without Apps reviewer override");
    let no_override_servers = apply_apps_server_policy(
        &no_override_config,
        &snapshot,
        &ConnectorSnapshot::default(),
        snapshot
            .effective_mcp_servers()
            .into_iter()
            .collect::<Vec<_>>(),
    );
    assert!(
        no_override_servers
            .iter()
            .all(|(_, server)| server.runtime_metadata().approvals_reviewer().is_none()),
        "removing an Apps override must restore dynamic per-turn fallback"
    );

    std::fs::write(
        codex_home.path().join("config.toml"),
        r#"
approvals_reviewer = "auto_review"

[apps._default]
approvals_reviewer = "user"
"#,
    )
    .expect("write default Apps reviewer");
    let default_reviewer_config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(codex_home.path().to_path_buf()))
        .build()
        .await
        .expect("reload config with default Apps reviewer");
    let default_reviewer_servers = apply_apps_server_policy(
        &default_reviewer_config,
        &snapshot,
        &ConnectorSnapshot::default(),
        snapshot
            .effective_mcp_servers()
            .into_iter()
            .collect::<Vec<_>>(),
    );
    assert_eq!(
        default_reviewer_servers
            .iter()
            .find(|(name, _)| name == "codex_apps__calendar")
            .and_then(|(_, server)| server.runtime_metadata().approvals_reviewer()),
        Some(ApprovalsReviewer::User),
        "the explicit Apps default overrides the thread reviewer"
    );

    apps.shutdown().await;
}
