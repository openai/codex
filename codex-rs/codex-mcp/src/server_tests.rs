use std::sync::Arc;

use codex_config::McpServerConfig;
use codex_config::McpServerToolConfig;
use codex_config::McpToolApproval;
use codex_config::types::ApprovalsReviewer;
use codex_protocol::mcp_approval_meta::McpToolSource;
use pretty_assertions::assert_eq;
use serde_json::json;

use super::EffectiveMcpServer;
use super::McpServerMetadata;
use super::McpServerRuntimeMetadata;
use super::McpToolApprovalIdentity;
use super::McpToolApprovalParameterLabel;
use super::McpToolApprovalPersistence;
use super::McpToolApprovalPresentation;
use super::McpToolRuntimeMetadata;
use super::McpToolTelemetryIdentity;
use super::RuntimeBearerTokenError;

fn config(value: serde_json::Value) -> McpServerConfig {
    serde_json::from_value(value).expect("valid MCP server config")
}

#[test]
fn runtime_bearer_token_requires_unambiguous_http_configuration() {
    assert_eq!(
        EffectiveMcpServer::configured_with_runtime_bearer_token(
            config(json!({"command": "echo"})),
            "secret".to_string(),
        )
        .expect_err("stdio must reject HTTP bearer tokens"),
        RuntimeBearerTokenError::UnsupportedTransport
    );
    assert_eq!(
        EffectiveMcpServer::configured_with_runtime_bearer_token(
            config(json!({"url": "http://127.0.0.1/mcp"})),
            String::new(),
        )
        .expect_err("empty bearer token must be rejected"),
        RuntimeBearerTokenError::EmptyToken
    );
    assert_eq!(
        EffectiveMcpServer::configured_with_runtime_bearer_token(
            config(json!({
                "url": "http://127.0.0.1/mcp",
                "http_headers": {"Authorization": "Bearer configured"},
            })),
            "runtime-secret".to_string(),
        )
        .expect_err("configured authorization must not compete with runtime auth"),
        RuntimeBearerTokenError::ConflictingAuthorization
    );

    let mut server = EffectiveMcpServer::configured_with_runtime_bearer_token(
        config(json!({"url": "http://127.0.0.1/mcp"})),
        "runtime-secret".to_string(),
    )
    .expect("valid runtime bearer token");
    server.set_enabled(/*enabled*/ false);
    assert!(!server.enabled());
    let debug = format!("{server:?}");
    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains("runtime-secret"));
}

#[test]
fn runtime_owner_is_redacted_retained_and_compared_by_pointer() {
    let owner = Arc::new("private-runtime-owner".to_string());
    let weak_owner = Arc::downgrade(&owner);
    let server = EffectiveMcpServer::configured(config(json!({
        "url": "http://127.0.0.1:1234/mcp"
    })))
    .with_runtime_owner(Arc::clone(&owner));
    drop(owner);

    assert!(weak_owner.upgrade().is_some());
    let cloned = server.clone();
    assert_eq!(server, cloned);
    let different_owner = EffectiveMcpServer::configured(config(json!({"command": "echo"})))
        .with_runtime_owner(Arc::new("private-runtime-owner".to_string()));
    assert_ne!(server, different_owner);
    let debug = format!("{server:?}");
    assert!(debug.contains("[REDACTED RUNTIME OWNER]"));
    assert!(!debug.contains("private-runtime-owner"));
    let serialized_config =
        serde_json::to_string(server.config()).expect("serialize configured server");
    assert!(!serialized_config.contains("private-runtime-owner"));

    let metadata = McpServerMetadata::from(&server);
    let metadata_debug = format!("{metadata:?}");
    assert!(metadata_debug.contains("[REDACTED RUNTIME OWNER]"));
    assert!(!metadata_debug.contains("private-runtime-owner"));

    drop(cloned);
    drop(server);
    assert!(weak_owner.upgrade().is_some());
    drop(metadata);
    assert!(weak_owner.upgrade().is_none());
}

#[test]
fn approvals_reviewer_is_runtime_metadata() {
    let runtime_metadata = McpServerRuntimeMetadata::default()
        .with_trusted_tool_input()
        .with_approvals_reviewer(ApprovalsReviewer::AutoReview);
    let server = EffectiveMcpServer::configured(config(json!({"command": "echo"})))
        .with_runtime_metadata(runtime_metadata);

    assert_eq!(
        server.runtime_metadata().approvals_reviewer(),
        Some(ApprovalsReviewer::AutoReview)
    );
    assert_eq!(
        McpServerMetadata::from(&server).approvals_reviewer,
        Some(ApprovalsReviewer::AutoReview)
    );
    let serialized = serde_json::to_string(server.config()).expect("serialize config");
    assert!(!serialized.contains("approvals_reviewer"));
}

#[test]
fn physical_tools_list_metric_is_enabled_unless_runtime_owner_suppresses_it() {
    assert!(McpServerRuntimeMetadata::default().records_physical_tools_list_metric());
    assert!(
        !McpServerRuntimeMetadata::default()
            .without_physical_tools_list_metric()
            .records_physical_tools_list_metric()
    );
}

