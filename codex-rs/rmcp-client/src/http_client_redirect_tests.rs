use std::sync::Arc;
use std::sync::Mutex;

use codex_exec_server::Environment;
use codex_exec_server::HttpHeader;
use pretty_assertions::assert_eq;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::path;

use crate::http_headers::with_http_headers_helper;

use super::*;

const PROXY_HEADERS_HELPER: &str = if cfg!(windows) {
    r#"echo {"Proxy-Authorization":"Bearer proxy-token"}"#
} else {
    r#"printf '{"Proxy-Authorization":"Bearer proxy-token"}'"#
};

fn request(url: impl Into<String>) -> HttpRequestParams {
    HttpRequestParams {
        method: "POST".to_string(),
        url: url.into(),
        headers: Vec::new(),
        body: Some(b"sensitive-body".to_vec().into()),
        timeout_ms: Some(5_000),
        redirect_policy: HttpRedirectPolicy::Follow,
        request_id: "redirect-test".to_string(),
        stream_response: false,
    }
}

fn headers<const N: usize>(headers: [(&str, &str); N]) -> Vec<HttpHeader> {
    headers
        .into_iter()
        .map(|(name, value)| HttpHeader {
            name: name.to_string(),
            value: value.to_string(),
        })
        .collect()
}

#[derive(Default)]
struct RecordingRedirectHttpClient {
    requests: Mutex<Vec<HttpRequestParams>>,
    delay: Duration,
    loop_redirects: bool,
}

impl HttpClient for RecordingRedirectHttpClient {
    fn http_request(
        &self,
        params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<HttpRequestResponse, ExecServerError>> {
        let mut requests = self.requests.lock().expect("request recorder lock");
        let redirect = requests.is_empty() || self.loop_redirects;
        let delay = self.delay;
        requests.push(params);
        async move {
            if !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }
            Ok(HttpRequestResponse {
                status: if redirect { 307 } else { 200 },
                headers: if redirect {
                    headers([("location", "/final")])
                } else {
                    Vec::new()
                },
                body: Vec::new().into(),
            })
        }
        .boxed()
    }

