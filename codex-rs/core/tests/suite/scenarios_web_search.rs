//! Model-visible context for standalone web search.

use super::*;
use anyhow::Context;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::openai_models::WebSearchProvider;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::apps_enabled_builder;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_tool_search_call;
use core_test_support::test_codex::TestCodexBuilder;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_partial_json;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn literature_search_and_open() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let search =
        json!({"literature_search_query": [{"q": "FlashAttention-2", "domains": ["arxiv.org"]}]});
    let open = json!({"open": [{"ref_id": "turn0academia0"}]});
    for (commands, output) in [
        (&search, "[turn0academia0] FlashAttention-2 by Tri Dao"),
        (
            &open,
            "FlashAttention-2 reports a 2x speedup over FlashAttention.",
        ),
    ] {
        Mock::given(method("POST"))
            .and(path("/v1/alpha/search"))
            .and(body_partial_json(json!({"commands": commands})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"output": output})))
            .expect(1)
            .mount(&server)
            .await;
    }
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_function_call_with_namespace("search-paper", "web", "run", &search.to_string()),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_function_call_with_namespace("open-paper", "web", "run", &open.to_string()),
                ev_completed("resp-2"),
            ]),
            sse(vec![
                ev_assistant_message("answer", "Tri Dao reports a 2x speedup."),
                ev_completed("resp-3"),
            ]),
        ],
    )
    .await;

    let auth = codex_core::test_support::auth_manager_from_auth(CodexAuth::from_api_key("dummy"));
    let mut extensions = ExtensionRegistryBuilder::new();
    codex_web_search_extension::install(&mut extensions, Arc::clone(&auth));
    let test = test_codex()
        .with_auth_manager(auth)
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            configure_scenario_catalog(config);
            config
                .features
                .enable(Feature::StandaloneWebSearch)
                .unwrap();
            config.web_search_mode.set(WebSearchMode::Live).unwrap();
        })
        .with_model_info_override("paper-model", |model| {
            model.tool_mode = Some(ToolMode::Direct);
            model
                .experimental_supported_tools
                .push("literature_search_query".to_string());
        })
        .build_with_auto_env(&server)
        .await?;
    test.submit_turn("Find the FlashAttention-2 paper and open it to check the reported speedup.")
        .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 3);
    let web_run = requests[0]
        .tool_by_name("web", "run")
        .expect("advertised web.run");
    let mut parameters = web_run
        .get("parameters")
        .expect("web.run parameters")
        .clone();
    parameters.sort_all_objects();
    insta::assert_snapshot!(
        "literature_search_web_run_schema",
        serde_json::to_string_pretty(&parameters)?
    );
    insta::assert_snapshot!(context_snapshot::format_request_history_snapshot(
        "A model opted into literature search finds a paper, opens the returned reference, and reads the result before answering.",
        &requests,
        &ContextSnapshotOptions::default()
            .rewrite_known_segments()
            .include_request_settings(),
    ));
    Ok(())
}

fn literature_search_pin() -> Value {
    json!({"config":{"tool_type":"SonicBrowserToolV99","allow_literature_search":true}})
}

async fn mount_model_search(
    server: &wiremock::MockServer,
    available: &Arc<AtomicBool>,
) -> Result<AppsTestServer> {
    let mut tool = json!({
        "name": "search_service.web_run", "description": "Find papers on the web.",
        "annotations": {"readOnlyHint": true},
        "inputSchema": {"type":"object","properties":{"system2_search_query":{"type":"array"},"open":{"type":"array"}}},
        "_meta": {"connector_id": codex_mcp::SEARCH_CONNECTOR,"connector_name":"search_service"}
    });
    let apps =
        AppsTestServer::mount_with_tools(server, Arc::new(Mutex::new(vec![tool.clone()]))).await?;
    tool["inputSchema"]["properties"]["literature_search_query"] = json!({"type":"array"});
    let pin = literature_search_pin();
    tool["_meta"]["_codex_apps"] = json!({"invocation_context":pin});
    // Host-owned Apps catalogs can exceed the generic MCP limit of 2,048 tools.
    let mut catalog: Vec<_> = (0..2_048)
        .map(|n| json!({"name":format!("other_{n}"),"inputSchema":{"type":"object"}}))
        .collect();
    catalog.push(tool);
    let available = Arc::clone(available);
    Mock::given(method("POST"))
        .and(body_partial_json(json!({
            "method":"tools/list","params":{"_meta":{"x-codex-turn-metadata":{"mcp_request_meta":{
                "selected_connector_ids":[codex_mcp::SEARCH_CONNECTOR]
            }}}}
        })))
        .respond_with(move |request: &wiremock::Request| {
            let body: serde_json::Value = request.body_json().expect("tools/list request JSON");
            let mut response = json!({"jsonrpc":"2.0","id":body["id"]});
            if available.load(Ordering::SeqCst) {
                response["result"] = json!({"tools":catalog});
            } else {
                response["error"] = json!({"code":-32603,"message":"discovery unavailable"});
            }
            ResponseTemplate::new(200).set_body_json(response)
        })
        .with_priority(1)
        .expect(2..)
        .mount(server)
        .await;
    Ok(apps)
}

