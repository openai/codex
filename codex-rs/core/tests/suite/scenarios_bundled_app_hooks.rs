#![cfg(test)]
//! The bundled Memory hook injects context at startup and after compaction without saved trust.

use super::*;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::recorded_apps_tool_calls;
use pretty_assertions::assert_eq;
use std::sync::Mutex;
use wiremock::Mock;
use wiremock::Request;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_partial_json;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bundled_memory_hook_injects_context() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let connector_id = "connector_openai_codex_dream_memory";
    let server = start_mock_server().await;
    let apps = AppsTestServer::mount_with_tools(&server, Arc::new(Mutex::new(vec![json!({
        "name": "codex_dream_memory.get_mec",
        "description": "Read memory context.",
        "inputSchema": { "type": "object", "properties": {} },
        "annotations": { "readOnlyHint": true },
        "_meta": {
            "connector_id": connector_id,
            "connector_name": "Memory",
            "ui": { "visibility": ["app"] },
            "_codex_apps": {
                "resource_uri": format!("connector://{connector_id}/tools/codex_dream_memory.get_mec"),
                "contains_mcp_source": true,
                "connector_id": connector_id,
            },
        },
    })]))).await?;
    let memory = ["The user prefers short, direct answers."; 1_000].join(" ");
    let hook_output = memory.clone();
    Mock::given(method("POST"))
        .and(path("/api/codex/ps/mcp"))
        .and(body_partial_json(json!({ "method": "tools/call" })))
        .respond_with(move |request: &Request| {
            let request: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
            ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
                "jsonrpc": "2.0", "id": request["id"],
                "result": { "content": [{ "type": "text", "text": json!({
                    "hookSpecificOutput": {
                        "hookEventName": "SessionStart", "additionalContext": hook_output,
                    },
                }).to_string() }] },
            }))
        })
        .with_priority(/*p*/ 1)
        .mount(&server)
        .await;
    let home = Arc::new(TempDir::new()?);
    let root = home
        .path()
        .join("plugins/cache/openai-bundled/codex-dream-memory/local");
    fs::create_dir_all(root.join(".codex-plugin"))?;
    fs::write(
        root.join(".codex-plugin/plugin.json"),
        serde_json::to_vec(&json!({
            "name": "codex-dream-memory",
            "hooks": { "hooks": { "SessionStart": [{
                "matcher": "^(startup|compact)$",
                "hooks": [{
                    "type": "mcp_tool",
                    "server": "codex_apps",
                    "tool": "codex_dream_memory.get_mec",
                    "additionalContextLimit": 10_000,
                }],
            }] } },
        }))?,
    )?;
    fs::write(
        root.join(".app.json"),
        serde_json::to_vec(&json!({
            "apps": { "memory": { "id": "connector_openai_codex_dream_memory" } },
        }))?,
    )?;
    let plugin_config = "[features]\napps = true\nplugins = true\nhooks = true\n[plugins.\"codex-dream-memory@openai-bundled\"]\nenabled = true\n[apps.connector_openai_codex_dream_memory]\n";
    fs::write(home.path().join("config.toml"), plugin_config)?;
    let mut builder = test_codex()
        .with_home(home)
        .with_model("gpt-5.5")
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(move |config| {
            configure_scenario_catalog(config);
            config.chatgpt_base_url = apps.chatgpt_base_url;
            for feature in [Feature::Apps, Feature::Plugins, Feature::CodexHooks] {
                config.features.enable(feature).unwrap();
            }
            config
                .features
                .disable(Feature::ExecutorCapabilityDiscovery)
                .unwrap();
            // Exercise local compaction using the ordinary mock Responses endpoint.
            config.model_provider.name = "OpenAI (test)".into();
        });
    let test = builder.build_with_auto_env(&server).await?;
    assert!(!test.config.bypass_hook_trust);
    let responses = mount_sse_sequence(
        &server,
        (0..3)
            .map(|index| {
                sse(vec![
                    ev_assistant_message(&format!("message-{index}"), "Done."),
                    ev_completed(&format!("response-{index}")),
                ])
            })
            .collect(),
    )
    .await;
    test.submit_text_turn("Remember my preferences.").await?;
    assert_eq!(recorded_apps_tool_calls(&server).await.len(), 1);
    test.codex.submit(Op::Compact).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.submit_text_turn("Continue after compaction.").await?;
    let calls = recorded_apps_tool_calls(&server).await;
    assert_eq!(calls.len(), 2);
    for call in calls {
        assert_eq!(call["params"]["arguments"], json!({}));
        assert_eq!(call["params"]["name"], "codex_dream_memory.get_mec");
    }
    let requests = responses.requests();
    for request in [&requests[0], &requests[2]] {
        assert!(request.message_input_texts("developer").contains(&memory));
    }
    insta::assert_snapshot!(
        "bundled_memory_hook_context",
        context_snapshot::format_request_history_snapshot(
            "A bundled App-only hook injects memory at startup and after compaction without saved trust.",
            &requests,
            &ContextSnapshotOptions::default().rewrite_known_segments(),
        )
    );
    Ok(())
}
