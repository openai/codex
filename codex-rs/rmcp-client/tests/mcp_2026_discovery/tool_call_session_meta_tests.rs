//! Verifies metadata on the actual HTTP session-expiry retry and subsequent tool calls.

use super::*;
use pretty_assertions::assert_eq;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

#[tokio::test]
async fn recovered_tool_calls_use_current_session_capabilities() -> anyhow::Result<()> {
    for (initial_support, recovered_support, fail_policy) in [
        (false, true, false),
        (true, false, false),
        (true, true, false),
        (false, false, false),
        (false, true, true),
    ] {
        let server = MockServer::start().await;
        let handshakes = Arc::new(AtomicUsize::new(0));
        let calls = Arc::new(AtomicUsize::new(0));
        let observed_handshakes = Arc::clone(&handshakes);
        let observed_calls = Arc::clone(&calls);
        Mock::given(method("POST"))
            .and(path("/mcp"))
            .respond_with(move |request: &Request| {
                let body: Value = request.body_json().expect("JSON-RPC request");
                match body["method"].as_str() {
                    Some("server/discover" | "initialize") => {
                        let initial = observed_handshakes.fetch_add(1, Ordering::SeqCst) == 0;
                        let supports_policy = if initial {
                            initial_support
                        } else {
                            recovered_support
                        };
                        let mut capabilities = json!({"tools": {}});
                        if supports_policy {
                            capabilities["experimental"] = json!({"codex/network-policy-meta": {}});
                        }
                        let mut result = if body["method"] == "server/discover" {
                            modern_discover_result(&body)
                        } else {
                            json!({"jsonrpc": "2.0", "id": body["id"], "result": {
                                "protocolVersion": LEGACY_VERSION,
                                "serverInfo": {"name": "recovery-test", "version": "1.0"},
                            }})
                        };
                        result["result"]["capabilities"] = capabilities;
                        ResponseTemplate::new(200)
                            .insert_header(
                                "mcp-session-id",
                                if initial { "initial" } else { "recovered" },
                            )
                            .set_body_json(result)
                    }
                    Some("notifications/initialized") => ResponseTemplate::new(202),
                    Some("tools/call") => {
                        // The first call succeeds; the next expires the old session.
                        if observed_calls.fetch_add(1, Ordering::SeqCst) == 1 {
                            return ResponseTemplate::new(404);
                        }
                        ResponseTemplate::new(200).set_body_json(json!({
                            "jsonrpc": "2.0", "id": body["id"], "result": {
                                "resultType": "complete", "content": [],
                            },
                        }))
                    }
                    other => panic!("unexpected request: {other:?}"),
                }
            })
            .mount(&server)
            .await;
        let client = create_client(&server, McpProtocolMode::Legacy)
            .await?
            .with_read_only_tools(/*requires_read_only_tools*/ true);
        initialize_client(&client).await?;
        for phase in ["warmup", "recovery", "later"] {
            let result = client
                .call_tool(
                    "echo".to_string(),
                    Some(json!({"message": phase})),
                    |capabilities| {
                        let mut meta = json!({"callId": phase, "openai/readOnly": false});
                        if capabilities
                            .experimental
                            .as_ref()
                            .is_some_and(|experimental| {
                                experimental.contains_key("codex/network-policy-meta")
                            })
                        {
                            anyhow::ensure!(!fail_policy, "policy source unavailable");
                            meta["codex/network-policy-meta"] = json!({"enabled": false});
                        }
                        Ok(Some(meta))
                    },
                    /*timeout*/ None,
                )
                .await;
            if fail_policy && phase != "warmup" {
                assert!(
                    result
                        .unwrap_err()
                        .to_string()
                        .contains("policy source unavailable")
                );
            } else {
                result?;
            }
        }
        let requests = server.received_requests().await.expect("recorded requests")
                .into_iter()
                .filter(|request| request.method == "POST")
                .filter_map(|request| {
                    let body: Value = request.body_json().expect("JSON-RPC request");
                    (body["method"] == "tools/call").then(|| {
                        let meta = &body["params"]["_meta"];
                        json!({
                            "session": request.headers.get("mcp-session-id").map(|value| value.to_str().unwrap()),
                            "callId": meta["callId"],
                            "policy": meta["codex/network-policy-meta"],
                            "readOnly": meta["openai/readOnly"],
                        })
                    })
                }).collect::<Vec<_>>();
        let initial_policy = initial_support.then(|| json!({"enabled": false}));
        let recovered_policy = recovered_support.then(|| json!({"enabled": false}));
        let mut expected = vec![
            json!({"session": "initial", "callId": "warmup", "policy": initial_policy, "readOnly": true}),
            json!({"session": "initial", "callId": "recovery", "policy": initial_policy, "readOnly": true}),
        ];
        if !fail_policy {
            expected.extend([
                    json!({"session": "recovered", "callId": "recovery", "policy": recovered_policy, "readOnly": true}),
                    json!({"session": "recovered", "callId": "later", "policy": recovered_policy, "readOnly": true}),
                ]);
        }
        assert_eq!(requests, expected);
        assert_eq!(handshakes.load(Ordering::SeqCst), 2);
        client.shutdown().await;
    }
    Ok(())
}