fn model_search_builder(apps: &AppsTestServer, tool_mode: ToolMode) -> TestCodexBuilder {
    let auth = codex_core::test_support::auth_manager_from_auth(
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    );
    let mut extensions = ExtensionRegistryBuilder::new();
    codex_web_search_extension::install(&mut extensions, Arc::clone(&auth));
    apps_enabled_builder(apps.chatgpt_base_url.clone())
        .with_auth_manager(auth)
        .with_extensions(Arc::new(extensions.build()))
        .with_config(move |config| {
            configure_scenario_catalog(config);
            if tool_mode == ToolMode::CodeMode {
                config
                    .features
                    .enable(Feature::CodeMode)
                    .expect("enable code mode");
            }
            config
                .features
                .enable(Feature::StandaloneWebSearch)
                .expect("enable standalone web search");
            config
                .web_search_mode
                .set(WebSearchMode::Live)
                .expect("set live web search mode");
        })
        .with_model_info_override("native-paper", |model| {
            model.web_search_provider = Some(WebSearchProvider::Standalone);
            model.tool_mode = Some(ToolMode::Direct);
        })
        .with_model_info_override("hosted-paper", move |model| {
            model.web_search_provider = Some(WebSearchProvider::SearchMcp);
            model.tool_mode = Some(tool_mode);
            if tool_mode == ToolMode::CodeMode {
                model
                    .experimental_supported_tools
                    .push("test_sync_tool".into());
            }
            model.supports_search_tool = true;
            model
                .experimental_supported_tools
                .push("literature_search_query".into());
        })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_selected_search_preserves_discovery_and_call_history() -> Result<()> {
    use codex_protocol::protocol::ThreadSettingsOverrides;

    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let discovery_available = Arc::new(AtomicBool::new(true));
    let apps = mount_model_search(&server, &discovery_available).await?;
    let pin = literature_search_pin();
    let search = json!({"literature_search_query":[{"q":"FlashAttention-2"}]});
    let reference_issued = Arc::new(AtomicBool::new(false));
    let issued = Arc::clone(&reference_issued);
    Mock::given(method("POST"))
        .and(body_partial_json(json!({
            "method":"tools/call","params":{"name":"search_service.web_run","arguments":search,
            "_meta":{"_codex_apps":{"invocation_context":pin}}}
        })))
        .respond_with(move |request: &wiremock::Request| {
            let body: serde_json::Value = request.body_json().unwrap();
            issued.store(true, Ordering::SeqCst);
            ResponseTemplate::new(200).set_body_json(json!({"jsonrpc":"2.0","id":body["id"],
        "result":{"content":[{"type":"text","text":"[turn0academia0] FlashAttention-2 by Tri Dao."}]}}))
        })
        .with_priority(1)
        .expect(1)
        .mount(&server)
        .await;
    let open = json!({"open":[{"ref_id":"turn0academia0"}]});
    Mock::given(method("POST"))
        .and(body_partial_json(json!({
            "method":"tools/call","params":{"name":"search_service.web_run","arguments":open,
            "_meta":{"_codex_apps":{"invocation_context":pin}}}
        })))
        .respond_with(move |request: &wiremock::Request| {
            let body: serde_json::Value = request.body_json().unwrap();
            let text = if reference_issued.load(Ordering::SeqCst) {
                "The paper reports a 2x speedup."
            } else {
                "Unknown reference."
            };
            ResponseTemplate::new(200).set_body_json(json!({"jsonrpc":"2.0","id":body["id"],
                "result":{"content":[{"type":"text","text":text}]}}))
        })
        .with_priority(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/alpha/search"))
        .respond_with(ResponseTemplate::new(400))
        .expect(0)
        .mount(&server)
        .await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_tool_search_call("find-search", &json!({"query":"search_service web_run"})),
                ev_completed("lookup"),
            ]),
            sse(vec![
                ev_function_call_with_namespace(
                    "paper-search",
                    "mcp__codex_apps__search_service",
                    "_web_run",
                    &search.to_string(),
                ),
                ev_completed("search"),
            ]),
            sse(vec![
                ev_assistant_message("author", "The paper is by Tri Dao."),
                ev_completed("first"),
            ]),
            sse(vec![
                ev_assistant_message("unavailable", "Search is temporarily unavailable."),
                ev_completed("outage"),
            ]),
            sse(vec![
                ev_tool_search_call("find-open", &json!({"query":"search_service web_run"})),
                ev_completed("lookup-open"),
            ]),
            sse(vec![
                ev_function_call_with_namespace(
                    "open",
                    "mcp__codex_apps__search_service",
                    "_web_run",
                    &open.to_string(),
                ),
                ev_completed("opened"),
            ]),
            sse(vec![
                ev_assistant_message("speed", "It reports a 2x speedup."),
                ev_completed("second"),
            ]),
            sse(vec![
                ev_assistant_message("recap", "Tri Dao reports a 2x speedup."),
                ev_completed("third"),
            ]),
        ],
    )
    .await;
    let test = model_search_builder(&apps, ToolMode::Direct)
        .build_with_auto_env(&server)
        .await?;
    test.submit_turn("Find the FlashAttention-2 paper.").await?;
    discovery_available.store(false, Ordering::SeqCst);
    test.submit_turn("Open the paper and check its speedup.")
        .await?;
    discovery_available.store(true, Ordering::SeqCst);
    test.submit_turn("Try opening the same paper again.")
        .await?;
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![text("Summarize the result.")]).with_thread_settings(
                ThreadSettingsOverrides {
                    model: Some("native-paper".into()),
                    ..Default::default()
                },
            ),
        )
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
    let requests = responses.requests();
    assert_eq!(requests.len(), 8);
    assert!(
        requests[0]
            .tool_by_name("mcp__codex_apps__search_service", "_web_run")
            .is_none()
    );
    assert!(
        requests[1]
            .tool_search_output("find-search")
            .to_string()
            .contains("literature_search_query")
    );
    let output = requests[2].function_call_output("paper-search");
    assert!(output.to_string().contains("Tri Dao"), "{output}");
    assert!(requests[3].tool_by_name("web", "run").is_none());
    let output = requests[6].function_call_output("open");
    assert!(output.to_string().contains("2x"), "{output}");
    insta::assert_snapshot!(context_snapshot::format_request_history_snapshot(
        "MCP returns a paper reference. A discovery outage withholds native search, then MCP opens the same reference after recovery. Switching to a standalone model retains the history.",
        &requests,
        &ContextSnapshotOptions::default()
            .rewrite_known_segments()
            .include_request_settings(),
    ));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_fallback_reference_survives_discovery_recovery_and_resume() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let available = Arc::new(AtomicBool::new(false));
    let apps = mount_model_search(&server, &available).await?;
    let mut test = model_search_builder(&apps, ToolMode::Direct)
        .build_with_auto_env(&server)
        .await?;
    // Merely offering fallback during an outage must not pin a healthy thread to native.
    let idle = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_assistant_message("hello", "Hello."),
                ev_completed("hello"),
            ]),
            sse(vec![
                ev_tool_search_call("lookup", &json!({"query":"search_service web_run"})),
                ev_completed("lookup"),
            ]),
            sse(vec![
                ev_assistant_message("ready", "Search is ready."),
                ev_completed("ready"),
            ]),
        ],
    )
    .await;
    test.submit_turn("Say hello.").await?;
    available.store(true, Ordering::SeqCst);
    test.submit_turn("Look up the search tool.").await?;
    let requests = idle.requests();
    assert!(requests[0].tool_by_name("web", "run").is_some());
    assert!(
        requests[2]
            .tool_search_output("lookup")
            .to_string()
            .contains("literature_search_query")
    );

    let search = json!({"search_query":[{"q":"FlashAttention-2"}]});
    let open = json!({"open":[{"ref_id":"turn0search0"}]});

    let mut browser_session = None;
    for (id, commands, output) in [
        ("search", search, "[turn0search0] FlashAttention-2"),
        ("open", open.clone(), "Tri Dao reports a 2x speedup."),
        ("reopen", open, "Tri Dao reports a 2x speedup."),
    ] {
        let first_turn = id == "search";
        available.store(!first_turn, Ordering::SeqCst);
        if id == "reopen" {
            test = model_search_builder(&apps, ToolMode::Direct)
                .restart_with_auto_env(&server, &test)
                .await?;
        }
        let native = Mock::given(method("POST"))
            .and(path("/v1/alpha/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"output":output})))
            .expect(1)
            .mount_as_scoped(&server)
            .await;
        let responses = mount_sse_sequence(
            &server,
            vec![
                sse(vec![
                    ev_function_call_with_namespace(id, "web", "run", &commands.to_string()),
                    ev_completed("tool"),
                ]),
                sse(vec![
                    ev_assistant_message("answer", output),
                    ev_completed("answer"),
                ]),
            ],
        )
        .await;
        test.submit_turn(if first_turn {
            "Find the paper."
        } else {
            "Open the paper you found."
        })
        .await?;
        let requests = responses.requests();
        assert!(requests[0].tool_by_name("web", "run").is_some());
        let result = requests[1].function_call_output(id);
        assert!(result.to_string().contains(output), "{result}");
        let body: Value = native.received_requests().await[0].body_json()?;
        assert_eq!(body["commands"], commands);
        let session_id = body["id"].as_str().context("native search session ID")?;
        assert_eq!(
            browser_session
                .get_or_insert_with(|| session_id.to_owned())
                .as_str(),
            session_id
        );
    }
    assert!(
        core_test_support::apps_test_server::recorded_apps_tool_calls(&server)
            .await
            .is_empty()
    );
    Ok(())
}

#[test_case::test_case(false; "unused_offer_releases_provider")]
#[test_case::test_case(true; "executed_call_keeps_provider")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_native_fallback(used: bool) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let available = Arc::new(AtomicBool::new(false));
    let apps = mount_model_search(&server, &available).await?;
    let test = model_search_builder(&apps, ToolMode::Direct)
        .with_config(|config| {
            config
                .features
                .disable(Feature::StandaloneWebSearch)
                .expect("disable standalone web search");
        })
        .with_model_info_override("hosted-paper", |model| model.use_responses_lite = false)
        .build_with_auto_env(&server)
        .await?;
    let mut events = Vec::new();
    if used {
        events.push(
            core_test_support::responses::ev_web_search_call_added_partial("native", "in_progress"),
        );
        events.push(core_test_support::responses::ev_web_search_call_done(
            "native",
            "completed",
            "FlashAttention-2",
        ));
    }
    events.extend([
        ev_assistant_message("first", "Ready."),
        ev_completed("first"),
    ]);
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(events),
            sse(vec![
                ev_assistant_message("second", "Ready."),
                ev_completed("second"),
            ]),
        ],
    )
    .await;
    test.submit_turn("Look for the paper.").await?;
    available.store(true, Ordering::SeqCst);
    test.submit_turn("Continue.").await?;
    let native_offers: Vec<_> = responses
        .requests()
        .iter()
        .map(|request| {
            request.body_json()["tools"]
                .as_array()
                .expect("request tools array")
                .iter()
                .any(|tool| tool["type"] == "web_search")
        })
        .collect();
    assert_eq!(native_offers, vec![true, used]);
    Ok(())
}

