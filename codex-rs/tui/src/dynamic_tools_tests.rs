use super::*;
use crate::app_server_session::AppServerSession;
use crate::app_server_session::ResumeModelSettings;
use crate::legacy_core::config::ConfigBuilder;
use app_test_support::create_fake_paginated_rollout;
use app_test_support::create_fake_rollout;
use app_test_support::rollout_path;
use codex_protocol::ThreadId;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

async fn test_server() -> color_eyre::Result<(TempDir, AppServerSession, String, String)> {
    let codex_home = tempfile::tempdir()?;
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .build()
        .await?;
    let target = create_fake_paginated_rollout(
        codex_home.path(),
        "2026-01-02T00-00-00",
        "2026-01-02T00:00:00Z",
        "Persisted test task",
        Some(config.model_provider_id.as_str()),
        /*git_info*/ None,
    )
    .map_err(|error| color_eyre::eyre::eyre!("failed to create test rollout: {error}"))?;
    let path = rollout_path(codex_home.path(), "2026-01-02T00-00-00", &target);
    let mut records = std::fs::read_to_string(&path)?
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    for payload in [
        json!({
            "type": "task_started",
            "turn_id": "persisted-turn",
            "model_context_window": null
        }),
        json!({
            "type": "item_completed",
            "thread_id": target,
            "turn_id": "persisted-turn",
            "item": {
                "type": "AgentMessage",
                "id": "persisted-message",
                "content": [{"type": "Text", "text": "Persisted assistant output".repeat(40)}]
            },
            "completed_at_ms": 0
        }),
        json!({
            "type": "item_completed",
            "thread_id": target,
            "turn_id": "persisted-turn",
            "item": {
                "type": "DynamicToolCall",
                "id": "persisted-tool",
                "namespace": "codex_tui",
                "tool": "list_threads",
                "arguments": {},
                "status": "completed",
                "success": true
            },
            "completed_at_ms": 0
        }),
        json!({
            "type": "task_complete",
            "turn_id": "persisted-turn",
            "last_agent_message": "Persisted assistant output"
        }),
    ] {
        serde_json::from_value::<codex_protocol::protocol::EventMsg>(payload.clone())?;
        records.push(json!({
            "timestamp": "2026-01-02T00:00:00Z",
            "ordinal": records.len(),
            "type": "event_msg",
            "payload": payload
        }));
    }
    let records = records
        .into_iter()
        .map(|record| record.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(path, format!("{records}\n"))?;
    let mut server = crate::start_embedded_app_server_for_picker(&config).await?;
    let source = server
        .start_thread(&config)
        .await?
        .session
        .thread_id
        .to_string();
    server
        .resume_thread(
            config,
            ThreadId::from_string(&target)?,
            ResumeModelSettings::RestoreFromThread,
        )
        .await?;
    Ok((codex_home, server, source, target))
}

async fn call_tool(
    server: &AppServerSession,
    source: &str,
    name: &str,
    arguments: Value,
) -> DynamicToolCallResponse {
    let (_status_sender, status_receiver) = broadcast::channel(/*capacity*/ 8);
    execute(
        server.request_handle(),
        DynamicToolCallParams {
            thread_id: source.to_string(),
            turn_id: "persisted-turn".to_string(),
            call_id: "call-1".to_string(),
            namespace: Some(NAMESPACE.to_string()),
            tool: name.to_string(),
            arguments,
        },
        ThreadStartParams {
            dynamic_tools: Some(tool_specs()),
            ephemeral: Some(true),
            ..ThreadStartParams::default()
        },
        status_receiver,
        /*app_event_tx*/ None,
    )
    .await
}

fn response_json(response: DynamicToolCallResponse) -> Value {
    assert!(response.success, "tool call failed: {response:?}");
    let [DynamicToolCallOutputContentItem::InputText { text }] = response.content_items.as_slice()
    else {
        panic!("expected one JSON text response")
    };
    serde_json::from_str(text).expect("dynamic tool response should contain JSON")
}

#[test]
fn oversized_responses_are_truncated_without_losing_identifiers() {
    let response = success_response(json!({
        "thread": {"threadId": "thread-1", "summary": "preview"},
        "turns": [{"turnId": "turn-1", "items": [{
            "type": "agentMessage", "id": "message-1",
            "output": output_summary(&"🦀".repeat(MAX_RESPONSE_BYTES), MAX_RESPONSE_BYTES)
        }]}]
    }))
    .expect("oversized responses should be shortened");
    let [DynamicToolCallOutputContentItem::InputText { text }] = response.content_items.as_slice()
    else {
        panic!("expected one JSON text response")
    };
    assert!(text.len() <= MAX_RESPONSE_BYTES);
    let value: Value = serde_json::from_str(text).expect("response remains valid JSON");
    assert_eq!(value["thread"]["threadId"], "thread-1");
    assert_eq!(value["turns"][0]["items"][0]["id"], "message-1");
    assert_eq!(value["turns"][0]["items"][0]["output"]["truncated"], true);
    assert_eq!(
        value["turns"][0]["items"][0]["output"]["originalChars"],
        MAX_RESPONSE_BYTES
    );
    assert_eq!(value["truncated"], true);
}

#[test]
fn oversized_task_lists_are_bounded() {
    let threads: Vec<_> = (0..10)
        .map(|index| {
            json!({
                "id": format!("00000000-0000-0000-0000-{index:012}"),
                "status": "active",
                "title": "A task with a descriptive title",
                "summary": "A task with a longer preview",
                "cwd": "/tmp/project",
                "updatedAt": 123
            })
        })
        .collect();
    let value = response_json(
        success_response(json!({"threads": threads}))
            .expect("oversized task lists should be shortened"),
    );
    let threads = value["threads"].as_array().expect("task summaries");
    assert!(!threads.is_empty() && threads.len() < 10);
    assert_eq!(value["truncated"], true);
}

#[test]
fn oversized_wait_snapshots_preserve_all_targets() {
    let polls: Vec<_> = (0..MAX_WAIT_TARGETS)
        .map(|index| {
            json!({
                "schemaVersion": 1,
                "thread": {
                    "id": format!("00000000-0000-0000-0000-{index:012}"),
                    "status": "idle"
                },
                "cursor": format!("opaque-cursor-{index}-{}", "x".repeat(100)),
                "revision": 123,
                "changed": false,
                "latestTurn": null,
                "latestAssistantMessageId": null,
                "latestAssistantMessage": null,
                "latestToolMarkerId": null,
                "latestToolMarker": null
            })
        })
        .collect();
    let value = response_json(
        success_response(json!({"timedOut": true, "wake": null, "polls": polls}))
            .expect("all wait targets should fit a compact response"),
    );
    assert_eq!(
        value["polls"].as_array().map(Vec::len),
        Some(MAX_WAIT_TARGETS)
    );
    assert_eq!(value["truncated"], true);
}

#[test]
fn oversized_read_pages_preserve_turns_and_pagination() {
    let turns: Vec<_> = (0..10)
        .map(|turn_index| {
            json!({
                "id": format!("turn-{turn_index}"),
                "status": "completed",
                "items": (0..20)
                    .map(|item_index| json!({
                        "id": format!("00000000-0000-0000-{turn_index:04}-{item_index:012}"),
                        "type": "dynamicToolCall",
                        "namespace": "codex_tui",
                        "tool": "read_thread",
                        "status": "completed"
                    }))
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    let response = success_response(json!({
        "thread": {"id": "thread-1"},
        "page": {"nextCursor": "opaque-next-page"},
        "turns": turns
    }))
    .expect("oversized read pages should be shortened");
    let [DynamicToolCallOutputContentItem::InputText { text }] = response.content_items.as_slice()
    else {
        panic!("expected one JSON text response")
    };
    assert!(text.len() <= MAX_RESPONSE_BYTES);
    let value: Value = serde_json::from_str(text).expect("response remains valid JSON");
    let turns = value["turns"].as_array().expect("turns remain present");
    assert_eq!(turns.len(), 10);
    assert_eq!(value["page"]["nextCursor"], "opaque-next-page");
    assert!(turns.iter().all(|turn| turn["items"].is_array()));
    assert!(turns.iter().any(|turn| {
        turn["items"]
            .as_array()
            .is_some_and(|items| items.len() < 20)
    }));
}

#[test]
fn delegated_prompts_match_desktop_xml_contract() {
    assert_eq!(
        delegated_prompt("thread-1", "Check <main> & report > status"),
        "<codex_delegation>\n  <source_thread_id>thread-1</source_thread_id>\n  <input>Check &lt;main&gt; &amp; report &gt; status</input>\n</codex_delegation>"
    );
    assert!(
        validate_prompt(
            &delegated_prompt("thread-1", &"&".repeat(MAX_INPUT_BYTES)),
            MAX_DELEGATED_INPUT_BYTES,
        )
        .is_err()
    );
}

#[test]
fn activity_metadata_is_retained_without_including_outputs() -> color_eyre::Result<()> {
    let turn: Turn = serde_json::from_value(json!({
        "id": "turn-1",
        "status": "completed",
        "items": [
            {"type": "reasoning", "id": "thought-1", "summary": ["Thinking"], "content": ["Private reasoning"]},
            {"type": "commandExecution", "id": "command-1", "command": "cargo test", "cwd": "/tmp",
                "status": "completed", "commandActions": [], "aggregatedOutput": "Command output", "exitCode": 0},
            {"type": "fileChange", "id": "patch-1", "status": "completed",
                "changes": [{"path": "src/main.rs", "kind": {"type": "add"}, "diff": "+hello"}]},
            {"type": "mcpToolCall", "id": "mcp-1", "server": "docs", "tool": "search",
                "status": "completed", "arguments": {}},
            {"type": "userMessage", "id": "user-1", "content": [
                {"type": "text", "text": delegated_prompt("source-1", "Check <main> & status")},
                {"type": "skill", "name": "debug", "path": "/tmp/SKILL.md"},
                {"type": "mention", "name": "docs", "path": "app://docs"}
            ]},
            {"type": "agentMessage", "id": "assistant-1", "text": "Working", "phase": "commentary"},
            {"type": "webSearch", "id": "web-1", "query": "latest docs", "action": null},
            {"type": "sleep", "id": "sleep-1", "durationMs": 1000},
            {"type": "imageGeneration", "id": "image-1", "status": "completed",
                "revisedPrompt": "a cat", "result": "image bytes"},
            {"type": "enteredReviewMode", "id": "review-1", "review": "review changes"}
        ]
    }))?;

    let summary = turn_summary(&turn, /*include_outputs*/ false, DEFAULT_OUTPUT_CHARS);
    assert_eq!(
        summary["items"],
        json!([
            {"type": "reasoning", "id": "thought-1", "summary": ["Thinking"]},
            {"type": "commandExecution", "id": "command-1", "command": "cargo test", "cwd": "/tmp", "exitCode": 0, "status": "completed", "durationMs": null},
            {"type": "fileChange", "id": "patch-1", "status": "completed",
                "changes": [{"path": "src/main.rs", "kind": {"type": "add"}}]},
            {"type": "mcpToolCall", "id": "mcp-1", "server": "docs", "tool": "search", "arguments": {}, "status": "completed", "durationMs": null},
            {"type": "userMessage", "id": "user-1", "content": [
                {"type": "text", "text": delegated_prompt("source-1", "Check <main> & status"),
                    "codexDelegation": {"sourceThreadId": "source-1", "input": "Check <main> & status"}},
                {"type": "skill", "name": "debug", "path": "/tmp/SKILL.md"},
                {"type": "mention", "name": "docs", "path": "app://docs"}
            ]},
            {"type": "agentMessage", "id": "assistant-1", "text": "Working", "phase": "commentary"},
            {"type": "webSearch", "id": "web-1", "query": "latest docs", "action": null},
            {"type": "sleep", "id": "sleep-1", "durationMs": 1000},
            {"type": "imageGeneration", "id": "image-1", "status": "completed",
                "revisedPrompt": "a cat", "savedPath": null},
            {"type": "enteredReviewMode", "id": "review-1", "review": "review changes"}
        ])
    );

    let full = turn_summary(&turn, /*include_outputs*/ true, DEFAULT_OUTPUT_CHARS);
    assert_eq!(
        full["items"][0]["content"],
        json!([{"text": "Private reasoning", "truncated": false}])
    );
    assert_eq!(
        full["items"][1]["output"],
        json!({"text": "Command output", "truncated": false})
    );
    assert_eq!(
        full["items"][2]["changes"][0]["diff"],
        json!({"text": "+hello", "truncated": false})
    );
    assert_eq!(
        full["items"][8]["result"],
        json!({"text": "image bytes", "truncated": false})
    );

    let no_outputs = turn_summary(
        &turn, /*include_outputs*/ true, /*output_chars*/ 0,
    );
    assert_eq!(no_outputs["items"][5]["text"], "Working");
    assert_eq!(
        no_outputs["items"][1]["output"],
        json!({"text": "", "truncated": true, "originalChars": 14})
    );
    assert_eq!(
        no_outputs["items"][8]["result"],
        json!({"text": "", "truncated": true, "originalChars": 11})
    );
    Ok(())
}

#[tokio::test]
async fn task_management_tools_use_existing_app_server_operations() -> color_eyre::Result<()> {
    let (codex_home, server, source, target) = test_server().await?;

    let listed = response_json(call_tool(&server, &source, "list_threads", json!({})).await);
    assert!(
        listed["threads"]
            .as_array()
            .is_some_and(|threads| { threads.iter().any(|thread| thread["id"] == target) })
    );

    let legacy = create_fake_rollout(
        codex_home.path(),
        "2026-01-03T00-00-00",
        "2026-01-03T00:00:00Z",
        "Legacy test task",
        Some("openai"),
        /*git_info*/ None,
    )
    .map_err(|error| color_eyre::eyre::eyre!("failed to create legacy rollout: {error}"))?;
    for thread_id in [&target, &legacy] {
        let read = response_json(
            call_tool(
                &server,
                &source,
                "read_thread",
                json!({"threadId": thread_id}),
            )
            .await,
        );
        assert_eq!(read["schemaVersion"], 1);
        assert_eq!(read["thread"]["id"], *thread_id);
        assert_eq!(read["page"]["order"], "newest_first");
        assert!(read["turns"].is_array());
    }

    let renamed = response_json(
        call_tool(
            &server,
            &source,
            "set_thread_title",
            json!({"threadId": target, "title": "Renamed task"}),
        )
        .await,
    );
    assert_eq!(
        renamed,
        json!({"threadId": target, "title": "Renamed task"})
    );

    let forked = response_json(
        call_tool(&server, &source, "fork_thread", json!({"threadId": target})).await,
    );
    assert_ne!(forked["threadId"], target);
    let self_forked = response_json(call_tool(&server, &target, "fork_thread", json!({})).await);
    assert_ne!(self_forked["threadId"], target);
    assert_eq!(self_forked["sourceThreadId"], target);
    assert_eq!(
        self_forked["environment"],
        json!({"type": "same-directory"})
    );

    let self_archive = call_tool(
        &server,
        &source,
        "set_thread_archived",
        json!({"threadId": source.to_uppercase(), "archived": true}),
    )
    .await;
    assert!(!self_archive.success);

    let archived = response_json(
        call_tool(
            &server,
            &source,
            "set_thread_archived",
            json!({"threadId": target, "archived": true}),
        )
        .await,
    );
    assert_eq!(archived, json!({"threadId": target, "archived": true}));

    let mut expected_archived = vec![target.clone()];
    for day in 4..12 {
        let archived_id = create_fake_paginated_rollout(
            codex_home.path(),
            &format!("2026-01-{day:02}T00-00-00"),
            &format!("2026-01-{day:02}T00:00:00Z"),
            "Archived task with a deliberately descriptive pagination title",
            Some("openai"),
            /*git_info*/ None,
        )
        .map_err(|error| color_eyre::eyre::eyre!("failed to create archived rollout: {error}"))?;
        let archived = call_tool(
            &server,
            &source,
            "set_thread_archived",
            json!({"threadId": archived_id, "archived": true}),
        )
        .await;
        assert!(archived.success, "{archived:?}");
        expected_archived.push(archived_id);
    }
    let mut archived_threads =
        response_json(call_tool(&server, &source, "list_archived_threads", json!({})).await);
    assert!(
        archived_threads["threads"]
            .as_array()
            .is_some_and(|threads| threads.len() < expected_archived.len())
    );
    let mut listed_archived = Vec::new();
    loop {
        listed_archived.extend(
            archived_threads["threads"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|thread| thread["id"].as_str().map(ToString::to_string)),
        );
        let Some(cursor) = archived_threads["nextCursor"].as_str() else {
            break;
        };
        archived_threads = response_json(
            call_tool(
                &server,
                &source,
                "list_archived_threads",
                json!({"cursor": cursor}),
            )
            .await,
        );
    }
    expected_archived.sort();
    listed_archived.sort();
    assert_eq!(listed_archived, expected_archived);

    let restored = response_json(
        call_tool(
            &server,
            &source,
            "set_thread_archived",
            json!({"threadId": target, "archived": false}),
        )
        .await,
    );
    assert_eq!(restored, json!({"threadId": target, "archived": false}));

    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn wait_threads_returns_bounded_snapshots_and_rejects_self_wait() -> color_eyre::Result<()> {
    let (_codex_home, server, source, target) = test_server().await?;

    let snapshot = response_json(
        call_tool(
            &server,
            &source,
            "wait_threads",
            json!({"targets": [{"threadId": target}, {"threadId": "missing-task"}], "timeoutMs": 0}),
        )
        .await,
    );
    assert_eq!(snapshot["timedOut"], false);
    assert_eq!(snapshot["wake"]["threadId"], target);
    assert_eq!(snapshot["wake"]["reason"], "turnCompleted");
    assert!(snapshot.get("errors").is_none());
    assert_eq!(
        snapshot["wake"]["turnId"],
        snapshot["polls"][0]["latestTurn"]["id"]
    );
    let assistant = &snapshot["polls"][0]["latestAssistantMessage"];
    assert_eq!(assistant["id"], "persisted-message");
    assert_eq!(
        assistant["turnId"],
        snapshot["polls"][0]["latestTurn"]["id"]
    );
    assert!(assistant["text"].as_str().is_some_and(|text| {
        text.ends_with('…')
            && "Persisted assistant output"
                .repeat(40)
                .starts_with(text.trim_end_matches('…'))
    }));
    assert_eq!(
        snapshot["polls"][0]["latestToolMarker"],
        json!({
            "id": "persisted-tool",
            "turnId": "persisted-turn",
            "type": "dynamicToolCall",
            "name": "list_threads",
            "status": "completed"
        })
    );
    let cursor = snapshot["polls"][0]["cursor"]
        .as_str()
        .expect("snapshot cursor")
        .to_string();
    let cursor_value: Value = serde_json::from_str(&cursor)?;
    assert_eq!(
        cursor_value["turnId"],
        snapshot["polls"][0]["latestTurn"]["id"]
    );
    assert_eq!(
        cursor_value["turnStatus"],
        snapshot["polls"][0]["latestTurn"]["status"]
    );
    assert_eq!(cursor_value["latestItemId"], "persisted-tool");

    let unchanged = response_json(
        call_tool(
            &server,
            &source,
            "wait_threads",
            json!({"targets": [{"threadId": target, "afterCursor": cursor}], "timeoutMs": 0}),
        )
        .await,
    );
    assert_eq!(unchanged["timedOut"], true);
    assert_eq!(unchanged["wake"], Value::Null);
    assert_eq!(unchanged["polls"][0]["changed"], false);
    assert_eq!(unchanged["polls"][0]["latestAssistantMessage"], Value::Null);

    let self_wait = call_tool(
        &server,
        &source,
        "wait_threads",
        json!({"targets": [{"threadId": source.to_uppercase()}], "timeoutMs": 0}),
    )
    .await;
    assert!(!self_wait.success);

    let duplicate_wait = call_tool(
        &server,
        &source,
        "wait_threads",
        json!({"targets": [{"threadId": target}, {"threadId": target.to_uppercase()}], "timeoutMs": 0}),
    )
    .await;
    assert!(!duplicate_wait.success);

    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn task_creation_and_followup_start_background_turns() -> color_eyre::Result<()> {
    let (_codex_home, server, source, target) = test_server().await?;

    for (tool, arguments) in [
        (
            "create_thread",
            json!({"prompt": "&".repeat(MAX_INPUT_BYTES)}),
        ),
        (
            "send_message_to_thread",
            json!({"threadId": target, "prompt": "&".repeat(MAX_INPUT_BYTES)}),
        ),
    ] {
        assert!(!call_tool(&server, &source, tool, arguments).await.success);
    }
    assert!(
        !call_tool(
            &server,
            &source,
            "send_message_to_thread",
            json!({"threadId": target, "prompt": "Follow up", "model": ""}),
        )
        .await
        .success
    );

    let ephemeral: codex_app_server_protocol::ThreadStartResponse =
        request(&server.request_handle(), |request_id| {
            ClientRequest::ThreadStart {
                request_id,
                params: ThreadStartParams {
                    ephemeral: Some(true),
                    ..ThreadStartParams::default()
                },
            }
        })
        .await
        .map_err(color_eyre::eyre::Error::msg)?;
    let rejected = call_tool(
        &server,
        &ephemeral.thread.id,
        "create_thread",
        json!({"prompt": "Start a background task"}),
    )
    .await;
    assert!(!rejected.success);

    let created = response_json(
        call_tool(
            &server,
            &target,
            "create_thread",
            json!({"prompt": "x".repeat(MAX_INPUT_BYTES), "title": "Background task"}),
        )
        .await,
    );
    assert!(created["threadId"].is_string());

    let continued = response_json(
        call_tool(
            &server,
            &source,
            "send_message_to_thread",
            json!({"threadId": target, "prompt": "x".repeat(MAX_INPUT_BYTES)}),
        )
        .await,
    );
    assert_eq!(continued["threadId"], target);

    let oversized = call_tool(
        &server,
        &source,
        "list_archived_threads",
        json!({"cursor": "x".repeat(MAX_RESPONSE_BYTES + 1)}),
    )
    .await;
    assert!(!oversized.success);
    assert!(
        matches!(&oversized.content_items[..], [DynamicToolCallOutputContentItem::InputText { text }] if text.len() <= MAX_RESPONSE_BYTES)
    );

    server.shutdown().await?;
    Ok(())
}
