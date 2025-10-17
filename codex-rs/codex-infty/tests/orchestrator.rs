#![cfg(not(target_os = "windows"))]

use std::time::Duration;

use codex_core::CodexAuth;
use codex_core::built_in_model_providers;
use codex_core::config::Config;
use codex_core::cross_session::AssistantMessage;
use codex_core::cross_session::PostUserTurnRequest;
use codex_core::cross_session::RoleOrId;
use codex_core::protocol::Op;
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
async fn orchestrator_routes_between_roles_and_records_store() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let bodies = vec![
        responses::sse(vec![
            responses::ev_response_created("solver-resp-1"),
            responses::ev_assistant_message("solver-msg-1", "Need direction"),
            responses::ev_completed("solver-resp-1"),
        ]),
        responses::sse(vec![
            responses::ev_response_created("director-resp-1"),
            responses::ev_assistant_message("director-msg-1", "Proceed iteratively"),
            responses::ev_completed("director-resp-1"),
        ]),
        responses::sse(vec![
            responses::ev_response_created("solver-resp-2"),
            responses::ev_assistant_message("solver-msg-2", "Acknowledged"),
            responses::ev_completed("solver-resp-2"),
        ]),
    ];
    let response_mock = responses::mount_sse_sequence(&server, bodies).await;

    let runs_root = TempDir::new()?;
    let orchestrator =
        InftyOrchestrator::with_runs_root(CodexAuth::from_api_key("dummy-key"), runs_root.path());
    let run_id = "run-orchestrator".to_string();

    let solver_config = build_config(&server).await?;
    let director_config = build_config(&server).await?;

    let sessions = orchestrator
        .spawn_run(RunParams {
            run_id: run_id.clone(),
            run_root: Some(runs_root.path().join("runs").join(&run_id)),
            solver: RoleConfig::new("solver", solver_config.clone()),
            director: RoleConfig::new("director", director_config.clone()),
            verifiers: Vec::new(),
        })
        .await?;

    let solver_message = call_role(
        &orchestrator,
        &sessions.run_id,
        "solver",
        "kick off plan",
        Duration::from_secs(1),
    )
    .await?;
    assert_eq!(solver_message.message.message, "Need direction");

    let director_message = relay_assistant_to_role(
        &orchestrator,
        &sessions.run_id,
        "director",
        &solver_message,
        Duration::from_secs(1),
    )
    .await?;
    assert_eq!(director_message.message.message, "Proceed iteratively");

    let solver_reply = relay_assistant_to_role(
        &orchestrator,
        &sessions.run_id,
        "solver",
        &director_message,
        Duration::from_secs(1),
    )
    .await?;
    assert_eq!(solver_reply.message.message, "Acknowledged");

    assert_eq!(response_mock.requests().len(), 3);
    let first_request = response_mock.requests().first().unwrap().body_json();
    let instructions = first_request["instructions"]
        .as_str()
        .expect("request should set instructions");
    assert!(
        instructions.contains("brilliant mathematician"),
        "missing solver prompt: {instructions}"
    );
    assert!(sessions.store.path().is_dir());
    let solver_meta = sessions.store.role_metadata("solver").unwrap();
    assert!(solver_meta.rollout_path.is_some());

    Ok(())
}

