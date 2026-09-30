use super::*;
use crate::async_scorer::request::PreparedRequest;
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
    let sampler = connect_sampler(sampler_config(format!(
        "http://{}/v1",
        ws.uri().trim_start_matches("ws://")
    )))
    .await?;
    let mut committed = sample_request("parent").input;
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
            sampling: &request,
            input: request
                .input
                .iter()
                .cloned()
                .map(ResponseItemEnvelope::new)
                .collect(),
            input_tokens,
            existing_context_tokens: 0,
            cursor: TranscriptCursor {
                parent_history_version: 0,
                transcript_entry_count: 0,
            },
            truncations: Vec::new(),
            section_costs: Vec::new(),
        };
        let (completed, score) = tokio::join!(sampler.sample_retained(prepared, ready), score);
        assert_eq!(score??, "low");
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
    assert_eq!(second["input"][0], first["input"][0]);
    assert_eq!(
        json!(&second["input"].as_array().unwrap()[1..1 + output.len()]),
        serde_json::to_value(&output)?
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retention_overflow_or_incomplete_stream_keeps_early_score() -> Result<()> {
    skip_if_no_network!(Ok(()));
    for (limit, output, completed) in [
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
    ] {
        let mut events = vec![ev_output_text_delta("low")];
        events.extend(output);
        if completed {
            events.push(ev_completed("score"));
        }
        let server = responses::start_mock_server().await;
        responses::mount_sse_once(&server, responses::sse(events)).await;
        let mut config = sampler_config(server.uri());
        config.max_input_tokens = limit;
        let sampler = LunaSampler::new(config);
        let (ready, score) = tokio::sync::oneshot::channel();
        let request = sample_request("parent");
        let input_tokens = request
            .input
            .iter()
            .map(codex_guardian_context::estimate_input_tokens)
            .fold(0usize, usize::saturating_add);
        let prepared = PreparedRequest {
            sampling: &request,
            input: request
                .input
                .iter()
                .cloned()
                .map(ResponseItemEnvelope::new)
                .collect(),
            input_tokens,
            existing_context_tokens: 0,
            cursor: TranscriptCursor {
                parent_history_version: 0,
                transcript_entry_count: 0,
            },
            truncations: Vec::new(),
            section_costs: Vec::new(),
        };
        let (history, score) = tokio::join!(sampler.sample_retained(prepared, ready), score);
        assert_eq!(score??, "low");
        assert_eq!(history, None);
    }
    Ok(())
}
