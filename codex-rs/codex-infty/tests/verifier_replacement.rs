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
async fn replaces_passing_verifiers_and_keeps_failing() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;

    // Round 1: alpha passes, beta fails
    let body_verifier_alpha_r1 = responses::sse(vec![
        responses::ev_response_created("verifier-alpha-r1"),
        responses::ev_assistant_message(
            "verifier-alpha-msg-r1",
            r#"{"verdict":"pass","reasons":[],"suggestions":[]}"#,
        ),
        responses::ev_completed("verifier-alpha-r1"),
    ]);
    let body_verifier_beta_r1 = responses::sse(vec![
        responses::ev_response_created("verifier-beta-r1"),
        responses::ev_assistant_message(
            "verifier-beta-msg-r1",
            r#"{"verdict":"fail","reasons":["missing"],"suggestions":[]}"#,
        ),
        responses::ev_completed("verifier-beta-r1"),
    ]);

    // Round 2: both pass
    let body_verifier_alpha_r2 = responses::sse(vec![
        responses::ev_response_created("verifier-alpha-r2"),
        responses::ev_assistant_message(
            "verifier-alpha-msg-r2",
            r#"{"verdict":"pass","reasons":[],"suggestions":[]}"#,
        ),
        responses::ev_completed("verifier-alpha-r2"),
    ]);
    let body_verifier_beta_r2 = responses::sse(vec![
        responses::ev_response_created("verifier-beta-r2"),
        responses::ev_assistant_message(
            "verifier-beta-msg-r2",
            r#"{"verdict":"pass","reasons":[],"suggestions":[]}"#,
        ),
        responses::ev_completed("verifier-beta-r2"),
    ]);

    // Mount verifier SSE bodies in the exact order collect_verification_summary posts to verifiers.
    // The implementation posts sequentially in the order of sessions.verifiers.
    let _m1 = responses::mount_sse_once(&server, body_verifier_alpha_r1).await;
    let _m2 = responses::mount_sse_once(&server, body_verifier_beta_r1).await;
    let _m3 = responses::mount_sse_once(&server, body_verifier_alpha_r2).await;
    let _m4 = responses::mount_sse_once(&server, body_verifier_beta_r2).await;

    let runs_root = TempDir::new()?;
    let orchestrator =
        InftyOrchestrator::with_runs_root(CodexAuth::from_api_key("dummy-key"), runs_root.path());
    let run_id = "run-verifier-replacement".to_string();

    let solver_config = build_config(&server).await?;
    let director_config = build_config(&server).await?;
    let verifier_config = build_config(&server).await?;

    // Spawn run with two verifiers in known order.
    let mut sessions = orchestrator
        .spawn_run(RunParams {
            run_id: run_id.clone(),
            run_root: Some(runs_root.path().join("runs").join(&run_id)),
            solver: RoleConfig::new("solver", solver_config),
            director: RoleConfig::new("director", director_config),
            verifiers: vec![
                RoleConfig::new("verifier-alpha", verifier_config.clone()),
                RoleConfig::new("verifier-beta", verifier_config),
            ],
        })
        .await?;

    let alpha_initial = sessions
        .store
        .role_metadata("verifier-alpha")
        .and_then(|m| m.rollout_path.clone())
        .expect("alpha initial rollout path");
    let beta_initial = sessions
        .store
        .role_metadata("verifier-beta")
        .and_then(|m| m.rollout_path.clone())
        .expect("beta initial rollout path");

    let options = RunExecutionOptions {
        verifier_timeout: Duration::from_secs(2),
        ..Default::default()
    };

    // Round 1: alpha pass (should be replaced), beta fail (should be kept)
    let _summary1 = orchestrator
        .verify_round_for_test(&mut sessions, "memory/claims/c1.json", &options)
        .await?;

    let alpha_after_r1 = sessions
        .store
        .role_metadata("verifier-alpha")
        .and_then(|m| m.rollout_path.clone())
        .expect("alpha rollout after r1");
    let beta_after_r1 = sessions
        .store
        .role_metadata("verifier-beta")
        .and_then(|m| m.rollout_path.clone())
        .expect("beta rollout after r1");

    assert_ne!(
        alpha_initial, alpha_after_r1,
        "alpha should be replaced after pass"
    );
    assert_eq!(
        beta_initial, beta_after_r1,
        "beta should be kept after fail"
    );

    // Round 2: both pass; beta should be replaced now.
    let _summary2 = orchestrator
        .verify_round_for_test(&mut sessions, "memory/claims/c2.json", &options)
        .await?;
    let beta_after_r2 = sessions
        .store
        .role_metadata("verifier-beta")
        .and_then(|m| m.rollout_path.clone())
        .expect("beta rollout after r2");
    assert_ne!(
        beta_initial, beta_after_r2,
        "beta should be replaced after pass in r2"
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
