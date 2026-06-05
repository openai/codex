mod streamable_http_test_support;

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::Environment;
use codex_exec_server::ExecServerError;
use codex_exec_server::HttpClient;
use codex_exec_server::HttpRequestParams;
use codex_exec_server::HttpRequestResponse;
use codex_exec_server::HttpResponseBodyStream;
use codex_rmcp_client::ElicitationAction;
use codex_rmcp_client::ElicitationResponse;
use codex_rmcp_client::RmcpClient;
use futures::FutureExt as _;
use futures::future::BoxFuture;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

use streamable_http_test_support::arm_initialize_failure;
use streamable_http_test_support::arm_session_post_failure;
use streamable_http_test_support::call_echo_tool;
use streamable_http_test_support::create_client;
use streamable_http_test_support::create_client_with_http_client;
use streamable_http_test_support::expected_echo_result;
use streamable_http_test_support::init_params;
use streamable_http_test_support::spawn_streamable_http_server;

const JSON_RPC_INTERNAL_ERROR_CODE: i64 = -32603;
const SIMULATED_NO_RESPONSE_MESSAGE: &str =
    "http/request failed: error sending request for url (simulated no response)";

#[derive(Clone, Copy)]
enum RequestFailure {
    LocalHttpRequest,
    RemoteServer,
}

#[derive(Clone)]
struct FailFirstMethodHttpClient {
    inner: Arc<dyn HttpClient>,
    method: &'static str,
    failure: RequestFailure,
    failures_remaining: Arc<AtomicUsize>,
    matching_post_attempts: Arc<AtomicUsize>,
}

