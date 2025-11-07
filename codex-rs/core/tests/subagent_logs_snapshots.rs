use codex_core::PageDirection;
use codex_core::render_logs_as_text;
use codex_core::render_logs_as_text_with_max_lines;
use codex_core::subagents::LogEntry;
use codex_protocol::ConversationId;
use insta::assert_snapshot;
use pretty_assertions::assert_eq;
use serde_json::json;

fn exec_sleep_logs() -> Vec<LogEntry> {
    let events_json = json!([
        {
            "timestamp_ms": 1762823213424i64,
            "event": {
                "id": "0",
                "msg": {
                    "type": "reasoning_content_delta",
                    "thread_id": "019a7073-88e5-7461-93a0-ae092f019246",
                    "turn_id": "0",
                    "item_id": "rs_0cb9136244ae700b0169128c2c63ec81a084a7fba2604df9fa",
                    "delta": "**"
                }
            }
        },
        {
            "timestamp_ms": 1762823213442i64,
            "event": {
                "id": "0",
                "msg": {
                    "type": "item_completed",
                    "thread_id": "019a7073-88e5-7461-93a0-ae092f019246",
                    "turn_id": "0",
                    "item": {
                        "Reasoning": {
                            "id": "rs_0cb9136244ae700b0169128c2c63ec81a084a7fba2604df9fa",
                            "summary_text": ["**Evaluating safe shell command execution**"],
                            "raw_content": []
                        }
                    }
                }
            }
        },
        {
            "timestamp_ms": 1762823213442i64,
            "event": {
                "id": "0",
                "msg": {
                    "type": "agent_reasoning",
                    "text": "**Evaluating safe shell command execution**"
                }
            }
        },
        {
            "timestamp_ms": 1762823213628i64,
            "event": {
                "id": "0",
                "msg": {
                    "type": "exec_command_begin",
                    "call_id": "call_hBhXJmeCagENc5VGd12udWE3",
                    "command": ["bash", "-lc", "sleep 60"],
                    "cwd": "/Users/friel/code/codex",
                    "parsed_cmd": [ { "type": "unknown", "cmd": "sleep 60" } ],
                    "is_user_shell_command": false
                }
            }
        }
    ]);

    serde_json::from_value(events_json).expect("valid exec_sleep_logs JSON")
}

#[test]
fn subagent_logs_paging_tail_vs_full_exec_sleep() {
    // Demonstrate that a one-line tail view is a suffix of the
    // full transcript, and that a generous max_lines reproduces
    // the full rendering.
    let logs = exec_sleep_logs();
    let session = ConversationId::from_string("019a7073-88e5-7461-93a0-adf67192b17b")
        .expect("valid session id");
    let earliest_ms = logs.first().map(|e| e.timestamp_ms);
    let latest_ms = logs.last().map(|e| e.timestamp_ms);
    let returned = logs.len();
    let total = 21; // from exp4-real-run1
    let more = true;

    // Full transcript for this window.
    let full = render_logs_as_text(
        session,
        &logs,
        earliest_ms,
        latest_ms,
        returned,
        total,
        more,
    );

    // Tail view: header + last content line only.
    let tail_one = render_logs_as_text_with_max_lines(
        session,
        &logs,
        earliest_ms,
        latest_ms,
        returned,
        total,
        more,
        1,
        PageDirection::Backward,
    );

    // Generous max_lines reproduces the full transcript.
    let tail_many = render_logs_as_text_with_max_lines(
        session,
        &logs,
        earliest_ms,
        latest_ms,
        returned,
        total,
        more,
        30,
        PageDirection::Backward,
    );

    assert_eq!(full, tail_many);

    // Snapshot the one-line tail to make the behavior obvious.
    assert_snapshot!(
        tail_one,
        @r###"Session 019a7073-88e5-7461-93a0-adf67192b17b • status=waiting_on_tool • older_logs=true • at_latest=true
    2025-11-11T01:06:53.628Z 🛠 exec bash -lc sleep 60 · cwd=/Users/friel/code/codex · running (0.0s)"###
    );
}

