#![cfg(test)]
//! Tool metadata identifies global picker choices. Selected resources use the displayed thread.

use super::inventory::StatuslineServer;
use super::test_support::Requests;
use super::test_support::reply;
use super::test_support::row;
use super::test_support::test_client;
use super::*;
use codex_app_server_protocol::McpServerStatus;
use pretty_assertions::assert_eq;
use serde_json::json;

fn server(raw: Value) -> StatuslineServer {
    StatuslineServer::try_from(serde_json::from_value::<McpServerStatus>(raw).unwrap()).unwrap()
}

fn statusline(uri: &str) -> Value {
    json!({"type":"statusline","resourceUri":uri})
}

fn tool(name: &str, entries: Value) -> Value {
    json!({"name":name,"inputSchema":{"type":"object"},"_meta":{"openai/ui":{"entrypoints":entries}}})
}

#[test]
fn a_tool_contributes_exactly_one_valid_statusline_entry() {
    let good = statusline("ui://demo/a%20b");
    let malformed = [
        Value::Null,
        json!({"type":"statusline"}),
        json!({"type":"statusline","resourceUri":12}),
        statusline("https://example.test/footer"),
        statusline("ui://["),
        json!({"type":"desktop","resourceUri":"ui://demo/window"}),
        statusline("ui://demo/ba\nd"),
        statusline("ui://demo/%"),
    ];
    for extra in &malformed {
        let mut raw = row("demo", vec![tool("status", json!([extra, good, extra]))]);
        raw["pluginId"] = json!("calendar@local");
        let row = server(raw);
        let providers = row.providers().unwrap();
        let [provider] = providers.as_slice() else {
            panic!("expected one statusline provider")
        };
        assert_eq!(
            (provider.uri.as_str(), provider.plugin_label.as_deref()),
            ("ui://demo/a%20b", Some("calendar@local"))
        );
    }
    for entries in [
        json!(malformed),
        json!([good, good]),
        json!([good, statusline("ui://demo/other")]),
        json!({}),
    ] {
        let row = server(row("demo", vec![tool("status", entries)]));
        assert!(row.providers().unwrap().is_empty());
    }
}

async fn scoped_config(commands: &mut Requests, cwd: &str, layers: &[Value]) {
    reply(
        commands,
        "config/read",
        json!({"includeLayers":true,"cwd":cwd}),
        json!({"config":{},"layers":layers}),
    )
    .await;
}

async fn named(commands: &mut Requests, thread: Option<ThreadId>, row: Value) {
    reply(
        commands,
        "mcpServerStatus/list",
        json!({"threadId":thread.map(|id| id.to_string()),"serverName":row["name"],
               "environmentId":null,"detail":"toolsAndAuthOnly","cursor":null,"limit":null}),
        json!({"data":[row],"nextCursor":null}),
    )
    .await;
}

#[tokio::test]
async fn picker_choice_reads_the_displayed_threads_uri_with_the_snapshot() -> Result<()> {
    let thread = ThreadId::new();
    let cwd = PathUri::parse("file:///C:/workspace")?;
    let listing = |uri: &str| row("calendar", vec![tool("next", json!([statusline(uri)]))]);
    let known = server(listing("ui://calendar/global"))
        .providers()?
        .pop()
        .unwrap();
    let snapshot = json!({"version":1,"thread":{"id":thread.to_string()}});
    let layers = [
        json!({"name":{"type":"sessionFlags"},"config":{"mcp_servers":{"calendar":{"command":"calendar"}}}}),
        json!({"name":{"type":"project"},"config":{"mcp_servers":{"calendar":{"command":"project"}}}}),
        json!({"name":{"type":"project"},"disabledReason":"untrusted",
            "config":{"mcp_servers":{"calendar":{"args":["ignored"]}}}}),
    ];
    let (client, mut requests) = test_client();
    let host = async {
        scoped_config(&mut requests, r"C:\workspace", &layers).await;
        let mut live = listing("ui://calendar/today");
        live["tools"]["next"]["title"] = json!("Next event");
        named(&mut requests, Some(thread), live).await;
        reply(
            &mut requests,
            "mcpServer/resource/read",
            json!({
                "threadId": thread.to_string(),
                "server": "calendar",
                "environmentId": null,
                "uri": "ui://calendar/today",
                "connectorId": null,
                "target": null,
                "originCallId": null,
                "_meta": {"com.openai/codex-statusline-state": snapshot},
            }),
            json!({"contents": [], "originCallId": null}),
        )
        .await;
    };
    let read = async {
        let provider = ResolvedStatuslineProvider::lookup(
            &client,
            thread,
            &cwd,
            ProviderSelection::Discovered(&known),
        )
        .await?
        .unwrap();
        assert_eq!(provider.provider().title, "Next event");
        provider.read(snapshot.clone()).await
    };
    let (result, ()) = tokio::join!(read, host);
    result?;
    Ok(())
}