impl FailFirstMethodHttpClient {
    fn new(inner: Arc<dyn HttpClient>, method: &'static str, failure: RequestFailure) -> Self {
        Self {
            inner,
            method,
            failure,
            failures_remaining: Arc::new(AtomicUsize::new(1)),
            matching_post_attempts: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn matching_post_attempts(&self) -> usize {
        self.matching_post_attempts.load(Ordering::SeqCst)
    }
}

impl HttpClient for FailFirstMethodHttpClient {
    fn http_request(
        &self,
        params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<HttpRequestResponse, ExecServerError>> {
        self.inner.http_request(params)
    }

    fn http_request_stream(
        &self,
        params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<(HttpRequestResponse, HttpResponseBodyStream), ExecServerError>> {
        let inner = Arc::clone(&self.inner);
        let method = self.method;
        let failure = self.failure;
        let failures_remaining = Arc::clone(&self.failures_remaining);
        let matching_post_attempts = Arc::clone(&self.matching_post_attempts);

        async move {
            if is_json_rpc_method(&params, method) {
                matching_post_attempts.fetch_add(1, Ordering::SeqCst);
                if failures_remaining
                    .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |remaining| {
                        remaining.checked_sub(1)
                    })
                    .is_ok()
                {
                    return Err(match failure {
                        RequestFailure::LocalHttpRequest => {
                            ExecServerError::HttpRequest(SIMULATED_NO_RESPONSE_MESSAGE.to_string())
                        }
                        RequestFailure::RemoteServer => ExecServerError::Server {
                            code: JSON_RPC_INTERNAL_ERROR_CODE,
                            message: SIMULATED_NO_RESPONSE_MESSAGE.to_string(),
                        },
                    });
                }
            }

            inner.http_request_stream(params).await
        }
        .boxed()
    }
}

fn is_json_rpc_method(params: &HttpRequestParams, method: &str) -> bool {
    if !params.method.eq_ignore_ascii_case("POST") {
        return false;
    }

    params
        .body
        .as_ref()
        .and_then(|body| serde_json::from_slice::<Value>(&body.0).ok())
        .and_then(|body| {
            body.get("method")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .is_some_and(|request_method| request_method == method)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn streamable_http_initialize_retries_retryable_status() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server().await?;

    arm_initialize_failure(&base_url, /*status*/ 503, /*remaining*/ 1).await?;

    let client = create_client(&base_url).await?;
    let result = call_echo_tool(&client, "after-init-retry").await?;
    assert_eq!(result, expected_echo_result("after-init-retry"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn streamable_http_initialize_retry_sleep_respects_startup_timeout() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server().await?;
    arm_initialize_failure(&base_url, /*status*/ 503, /*remaining*/ 1).await?;

    let client = RmcpClient::new_streamable_http_client(
        "test-streamable-http",
        &format!("{base_url}/mcp"),
        Some("test-bearer".to_string()),
        /*http_headers*/ None,
        /*env_http_headers*/ None,
        OAuthCredentialsStoreMode::File,
        Environment::default_for_tests().get_http_client(),
        /*auth_provider*/ None,
    )
    .await?;

    let started = Instant::now();
    let error = client
        .initialize(
            init_params(),
            Some(Duration::from_millis(100)),
            Box::new(|_, _| {
                async {
                    Ok(ElicitationResponse {
                        action: ElicitationAction::Accept,
                        content: Some(json!({})),
                        meta: None,
                    })
                }
                .boxed()
            }),
        )
        .await
        .unwrap_err();

    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_millis(500),
        "initialize retry exceeded startup timeout budget: {elapsed:?}"
    );
    assert!(
        error
            .to_string()
            .contains("timed out handshaking with MCP server"),
        "expected handshake timeout, got: {error:#}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn streamable_http_initialize_retries_http_request_error() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server().await?;
    let http_client = FailFirstMethodHttpClient::new(
        Environment::default_for_tests().get_http_client(),
        "initialize",
        RequestFailure::LocalHttpRequest,
    );

    let client = create_client_with_http_client(&base_url, Arc::new(http_client.clone())).await?;
    let result = call_echo_tool(&client, "after-no-response-retry").await?;

    assert_eq!(http_client.matching_post_attempts(), 2);
    assert_eq!(result, expected_echo_result("after-no-response-retry"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn streamable_http_initialize_retries_remote_http_request_error() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server().await?;
    let http_client = FailFirstMethodHttpClient::new(
        Environment::default_for_tests().get_http_client(),
        "initialize",
        RequestFailure::RemoteServer,
    );

    let client = create_client_with_http_client(&base_url, Arc::new(http_client.clone())).await?;
    let result = call_echo_tool(&client, "after-remote-no-response-retry").await?;

    assert_eq!(http_client.matching_post_attempts(), 2);
    assert_eq!(
        result,
        expected_echo_result("after-remote-no-response-retry")
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn streamable_http_tools_list_retries_retryable_status() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server().await?;
    let client = create_client(&base_url).await?;
    let expected_tools = client
        .list_tools(/*params*/ None, Some(Duration::from_secs(5)))
        .await?;

    arm_session_post_failure(
        &base_url,
        /*status*/ 503,
        /*remaining*/ 1,
        /*www_authenticate_headers*/ &[],
    )
    .await?;

    let tools = client
        .list_tools(/*params*/ None, Some(Duration::from_secs(5)))
        .await?;

    assert_eq!(tools, expected_tools);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn streamable_http_tools_list_retry_sleep_respects_operation_timeout() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server().await?;
    let client = create_client(&base_url).await?;

    arm_session_post_failure(
        &base_url,
        /*status*/ 503,
        /*remaining*/ 1,
        /*www_authenticate_headers*/ &[],
    )
    .await?;

    let started = Instant::now();
    let error = client
        .list_tools(/*params*/ None, Some(Duration::from_millis(100)))
        .await
        .unwrap_err();

    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_millis(500),
        "tools/list retry exceeded operation timeout budget: {elapsed:?}"
    );
    assert!(
        error.to_string().contains("timed out awaiting tools/list"),
        "expected tools/list timeout, got: {error:#}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn streamable_http_tools_list_retries_http_request_error() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server().await?;
    let baseline_client = create_client(&base_url).await?;
    let expected_tools = baseline_client
        .list_tools(/*params*/ None, Some(Duration::from_secs(5)))
        .await?;
    let http_client = FailFirstMethodHttpClient::new(
        Environment::default_for_tests().get_http_client(),
        "tools/list",
        RequestFailure::LocalHttpRequest,
    );
    let client = create_client_with_http_client(&base_url, Arc::new(http_client.clone())).await?;

    let tools = client
        .list_tools(/*params*/ None, Some(Duration::from_secs(5)))
        .await?;

    assert_eq!(http_client.matching_post_attempts(), 2);
    assert_eq!(tools, expected_tools);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn streamable_http_tools_list_retries_remote_http_request_error() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server().await?;
    let baseline_client = create_client(&base_url).await?;
    let expected_tools = baseline_client
        .list_tools(/*params*/ None, Some(Duration::from_secs(5)))
        .await?;
    let http_client = FailFirstMethodHttpClient::new(
        Environment::default_for_tests().get_http_client(),
        "tools/list",
        RequestFailure::RemoteServer,
    );
    let client = create_client_with_http_client(&base_url, Arc::new(http_client.clone())).await?;

    let tools = client
        .list_tools(/*params*/ None, Some(Duration::from_secs(5)))
        .await?;

    assert_eq!(http_client.matching_post_attempts(), 2);
    assert_eq!(tools, expected_tools);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn streamable_http_initialize_does_not_retry_non_retryable_status() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server().await?;

    arm_initialize_failure(&base_url, /*status*/ 403, /*remaining*/ 1).await?;

    let error = match create_client(&base_url).await {
        Ok(_) => panic!("initialize unexpectedly succeeded after non-retryable HTTP 403"),
        Err(error) => error,
    };
    assert!(format!("{error:#}").contains("403"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn streamable_http_404_session_expiry_recovers_and_retries_once() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server().await?;
    let client = create_client(&base_url).await?;

    let warmup = call_echo_tool(&client, "warmup").await?;
    assert_eq!(warmup, expected_echo_result("warmup"));

    arm_session_post_failure(
        &base_url,
        /*status*/ 404,
        /*remaining*/ 1,
        /*www_authenticate_headers*/ &[],
    )
    .await?;

    let recovered = call_echo_tool(&client, "recovered").await?;
    assert_eq!(recovered, expected_echo_result("recovered"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn streamable_http_401_does_not_trigger_recovery() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server().await?;
    let client = create_client(&base_url).await?;

    let warmup = call_echo_tool(&client, "warmup").await?;
    assert_eq!(warmup, expected_echo_result("warmup"));

    arm_session_post_failure(
        &base_url,
        /*status*/ 401,
        /*remaining*/ 2,
        /*www_authenticate_headers*/ &[],
    )
    .await?;

    let first_error = call_echo_tool(&client, "unauthorized").await.unwrap_err();
    assert!(first_error.to_string().contains("401"));

    let second_error = call_echo_tool(&client, "still-unauthorized")
        .await
        .unwrap_err();
    assert!(second_error.to_string().contains("401"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn streamable_http_403_scope_challenge_returns_insufficient_scope() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server().await?;
    let client = create_client(&base_url).await?;

    let warmup = call_echo_tool(&client, "warmup").await?;
    assert_eq!(warmup, expected_echo_result("warmup"));

    arm_session_post_failure(
        &base_url,
        /*status*/ 403,
        /*remaining*/ 1,
        /*www_authenticate_headers*/
        &[r#"Bearer error="insufficient_scope", scope="files:read files:write""#],
    )
    .await?;

    let error = call_echo_tool(&client, "forbidden").await.unwrap_err();
    assert!(
        error.to_string().contains("Insufficient scope"),
        "expected insufficient-scope transport error, got: {error:#}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn streamable_http_403_finds_bearer_challenge_in_later_header_value() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server().await?;
    let client = create_client(&base_url).await?;

    let warmup = call_echo_tool(&client, "warmup").await?;
    assert_eq!(warmup, expected_echo_result("warmup"));

    arm_session_post_failure(
        &base_url,
        /*status*/ 403,
        /*remaining*/ 1,
        /*www_authenticate_headers*/
        &[
            r#"Basic realm="example""#,
            r#"Bearer error="insufficient_scope", scope="files:read""#,
        ],
    )
    .await?;

    let error = call_echo_tool(&client, "forbidden").await.unwrap_err();
    assert!(
        error.to_string().contains("Insufficient scope"),
        "expected insufficient-scope transport error, got: {error:#}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn streamable_http_404_recovery_only_retries_once() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server().await?;
    let client = create_client(&base_url).await?;

    let warmup = call_echo_tool(&client, "warmup").await?;
    assert_eq!(warmup, expected_echo_result("warmup"));

    arm_session_post_failure(
        &base_url,
        /*status*/ 404,
        /*remaining*/ 2,
        /*www_authenticate_headers*/ &[],
    )
    .await?;

    let error = call_echo_tool(&client, "double-404").await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("handshaking with MCP server failed")
            || error.to_string().contains("Transport channel closed")
    );

    let recovered = call_echo_tool(&client, "after-double-404").await?;
    assert_eq!(recovered, expected_echo_result("after-double-404"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn streamable_http_non_session_failure_does_not_trigger_recovery() -> anyhow::Result<()> {
    let (_server, base_url) = spawn_streamable_http_server().await?;
    let client = create_client(&base_url).await?;

    let warmup = call_echo_tool(&client, "warmup").await?;
    assert_eq!(warmup, expected_echo_result("warmup"));

    arm_session_post_failure(
        &base_url,
        /*status*/ 500,
        /*remaining*/ 2,
        /*www_authenticate_headers*/ &[],
    )
    .await?;

    let first_error = call_echo_tool(&client, "server-error").await.unwrap_err();
    assert!(first_error.to_string().contains("500"));

    let second_error = call_echo_tool(&client, "still-server-error")
        .await
        .unwrap_err();
    assert!(second_error.to_string().contains("500"));

    Ok(())
}
