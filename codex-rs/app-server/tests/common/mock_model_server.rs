use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use core_test_support::responses;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Respond;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::path_regex;

/// Create a mock server that will provide the responses, in order, for
/// requests to the `/v1/chat/completions` endpoint.
pub async fn create_mock_chat_completions_server(responses: Vec<String>) -> MockServer {
    let server = MockServer::start().await;

    let num_calls = responses.len();
    let seq_responder = SeqResponder {
        num_calls: AtomicUsize::new(0),
        responses,
    };

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(seq_responder)
        .expect(num_calls as u64)
        .mount(&server)
        .await;

    server
}

/// Same as `create_mock_chat_completions_server` but does not enforce an
/// expectation on the number of calls.
pub async fn create_mock_chat_completions_server_unchecked(responses: Vec<String>) -> MockServer {
    let server = MockServer::start().await;

    let seq_responder = SeqResponder {
        num_calls: AtomicUsize::new(0),
        responses,
    };

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(seq_responder)
        .mount(&server)
        .await;

    server
}

struct SeqResponder {
    num_calls: AtomicUsize,
    responses: Vec<String>,
}

impl Respond for SeqResponder {
    fn respond(&self, _: &wiremock::Request) -> ResponseTemplate {
        let call_num = self.num_calls.fetch_add(1, Ordering::SeqCst);
        match self.responses.get(call_num) {
            Some(response) => ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_raw(response.clone(), "text/event-stream"),
            None => panic!("no response for {call_num}"),
        }
    }
}

/// Create a mock responses API server that returns the same assistant message for every request.
pub async fn create_mock_responses_server_repeating_assistant(message: &str) -> MockServer {
    let server = responses::start_mock_server().await;
    let body = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("msg-1", message),
        responses::ev_completed("resp-1"),
    ]);
    Mock::given(method("POST"))
        .and(path_regex(".*/responses$"))
        .respond_with(responses::sse_response(body))
        .mount(&server)
        .await;
    server
}