#[tokio::test]
async fn visible_project_registration_blocks_the_selected_provider() -> Result<()> {
    let cwd = PathUri::parse("file:///workspace/b")?;
    for (owner, session, project) in [
        (
            None,
            json!({"mcp_servers":{"calendar":{"command":"calendar"}}}),
            json!({"mcp_servers":{"calendar":{"command":"replacement","args":["from-project"]}}}),
        ),
        (
            Some("daily@local"),
            json!({}),
            json!({"plugins":{"daily@local":{"release_id":"replacement"}}}),
        ),
        (
            Some("daily@local"),
            json!({}),
            json!({"marketplaces":{"local":{"source_type":"local","source":"/replacement"}}}),
        ),
    ] {
        let mut listing = row(
            "calendar",
            vec![tool("today", json!([statusline("ui://calendar/today")]))],
        );
        listing["pluginId"] = json!(owner);
        let known = server(listing).providers()?.pop().unwrap();
        let (client, mut requests) = test_client();
        let host = async move {
            scoped_config(
                &mut requests,
                "/workspace/b",
                &[
                    json!({"name":{"type":"sessionFlags"},"config":session}),
                    json!({"name":{"type":"project"},"config":project}),
                ],
            )
            .await;
        };
        let lookup = ResolvedStatuslineProvider::lookup(
            &client,
            ThreadId::new(),
            &cwd,
            ProviderSelection::Discovered(&known),
        );
        let (provider, ()) = tokio::join!(lookup, host);
        assert!(provider?.is_none());
    }
    Ok(())
}

#[tokio::test]
async fn selecting_a_plugin_requires_the_globally_advertised_tool_and_the_connected_owner()
-> Result<()> {
    let (client, mut commands) = test_client();
    let thread = ThreadId::new();
    let named_plugin = |name| {
        let mut value = row(
            "calendar",
            vec![tool(name, json!([statusline("ui://calendar/status")]))],
        );
        value["pluginId"] = json!("daily@local");
        value
    };
    let global = named_plugin("status");
    let mut replacement = global.clone();
    replacement["pluginId"] = json!("project@local");
    replacement["runtimeStatus"] = json!("failed");
    let mut disconnected = global.clone();
    disconnected["runtimeStatus"] = json!("failed");
    let mut list_failure = disconnected.clone();
    list_failure["toolsError"] = json!("calendar catalog failed");
    let mut unverified = global.clone();
    unverified.as_object_mut().unwrap().remove("httpOrigin");
    let unavailable = [
        (named_plugin("other"), None),
        (global.clone(), Some(replacement)),
        (global.clone(), Some(named_plugin("renamed"))),
        (global.clone(), Some(unverified)),
    ];
    let rejected = unavailable.len();
    let mut live = global.clone();
    live["tools"]["status"]["_meta"]["openai/ui"]["entrypoints"] =
        json!([statusline("ui://calendar/current")]);
    let worker = tokio::spawn(async move {
        for (global, live) in unavailable.into_iter().chain([
            (global.clone(), Some(disconnected)),
            (global.clone(), Some(list_failure)),
            (global, Some(live)),
        ]) {
            scoped_config(&mut commands, "/workspace/a", &[]).await;
            named(&mut commands, /*thread*/ None, global).await;
            if let Some(live) = live {
                named(&mut commands, Some(thread), live).await;
            }
        }
        reply(
            &mut commands,
            "mcpServer/resource/read",
            json!({
                "threadId":thread.to_string(),"server":"calendar","uri":"ui://calendar/current",
                "environmentId":null,"connectorId":null,"target":null,"originCallId":null,
                "_meta":{"com.openai/codex-statusline-state":{"version":1}}
            }),
            json!({"contents":[],"originCallId":null}),
        )
        .await;
    });
    let cwd = PathUri::parse("file:///workspace/a")?;
    let id = ProviderId {
        server: "calendar".into(),
        tool: "status".into(),
    };
    let lookup =
        || ResolvedStatuslineProvider::lookup(&client, thread, &cwd, ProviderSelection::Saved(&id));
    for _ in 0..rejected {
        assert!(lookup().await?.is_none());
    }
    for message in ["is not connected", "calendar catalog failed"] {
        assert!(
            lookup()
                .await
                .is_err_and(|e| e.to_string().contains(message))
        );
    }
    lookup().await?.unwrap().read(json!({"version":1})).await?;
    worker.await?;
    Ok(())
}
