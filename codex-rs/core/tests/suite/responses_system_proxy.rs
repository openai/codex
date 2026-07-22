use anyhow::Result;
use codex_features::Feature;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::process::Command;
use wiremock::MockServer;

const SYSTEM_PROXY_TEST_SUBPROCESS_ENV_VAR: &str = "CODEX_SYSTEM_PROXY_TEST_SUBPROCESS";
const TEST_NAME: &str =
    "suite::responses_system_proxy::regular_responses_turn_honors_respect_system_proxy";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn regular_responses_turn_honors_respect_system_proxy() -> Result<()> {
    skip_if_no_network!(Ok(()));

    if std::env::var_os(SYSTEM_PROXY_TEST_SUBPROCESS_ENV_VAR).is_none() {
        let proxy = TcpListener::bind("127.0.0.1:0").await?;
        let proxy_url = format!("http://{}", proxy.local_addr()?);
        let response_body = sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]);
        let proxy_task = tokio::spawn(async move {
            let (mut stream, _) = proxy.accept().await?;
            let mut request = Vec::new();
            let mut chunk = [0_u8; 1024];
            let header_end = loop {
                let bytes_read = stream.read(&mut chunk).await?;
                if bytes_read == 0 {
                    anyhow::bail!("proxy client closed before sending request headers");
                }
                request.extend_from_slice(&chunk[..bytes_read]);
                if request.len() > 64 * 1024 {
                    anyhow::bail!("proxy request headers exceeded 64 KiB");
                }
                if let Some(header_end) =
                    request.windows(4).position(|window| window == b"\r\n\r\n")
                {
                    break header_end + 4;
                }
            };
            let headers = String::from_utf8_lossy(&request[..header_end]).into_owned();
            let content_length = headers
                .lines()
                .filter_map(|line| line.split_once(':'))
                .find_map(|(name, value)| {
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or_default();
            while request.len() < header_end + content_length {
                let bytes_read = stream.read(&mut chunk).await?;
                if bytes_read == 0 {
                    anyhow::bail!("proxy client closed before sending the request body");
                }
                request.extend_from_slice(&chunk[..bytes_read]);
            }

            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{response_body}",
                response_body.len()
            );
            stream.write_all(response.as_bytes()).await?;
            stream.shutdown().await?;
            Ok::<_, anyhow::Error>(headers.lines().next().unwrap_or_default().to_string())
        });

        let mut command = Command::new(std::env::current_exe()?);
        command.arg("--exact").arg(TEST_NAME);
        for &key in codex_network_proxy::PROXY_ENV_KEYS {
            command.env_remove(key);
        }
        // Keep the test harness's loopback HTTP and WebSocket traffic out of the proxy. The fake
        // API origin is not covered, so the inference request must still traverse the proxy.
        command
            .env(SYSTEM_PROXY_TEST_SUBPROCESS_ENV_VAR, "1")
            .env("HTTP_PROXY", &proxy_url)
            .env("http_proxy", proxy_url)
            .env("NO_PROXY", codex_network_proxy::DEFAULT_NO_PROXY_VALUE)
            .env("no_proxy", codex_network_proxy::DEFAULT_NO_PROXY_VALUE);

        let output = command.output().await?;
        if !output.status.success() {
            proxy_task.abort();
        }
        assert!(
            output.status.success(),
            "subprocess test `{TEST_NAME}` failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        let proxy_request = tokio::time::timeout(Duration::from_secs(1), proxy_task).await???;
        assert_eq!(
            proxy_request,
            "POST http://responses-proxy.invalid/v1/responses HTTP/1.1"
        );
        return Ok(());
    }

    let server = MockServer::start().await;
    let mut builder = test_codex().with_config(|config| {
        config.model_provider.base_url = Some("http://responses-proxy.invalid/v1".to_string());
        config
            .features
            .enable(Feature::RespectSystemProxy)
            .expect("test config should allow feature update");
        config.respect_system_proxy = true;
    });
    let test = builder.build_with_auto_env(&server).await?;

    test.submit_turn("hello through the system proxy").await?;

    Ok(())
}
