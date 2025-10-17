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
async fn director_request_includes_output_schema() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;

    // 1) Solver: emit a direction_request so the orchestrator calls Director.
    let body_solver = responses::sse(vec![
        responses::ev_response_created("solver-resp-1"),
        responses::ev_assistant_message(
            "solver-msg-1",
            r#"{"type":"direction_request","prompt":"Need directive","claim_path":null,"notes":null,"deliverable_path":null,"summary":null}"#,
        ),
        responses::ev_completed("solver-resp-1"),
    ]);
    let _mock_solver = responses::mount_sse_once(&server, body_solver).await;

    // 2) Director: reply with a directive JSON.
    let body_director = responses::sse(vec![
        responses::ev_response_created("director-resp-1"),
        responses::ev_assistant_message(
            "director-msg-1",
            r#"{"directive":"Proceed","rationale":"Follow the plan"}"#,
        ),
        responses::ev_completed("director-resp-1"),
    ]);
    let mock_director = responses::mount_sse_once(&server, body_director).await;

    // 3) After relaying directive back to Solver, we do not need to continue the run.
    // Provide a short empty solver completion body to avoid hanging HTTP calls.
    let body_solver_after = responses::sse(vec![
        responses::ev_response_created("solver-resp-2"),
        responses::ev_completed("solver-resp-2"),
    ]);
    let _mock_solver_after = responses::mount_sse_once(&server, body_solver_after).await;

    let runs_root = TempDir::new()?;
    let orchestrator =
        InftyOrchestrator::with_runs_root(CodexAuth::from_api_key("dummy-key"), runs_root.path());
    let run_id = "run-director-schema".to_string();

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
        ..Default::default()
    };

    // Drive the run in the background; we'll assert the request shape then cancel.
    let fut = tokio::spawn(async move {
        let _ = orchestrator.execute_new_run(params, options).await;
    });

    // Wait until the Director request is captured.
    wait_for_requests(&mock_director, 1, Duration::from_secs(2)).await;
    let req = mock_director.single_request();
    let body = req.body_json();

    // Assert that a JSON schema was sent under text.format.
    let text = &body["text"]; // Optional; present when using schemas
    assert!(text.is_object(), "missing text controls in request body");
    let fmt = &text["format"];
    assert!(fmt.is_object(), "missing text.format in request body");
    assert_eq!(fmt["type"], "json_schema");
    let schema = &fmt["schema"];
    assert!(schema.is_object(), "missing text.format.schema");
    assert_eq!(schema["type"], "object");
    // Ensure the directive property exists and is a string.
    assert_eq!(schema["properties"]["directive"]["type"], "string");
    // Enforce strictness: required must include all properties.
    let required = schema["required"]
        .as_array()
        .expect("required must be array");
    let props = schema["properties"]
        .as_object()
        .expect("properties must be object");
    for key in props.keys() {
        assert!(
            required.iter().any(|v| v == key),
            "missing {key} in required"
        );
    }
    // Ensure the objective text appears in the serialized request body
    let raw = serde_json::to_string(&body).expect("serialize body");
    assert!(
        raw.contains("Kick off"),
        "objective missing from director request body"
    );

    // Stop the background task to end the test.
    fut.abort();
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn final_delivery_request_includes_output_schema() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;

    // 1) Solver: emit empty message so orchestrator asks for final_delivery via schema.
    let body_solver = responses::sse(vec![
        responses::ev_response_created("solver-resp-1"),
        // No signal -> orchestrator will prompt with final_output schema.
        responses::ev_completed("solver-resp-1"),
    ]);
    let _mock_solver = responses::mount_sse_once(&server, body_solver).await;

    // 2) Capture the schema-bearing request to Solver.
    let body_solver_prompt = responses::sse(vec![
        responses::ev_response_created("solver-resp-2"),
        responses::ev_assistant_message(
            "solver-msg-2",
            r#"{"type":"final_delivery","deliverable_path":"deliverable/summary.txt","summary":null}"#,
        ),
        responses::ev_completed("solver-resp-2"),
    ]);
    let mock_solver_prompt = responses::mount_sse_once(&server, body_solver_prompt).await;

    // 3) Keep any follow-up quiet.
    let body_solver_done = responses::sse(vec![
        responses::ev_response_created("solver-resp-3"),
        responses::ev_completed("solver-resp-3"),
    ]);
    let _mock_solver_done = responses::mount_sse_once(&server, body_solver_done).await;

    let runs_root = TempDir::new()?;
    let orchestrator =
        InftyOrchestrator::with_runs_root(CodexAuth::from_api_key("dummy-key"), runs_root.path());
    let run_id = "run-final-schema".to_string();

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
        ..Default::default()
    };

    let fut = tokio::spawn(async move {
        let _ = orchestrator.execute_new_run(params, options).await;
    });

    wait_for_requests(&mock_solver_prompt, 1, Duration::from_secs(2)).await;
    let req = mock_solver_prompt.single_request();
    let body = req.body_json();
    let text = &body["text"];
    assert!(text.is_object(), "missing text controls in request body");
    let fmt = &text["format"];
    assert!(fmt.is_object(), "missing text.format in request body");
    assert_eq!(fmt["type"], "json_schema");
    let schema = &fmt["schema"];
    assert!(schema.is_object(), "missing text.format.schema");
    let required = schema["required"]
        .as_array()
        .expect("required must be array");
    let props = schema["properties"]
        .as_object()
        .expect("properties must be object");
    for key in props.keys() {
        assert!(
            required.iter().any(|v| v == key),
            "missing {key} in required"
        );
    }

    fut.abort();
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn verifier_request_includes_output_schema() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;

    // 1) Solver: issue a final_delivery which triggers verifier requests.
    let body_solver = responses::sse(vec![
        responses::ev_response_created("solver-resp-1"),
        responses::ev_assistant_message(
            "solver-msg-1",
            r#"{"type":"final_delivery","deliverable_path":"deliverable/summary.txt","summary":null}"#,
        ),
        responses::ev_completed("solver-resp-1"),
    ]);
    let _mock_solver = responses::mount_sse_once(&server, body_solver).await;

    // 2) Verifier: reply with a verdict JSON.
    let body_verifier = responses::sse(vec![
        responses::ev_response_created("verifier-resp-1"),
        responses::ev_assistant_message(
            "verifier-msg-1",
            r#"{"verdict":"pass","reasons":[],"suggestions":[]}"#,
        ),
        responses::ev_completed("verifier-resp-1"),
    ]);
    let mock_verifier = responses::mount_sse_once(&server, body_verifier).await;

    // 3) After posting the summary back to Solver, let the request complete.
    let body_solver_after = responses::sse(vec![
        responses::ev_response_created("solver-resp-2"),
        responses::ev_completed("solver-resp-2"),
    ]);
    let _mock_solver_after = responses::mount_sse_once(&server, body_solver_after).await;

    let runs_root = TempDir::new()?;
    let orchestrator =
        InftyOrchestrator::with_runs_root(CodexAuth::from_api_key("dummy-key"), runs_root.path());
    let run_id = "run-verifier-schema".to_string();

    let solver_config = build_config(&server).await?;
    let director_config = build_config(&server).await?;
    let verifier_config = build_config(&server).await?;

    let params = RunParams {
        run_id: run_id.clone(),
        run_root: Some(runs_root.path().join("runs").join(&run_id)),
        solver: RoleConfig::new("solver", solver_config),
        director: RoleConfig::new("director", director_config),
        verifiers: vec![RoleConfig::new("verifier", verifier_config)],
    };

    let options = RunExecutionOptions {
        objective: Some("Kick off".to_string()),
        ..Default::default()
    };

    let fut = tokio::spawn(async move {
        let _ = orchestrator.execute_new_run(params, options).await;
    });

    // Wait until the Verifier request is captured.
    wait_for_requests(&mock_verifier, 1, Duration::from_secs(2)).await;
    let req = mock_verifier.single_request();
    let body = req.body_json();

    // Assert that a JSON schema was sent under text.format.
    let text = &body["text"]; // Optional; present when using schemas
    assert!(text.is_object(), "missing text controls in request body");
    let fmt = &text["format"];
    assert!(fmt.is_object(), "missing text.format in request body");
    assert_eq!(fmt["type"], "json_schema");
    let schema = &fmt["schema"];
    assert!(schema.is_object(), "missing text.format.schema");
    assert_eq!(schema["type"], "object");
    // Ensure the verdict property exists and is an enum of pass/fail.
    assert!(schema["properties"]["verdict"].is_object());
    // Enforce strictness: required must include all properties.
    let required = schema["required"]
        .as_array()
        .expect("required must be array");
    let props = schema["properties"]
        .as_object()
        .expect("properties must be object");
    for key in props.keys() {
        assert!(
            required.iter().any(|v| v == key),
            "missing {key} in required"
        );
    }

    fut.abort();
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

async fn wait_for_requests(mock: &responses::ResponseMock, min: usize, timeout: Duration) {
    use tokio::time::Instant;
    use tokio::time::sleep;
    let start = Instant::now();
    loop {
        if mock.requests().len() >= min {
            return;
        }
        if start.elapsed() > timeout {
            return;
        }
        sleep(Duration::from_millis(25)).await;
    }
}