#[test]
fn runtime_tool_metadata_survives_server_launch_without_entering_config() {
    let presentation = McpToolApprovalPresentation::new(
        "Allow Docs to publish?".to_string(),
        vec![
            McpToolApprovalParameterLabel::new("title".to_string(), "Title".to_string())
                .expect("valid label"),
        ],
    )
    .expect("valid presentation");
    let persistence = McpToolApprovalPersistence::new(|| async { Ok(()) });
    let approval_identity = McpToolApprovalIdentity::new(
        /*server_name*/ "legacy-server",
        /*source_id*/ "source-1",
        /*tool_name*/ "RawPublish",
    )
    .expect("valid identity");
    let telemetry_identity =
        McpToolTelemetryIdentity::new("legacy-server", "RawPublish").expect("valid identity");
    let runtime_tool = McpToolRuntimeMetadata::default()
        .with_approval_identity(approval_identity.clone())
        .with_approval_presentation(presentation.clone())
        .with_approval_header("  Approve hosted tool call?  ")
        .with_approval_form_metadata(
            json!({
                "brand": "hosted",
                "nested": {"id": "opaque"},
            })
            .as_object()
            .expect("form metadata object")
            .clone(),
        )
        .with_approval_persistence(persistence.clone())
        .with_approval_source(
            McpToolSource::new(
                "source-1",
                "Documents",
                Some("Search company documents.".to_string()),
            )
            .expect("valid runtime source"),
        )
        .with_metric_labels([("source_id", "source-1"), ("source_name", "Documents")])
        .with_telemetry_identity(telemetry_identity.clone());
    let server = EffectiveMcpServer::configured(config(json!({"command": "echo"})))
        .with_runtime_metadata(
            McpServerRuntimeMetadata::default()
                .with_telemetry_origin("https://hosted.example/ps/mcp")
                .with_tools(std::collections::HashMap::from([(
                    "publish".to_string(),
                    runtime_tool,
                )]))
                .with_trusted_tool_input()
                .with_trusted_approval_context()
                .with_primary_turn_sandbox_state(),
        );

    let serialized = serde_json::to_string(server.config()).expect("serialize config");
    assert!(!serialized.contains("Allow Docs"));
    assert!(!serialized.contains("runtime_tools"));
    assert!(!serialized.contains("PrimaryTurnEnvironment"));
    assert!(!serialized.contains("hosted.example"));

    let metadata = McpServerMetadata::from(&server);
    assert_eq!(
        metadata.origin.as_ref().map(super::McpServerOrigin::as_str),
        Some("https://hosted.example")
    );
    let launched = metadata
        .tool_runtime_metadata
        .get("publish")
        .expect("runtime tool metadata");
    assert_eq!(launched.approval_identity(), Some(&approval_identity));
    assert_eq!(launched.approval_presentation(), Some(&presentation));
    assert_eq!(
        launched.approval_header(),
        Some("Approve hosted tool call?")
    );
    assert_eq!(
        launched.approval_form_metadata(),
        json!({
            "brand": "hosted",
            "nested": {"id": "opaque"},
        })
        .as_object()
        .expect("form metadata object")
    );
    assert_eq!(launched.approval_persistence(), Some(&persistence));
    let source = launched.approval_source().expect("runtime approval source");
    assert_eq!(source.id(), "source-1");
    assert_eq!(source.name(), "Documents");
    assert_eq!(source.description(), Some("Search company documents."));
    assert_eq!(
        launched.metric_labels(),
        &[
            ("source_id".to_string(), "source-1".to_string()),
            ("source_name".to_string(), "Documents".to_string()),
        ]
    );
    assert_eq!(launched.telemetry_identity(), Some(&telemetry_identity));
    assert!(metadata.trusts_tool_input);
    assert!(metadata.trusts_approval_context);
    assert_eq!(
        metadata.sandbox_state_source,
        super::McpSandboxStateSource::PrimaryTurnEnvironment
    );
}

#[test]
fn ordinary_tool_runtime_metadata_has_no_approval_branding() {
    let metadata = McpToolRuntimeMetadata::default();

    assert_eq!(metadata.approval_header(), None);
    assert!(metadata.approval_form_metadata().is_empty());
    assert!(metadata.approval_identity().is_none());
    assert!(metadata.approval_source().is_none());
    assert!(metadata.metric_labels().is_empty());
    assert!(metadata.telemetry_identity().is_none());
}