#[test_case::test_case(false; "mcp_finishes_before_native_is_used")]
#[test_case::test_case(true; "native_runs_before_retained_mcp_call")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn background_search_and_native_fallback(native_first: bool) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let available = Arc::new(AtomicBool::new(true));
    let apps = mount_model_search(&server, &available).await?;
    let test = model_search_builder(&apps, ToolMode::CodeMode)
        .build_with_auto_env(&server)
        .await?;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"method":"tools/call","params":{"name":"search_service.web_run","_meta":{"_codex_apps":{"invocation_context":literature_search_pin()}}}})))
        .respond_with(|request: &wiremock::Request| {
            let body: Value = request.body_json().expect("tools/call request JSON");
            ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc":"2.0", "id":body["id"], "result":{"content":[{"type":"text","text":"[turn0academia0] FlashAttention-2"}]}
            }))
        })
        .with_priority(1).expect(u64::from(!native_first)).mount(&server).await;

    // The barrier holds the original step's MCP call until the next turn has offered native.
    let barrier = json!({"barrier":{"id":"background-search","participants":2,"timeout_ms":60000}});
    let pending = format!(
        r#"yield_control();
await tools.test_sync_tool({barrier});
text((await tools.mcp__codex_apps__search_service_web_run({{literature_search_query:[{{q:"FlashAttention-2"}}]}})).content);"#
    );
    let first = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_custom_tool_call("pending", "exec", &pending),
                ev_completed("yielded"),
            ]),
            sse(vec![
                ev_assistant_message("message", "Searching."),
                ev_completed("first"),
            ]),
        ],
    )
    .await;
    test.submit_turn("Start a background paper search.").await?;
    let yielded = first.requests()[1].custom_tool_call_output("pending");
    let running = yielded["output"]
        .as_str()
        .or_else(|| yielded.pointer("/output/0/text")?.as_str())
        .expect("yielded exec output");
    let cell_id = running
        .lines()
        .next()
        .expect("yielded exec output line")
        .strip_prefix("Script running with cell ID ")
        .expect("yielded exec cell");
    available.store(false, Ordering::SeqCst);
    let native_commands = json!({"search_query":[{"q":"FlashAttention-2"}]});
    Mock::given(method("POST"))
        .and(path("/v1/alpha/search"))
        .and(body_partial_json(json!({"commands":native_commands})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"output":"[turn0search0] native paper"})),
        )
        .expect(u64::from(native_first))
        .mount(&server)
        .await;
    let mut steps = Vec::new();
    if native_first {
        steps.push(sse(vec![
            ev_function_call_with_namespace("native", "web", "run", &native_commands.to_string()),
            ev_completed("native"),
        ]));
    }
    steps.extend([
        sse(vec![
            ev_function_call("release", "test_sync_tool", &barrier.to_string()),
            ev_completed("released"),
        ]),
        sse(vec![
            ev_function_call_with_namespace(
                "wait-search",
                "functions",
                "wait",
                &json!({"cell_id":cell_id,"yield_time_ms":10000}).to_string(),
            ),
            ev_completed("waited"),
        ]),
        sse(vec![
            ev_assistant_message("answer", "The background result is available."),
            ev_completed("done"),
        ]),
    ]);
    let second = mount_sse_sequence(&server, steps).await;
    test.submit_turn("Wait for the paper.").await?;
    let requests = second.requests();
    assert!(
        requests[0].tool_by_name("web", "run").is_some(),
        "outage initially allows native"
    );
    let last = requests.last().expect("final response request");
    let result = last.function_call_output("wait-search").to_string();
    if native_first {
        assert!(
            requests[1]
                .function_call_output("native")
                .to_string()
                .contains("turn0search0")
        );
        assert!(result.contains("native search"), "{result}");
        assert!(
            last.tool_by_name("web", "run").is_some(),
            "the skipped MCP call must not change ownership"
        );
        available.store(true, Ordering::SeqCst);
        let recovered = mount_sse_sequence(
            &server,
            vec![sse(vec![
                ev_assistant_message("answer", "Ready."),
                ev_completed("recovered"),
            ])],
        )
        .await;
        test.submit_turn("Continue using the same paper.").await?;
        assert!(
            recovered
                .single_request()
                .tool_by_name("web", "run")
                .is_some()
        );
    } else {
        assert!(result.contains("turn0academia0"), "{result}");
        assert!(
            last.tool_by_name("web", "run").is_none(),
            "MCP owns the reference"
        );
    }
    Ok(())
}
