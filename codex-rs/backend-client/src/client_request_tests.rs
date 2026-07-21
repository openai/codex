use std::io::Read;
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use pretty_assertions::assert_eq;

use super::*;

#[test]
fn client_preserves_supplied_http_client_factory_policy() {
    let client = Client::new(
        "https://example.test",
        HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy),
    );

    assert_eq!(
        client.http.outbound_proxy_policy(),
        OutboundProxyPolicy::RespectSystemProxy
    );
}

#[test]
fn list_tasks_url_omits_empty_query_and_encodes_all_parameters() {
    let client = Client::new(
        "https://example.test",
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    );

    assert_eq!(
        client
            .list_tasks_url(
                /*limit*/ None, /*task_filter*/ None, /*environment_id*/ None,
                /*cursor*/ None,
            )
            .unwrap(),
        "https://example.test/api/codex/tasks/list"
    );
    assert_eq!(
        client
            .list_tasks_url(
                /*limit*/ Some(10),
                /*task_filter*/ Some("mine / shared"),
                /*environment_id*/ Some("env&one"),
                /*cursor*/ Some("next=page"),
            )
            .unwrap(),
        "https://example.test/api/codex/tasks/list?limit=10&task_filter=mine+%2F+shared&cursor=next%3Dpage&environment_id=env%26one"
    );
}

#[tokio::test]
async fn migrated_requests_preserve_query_auth_and_json_body() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("HTTP listener should bind");
    let address = listener
        .local_addr()
        .expect("HTTP listener should have an address");
    let server = std::thread::spawn(move || {
        let mut requests = Vec::new();
        for body in [r#"{"items":[]}"#, r#"{"task":{"id":"task-created"}}"#] {
            let (mut stream, _) = listener.accept().expect("HTTP listener should accept");
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("HTTP stream should get a read timeout");
            let mut request = Vec::new();
            let mut buffer = [0_u8; 4096];
            loop {
                let size = stream.read(&mut buffer).expect("HTTP request should read");
                if size == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..size]);
                let Some(headers_end) = request.windows(4).position(|part| part == b"\r\n\r\n")
                else {
                    continue;
                };
                let headers = String::from_utf8_lossy(&request[..headers_end]);
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or(0);
                if request.len() >= headers_end + 4 + content_length {
                    break;
                }
            }
            requests.push(String::from_utf8(request).expect("request should be UTF-8"));
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .expect("HTTP response should write");
        }
        requests
    });
    let client = Client::new(
        format!("http://{address}"),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    )
    .with_auth_provider(Arc::new(codex_model_provider::BearerAuthProvider::new(
        "request-token".to_string(),
    )));

    let tasks = client
        .list_tasks(
            Some(10),
            Some("mine / shared"),
            Some("env&one"),
            Some("next=page"),
        )
        .await
        .expect("list request should succeed");
    let task_id = client
        .create_task(serde_json::json!({ "prompt": "hello" }))
        .await
        .expect("create request should succeed");
    let requests = server.join().expect("HTTP server should finish");

    assert_eq!(tasks, PaginatedListTaskListItem::new(Vec::new()));
    assert_eq!(task_id, "task-created");
    assert_eq!(requests.len(), 2);
    assert!(requests[0].starts_with(
        "GET /api/codex/tasks/list?limit=10&task_filter=mine+%2F+shared&cursor=next%3Dpage&environment_id=env%26one HTTP/1.1\r\n"
    ));
    assert!(
        requests[0]
            .to_ascii_lowercase()
            .contains("authorization: bearer request-token\r\n")
    );
    assert!(requests[1].starts_with("POST /api/codex/tasks HTTP/1.1\r\n"));
    assert!(
        requests[1]
            .to_ascii_lowercase()
            .contains("authorization: bearer request-token\r\n")
    );
    assert!(requests[1].ends_with(r#"{"prompt":"hello"}"#));
}
