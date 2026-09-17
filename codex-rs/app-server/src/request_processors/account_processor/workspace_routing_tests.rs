//! Origin resolution and strict discovery validation.

use super::*;
use pretty_assertions::assert_eq;
use serde_json::Value;
use test_case::test_case;

#[test_case(Some("https://gov.chatgpt.com/backend-api/"), "NO_CONSTRAINT", "https://gov.chatgpt.com"; "configured_only")]
#[test_case(None, "https://gov.chatgpt.com", "https://gov.chatgpt.com"; "discovered_only")]
#[test_case(Some("https://GOV.chatgpt.com:443/backend-api/"), "https://gov.chatgpt.com", "https://gov.chatgpt.com"; "matching_effective_port")]
#[test_case(Some("https://example.com:8443/backend-api/"), "https://example.com:8443", "https://example.com:8443"; "custom_port")]
#[test_case(None, "NO_CONSTRAINT", "https://chatgpt.com"; "existing_default")]
fn resolves_origins(required: Option<&str>, discovered: &str, expected: &str) {
    let entry = serde_json::from_value(serde_json::json!({
        "id": "workspace", "workspace_backend_origin": discovered,
        "account_routing_override": "NO_CONSTRAINT",
    }))
    .unwrap();
    assert_eq!(
        resolve_routing(entry, required, "https://chatgpt.com/backend-api/").unwrap(),
        WorkspaceRouting {
            chatgpt_account_id: "workspace".into(),
            backend_origin: expected.into(),
            account_routing_override: "NO_CONSTRAINT".into(),
        }
    );
}

#[test_case("https://other.example/backend-api/", "https://gov.chatgpt.com"; "host_conflict")]
#[test_case("http://gov.chatgpt.com/backend-api/", "https://gov.chatgpt.com"; "scheme_conflict")]
#[test_case("https://gov.chatgpt.com:444/backend-api/", "https://gov.chatgpt.com"; "port_conflict")]
fn rejects_conflicting_origins(required: &str, discovered: &str) {
    let entry = serde_json::from_value(serde_json::json!({
        "id": "workspace", "workspace_backend_origin": discovered,
        "account_routing_override": "us_cr",
    }))
    .unwrap();
    assert!(resolve_routing(entry, Some(required), "https://chatgpt.com").is_err());
}

#[test_case(serde_json::json!(null), serde_json::json!("us_cr"), WorkspaceRoutingError::MissingBackendOrigin; "null_origin")]
#[test_case(serde_json::json!("NO_CONSTRAINT"), serde_json::json!(null), WorkspaceRoutingError::InvalidRoutingOverride; "null_routing")]
#[test_case(serde_json::json!(""), serde_json::json!("us_cr"), WorkspaceRoutingError::InvalidBackendUrl; "empty_origin")]
#[test_case(serde_json::json!("https://example.com/backend-api/"), serde_json::json!("us_cr"), WorkspaceRoutingError::BackendIsNotOrigin; "origin_with_path")]
#[test_case(serde_json::json!("https://example.com?query"), serde_json::json!("us_cr"), WorkspaceRoutingError::BackendIsNotOrigin; "origin_with_query")]
#[test_case(serde_json::json!("https://example.com#fragment"), serde_json::json!("us_cr"), WorkspaceRoutingError::BackendIsNotOrigin; "origin_with_fragment")]
#[test_case(serde_json::json!("https://user:pass@example.com"), serde_json::json!("us_cr"), WorkspaceRoutingError::InvalidBackendOrigin; "credentials")]
#[test_case(serde_json::json!("http://example.com"), serde_json::json!("us_cr"), WorkspaceRoutingError::InvalidBackendOrigin; "insecure_origin")]
#[test_case(serde_json::json!("NO_CONSTRAINT"), serde_json::json!("unknown"), WorkspaceRoutingError::InvalidRoutingOverride; "unknown_routing")]
#[test_case(serde_json::json!("NO_CONSTRAINT"), serde_json::json!(""), WorkspaceRoutingError::InvalidRoutingOverride; "empty_routing")]
fn rejects_invalid_discovery(backend: Value, routing: Value, expected: WorkspaceRoutingError) {
    let entry = serde_json::from_value(serde_json::json!({
        "id": "workspace", "workspace_backend_origin": backend, "account_routing_override": routing,
    }))
    .unwrap();
    assert_eq!(
        resolve_routing(
            entry,
            /*required_chatgpt_base_url*/ None,
            "https://chatgpt.com"
        ),
        Err(AccountReadError::Routing(expected))
    );
}

#[test]
fn unrestricted_discovery_uses_effective_custom_backend() {
    let entry = serde_json::from_value(serde_json::json!({
        "id": "workspace", "workspace_backend_origin": "NO_CONSTRAINT", "account_routing_override": "us",
    })).unwrap();
    assert_eq!(
        resolve_routing(
            entry,
            /*required_chatgpt_base_url*/ None,
            "https://custom.example:8443/backend-api/"
        )
        .unwrap(),
        WorkspaceRouting {
            chatgpt_account_id: "workspace".into(),
            backend_origin: "https://custom.example:8443".into(),
            account_routing_override: "us".into(),
        }
    );
}
