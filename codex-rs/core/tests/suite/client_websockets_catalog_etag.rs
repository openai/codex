//! Catalog invalidation from connection metadata, including warmup and reconnects.

use super::*;
use pretty_assertions::assert_eq;

#[test_case::test_case(false; "without warmup")]
#[test_case::test_case(true; "after warmup")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn handshake_models_etag_is_delivered_once_per_connection(warmup: bool) {
    skip_if_no_network!();

    let mut requests = Vec::new();
    if warmup {
        requests.push(vec![ev_response_created("warmup"), ev_completed("warmup")]);
    }
    requests.extend([
        vec![
            json!({
                "type": "codex.response.metadata",
                "headers": {"x-models-etag": "response-etag"},
            }),
            ev_response_created("resp-1"),
            ev_completed("resp-1"),
        ],
        vec![json!({
            "type": "response.failed",
            "response": {"error": {"code": "invalid_prompt"}},
        })],
    ]);
    let server = start_websocket_server_with_headers(vec![
        WebSocketConnectionConfig {
            requests,
            response_headers: vec![("x-models-etag".into(), "handshake-etag".into())],
            accept_delay: None,
            close_after_requests: true,
        },
        WebSocketConnectionConfig {
            requests: vec![vec![ev_response_created("resp-3"), ev_completed("resp-3")]],
            response_headers: vec![("x-models-etag".into(), "reconnect-etag".into())],
            accept_delay: None,
            close_after_requests: true,
        },
    ])
    .await;
    let harness = websocket_harness(&server).await;
    let mut session = harness.client.new_session();
    let prompt = prompt_with_input(vec![message_item("hello")]);
    let metadata = turn_metadata(&harness, /*turn_id*/ None);

    if warmup {
        session
            .prewarm_websocket(
                &prompt,
                &harness.model_info,
                &harness.session_telemetry,
                harness.effort.clone(),
                harness.summary,
                /*service_tier*/ None,
                &metadata,
                /*reuse_response*/ true,
            )
            .await
            .expect("warmup succeeds");
    }

    let mut etags_by_request = Vec::new();
    for request_index in 0..3 {
        let mut stream = session
            .stream(
                &prompt,
                &harness.model_info,
                &harness.session_telemetry,
                harness.effort.clone(),
                harness.summary,
                /*service_tier*/ None,
                &metadata,
                &InferenceTraceContext::disabled(),
            )
            .await
            .expect("open response stream");
        let mut etags = Vec::new();
        let mut failed = false;
        while let Some(event) = stream.next().await {
            match event {
                Ok(ResponseEvent::ModelsEtag(etag)) => etags.push(etag),
                Ok(ResponseEvent::Completed { .. }) => break,
                Err(error) => {
                    assert_eq!(
                        error.to_codex_protocol_error(),
                        CodexErrorInfo::InvalidPrompt
                    );
                    failed = true;
                    break;
                }
                Ok(_) => {}
            }
        }
        assert_eq!(failed, request_index == 1);
        etags_by_request.push(etags);
    }
    assert_eq!(
        etags_by_request,
        vec![
            vec!["handshake-etag", "response-etag"],
            vec![],
            vec!["reconnect-etag"],
        ]
    );
    assert_eq!(server.handshakes().len(), 2);
    server.shutdown().await;
}