#[test]
fn subagent_logs_paging_line_by_line_exec_sleep() {
    // Show what the transcript looks like as we increase the
    // line budget from 1 to 3 (backward paging), to mimic a
    // user scrolling back line-by-line.
    let logs = exec_sleep_logs();
    let session = ConversationId::from_string("019a7073-88e5-7461-93a0-adf67192b17b")
        .expect("valid session id");
    let earliest_ms = logs.first().map(|e| e.timestamp_ms);
    let latest_ms = logs.last().map(|e| e.timestamp_ms);
    let returned = logs.len();
    let total = 21; // from exp4-real-run1
    let more = true;

    let mut pages = Vec::new();
    for max_lines in 1..=3 {
        let rendered = render_logs_as_text_with_max_lines(
            session,
            &logs,
            earliest_ms,
            latest_ms,
            returned,
            total,
            more,
            max_lines,
            PageDirection::Backward,
        );
        pages.push(format!("lines={max_lines}\n{rendered}"));
    }

    let snapshot = pages.join("\n---\n");

    assert_snapshot!(
        snapshot,
        @r###"lines=1
Session 019a7073-88e5-7461-93a0-adf67192b17b • status=waiting_on_tool • older_logs=true • at_latest=true
2025-11-11T01:06:53.628Z 🛠 exec bash -lc sleep 60 · cwd=/Users/friel/code/codex · running (0.0s)
---
lines=2
Session 019a7073-88e5-7461-93a0-adf67192b17b • status=waiting_on_tool • older_logs=true • at_latest=true
2025-11-11T01:06:53.442Z Reasoning summary: **Evaluating safe shell command execution**
2025-11-11T01:06:53.628Z 🛠 exec bash -lc sleep 60 · cwd=/Users/friel/code/codex · running (0.0s)
---
lines=3
Session 019a7073-88e5-7461-93a0-adf67192b17b • status=waiting_on_tool • older_logs=true • at_latest=true
2025-11-11T01:06:53.424Z Thinking: ** (1 delta)
2025-11-11T01:06:53.442Z Reasoning summary: **Evaluating safe shell command execution**
2025-11-11T01:06:53.628Z 🛠 exec bash -lc sleep 60 · cwd=/Users/friel/code/codex · running (0.0s)"###
    );
}

#[test]
fn subagent_logs_snapshot_baseline() {
    // Grounded in exp1-real-run1 first subagent_logs response (t=0).
    let events_json = json!([
        {
            "timestamp_ms": 1762823311742i64,
            "event": { "id": "0", "msg": { "type": "agent_message", "message": "Hello world" } }
        },
        {
            "timestamp_ms": 1762823311766i64,
            "event": {
                "id": "0",
                "msg": {
                    "type": "token_count",
                    "info": {
                        "total_token_usage": {
                            "input_tokens": 11073,
                            "cached_input_tokens": 11008,
                            "output_tokens": 8,
                            "reasoning_output_tokens": 0,
                            "total_tokens": 11081
                        },
                        "last_token_usage": {
                            "input_tokens": 11073,
                            "cached_input_tokens": 11008,
                            "output_tokens": 8,
                            "reasoning_output_tokens": 0,
                            "total_tokens": 11081
                        },
                        "model_context_window": 258400
                    },
                    "rate_limits": { "primary": null, "secondary": null }
                }
            }
        },
        {
            "timestamp_ms": 1762823311766i64,
            "event": {
                "id": "0",
                "msg": {
                    "type": "raw_response_item",
                    "item": {
                        "type": "reasoning",
                        "summary": [ { "type": "summary_text", "text": "**Identifying sandbox requirements**" } ],
                        "content": null,
                        "encrypted_content": "[encrypted]"
                    }
                }
            }
        },
        {
            "timestamp_ms": 1762823311766i64,
            "event": {
                "id": "0",
                "msg": {
                    "type": "raw_response_item",
                    "item": {
                        "type": "message",
                        "role": "assistant",
                        "content": [ { "type": "output_text", "text": "Hello world" } ]
                    }
                }
            }
        },
        {
            "timestamp_ms": 1762823311766i64,
            "event": {
                "id": "0",
                "msg": { "type": "task_complete", "last_agent_message": "Hello world" }
            }
        }
    ]);

    let logs: Vec<LogEntry> =
        serde_json::from_value(events_json).expect("valid baseline logs JSON");
    let session = ConversationId::from_string("019a7075-0760-79c2-8dd1-985772995ecf")
        .expect("valid session id");
    let earliest_ms = logs.first().map(|e| e.timestamp_ms);
    let latest_ms = logs.last().map(|e| e.timestamp_ms);
    let returned = logs.len();
    let total = logs.len();
    let more = false;

    let rendered = render_logs_as_text(
        session,
        &logs,
        earliest_ms,
        latest_ms,
        returned,
        total,
        more,
    );

    assert_snapshot!(
        rendered,
        @r###"Session 019a7075-0760-79c2-8dd1-985772995ecf • status=idle • older_logs=false • at_latest=true
2025-11-11T01:08:31.766Z Assistant: Hello world
2025-11-11T01:08:31.766Z Thinking: **Identifying sandbox requirements**
2025-11-11T01:08:31.766Z Task complete"###
    );
}

