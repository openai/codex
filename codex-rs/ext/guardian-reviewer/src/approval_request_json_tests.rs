#![cfg(test)]

use super::*;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;

#[test]
fn guardian_approval_request_to_json_renders_mcp_tool_call_shape() -> serde_json::Result<()> {
    let action = GuardianApprovalRequest::McpToolCall {
        id: "call-1".to_string(),
        server: "mcp_server".to_string(),
        tool_name: "browser_navigate".to_string(),
        arguments: Some(serde_json::json!({
            "url": "https://example.com",
        })),
        connector_id: None,
        connector_name: Some("Playwright".to_string()),
        connector_description: None,
        connected_account_email: Some("owner@example.com".to_string()),
        tool_title: Some("Navigate".to_string()),
        tool_description: None,
        annotations: Some(GuardianMcpAnnotations {
            destructive_hint: Some(true),
            open_world_hint: None,
            read_only_hint: Some(false),
        }),
    };

    assert_eq!(
        guardian_approval_request_to_json(&action)?,
        serde_json::json!({
            "tool": "mcp_tool_call",
            "server": "mcp_server",
            "tool_name": "browser_navigate",
            "arguments": {
                "url": "https://example.com",
            },
            "connector_name": "Playwright",
            "connected_account_email": "owner@example.com",
            "tool_title": "Navigate",
            "annotations": {
                "destructive_hint": true,
                "read_only_hint": false,
            },
        })
    );
    Ok(())
}

#[test]
fn guardian_approval_request_to_json_renders_network_access_trigger() -> serde_json::Result<()> {
    let cwd = PathUri::parse("file:///C:/repo").expect("valid Windows path URI");
    let action = GuardianApprovalRequest::NetworkAccess {
        id: "network-1".to_string(),
        turn_id: "turn-1".to_string(),
        environment_id: "local".to_string(),
        target: "https://example.com:443".to_string(),
        host: "example.com".to_string(),
        protocol: NetworkApprovalProtocol::Https,
        port: 443,
        trigger: Some(GuardianNetworkAccessTrigger {
            call_id: "call-1".to_string(),
            tool_name: "shell".to_string(),
            command: vec!["curl".to_string(), "https://example.com".to_string()],
            cwd,
            sandbox_permissions: SandboxPermissions::UseDefault,
            additional_permissions: None,
            justification: Some("Fetch the release metadata.".to_string()),
            tty: None,
        }),
    };

    assert_eq!(
        guardian_approval_request_to_json(&action)?,
        serde_json::json!({
            "tool": "network_access",
            "environment_id": "local",
            "target": "https://example.com:443",
            "host": "example.com",
            "protocol": "https",
            "port": 443,
            "trigger": {
                "callId": "call-1",
                "toolName": "shell",
                "command": ["curl", "https://example.com"],
                "cwd": "C:\\repo",
                "sandboxPermissions": "use_default",
                "justification": "Fetch the release metadata.",
            },
        })
    );

    Ok(())
}
