//! Verifies that the effective system-proxy feature reaches both HTTP Responses API paths.
//!
//! The test uses two processes because proxy environment variables are process-global:
//!
//! - The parent owns a mock proxy and launches this test again with an isolated proxy environment.
//! - The child loads the feature through `config.toml` and targets an unreachable origin.
//!
//! The child also sets the CGI marker that makes reqwest ignore its implicit `HTTP_PROXY`
//! handling. Consequently, requests reach the parent only when Codex explicitly selects the
//! configured proxy through `HttpClientFactory`. Falling back to reqwest's default client in
//! either Responses path makes the child fail against the unreachable origin.

use anyhow::Result;
use codex_config::CONFIG_TOML_FILE;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_compact_json_once;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use std::process::Command;

const SUBPROCESS_ENV: &str = "CODEX_RESPONSES_SYSTEM_PROXY_TEST_CHILD";
const SUBPROCESS_SENTINEL: &str = "responses system proxy child ran";
const TEST_NAME: &str =
    "suite::responses_api_system_proxy::responses_and_compact_use_enabled_system_proxy";
const TARGET_BASE_URL: &str = "http://responses-api.invalid/v1";

#[cfg_attr(
    not(target_os = "linux"),
    ignore = "system proxy environment fallback is Linux-specific"
)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_and_compact_use_enabled_system_proxy() -> Result<()> {
    if std::env::var_os(SUBPROCESS_ENV).is_none() {
        return run_test_in_subprocess().await;
    }

    // Child process: exercise the real config -> session -> Responses transport path.
    eprintln!("{SUBPROCESS_SENTINEL}");
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex()
        .with_pre_build_hook(|codex_home| {
            std::fs::write(
                codex_home.join(CONFIG_TOML_FILE),
                "[features]\nrespect_system_proxy = true\n",
            )
            .expect("system-proxy feature config should be written");
        })
        .with_config(|config| {
            config.model_provider.base_url = Some(TARGET_BASE_URL.to_string());
        });
    let test = builder.build_with_auto_env(&server).await?;
    assert!(test.config.respect_system_proxy);

    test.submit_turn("exercise the Responses API proxy route")
        .await?;
    test.codex.submit(Op::Compact).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    Ok(())
}

async fn run_test_in_subprocess() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let proxy = start_mock_server().await;
    let responses_mock = mount_sse_once(
        &proxy,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let compact_mock = mount_compact_json_once(
        &proxy,
        serde_json::json!({
            "output": [ResponseItem::Compaction {
                id: None,
                encrypted_content: "encrypted compacted history".to_string(),
                internal_chat_message_metadata_passthrough: None,
            }]
        }),
    )
    .await;

    // Parent process: own the proxy while the child runs with isolated environment variables.
    let output = Command::new(std::env::current_exe()?)
        .args(["--exact", TEST_NAME, "--nocapture"])
        .env(SUBPROCESS_ENV, "1")
        .env("HTTP_PROXY", proxy.uri())
        // Reqwest treats `REQUEST_METHOD` as a CGI marker and ignores uppercase `HTTP_PROXY`.
        // Codex's explicit route resolver still reads it, which distinguishes the two clients.
        .env("REQUEST_METHOD", "GET")
        .env_remove("http_proxy")
        .env_remove("ALL_PROXY")
        .env_remove("all_proxy")
        .env_remove("NO_PROXY")
        .env_remove("no_proxy")
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "system-proxy subprocess failed\nstdout:\n{}\nstderr:\n{stderr}",
        String::from_utf8_lossy(&output.stdout),
    );
    assert!(
        stderr.contains(SUBPROCESS_SENTINEL),
        // `--exact` exits successfully even when it selects no tests. Require proof that the
        // child branch actually executed so a renamed test cannot produce a false positive.
        "system-proxy subprocess did not execute the expected test\nstderr:\n{stderr}"
    );

    assert_eq!(responses_mock.single_request().path(), "/v1/responses");
    assert_eq!(
        compact_mock.single_request().path(),
        "/v1/responses/compact"
    );
    Ok(())
}
