use pretty_assertions::assert_eq;
use pretty_assertions::assert_ne;
use reqwest::StatusCode;
use reqwest::header::ORIGIN;

use super::CODEX_APPS_RESOURCE_MCP_SERVER_NAME;
use crate::names::MAX_VIRTUAL_MCP_IDENTIFIER_BYTES;
use crate::tests::apps_with_tools;
use crate::tests::connector_tool;

#[tokio::test]
async fn loopback_routes_isolate_bearers_and_reject_authenticated_origins() {
    let (apps, _) = apps_with_tools(vec![
        connector_tool(Some("gmail"), Some("Gmail"), "GmailSearch"),
        connector_tool(Some("calendar"), Some("Calendar"), "CalendarList"),
    ])
    .await;
    let snapshot = apps.snapshot();
    let http_server = &snapshot.owner.generation.http_server;
    let gmail_token = http_server
        .bearer_token("codex_apps__gmail")
        .expect("Gmail route bearer");
    let calendar_token = http_server
        .bearer_token("codex_apps__calendar")
        .expect("Calendar route bearer");
    let resource_token = http_server
        .bearer_token(CODEX_APPS_RESOURCE_MCP_SERVER_NAME)
        .expect("resource route bearer");

    assert_ne!(gmail_token, calendar_token);
    assert_ne!(gmail_token, resource_token);
    assert_ne!(calendar_token, resource_token);

    let client = reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("HTTP client");
    for (url, wrong_token) in [
        (http_server.url("codex_apps__gmail"), calendar_token),
        (http_server.url("codex_apps__calendar"), resource_token),
        (
            http_server.url(CODEX_APPS_RESOURCE_MCP_SERVER_NAME),
            gmail_token,
        ),
    ] {
        assert_eq!(
            client
                .post(url)
                .bearer_auth(wrong_token)
                .send()
                .await
                .expect("cross-route request")
                .status(),
            StatusCode::UNAUTHORIZED,
        );
    }

    for (url, correct_token) in [
        (http_server.url("codex_apps__gmail"), gmail_token),
        (
            http_server.url(CODEX_APPS_RESOURCE_MCP_SERVER_NAME),
            resource_token,
        ),
    ] {
        assert_eq!(
            client
                .post(url)
                .bearer_auth(correct_token)
                .header(ORIGIN, "https://example.com")
                .send()
                .await
                .expect("authenticated browser-origin request")
                .status(),
            StatusCode::FORBIDDEN,
        );
    }

    apps.shutdown().await;
}

#[tokio::test]
async fn connector_server_and_tool_identifiers_are_bounded_before_registration() {
    let shared_connector_prefix = "VeryLongConnector".repeat(12);
    let first_connector_name = format!("{shared_connector_prefix}🙂一");
    let second_connector_name = format!("{shared_connector_prefix}🙂二");
    let shared_tool_prefix = "VeryLongOperation".repeat(12);
    let (apps, _) = apps_with_tools(vec![
        connector_tool(
            Some("first"),
            Some(&first_connector_name),
            &format!("{shared_tool_prefix}🙂one"),
        ),
        connector_tool(
            Some("first"),
            Some(&first_connector_name),
            &format!("{shared_tool_prefix}🙂two"),
        ),
        connector_tool(
            Some("second"),
            Some(&second_connector_name),
            &format!("{shared_tool_prefix}🙂three"),
        ),
    ])
    .await;
    let snapshot = apps.snapshot();
    let server_names = snapshot
        .all_connectors()
        .iter()
        .map(super::CodexApp::mcp_server_name)
        .collect::<Vec<_>>();
    let tool_names = snapshot
        .tools()
        .map(|(_, tool_name, _)| tool_name)
        .collect::<Vec<_>>();

    assert_eq!(server_names.len(), 2);
    assert_ne!(server_names[0], server_names[1]);
    assert_eq!(tool_names.len(), 3);
    assert_eq!(
        tool_names
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        3
    );
    for identifier in server_names.into_iter().chain(tool_names) {
        assert!(
            identifier.len() <= MAX_VIRTUAL_MCP_IDENTIFIER_BYTES,
            "unbounded identifier: {identifier}"
        );
        assert!(
            identifier
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'),
            "invalid MCP identifier: {identifier}"
        );
    }

    apps.shutdown().await;
}