#[test]
fn subagent_logs_snapshot_exec_sleep_command() {
    // Grounded in exp4-real-run1 first subagent_logs response (t=0).
    let logs = exec_sleep_logs();
    let session = ConversationId::from_string("019a7073-88e5-7461-93a0-adf67192b17b")
        .expect("valid session id");
    let earliest_ms = logs.first().map(|e| e.timestamp_ms);
    let latest_ms = logs.last().map(|e| e.timestamp_ms);
    let returned = logs.len();
    let total = logs.len();
    let more = false;

    let rendered = render_logs_as_text(
        session,
        &logs,
        earliest_ms,
        latest_ms,
        returned,
        total,
        more,
    );

    assert_snapshot!(
        rendered,
        @r###"Session 019a7073-88e5-7461-93a0-adf67192b17b • status=waiting_on_tool • older_logs=false • at_latest=true
2025-11-11T01:06:53.424Z Thinking: ** (1 delta)
2025-11-11T01:06:53.442Z Reasoning summary: **Evaluating safe shell command execution**
2025-11-11T01:06:53.628Z 🛠 exec bash -lc sleep 60 · cwd=/Users/friel/code/codex · running (0.0s)"###
    );
}

#[test]
fn subagent_logs_snapshot_streaming_deltas() {
    // Grounded in exp5-real-run1 agent_message_content_delta stream (t≈?s).
    let events_json = json!([
        {
            "timestamp_ms": 1762836527094i64,
            "event": { "id": "0", "msg": { "type": "agent_message_content_delta", "thread_id": "019a713e-6ce6-7f82-b1e7-359628267934", "turn_id": "0", "item_id": "msg_0c5117240874292f016912c020d658819cb71e8bad4676a7c0", "delta": " is" } }
        },
        {
            "timestamp_ms": 1762836527105i64,
            "event": { "id": "0", "msg": { "type": "agent_message_content_delta", "thread_id": "019a713e-6ce6-7f82-b1e7-359628267934", "turn_id": "0", "item_id": "msg_0c5117240874292f016912c020d658819cb71e8bad4676a7c0", "delta": " composing" } }
        },
        {
            "timestamp_ms": 1762836527121i64,
            "event": { "id": "0", "msg": { "type": "agent_message_content_delta", "thread_id": "019a713e-6ce6-7f82-b1e7-359628267934", "turn_id": "0", "item_id": "msg_0c5117240874292f016912c020d658819cb71e8bad4676a7c0", "delta": " a" } }
        },
        {
            "timestamp_ms": 1762836527137i64,
            "event": { "id": "0", "msg": { "type": "agent_message_content_delta", "thread_id": "019a713e-6ce6-7f82-b1e7-359628267934", "turn_id": "0", "item_id": "msg_0c5117240874292f016912c020d658819cb71e8bad4676a7c0", "delta": " longer" } }
        },
        {
            "timestamp_ms": 1762836527148i64,
            "event": { "id": "0", "msg": { "type": "agent_message_content_delta", "thread_id": "019a713e-6ce6-7f82-b1e7-359628267934", "turn_id": "0", "item_id": "msg_0c5117240874292f016912c020d658819cb71e8bad4676a7c0", "delta": " answer" } }
        }
    ]);
    let logs: Vec<LogEntry> =
        serde_json::from_value(events_json).expect("valid streaming_deltas JSON");
    let session = ConversationId::from_string("019a713e-6ce4-73e0-bf9b-e070890e3790")
        .expect("valid session id");
    let earliest_ms = logs.first().map(|e| e.timestamp_ms);
    let latest_ms = logs.last().map(|e| e.timestamp_ms);
    let returned = logs.len();
    let total = logs.len();
    let more = false;

    let rendered = render_logs_as_text(
        session,
        &logs,
        earliest_ms,
        latest_ms,
        returned,
        total,
        more,
    );

    assert_snapshot!(
        rendered,
        @r###"Session 019a713e-6ce4-73e0-bf9b-e070890e3790 • status=working • older_logs=false • at_latest=true
    2025-11-11T04:48:47.148Z Assistant (typing):  is composing a longer answer (5 chunks)"###
    );
}

