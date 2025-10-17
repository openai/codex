#![cfg(not(target_os = "windows"))]

use std::time::Duration;

use codex_core::CodexAuth;
use codex_core::built_in_model_providers;
use codex_core::config::Config;
use codex_infty::InftyOrchestrator;
use codex_infty::RoleConfig;
use codex_infty::RunExecutionOptions;
use codex_infty::RunParams;
use core_test_support::load_default_config_for_test;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use tempfile::TempDir;
use wiremock::MockServer;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn direction_request_times_out_when_director_is_silent() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;

    // Solver emits a direction_request.
    let body_solver = responses::sse(vec![
        responses::ev_response_created("solver-resp-1"),
        responses::ev_assistant_message(
            "solver-msg-1",
            r#"{"type":"direction_request","prompt":"Need directive","claim_path":null,"notes":null,"deliverable_path":null,"summary":null}"#,
        ),
        responses::ev_completed("solver-resp-1"),
    ]);
    let _mock_solver = responses::mount_sse_once(&server, body_solver).await;

    // Director remains silent (no assistant message); the model completes immediately.
    let body_director_silent = responses::sse(vec![
        responses::ev_response_created("director-resp-1"),
        // intentionally no message
        responses::ev_completed("director-resp-1"),
    ]);
    let _mock_director = responses::mount_sse_once(&server, body_director_silent).await;

    // After attempting to relay a directive back to the solver, orchestrator won't proceed
    // as we will time out waiting for the director; however, the solver will still receive
    // a follow-up post later in the flow, so we pre-mount an empty completion to satisfy it
    // if the code ever reaches that point in future changes.
    let body_solver_after = responses::sse(vec![
        responses::ev_response_created("solver-resp-2"),
        responses::ev_completed("solver-resp-2"),
    ]);
    let _mock_solver_after = responses::mount_sse_once(&server, body_solver_after).await;

    let runs_root = TempDir::new()?;
    let orchestrator =
        InftyOrchestrator::with_runs_root(CodexAuth::from_api_key("dummy-key"), runs_root.path());
    let run_id = "run-director-timeout".to_string();

    let solver_config = build_config(&server).await?;
    let director_config = build_config(&server).await?;

    let params = RunParams {
        run_id: run_id.clone(),
        run_root: Some(runs_root.path().join("runs").join(&run_id)),
        solver: RoleConfig::new("solver", solver_config),
        director: RoleConfig::new("director", director_config),
        verifiers: Vec::new(),
    };

    let options = RunExecutionOptions {
        objective: Some("Kick off".to_string()),
        director_timeout: Duration::from_millis(50),
        ..Default::default()
    };

    let err = orchestrator
        .execute_new_run(params, options)
        .await
        .err()
        .expect("expected timeout error");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("timed out waiting") || msg.contains("AwaitTimeout"),
        "unexpected error: {msg}"
    );

    Ok(())
}

async fn build_config(server: &MockServer) -> anyhow::Result<Config> {
    let home = TempDir::new()?;
    let cwd = TempDir::new()?;
    let mut config = load_default_config_for_test(&home);
    config.cwd = cwd.path().to_path_buf();
    let mut provider = built_in_model_providers()["openai"].clone();
    provider.base_url = Some(format!("{}/v1", server.uri()));
    config.model_provider = provider;
    Ok(config)
}
