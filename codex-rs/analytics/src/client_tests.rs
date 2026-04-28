use super::AnalyticsEventsClient;
use super::AnalyticsEventsQueue;
use crate::facts::AnalyticsFact;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ClientResponsePayload;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadArchiveParams;
use codex_app_server_protocol::ThreadArchiveResponse;
use codex_app_server_protocol::TurnSteerParams;
use codex_app_server_protocol::TurnSteerResponse;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;

fn client_with_receiver() -> (AnalyticsEventsClient, mpsc::Receiver<AnalyticsFact>) {
    let (sender, receiver) = mpsc::channel(4);
    let queue = AnalyticsEventsQueue {
        sender,
        app_used_emitted_keys: Arc::new(Mutex::new(HashSet::new())),
        plugin_used_emitted_keys: Arc::new(Mutex::new(HashSet::new())),
    };
    (AnalyticsEventsClient { queue: Some(queue) }, receiver)
}

fn sample_turn_steer_request() -> ClientRequest {
    ClientRequest::TurnSteer {
        request_id: RequestId::Integer(1),
        params: TurnSteerParams {
            thread_id: "thread-1".to_string(),
            expected_turn_id: "turn-1".to_string(),
            input: Vec::new(),
            responsesapi_client_metadata: None,
        },
    }
}

fn sample_thread_archive_request() -> ClientRequest {
    ClientRequest::ThreadArchive {
        request_id: RequestId::Integer(2),
        params: ThreadArchiveParams {
            thread_id: "thread-1".to_string(),
        },
    }
}

#[test]
fn track_request_only_enqueues_analytics_relevant_requests() {
    let (client, mut receiver) = client_with_receiver();

    client.track_request(
        /*connection_id*/ 7,
        RequestId::Integer(1),
        &sample_turn_steer_request(),
    );
    assert!(matches!(
        receiver.try_recv(),
        Ok(AnalyticsFact::ClientRequest { .. })
    ));

    client.track_request(
        /*connection_id*/ 7,
        RequestId::Integer(2),
        &sample_thread_archive_request(),
    );
    assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));
}

#[test]
fn track_response_only_enqueues_analytics_relevant_responses() {
    let (client, mut receiver) = client_with_receiver();

    client.track_response(
        /*connection_id*/ 7,
        RequestId::Integer(1),
        ClientResponsePayload::TurnSteer(TurnSteerResponse {
            turn_id: "turn-1".to_string(),
        }),
    );
    assert!(matches!(
        receiver.try_recv(),
        Ok(AnalyticsFact::ClientResponse { .. })
    ));

    client.track_response(
        /*connection_id*/ 7,
        RequestId::Integer(2),
        ClientResponsePayload::ThreadArchive(ThreadArchiveResponse {}),
    );
    assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));
}
