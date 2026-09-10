//! Exercises async overflow and score recovery through real MCP approval routing.

use super::*;
use pretty_assertions::assert_eq;
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn oversized_async_action_requires_sync_review_and_later_scores_recover() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let state = Arc::new(MockResponsesState::default());
    let gates = [Arc::new(Notify::new()), Arc::new(Notify::new())];
    let parent_gates = gates.clone();
    let oversized = "required argument ".repeat(/*n*/ 100) + "required suffix";
    let action_message = oversized.clone();
    let oversized_waiting = Arc::new(Notify::new());
    let permission_gate = Arc::clone(&gates[1]);
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let responses_url = format!("http://{}", listener.local_addr()?);
    let router = Router::new()
        .route(
            "/v1/responses",
            get(luna_websocket).post(
                move |State(state): State<Arc<MockResponsesState>>, Json(request): Json<Value>| {
                    let gates = parent_gates.clone();
                    let oversized = action_message.clone();
                    async move {
                        if request.pointer("/client_metadata/x-openai-subagent")
                            == Some(&json!("guardian"))
                        {
                            return parent_response(State(state), Json(request))
                                .await
                                .into_response();
                        }
                        let index =
                            state.parent_requests.fetch_add(/*val*/ 1, Ordering::SeqCst);
                        if index == 1 {
                            gates[0].notified().await;
                        }
                        let actions: &[usize] = match index {
                            0 => &[0],
                            1 => &[1, 2],
                            2 => &[3],
                            _ => &[],
                        };
                        let response_id = format!("response-{index}");
                        let mut events = vec![responses::ev_response_created(&response_id)];
                        for &action in actions {
                            let call_id = format!("action-{action}");
                            let message = if action == 1 {
                                oversized.clone()
                            } else {
                                format!("small-{action}")
                            };
                            events.push(responses::ev_function_call_with_namespace(
                                &call_id,
                                &format!("mcp__{TEST_SERVER_NAME}"),
                                TEST_TOOL_NAME,
                                &json!({"message": message}).to_string(),
                            ));
                        }
                        if actions.is_empty() {
                            events.push(responses::ev_assistant_message("done", "done"));
                        }
                        events.push(responses::ev_completed(&response_id));
                        (
                            [(header::CONTENT_TYPE, "text/event-stream")],
                            responses::sse(events),
                        )
                            .into_response()
                    }
                },
            ),
        )
        .route(
            "/approval-barrier",
            post(move |Json(input): Json<Value>| {
                let oversized_waiting = Arc::clone(&oversized_waiting);
                let permission_gate = Arc::clone(&permission_gate);
                async move {
                    let message = input["tool_input"]["message"].as_str().unwrap_or_default();
                    if input["hook_event_name"] == "PermissionRequest"
                        && message.starts_with("required argument")
                    {
                        // Let the later call start scoring only after overflow was recorded.
                        oversized_waiting.notify_one();
                        permission_gate.notified().await;
                    } else if input["hook_event_name"] == "PreToolUse" && message == "small-2" {
                        oversized_waiting.notified().await;
                    }
                    Json(json!({}))
                }
            }),
        )
        .with_state(Arc::clone(&state));
    let responses_server = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    let (mcp_url, mcp_server) = start_mcp_server(/*sensitive_action*/ None).await?;
    let codex_home = TempDir::new()?;
    let hook_path = codex_home.path().join("approval-barrier.py");
    let endpoint = format!("{responses_url}/approval-barrier");
    std::fs::write(
        &hook_path,
        format!(
            r#"import sys
import urllib.request

request = urllib.request.Request({endpoint:?}, data=sys.stdin.buffer.read(),
                                 headers={{"Content-Type": "application/json"}})
with urllib.request.urlopen(request, timeout=30) as response:
    print(response.read().decode())
"#
        ),
    )?;
    let command = serde_json::to_string(&format!("python3 \"{}\"", hook_path.display()))?;
    let hooks = ["PreToolUse", "PermissionRequest"]
        .map(|event| format!(
                "[[hooks.{event}]]\nmatcher = '^mcp__'\n[[hooks.{event}.hooks]]\ntype = 'command'\ncommand = {command}\n"
        ))
        .join("\n");
    std::fs::write(codex_home.path().join("requirements.toml"), hooks)?;
    let analytics_server = responses::start_mock_server().await;
    mount_analytics_capture(&analytics_server, codex_home.path()).await?;
    MockResponsesConfig::new(&responses_url)
        .with_model(MODEL)
        .with_provider_config("supports_websockets = false")
        .with_approval_policy("on-request")
        .with_root_config(&format!("approvals_reviewer = \"auto_review\"\nchatgpt_base_url = \"{}\"", analytics_server.uri()))
        .with_extra_config(&format!(
            "[mcp_servers.{TEST_SERVER_NAME}]\nurl = \"{mcp_url}/mcp\"\ndefault_tools_approval_mode = \"prompt\"\n\n[analytics]\nenabled = true\n\n[features.guardianv2]\nenabled = true\nmax_action_tokens = 128\n\n[features.guardianv2.review_scope]\ncomputer_use_only = false"
        ))
        .enable_feature(Feature::GuardianApproval)
        .write(codex_home.path())?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(TIMEOUT)
        .await?;
    let thread = app_server
        .start_thread(ThreadStartParams::default())
        .await?
        .thread;
    state.allow_guardian_review.notify_one();
    let id = app_server
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id,
            input: vec![UserInput::Text {
                text: USER_CONTEXT.to_owned(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse = timeout(TIMEOUT, app_server.read_response(id)).await??;
    for (index, gate) in gates.iter().enumerate() {
        wait_for_luna_request(&state, index).await?;
        // The oversized call is still in its permission hook;
        // only the seed and the later small call can have begun synchronous review.
        wait_for_guardian_reviews(&state, 1 + index).await?;
        // First establish a permissive score; after overflow, establish a new one.
        state.allow_luna.notify_one();
        timeout(TIMEOUT, async {
            loop {
                let events = captured_analytics_events(&analytics_server).await;
                if events
                    .iter()
                    .filter(|event| {
                        event["event_type"] == "codex_guardian_v2_classification"
                            && event["event_params"]["outcome"] == "success"
                    })
                    .count()
                    > index
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(/*millis*/ 25)).await;
            }
        })
        .await?;
        gate.notify_one();
    }
    let completed: TurnCompletedNotification =
        timeout(TIMEOUT, app_server.read_notification("turn/completed")).await??;
    assert_eq!(completed.turn.status, TurnStatus::Completed);
    wait_for_luna_request(&state, /*index*/ 2).await?;
    app_server.shutdown_gracefully().await?;
    // First call, oversized call, and recovery call need review. The fourth reuses
    // the recovered score. Only the three small actions reach the async model.
    assert_eq!(
        (
            state.guardian_reviews.load(Ordering::SeqCst),
            state
                .luna_requests
                .lock()
                .expect("Luna request lock should not be poisoned")
                .len()
        ),
        (3, 3)
    );
    let reviews = state
        .guardian_requests
        .lock()
        .expect("Guardian request lock should not be poisoned");
    let action = reviews[2]["input"]
        .as_array()
        .expect("Guardian request should contain input items")
        .iter()
        .rev()
        .filter_map(|item| item["content"].as_array())
        .flatten()
        .filter_map(|part| part["text"].as_str())
        .filter_map(|text| serde_json::from_str::<Value>(text).ok())
        .find(|value| value["tool"] == "mcp_tool_call")
        .expect("complete MCP action");
    assert_eq!(action["arguments"], json!({"message": oversized}));
    mcp_server.abort();
    responses_server.abort();
    Ok(())
}