#[test]
fn subagent_logs_snapshot_reasoning_stream() {
    // Synthetic example of mid-reasoning without a summary yet.
    let events_json = json!([
        {
            "timestamp_ms": 1_000i64,
            "event": {
                "id": "0",
                "msg": {
                    "type": "reasoning_content_delta",
                    "thread_id": "thread-1",
                    "turn_id": "0",
                    "item_id": "rs_test",
                    "delta": " thinking"
                }
            }
        },
        {
            "timestamp_ms": 1_050i64,
            "event": {
                "id": "0",
                "msg": {
                    "type": "reasoning_content_delta",
                    "thread_id": "thread-1",
                    "turn_id": "0",
                    "item_id": "rs_test",
                    "delta": " about"
                }
            }
        },
        {
            "timestamp_ms": 1_100i64,
            "event": {
                "id": "0",
                "msg": {
                    "type": "reasoning_content_delta",
                    "thread_id": "thread-1",
                    "turn_id": "0",
                    "item_id": "rs_test",
                    "delta": " streaming state"
                }
            }
        }
    ]);
    let logs: Vec<LogEntry> =
        serde_json::from_value(events_json).expect("valid reasoning_stream JSON");
    let session = ConversationId::from_string("019a713e-eeee-73e0-bf9b-e070890e3790")
        .expect("valid session id");
    let earliest_ms = logs.first().map(|e| e.timestamp_ms);
    let latest_ms = logs.last().map(|e| e.timestamp_ms);
    let returned = logs.len();
    let total = logs.len();
    let more = false;

    let rendered = render_logs_as_text(
        session,
        &logs,
        earliest_ms,
        latest_ms,
        returned,
        total,
        more,
    );

    assert_snapshot!(
        rendered,
        @r###"Session 019a713e-eeee-73e0-bf9b-e070890e3790 • status=working • older_logs=false • at_latest=true
    1970-01-01T00:00:01.100Z Thinking:  thinking about streaming state (3 deltas)"###
    );
}

#[test]
fn subagent_logs_snapshot_no_older_history() {
    // Minimal case: single assistant message, no older history, at latest.
    let events_json = json!([
        {
            "timestamp_ms": 1_000i64,
            "event": {
                "id": "0",
                "msg": { "type": "agent_message", "message": "only event" }
            }
        }
    ]);
    let logs: Vec<LogEntry> = serde_json::from_value(events_json).expect("valid single-event JSON");
    let session = ConversationId::from_string("019a9999-aaaa-bbbb-cccc-ddddeeeeffff")
        .expect("valid session id");
    let earliest_ms = logs.first().map(|e| e.timestamp_ms);
    let latest_ms = logs.last().map(|e| e.timestamp_ms);
    let returned = logs.len();
    let total = logs.len();
    let more = false;

    let rendered = render_logs_as_text(
        session,
        &logs,
        earliest_ms,
        latest_ms,
        returned,
        total,
        more,
    );

    assert_snapshot!(
        rendered,
        @r###"Session 019a9999-aaaa-bbbb-cccc-ddddeeeeffff • status=idle • older_logs=false • at_latest=true
    1970-01-01T00:00:01.000Z Assistant: only event"###
    );
}

// Note: payload-shape and paging semantics (since_ms/before_ms/limit/max_bytes)
// are covered in focused unit tests in core/src/tools/handlers/subagent.rs.