// resumable runs are disabled; resume test removed

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn execute_new_run_drives_to_completion() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let bodies = vec![
        responses::sse(vec![
            responses::ev_response_created("solver-resp-1"),
            responses::ev_assistant_message(
                "solver-msg-1",
                r#"{"type":"direction_request","prompt":"Need directive","claim_path":null,"notes":null,"deliverable_path":null,"summary":null}"#,
            ),
            responses::ev_completed("solver-resp-1"),
        ]),
        responses::sse(vec![
            responses::ev_response_created("director-resp-1"),
            responses::ev_assistant_message(
                "director-msg-1",
                r#"{"directive":"Proceed","rationale":"Follow the plan"}"#,
            ),
            responses::ev_completed("director-resp-1"),
        ]),
        responses::sse(vec![
            responses::ev_response_created("solver-resp-2"),
            responses::ev_assistant_message("solver-msg-2", "Acknowledged"),
            responses::ev_assistant_message(
                "solver-msg-4",
                r#"{"type":"final_delivery","prompt":null,"claim_path":null,"notes":null,"deliverable_path":"deliverable","summary":"done"}"#,
            ),
            responses::ev_completed("solver-resp-2"),
        ]),
        // Final verification of the deliverable
        responses::sse(vec![
            responses::ev_response_created("verifier-resp-3"),
            responses::ev_assistant_message(
                "verifier-msg-3",
                r#"{"verdict":"pass","reasons":[],"suggestions":[]}"#,
            ),
            responses::ev_completed("verifier-resp-3"),
        ]),
        // Feedback turn summarizing the verification outcome back to the solver
        responses::sse(vec![
            responses::ev_response_created("solver-resp-5"),
            responses::ev_completed("solver-resp-5"),
        ]),
    ];
    for body in bodies {
        responses::mount_sse_once(&server, body).await;
    }

    let runs_root = TempDir::new()?;
    let orchestrator =
        InftyOrchestrator::with_runs_root(CodexAuth::from_api_key("dummy-key"), runs_root.path());
    let run_id = "run-auto".to_string();
    let run_root = runs_root.path().join("runs").join(&run_id);

    let solver_config = build_config(&server).await?;
    let director_config = build_config(&server).await?;
    let verifier_config = build_config(&server).await?;

    let options = RunExecutionOptions {
        objective: Some("Implement feature".to_string()),
        ..RunExecutionOptions::default()
    };

    let outcome = orchestrator
        .execute_new_run(
            RunParams {
                run_id: run_id.clone(),
                run_root: Some(run_root.clone()),
                solver: RoleConfig::new("solver", solver_config),
                director: RoleConfig::new("director", director_config),
                verifiers: vec![RoleConfig::new("verifier", verifier_config)],
            },
            options,
        )
        .await?;

    assert_eq!(outcome.run_id, run_id);
    assert_eq!(outcome.summary.as_deref(), Some("done"));
    assert!(outcome.raw_message.contains("final_delivery"));
    let canonical_run_root = std::fs::canonicalize(&run_root)?;
    let canonical_deliverable = std::fs::canonicalize(&outcome.deliverable_path)?;
    assert!(canonical_deliverable.starts_with(&canonical_run_root));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spawn_run_cleans_up_on_failure() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let bodies = vec![
        responses::sse(vec![
            responses::ev_response_created("solver-resp-1"),
            responses::ev_completed("solver-resp-1"),
        ]),
        responses::sse(vec![
            responses::ev_response_created("director-resp-1"),
            responses::ev_completed("director-resp-1"),
        ]),
        responses::sse(vec![
            responses::ev_response_created("dup-resp"),
            responses::ev_completed("dup-resp"),
        ]),
    ];
    for body in bodies {
        responses::mount_sse_once(&server, body).await;
    }

    let runs_root = TempDir::new()?;
    let orchestrator =
        InftyOrchestrator::with_runs_root(CodexAuth::from_api_key("dummy-key"), runs_root.path());
    let run_id = "run-cleanup".to_string();
    let run_path = runs_root.path().join("runs").join(&run_id);

    let solver_config = build_config(&server).await?;
    let director_config = build_config(&server).await?;

    let result = orchestrator
        .spawn_run(RunParams {
            run_id: run_id.clone(),
            run_root: Some(run_path.clone()),
            solver: RoleConfig::new("solver", solver_config.clone()),
            director: RoleConfig::new("director", director_config.clone()),
            verifiers: vec![RoleConfig::new("solver", solver_config.clone())],
        })
        .await;
    assert!(result.is_err());
    assert!(!run_path.exists(), "failed run should remove run directory");

    let bodies = vec![
        responses::sse(vec![
            responses::ev_response_created("solver-resp-2"),
            responses::ev_completed("solver-resp-2"),
        ]),
        responses::sse(vec![
            responses::ev_response_created("director-resp-2"),
            responses::ev_completed("director-resp-2"),
        ]),
    ];
    for body in bodies {
        responses::mount_sse_once(&server, body).await;
    }

    let sessions = orchestrator
        .spawn_run(RunParams {
            run_id: run_id.clone(),
            run_root: Some(run_path.clone()),
            solver: RoleConfig::new("solver", solver_config),
            director: RoleConfig::new("director", director_config),
            verifiers: Vec::new(),
        })
        .await?;

    sessions.solver.conversation.submit(Op::Shutdown).await.ok();
    sessions
        .director
        .conversation
        .submit(Op::Shutdown)
        .await
        .ok();

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

async fn call_role(
    orchestrator: &InftyOrchestrator,
    run_id: &str,
    role: &str,
    text: &str,
    timeout: Duration,
) -> anyhow::Result<AssistantMessage> {
    let hub = orchestrator.hub();
    let handle = hub
        .post_user_turn(PostUserTurnRequest {
            target: RoleOrId::RunRole {
                run_id: run_id.to_string(),
                role: role.to_string(),
            },
            text: text.to_string(),
            final_output_json_schema: None,
        })
        .await?;
    let reply = hub.await_first_assistant(&handle, timeout).await?;
    Ok(reply)
}

async fn relay_assistant_to_role(
    orchestrator: &InftyOrchestrator,
    run_id: &str,
    target_role: &str,
    assistant: &AssistantMessage,
    timeout: Duration,
) -> anyhow::Result<AssistantMessage> {
    call_role(
        orchestrator,
        run_id,
        target_role,
        &assistant.message.message,
        timeout,
    )
    .await
}