    fn http_request_stream(
        &self,
        _params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<(HttpRequestResponse, HttpResponseBodyStream), ExecServerError>> {
        async {
            Err(ExecServerError::HttpRequest(
                "unexpected streaming request".to_string(),
            ))
        }
        .boxed()
    }
}

#[tokio::test]
async fn plaintext_hostname_redirects_are_rejected_before_dns_can_rebind() -> anyhow::Result<()> {
    for (url, expected_error) in [
        (
            "http://mcp.example/start",
            Some("non-loopback hostnames require HTTPS"),
        ),
        ("http://localhost/start", None),
        ("http://127.0.0.1/start", None),
        ("http://[::1]/start", None),
        ("https://mcp.example/start", None),
    ] {
        let recorder = Arc::new(RecordingRedirectHttpClient::default());
        let client = SameOriginRedirectHttpClient::new(recorder.clone());
        let response = client
            .http_request(HttpRequestParams {
                headers: headers([("authorization", "Bearer sensitive-token")]),
                ..request(url)
            })
            .await;

        if let Some(expected_error) = expected_error {
            let error = response.expect_err("plaintext hostname redirects must fail");
            assert!(error.to_string().contains(expected_error), "{error}");
        } else {
            assert_eq!(response?.status, 200);
        }
        let requests = recorder.requests.lock().expect("request recorder lock");
        assert_eq!(requests.len(), if expected_error.is_some() { 1 } else { 2 });
    }

    Ok(())
}

#[tokio::test]
async fn same_origin_redirects_enforce_shared_timeout_and_hop_limit() {
    for (delay, timeout_ms, expected_error, expected_requests) in [
        (Duration::from_millis(100), 175, "timed out", 2),
        (Duration::ZERO, 5_000, "redirect limit", MAX_REDIRECTS + 1),
    ] {
        let recorder = Arc::new(RecordingRedirectHttpClient {
            delay,
            loop_redirects: true,
            ..Default::default()
        });
        let client = SameOriginRedirectHttpClient::new(recorder.clone());
        let error = client
            .http_request(HttpRequestParams {
                method: "GET".to_string(),
                body: None,
                timeout_ms: Some(timeout_ms),
                ..request("https://mcp.example/loop")
            })
            .await
            .expect_err("redirect loops must respect both limits");

        assert!(error.to_string().contains(expected_error), "{error}");
        let requests = recorder.requests.lock().expect("request recorder lock");
        assert_eq!(requests.len(), expected_requests);
    }
}

#[tokio::test]
async fn same_origin_redirects_preserve_method_body_and_headers() -> anyhow::Result<()> {
    for (method, redirect_status, redirected_method) in [
        ("POST", 301, "GET"),
        ("POST", 302, "GET"),
        ("POST", 303, "GET"),
        ("HEAD", 303, "HEAD"),
        ("POST", 307, "POST"),
        ("GET", 307, "GET"),
        ("DELETE", 307, "DELETE"),
        ("POST", 308, "POST"),
    ] {
        let server = MockServer::start().await;
        Mock::given(path("/start"))
            .respond_with(
                ResponseTemplate::new(redirect_status).insert_header("location", "/final"),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(path("/final"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;

        let client =
            SameOriginRedirectHttpClient::new(Environment::default_for_tests().get_http_client());
        let expected_content_length = b"sensitive-body".len().to_string();
        let mut params = HttpRequestParams {
            method: method.to_string(),
            headers: headers([
                ("x-api-key", "sensitive-key"),
                ("proxy-authorization", "sensitive-proxy-credentials"),
                ("referer", "https://stale.example/"),
            ]),
            body: (method == "POST").then(|| b"sensitive-body".to_vec().into()),
            stream_response: method != "DELETE",
            ..request(format!("{}/start", server.uri()))
        };
        if method == "POST" {
            params.headers.extend(headers([
                ("content-type", "application/json"),
                ("content-encoding", "identity"),
                ("content-length", &expected_content_length),
            ]));
        }
        let status = if method == "DELETE" {
            client.http_request(params).await?.status
        } else {
            client.http_request_stream(params).await?.0.status
        };
        assert_eq!(status, 200);

        let requests = server.received_requests().await.expect("recorded requests");
        let redirected = &requests[1];
        let expected_referer = format!("{}/start", server.uri());
        let body_preserved = redirected_method == "POST";
        for (name, expected) in [
            ("x-api-key", Some("sensitive-key")),
            ("proxy-authorization", None),
            ("referer", Some(expected_referer.as_str())),
            ("content-type", body_preserved.then_some("application/json")),
            ("content-encoding", body_preserved.then_some("identity")),
            (
                "content-length",
                body_preserved.then_some(expected_content_length.as_str()),
            ),
        ] {
            assert_eq!(
                redirected
                    .headers
                    .get(name)
                    .and_then(|value| value.to_str().ok()),
                expected,
                "unexpected redirected {name} header"
            );
        }
        assert_eq!(
            (redirected.method.as_str(), redirected.body.as_slice()),
            (
                redirected_method,
                if body_preserved {
                    b"sensitive-body".as_slice()
                } else {
                    b"".as_slice()
                }
            )
        );
    }

    Ok(())
}

#[tokio::test]
async fn https_redirects_preserve_configured_and_helper_proxy_authorization() -> anyhow::Result<()>
{
    for helper_enabled in [false, true] {
        let recorder = Arc::new(RecordingRedirectHttpClient::default());
        let inner: Arc<dyn HttpClient> = recorder.clone();
        let directory = tempfile::tempdir()?;
        let url = "https://mcp.example/start";
        let inner = if helper_enabled {
            with_http_headers_helper(
                inner,
                url,
                PROXY_HEADERS_HELPER,
                directory.path().to_path_buf(),
            )?
        } else {
            inner
        };
        let client = SameOriginRedirectHttpClient::new(inner);
        let response = client
            .http_request(HttpRequestParams {
                headers: if helper_enabled {
                    Vec::new()
                } else {
                    headers([("Proxy-Authorization", "Bearer proxy-token")])
                },
                ..request(url)
            })
            .await?;

        let requests = recorder.requests.lock().expect("request recorder lock");
        assert_eq!(
            (
                response.status,
                requests.len(),
                requests[1].url.as_str(),
                requests[1]
                    .headers
                    .iter()
                    .find(|header| header.name.eq_ignore_ascii_case("proxy-authorization"))
                    .map(|header| header.value.as_str()),
            ),
            (
                200,
                2,
                "https://mcp.example/final",
                Some("Bearer proxy-token"),
            ),
        );
    }

    Ok(())
}

#[tokio::test]
async fn plaintext_helper_redirects_block_mcp_but_preserve_oauth_stop() -> anyhow::Result<()> {
    for (request_id, redirect_policy) in [
        ("mcp-request-1", HttpRedirectPolicy::Follow),
        ("oauth-request-1", HttpRedirectPolicy::Stop),
    ] {
        let server = MockServer::start().await;
        Mock::given(path("/start"))
            .respond_with(ResponseTemplate::new(307).insert_header("location", "/final"))
            .expect(1)
            .mount(&server)
            .await;

        let directory = tempfile::tempdir()?;
        let url = format!("{}/start", server.uri());
        let helper = with_http_headers_helper(
            Environment::default_for_tests().get_http_client(),
            &url,
            PROXY_HEADERS_HELPER,
            directory.path().to_path_buf(),
        )?;
        let client = SameOriginRedirectHttpClient::new(helper);
        let response = client
            .http_request_stream(HttpRequestParams {
                request_id: request_id.to_string(),
                redirect_policy,
                stream_response: true,
                ..request(url)
            })
            .await;

        match (redirect_policy, response) {
            (HttpRedirectPolicy::Stop, Ok((response, _))) => assert_eq!(response.status, 307),
            (HttpRedirectPolicy::Follow, Err(error)) => {
                assert!(error.to_string().contains("Proxy-Authorization"));
            }
            _ => panic!("unexpected plaintext proxy-credential redirect behavior"),
        }
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }
    Ok(())
}

#[tokio::test]
async fn cross_origin_redirects_never_reach_their_destination() -> anyhow::Result<()> {
    for status in [307, 308] {
        for method in ["POST", "GET", "DELETE"] {
            let destination = MockServer::start().await;
            let server = MockServer::start().await;
            Mock::given(path("/start"))
                .respond_with(
                    ResponseTemplate::new(status)
                        .insert_header("location", format!("{}/private", destination.uri())),
                )
                .expect(1)
                .mount(&server)
                .await;

            let client = SameOriginRedirectHttpClient::new(
                Environment::default_for_tests().get_http_client(),
            );
            let params = HttpRequestParams {
                method: method.to_string(),
                headers: headers([("x-api-key", "sensitive-key")]),
                body: (method == "POST").then(|| b"sensitive-body".to_vec().into()),
                stream_response: method != "DELETE",
                ..request(format!("{}/start", server.uri()))
            };
            let error = if method == "DELETE" {
                client
                    .http_request(params)
                    .await
                    .expect_err("cross-origin DELETE redirect must fail")
            } else {
                match client.http_request_stream(params).await {
                    Ok(_) => panic!("cross-origin {method} redirect must fail"),
                    Err(error) => error,
                }
            };

            assert!(
                error.to_string().contains("different origin"),
                "cross-origin {method} redirect must explain its rejection: {error}"
            );
            assert!(destination.received_requests().await.unwrap().is_empty());
        }
    }

    Ok(())
}