#[test]
fn runtime_approval_identity_is_exact_distinct_and_bounded() {
    let identity = McpToolApprovalIdentity::new(
        /*server_name*/ "  stable-server  ",
        /*source_id*/ "  source-1  ",
        /*tool_name*/ "  RawTool  ",
    )
    .expect("valid identity");
    assert_eq!(identity.server_name(), "  stable-server  ");
    assert_eq!(identity.source_id(), "  source-1  ");
    assert_eq!(identity.tool_name(), "  RawTool  ");
    let unpadded = McpToolApprovalIdentity::new(
        /*server_name*/ "stable-server",
        /*source_id*/ "source-1",
        /*tool_name*/ "RawTool",
    )
    .expect("valid unpadded identity");
    assert_ne!(identity, unpadded);

    assert!(
        McpToolApprovalIdentity::new(
            /*server_name*/ "", /*source_id*/ "source", /*tool_name*/ "tool",
        )
        .is_none()
    );
    assert!(
        McpToolApprovalIdentity::new(
            /*server_name*/ "server", /*source_id*/ " ", /*tool_name*/ "tool",
        )
        .is_none()
    );
    assert!(
        McpToolApprovalIdentity::new(
            /*server_name*/ "server", /*source_id*/ "source", /*tool_name*/ " ",
        )
        .is_none()
    );

    let long_server = "s".repeat(257);
    let long_source = "o".repeat(257);
    let long_tool = "t".repeat(257);
    let hashed = McpToolApprovalIdentity::new(
        /*server_name*/ &long_server,
        /*source_id*/ &long_source,
        /*tool_name*/ &long_tool,
    )
    .expect("nonempty long identity is represented by bounded hashes");
    let same = McpToolApprovalIdentity::new(
        /*server_name*/ &long_server,
        /*source_id*/ &long_source,
        /*tool_name*/ &long_tool,
    )
    .expect("same long identity");
    let distinct = McpToolApprovalIdentity::new(
        /*server_name*/ long_server,
        /*source_id*/ format!("{long_source}x"),
        /*tool_name*/ long_tool,
    )
    .expect("distinct long identity");
    assert_eq!(hashed, same);
    assert_ne!(hashed, distinct);
    for component in [hashed.server_name(), hashed.source_id(), hashed.tool_name()] {
        assert!(component.starts_with("sha256:"));
        assert_eq!(component.len(), 71);
    }

    let hashed_tool = McpToolApprovalIdentity::new(
        /*server_name*/ "server",
        /*source_id*/ "source",
        /*tool_name*/ "t".repeat(257),
    )
    .expect("hashed long tool identity");
    let literal_hash_name = McpToolApprovalIdentity::new(
        /*server_name*/ "server",
        /*source_id*/ "source",
        /*tool_name*/ hashed_tool.tool_name(),
    )
    .expect("literal hash-shaped tool identity");
    assert_ne!(hashed_tool, literal_hash_name);
    assert_ne!(
        serde_json::to_string(&hashed_tool).expect("serialize hashed identity"),
        serde_json::to_string(&literal_hash_name).expect("serialize raw identity")
    );
}

#[test]
fn runtime_telemetry_identity_is_trimmed_and_bounded() {
    let identity =
        McpToolTelemetryIdentity::new("  stable-server  ", "  RawTool  ").expect("valid identity");
    assert_eq!(identity.server_name(), "stable-server");
    assert_eq!(identity.tool_name(), "RawTool");

    assert!(McpToolTelemetryIdentity::new("", "tool").is_none());
    assert!(McpToolTelemetryIdentity::new("server", " ").is_none());
    assert!(McpToolTelemetryIdentity::new("s".repeat(257), "tool").is_none());
    assert!(McpToolTelemetryIdentity::new("server", "t".repeat(257)).is_none());
}

#[test]
fn runtime_metric_labels_are_validated_deduplicated_and_bounded() {
    let labels = [
        (String::new(), "empty".to_string()),
        ("invalid-key".to_string(), "invalid".to_string()),
        ("label_0".to_string(), "x".repeat(300)),
        ("label_0".to_string(), "duplicate".to_string()),
    ]
    .into_iter()
    .chain((1..10).map(|index| (format!("label_{index}"), "x".repeat(300))));

    let metadata = McpToolRuntimeMetadata::default().with_metric_labels(labels);

    assert_eq!(metadata.metric_labels().len(), 8);
    assert!(
        metadata
            .metric_labels()
            .iter()
            .all(|(key, value)| key.starts_with("label_") && value.chars().count() == 256)
    );
}

#[test]
fn tool_policy_updates_only_serializable_tool_fields() {
    let server = EffectiveMcpServer::configured_with_runtime_bearer_token(
        config(json!({
            "url": "http://127.0.0.1/mcp",
            "enabled": false,
            "required": true,
        })),
        "runtime-secret".to_string(),
    )
    .expect("valid runtime bearer token")
    .with_tool_policy(
        vec!["search".to_string()],
        std::collections::HashMap::from([(
            "search".to_string(),
            McpServerToolConfig {
                approval_mode: Some(McpToolApproval::Approve),
            },
        )]),
    );

    assert_eq!(
        server.config().enabled_tools,
        Some(vec!["search".to_string()])
    );
    assert_eq!(
        server.config().tools,
        std::collections::HashMap::from([(
            "search".to_string(),
            McpServerToolConfig {
                approval_mode: Some(McpToolApproval::Approve),
            },
        )])
    );
    assert!(!server.enabled());
    assert!(server.required());
    assert_eq!(server.runtime_bearer_token(), Some("runtime-secret"));
}
