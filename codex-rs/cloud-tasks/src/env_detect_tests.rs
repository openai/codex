use std::collections::HashMap;
use std::io;
use std::io::Read;
use std::io::Write;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use http::HeaderMap;
use http::HeaderValue;
use http::StatusCode;
use http::header::AUTHORIZATION;
use pretty_assertions::assert_eq;

use super::*;

const BASE_URL: &str = "https://chatgpt.com/backend-api";
const BY_REPO_URL: &str =
    "https://chatgpt.com/backend-api/wham/environments/by-repo/github/openai/codex";
const GLOBAL_URL: &str = "https://chatgpt.com/backend-api/wham/environments";

#[tokio::test]
async fn production_http_forwards_headers_and_decodes_response() {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0))
        .expect("environment HTTP listener should bind");
    let address = listener
        .local_addr()
        .expect("environment HTTP listener should have an address");
    listener
        .set_nonblocking(true)
        .expect("environment HTTP listener should become nonblocking");
    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(2);
        let (mut stream, _) = loop {
            match listener.accept() {
                Ok(connection) => break connection,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    assert!(
                        Instant::now() < deadline,
                        "environment HTTP listener should receive a request"
                    );
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("environment HTTP listener should accept: {error}"),
            }
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("environment HTTP stream should get a read timeout");
        let mut request = Vec::new();
        let mut buffer = [0_u8; 1024];
        loop {
            let bytes_read = stream
                .read(&mut buffer)
                .expect("environment HTTP request should read");
            if bytes_read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..bytes_read]);
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }

        let body = r#"[{"id":"env-real","label":"Real"}]"#;
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .expect("environment HTTP response should write");
        String::from_utf8(request).expect("environment HTTP request should be UTF-8")
    });
    let http = RouteAwareClientPool::new_without_request_logging(
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        ClientRouteClass::Api,
    );
    let base_url = format!("http://{address}");
    let headers =
        HeaderMap::from_iter([(AUTHORIZATION, HeaderValue::from_static("Bearer real-token"))]);

    let selection = tokio::time::timeout(
        Duration::from_secs(2),
        autodetect_environment_id_with_origins(
            &http,
            &base_url,
            &headers,
            /*desired_label*/ None,
            &[],
        ),
    )
    .await
    .expect("environment request should finish")
    .expect("environment response should decode");
    let request = server
        .join()
        .expect("environment HTTP server should finish");

    assert_eq!(
        selection,
        AutodetectSelection {
            id: "env-real".to_string(),
            label: Some("Real".to_string()),
        }
    );
    assert!(request.starts_with("GET /api/codex/environments HTTP/1.1\r\n"));
    assert!(
        request
            .to_ascii_lowercase()
            .contains("authorization: bearer real-token\r\n")
    );
}

#[tokio::test]
async fn autodetect_requests_exact_repository_endpoint_and_decodes_selection() {
    let http = FakeHttp::new(HashMap::from([(
        BY_REPO_URL.to_string(),
        json_response(r#"[{"id":"env-repo","label":"Repository","is_pinned":true}]"#),
    )]));

    let headers = HeaderMap::from_iter([(
        AUTHORIZATION,
        HeaderValue::from_static("Bearer forwarded-token"),
    )]);
    let selection = autodetect_environment_id_with_origins(
        &http,
        BASE_URL,
        &headers,
        Some("Repository".to_string()),
        &["git@github.com:openai/codex.git".to_string()],
    )
    .await
    .expect("repository environment should be selected");

    assert_eq!(
        selection,
        AutodetectSelection {
            id: "env-repo".to_string(),
            label: Some("Repository".to_string()),
        }
    );
    assert_eq!(
        http.requests(),
        vec![RecordedRequest {
            url: BY_REPO_URL.to_string(),
            headers,
        }]
    );
}

#[tokio::test]
async fn autodetect_falls_back_to_exact_global_endpoint_and_decodes_selection() {
    let http = FakeHttp::new(HashMap::from([
        (BY_REPO_URL.to_string(), json_response("[]")),
        (
            GLOBAL_URL.to_string(),
            json_response(r#"[{"id":"env-global","label":"Global"}]"#),
        ),
    ]));

    let selection = autodetect_environment_id_with_origins(
        &http,
        BASE_URL,
        &HeaderMap::new(),
        /*desired_label*/ None,
        &["git@github.com:openai/codex.git".to_string()],
    )
    .await
    .expect("global environment should be selected");

    assert_eq!(
        selection,
        AutodetectSelection {
            id: "env-global".to_string(),
            label: Some("Global".to_string()),
        }
    );
    assert_eq!(
        http.requested_urls(),
        vec![BY_REPO_URL.to_string(), GLOBAL_URL.to_string()]
    );
}

#[tokio::test]
async fn list_requests_exact_repository_and_global_endpoints_and_merges_results() {
    let http = FakeHttp::new(HashMap::from([
        (
            BY_REPO_URL.to_string(),
            json_response(r#"[{"id":"env-repo","label":"Repository"}]"#),
        ),
        (
            GLOBAL_URL.to_string(),
            json_response(
                r#"[{"id":"env-repo","is_pinned":true},{"id":"env-global","label":"Global"}]"#,
            ),
        ),
    ]));

    let rows = list_environments_with_origins(
        &http,
        BASE_URL,
        &HeaderMap::new(),
        &["https://github.com/openai/codex.git".to_string()],
    )
    .await
    .expect("environment list should decode");

    assert_eq!(
        rows.into_iter()
            .map(|row| (row.id, row.label, row.is_pinned, row.repo_hints))
            .collect::<Vec<_>>(),
        vec![
            (
                "env-repo".to_string(),
                Some("Repository".to_string()),
                true,
                Some("openai/codex".to_string()),
            ),
            (
                "env-global".to_string(),
                Some("Global".to_string()),
                false,
                None,
            ),
        ]
    );
    assert_eq!(
        http.requested_urls(),
        vec![BY_REPO_URL.to_string(), GLOBAL_URL.to_string()]
    );
}

struct FakeHttp {
    responses: HashMap<String, EnvironmentResponse>,
    requests: Mutex<Vec<RecordedRequest>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RecordedRequest {
    url: String,
    headers: HeaderMap,
}

impl FakeHttp {
    fn new(responses: HashMap<String, EnvironmentResponse>) -> Self {
        Self {
            responses,
            requests: Mutex::new(Vec::new()),
        }
    }

    fn requests(&self) -> Vec<RecordedRequest> {
        self.requests.lock().expect("request lock").clone()
    }

    fn requested_urls(&self) -> Vec<String> {
        self.requests
            .lock()
            .expect("request lock")
            .iter()
            .map(|request| request.url.clone())
            .collect()
    }
}

impl EnvironmentHttp for FakeHttp {
    async fn get(&self, url: &str, headers: &HeaderMap) -> anyhow::Result<EnvironmentResponse> {
        self.requests
            .lock()
            .expect("request lock")
            .push(RecordedRequest {
                url: url.to_string(),
                headers: headers.clone(),
            });
        self.responses
            .get(url)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("unexpected URL: {url}"))
    }
}

fn json_response(body: &str) -> EnvironmentResponse {
    EnvironmentResponse {
        status: StatusCode::OK,
        content_type: "application/json".to_string(),
        body: body.to_string(),
    }
}
