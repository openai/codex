use futures::FutureExt;
use reqwest::StatusCode;
use serde::Serialize;
use serde_json::json;
use std::fmt::{Display, Formatter};
use std::io::ErrorKind;
use std::iter;
use std::net::TcpStream;
use std::time::Duration;
use wiremock::matchers::{PathExactMatcher, body_json, body_partial_json, method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

#[async_std::test]
async fn new_starts_the_server() {
    // Act
    let mock_server = MockServer::start().await;

    // Assert
    assert!(TcpStream::connect(mock_server.address()).is_ok())
}

#[async_std::test]
async fn returns_404_if_nothing_matches() {
    // Arrange - no mocks mounted
    let mock_server = MockServer::start().await;

    // Act
    let status = reqwest::get(&mock_server.uri()).await.unwrap().status();

    // Assert
    assert_eq!(status, 404);
}

#[async_std::test]
#[should_panic]
async fn panics_if_the_expectation_is_not_satisfied() {
    // Arrange
    let mock_server = MockServer::start().await;
    let response = ResponseTemplate::new(200);
    Mock::given(method("GET"))
        .respond_with(response)
        .expect(1..)
        .named("panics_if_the_expectation_is_not_satisfied expectation failed")
        .mount(&mock_server)
        .await;

    // Act - we never call the mock
}

#[async_std::test]
#[should_panic(expected = "Verifications failed:
- Mock #0.
\tExpected range of matching incoming requests: 1 <= x
\tNumber of matched incoming requests: 0

The server did not receive any request.")]
async fn no_received_request_line_is_printed_in_the_panic_message_if_expectations_are_not_verified()
{
    // Arrange
    let mock_server = MockServer::start().await;
    let response = ResponseTemplate::new(200);
    Mock::given(method("GET"))
        .respond_with(response)
        .expect(1..)
        .mount(&mock_server)
        .await;

    // Act - we never call the mock
}

#[async_std::test]
#[should_panic(expected = "Verifications failed:
- Mock #0.
\tExpected range of matching incoming requests: 1 <= x
\tNumber of matched incoming requests: 0

Received requests:
- Request #1
\tGET http://localhost/")]
async fn received_request_are_printed_as_panic_message_if_expectations_are_not_verified() {
    // Arrange
    let mock_server = MockServer::start().await;
    let response = ResponseTemplate::new(200);
    Mock::given(method("POST"))
        .respond_with(response)
        .expect(1..)
        .mount(&mock_server)
        .await;

    // Act - we sent a request that does not match (GET)
    reqwest::get(&mock_server.uri()).await.unwrap();

    // Assert - verified on drop
}

#[async_std::test]
#[should_panic]
async fn panic_during_expectation_does_not_crash() {
    // Arrange
    let mock_server = MockServer::start().await;
    let response = ResponseTemplate::new(200);
    Mock::given(method("GET"))
        .respond_with(response)
        .expect(1..)
        .named("panic_during_expectation_does_not_crash expectation failed")
        .mount(&mock_server)
        .await;

    // Act - start a panic
    panic!("forced panic")
}

#[async_std::test]
async fn simple_route_mock() {
    // Arrange
    let mock_server = MockServer::start().await;
    let response = ResponseTemplate::new(200).set_body_bytes("world");
    let mock = Mock::given(method("GET"))
        .and(PathExactMatcher::new("hello"))
        .respond_with(response);
    mock_server.register(mock).await;

    // Act
    let response = reqwest::get(format!("{}/hello", &mock_server.uri()))
        .await
        .unwrap();

    // Assert
    assert_eq!(response.status(), 200);
    assert_eq!(response.text().await.unwrap(), "world");
}

#[async_std::test]
async fn two_route_mocks() {
    // Arrange
    let mock_server = MockServer::start().await;

    // First
    let response = ResponseTemplate::new(200).set_body_bytes("aaa");
    Mock::given(method("GET"))
        .and(PathExactMatcher::new("first"))
        .respond_with(response)
        .named("/first")
        .mount(&mock_server)
        .await;

    // Second
    let response = ResponseTemplate::new(200).set_body_bytes("bbb");
    Mock::given(method("GET"))
        .and(PathExactMatcher::new("second"))
        .respond_with(response)
        .named("/second")
        .mount(&mock_server)
        .await;

    // Act
    let first_response = reqwest::get(format!("{}/first", &mock_server.uri()))
        .await
        .unwrap();
    let second_response = reqwest::get(format!("{}/second", &mock_server.uri()))
        .await
        .unwrap();

    // Assert
    assert_eq!(first_response.status(), 200);
    assert_eq!(second_response.status(), 200);

    assert_eq!(first_response.text().await.unwrap(), "aaa");
    assert_eq!(second_response.text().await.unwrap(), "bbb");
}

#[async_std::test]
async fn body_json_matches_independent_of_key_ordering() {
    #[derive(Serialize)]
    struct X {
        b: u8,
        a: u8,
    }

    // Arrange
    let expected_body = json!({ "a": 1, "b": 2 });
    let body = serde_json::to_string(&X { a: 1, b: 2 }).unwrap();

    let mock_server = MockServer::start().await;
    let response = ResponseTemplate::new(200);
    let mock = Mock::given(method("POST"))
        .and(body_json(expected_body))
        .respond_with(response);
    mock_server.register(mock).await;

    // Act
    let client = reqwest::Client::new();
    let response = client
        .post(mock_server.uri())
        .body(body)
        .send()
        .await
        .unwrap();

    // Assert
    assert_eq!(response.status(), 200);
}

#[async_std::test]
async fn body_json_partial_matches_a_part_of_response_json() {
    // Arrange
    let expected_body = json!({ "a": 1, "c": { "e": 2 } });
    let body = json!({ "a": 1, "b": 2, "c": { "d": 1, "e": 2 } });

    let mock_server = MockServer::start().await;
    let response = ResponseTemplate::new(200);
    let mock = Mock::given(method("POST"))
        .and(body_partial_json(expected_body))
        .respond_with(response);
    mock_server.register(mock).await;

    let client = reqwest::Client::new();

    // Act
    let response = client
        .post(mock_server.uri())
        .json(&body)
        .send()
        .await
        .unwrap();

    // Assert
    assert_eq!(response.status(), StatusCode::OK);
}

#[should_panic(expected = "\
Wiremock can't match the path `abcd?` because it contains a `?`. You must use `wiremock::matchers::query_param` to match on query parameters (the part of the path after the `?`).")]
#[async_std::test]
async fn query_parameter_is_not_accepted_in_path() {
    Mock::given(method("GET")).and(path("abcd?"));
}

#[should_panic(expected = "\
Wiremock can't match the path `https://domain.com/abcd` because it contains the host `domain.com`. You don't have to specify the host - wiremock knows it. Try replacing your path with `path(\"/abcd\")`")]
#[async_std::test]
async fn host_is_not_accepted_in_path() {
    Mock::given(method("GET")).and(path("https://domain.com/abcd"));
}

#[async_std::test]
async fn use_mock_guard_to_verify_requests_from_mock() {
    // Arrange
    let mock_server = MockServer::start().await;

    let first = mock_server
        .register_as_scoped(
            Mock::given(method("POST"))
                .and(PathExactMatcher::new("first"))
                .respond_with(ResponseTemplate::new(200)),
        )
        .await;

    let second = mock_server
        .register_as_scoped(
            Mock::given(method("POST"))
                .and(PathExactMatcher::new("second"))
                .respond_with(ResponseTemplate::new(200)),
        )
        .await;

    let client = reqwest::Client::new();

    // Act
    let uri = mock_server.uri();
    let response = client
        .post(format!("{uri}/first"))
        .json(&json!({ "attempt": 1}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let response = client
        .post(format!("{uri}/first"))
        .json(&json!({ "attempt": 2}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let response = client
        .post(format!("{uri}/second"))
        .json(&json!({ "attempt": 99}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Assert
    let all_requests_to_first = first.received_requests().await;
    assert_eq!(all_requests_to_first.len(), 2);

    let value: serde_json::Value = second.received_requests().await[0].body_json().unwrap();

    assert_eq!(value, json!({"attempt": 99}));
}

#[async_std::test]
async fn use_mock_guard_to_await_satisfaction_readiness() {
    // Arrange
    let mock_server = MockServer::start().await;

    let satisfy = mock_server
        .register_as_scoped(
            Mock::given(method("POST"))
                .and(PathExactMatcher::new("satisfy"))
                .respond_with(ResponseTemplate::new(200))
                .expect(1),
        )
        .await;

    let eventually_satisfy = mock_server
        .register_as_scoped(
            Mock::given(method("POST"))
                .and(PathExactMatcher::new("eventually_satisfy"))
                .respond_with(ResponseTemplate::new(200))
                .expect(1),
        )
        .await;

    // Act one
    let uri = mock_server.uri();
    let client = reqwest::Client::new();
    let response = client.post(format!("{uri}/satisfy")).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Assert
    satisfy
        .wait_until_satisfied()
        .now_or_never()
        .expect("should be satisfied immediately");

    eventually_satisfy
        .wait_until_satisfied()
        .now_or_never()
        .ok_or(())
        .expect_err("should not be satisfied yet");

    // Act two
    async_std::task::spawn(async move {
        async_std::task::sleep(Duration::from_millis(100)).await;
        let client = reqwest::Client::new();
        let response = client
            .post(format!("{uri}/eventually_satisfy"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    });

    // Assert
    eventually_satisfy
        .wait_until_satisfied()
        .now_or_never()
        .ok_or(())
        .expect_err("should not be satisfied yet");

    async_std::io::timeout(
        Duration::from_millis(1000),
        eventually_satisfy.wait_until_satisfied().map(Ok),
    )
    .await
    .expect("should be satisfied");
}

#[async_std::test]
async fn debug_prints_mock_server_variants() {
    let pooled_mock_server = MockServer::start().await;
    let pooled_debug_str = format!("{:?}", pooled_mock_server);

    assert!(pooled_debug_str.starts_with("MockServer(Pooled(Object {"));
    assert!(
        pooled_debug_str
            .find(
                format!(
                    "BareMockServer {{ address: {} }}",
                    pooled_mock_server.address()
                )
                .as_str()
            )
            .is_some()
    );

    let bare_mock_server = MockServer::builder().start().await;
    assert_eq!(
        format!(
            "MockServer(Bare(BareMockServer {{ address: {} }}))",
            bare_mock_server.address()
        ),
        format!("{:?}", bare_mock_server)
    );
}

#[tokio::test]
async fn io_err() {
    // Act
    let mock_server = MockServer::start().await;
    let mock = Mock::given(method("GET")).respond_with_err(|_: &Request| {
        std::io::Error::new(ErrorKind::ConnectionReset, "connection reset")
    });
    mock_server.register(mock).await;

    // Assert
    let err = reqwest::get(&mock_server.uri()).await.unwrap_err();
    // We're skipping the original error since it can be either `error sending request` or
    // `error sending request for url (http://127.0.0.1:<port>/)`
    let actual_err: Vec<String> =
        iter::successors(std::error::Error::source(&err), |err| err.source())
            .map(|err| err.to_string())
            .collect();

    let expected_err = vec![
        "client error (SendRequest)".to_string(),
        "connection closed before message completed".to_string(),
    ];
    assert_eq!(actual_err, expected_err);
}

#[tokio::test]
async fn custom_err() {
    // Act
    #[derive(Debug)]
    struct CustomErr;
    impl Display for CustomErr {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            f.write_str("custom error")
        }
    }
    impl std::error::Error for CustomErr {}

    let mock_server = MockServer::start().await;
    let mock = Mock::given(method("GET")).respond_with_err(|_: &Request| CustomErr);
    mock_server.register(mock).await;

    // Assert
    let err = reqwest::get(&mock_server.uri()).await.unwrap_err();
    // We're skipping the original error since it can be either `error sending request` or
    // `error sending request for url (http://127.0.0.1:<port>/)`
    let actual_err: Vec<String> =
        iter::successors(std::error::Error::source(&err), |err| err.source())
            .map(|err| err.to_string())
            .collect();

    let expected_err = vec![
        "client error (SendRequest)".to_string(),
        "connection closed before message completed".to_string(),
    ];
    assert_eq!(actual_err, expected_err);
}

#[async_std::test]
async fn method_matcher_is_case_insensitive() {
    // Arrange
    let mock_server = MockServer::start().await;
    let response = ResponseTemplate::new(200).set_body_bytes("world");
    let mock = Mock::given(method("Get"))
        .and(PathExactMatcher::new("hello"))
        .respond_with(response);
    mock_server.register(mock).await;

    // Act
    let response = reqwest::get(format!("{}/hello", &mock_server.uri()))
        .await
        .unwrap();

    // Assert
    assert_eq!(response.status(), 200);
    assert_eq!(response.text().await.unwrap(), "world");
}

#[async_std::test]
async fn http_crate_method_can_be_used_directly() {
    use http::Method;
    // Arrange
    let mock_server = MockServer::start().await;
    let response = ResponseTemplate::new(200).set_body_bytes("world");
    let mock = Mock::given(method(Method::GET))
        .and(PathExactMatcher::new("hello"))
        .respond_with(response);
    mock_server.register(mock).await;

    // Act
    let response = reqwest::get(format!("{}/hello", &mock_server.uri()))
        .await
        .unwrap();

    // Assert
    assert_eq!(response.status(), 200);
    assert_eq!(response.text().await.unwrap(), "world");
}
