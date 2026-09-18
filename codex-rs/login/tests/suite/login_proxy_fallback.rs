//! Browser callback coverage for one-time OAuth codes and system-proxy fallback.

use std::io::Read;
use std::net::TcpListener;
use std::process::Command;
use std::time::Duration;

use anyhow::Result;
use base64::Engine;
use codex_config::types::AuthCredentialsStoreMode;
use codex_http_client::HttpClientBuilder;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_http_client::cache_system_proxy_route_for_test;
use codex_login::AuthKeyringBackendKind;
use codex_login::AuthRouteConfig;
use codex_login::ServerOptions;
use codex_login::run_login_server;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use tempfile::tempdir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

const CHILD_CASE: &str = "CODEX_TOKEN_PROXY_FALLBACK_TEST_CASE";
const CHILD_PROXY: &str = "CODEX_TOKEN_PROXY_FALLBACK_TEST_PROXY";
const CHILD_ISSUER: &str = "CODEX_TOKEN_PROXY_FALLBACK_TEST_ISSUER";
const BLOCKED_ORIGIN: &str = "http://127.0.0.1:0";
const PROXY_ENV_KEYS: [&str; 8] = [
    "HTTP_PROXY",
    "http_proxy",
    "HTTPS_PROXY",
    "https_proxy",
    "ALL_PROXY",
    "all_proxy",
    "NO_PROXY",
    "no_proxy",
];

fn run_case(test_name: &str, issuer: &str, proxy: &str) -> Result<()> {
    let mut command = Command::new(std::env::current_exe()?);
    command.arg("--exact").arg(test_name);
    for key in PROXY_ENV_KEYS {
        command.env_remove(key);
    }
    command
        .env(CHILD_CASE, "1")
        .env(CHILD_PROXY, proxy)
        .env(CHILD_ISSUER, issuer);
    let output = command.output()?;
    assert!(
        output.status.success(),
        "{test_name} child failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

enum CallbackOutcome {
    Succeeded,
    Failed,
}

async fn exercise_callback(expected: CallbackOutcome) -> Result<()> {
    let issuer = std::env::var(CHILD_ISSUER)?;
    let proxy = std::env::var(CHILD_PROXY)?;
    cache_system_proxy_route_for_test(&format!("{issuer}/oauth/token"), proxy);

    let tmp = tempdir()?;
    let mut opts = ServerOptions::new(
        tmp.path().to_path_buf(),
        codex_login::CLIENT_ID.to_string(),
        /*forced_chatgpt_workspace_id*/ None,
        AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::Direct,
        AuthRouteConfig::from_http_client_factory(
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault)
                .with_system_proxy_fallback(),
        ),
    );
    opts.issuer = issuer;
    opts.port = 0;
    opts.open_browser = false;
    opts.force_state = Some("proxy-fallback".to_string());
    let server = run_login_server(opts)?;
    let callback = HttpClientBuilder::new()
        .without_redirects()
        .build_direct()?
        .get(format!(
            "http://127.0.0.1:{}/auth/callback?code=abc&state=proxy-fallback",
            server.actual_port
        ))
        .timeout(Duration::from_secs(20))
        .send()
        .await?;

    match expected {
        CallbackOutcome::Succeeded => {
            assert_eq!(callback.status(), 302);
            let success_url = callback.headers()["location"].to_str()?;
            HttpClientBuilder::new()
                .build_direct()?
                .get(success_url)
                .send()
                .await?
                .error_for_status()?;
            tokio::time::timeout(Duration::from_secs(20), server.block_until_done()).await??;
            let auth: serde_json::Value =
                serde_json::from_slice(&std::fs::read(tmp.path().join("auth.json"))?)?;
            assert_eq!(auth["tokens"]["access_token"], "redirect-access");
        }
        CallbackOutcome::Failed => {
            assert!(callback.text().await?.contains("Token exchange failed"));
            let error = tokio::time::timeout(Duration::from_secs(20), server.block_until_done())
                .await?
                .expect_err("the callback should fail after the token exchange fails");
            assert!(error.to_string().contains("Token exchange failed"));
            assert!(!tmp.path().join("auth.json").exists());
        }
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn token_exchange_http_responses_do_not_retry_one_time_code() -> Result<()> {
    skip_if_no_network!(Ok(()));
    if std::env::var_os(CHILD_CASE).is_some() {
        return exercise_callback(CallbackOutcome::Failed).await;
    }
    for response in [
        ResponseTemplate::new(400).set_body_string("invalid_grant"),
        ResponseTemplate::new(307).insert_header("Location", BLOCKED_ORIGIN),
    ] {
        let issuer = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(response)
            .expect(1)
            .mount(&issuer)
            .await;
        let proxy = MockServer::start().await;
        run_case(
            "suite::login_proxy_fallback::token_exchange_http_responses_do_not_retry_one_time_code",
            &issuer.uri(),
            &proxy.uri(),
        )?;
        issuer.verify().await;
        assert_eq!(proxy.received_requests().await.unwrap_or_default().len(), 0);
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn token_exchange_does_not_retry_after_post_is_sent() -> Result<()> {
    skip_if_no_network!(Ok(()));
    if std::env::var_os(CHILD_CASE).is_some() {
        return exercise_callback(CallbackOutcome::Failed).await;
    }

    let issuer = TcpListener::bind(("127.0.0.1", 0))?;
    let issuer_url = format!("http://{}", issuer.local_addr()?);
    let issuer_thread = std::thread::spawn(move || -> std::io::Result<String> {
        let (mut stream, _) = issuer.accept()?;
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        let mut request = [0_u8; 4096];
        let count = stream.read(&mut request)?;
        Ok(String::from_utf8_lossy(&request[..count]).into_owned())
    });
    let proxy = MockServer::start().await;
    run_case(
        "suite::login_proxy_fallback::token_exchange_does_not_retry_after_post_is_sent",
        &issuer_url,
        &proxy.uri(),
    )?;
    let request = issuer_thread.join().expect("issuer thread should finish")?;
    assert!(request.starts_with("POST /oauth/token HTTP/1.1"));
    assert_eq!(proxy.received_requests().await.unwrap_or_default().len(), 0);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn token_exchange_preserves_redirects() -> Result<()> {
    skip_if_no_network!(Ok(()));
    if std::env::var_os(CHILD_CASE).is_some() {
        return exercise_callback(CallbackOutcome::Succeeded).await;
    }
    for (status, redirected_method) in [(302, "GET"), (307, "POST")] {
        let issuer = MockServer::start().await;
        let target = MockServer::start().await;
        let proxy = MockServer::start().await;
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(serde_json::to_vec(
            &serde_json::json!({
                "email": "proxy@example.com",
                "https://api.openai.com/auth": {"chatgpt_account_id": "proxy-workspace"}
            }),
        )?);
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .respond_with(ResponseTemplate::new(status).insert_header("Location", target.uri()))
            .expect(2)
            .mount(&issuer)
            .await;
        Mock::given(method(redirected_method))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id_token": format!("e30.{payload}.sig"),
                "access_token": "redirect-access",
                "refresh_token": "redirect-refresh",
            })))
            .expect(2)
            .mount(&target)
            .await;
        run_case(
            "suite::login_proxy_fallback::token_exchange_preserves_redirects",
            &issuer.uri(),
            &proxy.uri(),
        )?;
        issuer.verify().await;
        target.verify().await;
        assert_eq!(proxy.received_requests().await.unwrap_or_default().len(), 0);
    }
    Ok(())
}
