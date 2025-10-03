#![allow(clippy::unwrap_used, clippy::expect_used)]
use core_test_support::responses::ev_completed;
use core_test_support::responses::sse;
use core_test_support::responses::sse_response;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex_exec::test_codex_exec;
use wiremock::Mock;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_uses_codex_api_key_env_var() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let server = start_mock_server().await;

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header("Authorization", "Bearer dummy"))
        .respond_with(sse_response(sse(vec![ev_completed("request_0")])))
        .expect(1)
        .mount(&server)
        .await;

    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(env!("CARGO_MANIFEST_DIR"))
        .arg("echo testing codex api key")
        .assert()
        .success();

    Ok(())
}
