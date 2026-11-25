use super::*;
use crate::async_scorer::request::PreparedRequest;
use crate::async_scorer::sampler::ConversationSample;
use codex_guardian_context::TranscriptCursor;
use codex_history::ResponseItemEnvelope;
use pretty_assertions::assert_eq;

// The app-server approval matrix covers successful HTTP retention and both experiment arms.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retained_websocket_requests_preserve_model_output_and_ids() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let events = vec![
        ev_output_text_delta("low"),
        json!({"type":"response.output_item.done", "item":{"type":"reasoning", "id":"rs_score", "summary":[], "encrypted_content":"opaque reasoning"}}),
        ev_assistant_message("msg_score", "low"),
        ev_completed("resp_score"),
    ];
    let ws =
        responses::start_websocket_server(vec![Vec::new(), vec![events.clone(), events]]).await;
    let mut config = sampler_config(format!(
        "http://{}/v1",
        ws.uri().trim_start_matches("ws://")
    ));
    config.quote_guardian_replies = true;
    let sampler = connect_sampler(config).await?;
    let mut committed = sample_request("parent").input;
    let root = codex_protocol::AgentPath::root();
    let mut child = codex_protocol::protocol::InterAgentCommunication::new_encrypted(
        root.join("scoring").expect("scoring path"),
        root,
        Vec::new(),
        "opaque-score".into(),
        /*trigger_turn*/ false,
    )
    .to_model_input_item();
    child.set_id(Some(codex_protocol::ResponseItemId::with_suffix(
        "amsg", "child",
    )));
    committed.push(child);
    let mut output = Vec::new();
    for _ in 0..2 {
        let (ready, score) = tokio::sync::oneshot::channel();
        let mut request = sample_request("parent");
        request.input = committed.clone();
        let input_tokens = request
            .input
            .iter()
            .map(codex_guardian_context::estimate_input_tokens)
            .fold(0usize, usize::saturating_add);
        let prepared = PreparedRequest {
            input: request
                .input
                .iter()
                .cloned()
                .map(ResponseItemEnvelope::new)
                .collect(),
            input_tokens,
            max_input_tokens: sampler.max_input_tokens(),
            existing_context_tokens: 0,
            cursor: TranscriptCursor {
                parent_history_version: 0,
                transcript_entry_count: 0,
            },
            truncations: Vec::new(),
            section_costs: Vec::new(),
        };
        let request = ConversationSample {
            input: prepared,
            sampling: request,
        };
        let (completed, score) = tokio::join!(sampler.sample_retained(request, ready), score);
        assert_eq!(score??.accept(), "low");
        let completed = completed
            .expect("completed model history")
            .into_iter()
            .map(ResponseItemEnvelope::into_item)
            .collect::<Vec<_>>();
        assert_eq!(&completed[..committed.len()], committed);
        output = completed[committed.len()..].to_vec();
        committed = completed;
        committed.push(responses::user_message_item("Next action."));
    }
    assert!(
        matches!(&output[..], [ResponseItem::Reasoning { encrypted_content: Some(_), .. }, ResponseItem::Message { id: Some(id), role, .. }] if id.as_str() == "msg_score" && role == "assistant")
    );
    let requests = ws.connections().into_iter().flatten().collect::<Vec<_>>();
    assert_eq!(requests.len(), 2);
    let first = requests[0].body_json();
    assert_eq!(first["include"], json!(["reasoning.encrypted_content"]));
    let second = requests[1].body_json();
    let reply = &first["input"][1];
    assert_eq!(reply["type"], "function_call_output");
    assert_eq!(reply["id"], "fco_child");
    assert_eq!(reply["namespace"], "guardian");
    assert_eq!(reply["name"], "subagent_reply");
    assert!(reply.get("call_id").is_none());
    assert_eq!(
        reply["output"][1],
        json!({"type": "encrypted_content", "encrypted_content": "opaque-score"})
    );
    assert_eq!(second["input"][0], first["input"][0]);
    assert_eq!(&second["input"][1], reply);
    assert_eq!(
        json!(&second["input"].as_array().unwrap()[2..2 + output.len()]),
        serde_json::to_value(&output)?
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retention_overflow_or_incomplete_stream_keeps_early_score() -> Result<()> {
    skip_if_no_network!(Ok(()));
    for (limit, output, completed, receive_score, early_delta) in [
        (
            512,
            vec![ev_assistant_message(
                "msg_large",
                &"large ".repeat(/*n*/ 1_000),
            )],
            true,
        ),
        (
            64_000,
            vec![
                json!({"type":"response.output_item.done", "item":{"type":"reasoning", "id":"rs_large", "summary":[], "encrypted_content":"x".repeat(/*n*/ 48_000)}}),
                ev_assistant_message("msg_score", "low"),
            ],
            true,
        ),
        (
            512,
            vec![ev_assistant_message("msg_score", "lowhigh")],
            true,
        ),
        (
            512,
            vec![
                json!({"type":"response.output_item.done", "item":{"type":"message", "role":"developer", "content":[{"type":"output_text", "text":"low"}]}}),
            ],
            true,
        ),
        (512, vec![], false),
    ].into_iter().flat_map(|(limit, output, completed)| {
        [(true, true), (false, true), (false, false)].map(|(receive_score, early_delta)| {
            (limit, output.clone(), completed, receive_score, early_delta)
        })
    }) {
        let mut events = if early_delta {
            vec![ev_output_text_delta("low")]
        } else {
            Vec::new()
        };
        events.extend(output);
        if completed {
            events.push(ev_completed("score"));
        }
        let server = responses::start_mock_server().await;
        responses::mount_sse_once(&server, responses::sse(events)).await;
        let mut config = sampler_config(server.uri());
        config.max_input_tokens = limit;
        let metrics = Arc::new(RecordingMetrics::default());
        config.metrics = Some(metrics.clone());
        let sampler = LunaSampler::new(config);
        let (ready, score) = tokio::sync::oneshot::channel();
        let request = sample_request("parent");
        let input_tokens = request
            .input
            .iter()
            .map(codex_guardian_context::estimate_input_tokens)
            .fold(0usize, usize::saturating_add);
        let prepared = PreparedRequest {
            input: request
                .input
                .iter()
                .cloned()
                .map(ResponseItemEnvelope::new)
                .collect(),
            input_tokens,
            max_input_tokens: sampler.max_input_tokens(),
            existing_context_tokens: 0,
            cursor: TranscriptCursor {
                parent_history_version: 0,
                transcript_entry_count: 0,
            },
            truncations: Vec::new(),
            section_costs: Vec::new(),
        };
        let request = ConversationSample {
            input: prepared,
            sampling: request,
        };
        let history = if receive_score {
            let (history, score) = tokio::join!(sampler.sample_retained(request, ready), score);
            assert_eq!(score??.accept(), "low");
            history
        } else {
            let score = early_delta.then_some(score);
            let history = sampler.sample_retained(request, ready).await;
            // A queued score discarded without expiry remains a genuine cancellation.
            drop(score);
            history
        };
        let (outcome, failure_reason) = if receive_score {
            ("success", "none")
        } else {
            ("cancelled", "cancelled")
        };
        assert_eq!(history, None);
        // Before the first score, an incomplete stream retries against the exhausted mock.
        let attempts = if completed || early_delta { 1 } else { 2 };
        metrics.assert_outcome(outcome, attempts, failure_reason);
    }
    Ok(())
}
