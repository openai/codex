use std::io::BufRead;
use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_http_client::cache_system_proxy_route_for_test;
use pretty_assertions::assert_eq;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

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

const BLOCKED_ORIGIN: &str = "http://127.0.0.1:0";

fn run_without_environment_proxies(test_name: &str) -> bool {
    const CHILD: &str = "CODEX_BOOTSTRAP_PROXY_TEST_CHILD";
    if std::env::var_os(CHILD).is_some() {
        return false;
    }
    let mut command = Command::new(std::env::current_exe().expect("test executable"));
    command.arg("--exact").arg(test_name).env(CHILD, "1");
    for key in [
        "HTTP_PROXY",
        "http_proxy",
        "HTTPS_PROXY",
        "https_proxy",
        "ALL_PROXY",
        "all_proxy",
        "NO_PROXY",
        "no_proxy",
    ] {
        command.env_remove(key);
    }
    let output = command.output().expect("run isolated proxy test");
    assert!(
        output.status.success(),
        "{test_name} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    true
}

fn fallback_client(base_url: &str) -> Client {
    Client::new(
        base_url,
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault).with_system_proxy_fallback(),
    )
    .with_auth_provider(Arc::new(codex_model_provider::BearerAuthProvider::new(
        "bootstrap-token".to_string(),
    )))
    .with_chatgpt_account_id("workspace-123")
    .with_user_agent("bootstrap-test")
}

#[tokio::test]
async fn bootstrap_gets_resolve_each_fallback_destination_and_preserve_headers() {
    if run_without_environment_proxies(
        "client::request_tests::bootstrap_gets_resolve_each_fallback_destination_and_preserve_headers",
    ) {
        return;
    }
    for (base_suffix, api_prefix) in [("", "/api/codex"), ("/backend-api", "/wham")] {
        let bundle_proxy = MockServer::start().await;
        let accounts_proxy = MockServer::start().await;
        let base_url = format!("{BLOCKED_ORIGIN}{base_suffix}");
        let client = fallback_client(&base_url);
        for (endpoint, proxy) in [
            ("config/bundle", &bundle_proxy),
            ("accounts/check", &accounts_proxy),
        ] {
            let endpoint_path = format!("{base_suffix}{api_prefix}/{endpoint}");
            cache_system_proxy_route_for_test(
                &format!("{base_url}{api_prefix}/{endpoint}"),
                proxy.uri(),
            );
            Mock::given(method("GET"))
                .and(path(endpoint_path))
                .and(header("authorization", "Bearer bootstrap-token"))
                .and(header("chatgpt-account-id", "workspace-123"))
                .and(header("user-agent", "bootstrap-test"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
                .expect(1)
                .mount(proxy)
                .await;
        }
        client.get_config_bundle().await.expect("cloud bundle");
        client.get_accounts_check().await.expect("accounts check");
        bundle_proxy.verify().await;
        accounts_proxy.verify().await;
    }
}

#[tokio::test]
async fn bootstrap_gets_keep_default_responses_without_proxy_retry() {
    if run_without_environment_proxies(
        "client::request_tests::bootstrap_gets_keep_default_responses_without_proxy_retry",
    ) {
        return;
    }
    for status in [200, 401] {
        let issuer = MockServer::start().await;
        let proxy = MockServer::start().await;
        for endpoint in ["config/bundle", "accounts/check"] {
            cache_system_proxy_route_for_test(
                &format!("{}/api/codex/{endpoint}", issuer.uri()),
                proxy.uri(),
            );
        }
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(status).set_body_json(serde_json::json!({})))
            .expect(2)
            .mount(&issuer)
            .await;
        let client = fallback_client(&issuer.uri());
        let bundle = client.get_config_bundle().await;
        let accounts = client.get_accounts_check().await;
        if status == 200 {
            bundle.expect("default cloud route succeeds");
            accounts.expect("default accounts route succeeds");
        } else {
            assert_eq!(
                [
                    bundle.expect_err("cloud status error").status(),
                    accounts.expect_err("accounts status error").status(),
                ],
                [Some(http::StatusCode::UNAUTHORIZED); 2]
            );
        }
        issuer.verify().await;
        assert_eq!(proxy.received_requests().await.unwrap_or_default().len(), 0);
    }
}

#[tokio::test]
async fn bootstrap_gets_honor_disabled_fallback() {
    if run_without_environment_proxies(
        "client::request_tests::bootstrap_gets_honor_disabled_fallback",
    ) {
        return;
    }
    let proxy = MockServer::start().await;
    let base_url = BLOCKED_ORIGIN;
    for endpoint in ["config/bundle", "accounts/check"] {
        cache_system_proxy_route_for_test(&format!("{base_url}/api/codex/{endpoint}"), proxy.uri());
    }
    let client = Client::new(
        base_url,
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    );
    assert!(client.get_config_bundle().await.is_err());
    assert!(client.get_accounts_check().await.is_err());
    assert_eq!(proxy.received_requests().await.unwrap_or_default().len(), 0);
}

#[tokio::test]
async fn bootstrap_get_recovers_from_stalled_body_before_cloud_startup_timeout() {
    if run_without_environment_proxies(
        "client::request_tests::bootstrap_get_recovers_from_stalled_body_before_cloud_startup_timeout",
    ) {
        return;
    }
    let issuer = TcpListener::bind(("127.0.0.1", 0)).expect("bind stalled issuer");
    let issuer_url = format!("http://{}", issuer.local_addr().expect("issuer address"));
    let (finish, finished) = std::sync::mpsc::channel();
    let issuer_thread = std::thread::spawn(move || {
        let (mut stream, _) = issuer.accept().expect("accept bootstrap GET");
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .expect("set read timeout");
        let mut request = String::new();
        std::io::BufReader::new(&stream)
            .read_line(&mut request)
            .expect("read request line");
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{",
            )
            .expect("send headers and incomplete body");
        finished
            .recv_timeout(Duration::from_secs(20))
            .expect("proxy request finishes while issuer body is pending");
    });
    let proxy = MockServer::start().await;
    cache_system_proxy_route_for_test(
        &format!("{issuer_url}/api/codex/config/bundle"),
        proxy.uri(),
    );
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&proxy)
        .await;
    let result = tokio::time::timeout(
        Duration::from_secs(15),
        fallback_client(&issuer_url).get_config_bundle(),
    )
    .await;
    finish.send(()).expect("release stalled issuer");
    issuer_thread.join().expect("issuer finishes");
    result
        .expect("proxy retry must fit inside the cloud startup budget")
        .expect("cloud bundle via proxy");
    proxy.verify().await;
}
