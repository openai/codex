use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use codex_core::StartThreadOptions;
#[cfg(not(target_os = "windows"))]
use codex_core::config::Constrained;
#[cfg(not(target_os = "windows"))]
use codex_core::sandboxing::SandboxPermissions;
#[cfg(not(target_os = "windows"))]
use codex_protocol::config_types::ApprovalsReviewer;
#[cfg(not(target_os = "windows"))]
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadSource;
use codex_protocol::user_input::UserInput;
use core_test_support::PathBufExt;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::start_websocket_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use tempfile::TempDir;

const ORIGIN_URL: &str = "https://example.invalid/cxa5426/repo.git";

fn run_git(cwd: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .with_context(|| format!("run git {}", args.join(" ")))?;
    ensure!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

fn create_git_repo() -> Result<(TempDir, String)> {
    let repo = TempDir::new()?;
    run_git(repo.path(), &["init", "-q", "-b", "main"])?;
    std::fs::write(repo.path().join("README.md"), "initial\n")?;
    run_git(repo.path(), &["add", "README.md"])?;
    run_git(
        repo.path(),
        &[
            "-c",
            "user.name=CXA-5426",
            "-c",
            "user.email=cxa5426@example.invalid",
            "commit",
            "-q",
            "-m",
            "initial",
        ],
    )?;
    run_git(repo.path(), &["remote", "add", "origin", ORIGIN_URL])?;
    let head = run_git(repo.path(), &["rev-parse", "HEAD"])?;
    Ok((repo, head))
}

fn turn_metadata(request: &Value) -> Result<Value> {
    serde_json::from_str(
        request["client_metadata"]["x-codex-turn-metadata"]
            .as_str()
            .context("turn metadata")?,
    )
    .context("valid turn metadata")
}

fn expected_workspace(repo: &Path, head: &str, has_changes: bool) -> Value {
    json!({
        repo.to_string_lossy().as_ref(): {
            "associated_remote_urls": {"origin": ORIGIN_URL},
            "latest_git_commit_hash": head,
            "has_changes": has_changes,
        }
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn startup_prewarm_skips_git_enrichment_and_user_turn_observes_fresh_state() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let (repo, head) = create_git_repo()?;
    let server = start_websocket_server(vec![vec![
        vec![ev_response_created("warm-1"), ev_completed("warm-1")],
        vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ],
    ]])
    .await;
    let cwd = repo.path().to_path_buf();
    let mut builder = test_codex().with_config(move |config| {
        config.cwd = cwd.abs();
    });
    let test = builder.build_with_websocket_server(&server).await?;

    let prewarm = tokio::time::timeout(
        Duration::from_secs(5),
        server.wait_for_request(/*connection_index*/ 0, /*request_index*/ 0),
    )
    .await?
    .body_json();
    assert!(turn_metadata(&prewarm)?.get("workspaces").is_none());

    std::fs::write(repo.path().join("untracked.txt"), "dirty\n")?;
    test.submit_turn("inspect the workspace").await?;
    let turn = server
        .single_connection()
        .get(1)
        .context("turn request")?
        .body_json();
    assert_eq!(
        turn_metadata(&turn)?["workspaces"],
        expected_workspace(repo.path(), &head, /*has_changes*/ true)
    );

    test.codex.shutdown_and_wait().await?;
    server.shutdown().await;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_prewarm_and_review_skip_redundant_git_enrichment() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let (repo, head) = create_git_repo()?;
    let tool_args = json!({
        "cmd": "true",
        "sandbox_permissions": SandboxPermissions::RequireEscalated,
        "justification": "Exercise Guardian approval routing.",
    })
    .to_string();
    let server = start_websocket_server(vec![
        vec![vec![ev_response_created("warm-1"), ev_completed("warm-1")]],
        vec![vec![ev_response_created("warm-2"), ev_completed("warm-2")]],
        vec![vec![
            ev_response_created("approval-request"),
            ev_function_call("approval-call", "exec_command", &tool_args),
            ev_completed("approval-request"),
        ]],
        vec![vec![
            ev_response_created("guardian-review"),
            ev_completed("guardian-review"),
        ]],
    ])
    .await;
    let cwd = repo.path().to_path_buf();
    let mut builder = test_codex().with_config(move |config| {
        config.cwd = cwd.abs();
        config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
        config.approvals_reviewer = ApprovalsReviewer::AutoReview;
    });
    let test = builder.build_with_websocket_server(&server).await?;

    let (first, second) = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::join!(
            server.wait_for_request(/*connection_index*/ 0, /*request_index*/ 0),
            server.wait_for_request(/*connection_index*/ 1, /*request_index*/ 0)
        )
    })
    .await?;
    for prewarm in [first.body_json(), second.body_json()] {
        assert_eq!(prewarm["generate"].as_bool(), Some(false));
        assert!(turn_metadata(&prewarm)?.get("workspaces").is_none());
    }

    std::fs::write(repo.path().join("untracked.txt"), "dirty\n")?;
    test.codex
        .submit(
            vec![UserInput::Text {
                text: "run a command that requires Guardian review".into(),
                text_elements: Vec::new(),
            }]
            .into(),
        )
        .await?;
    let (user_turn, guardian_turn) = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::join!(
            server.wait_for_request(/*connection_index*/ 2, /*request_index*/ 0),
            server.wait_for_request(/*connection_index*/ 3, /*request_index*/ 0)
        )
    })
    .await?;
    let user_turn = user_turn.body_json();
    let guardian_turn = guardian_turn.body_json();
    assert_eq!(
        turn_metadata(&user_turn)?["workspaces"],
        expected_workspace(repo.path(), &head, /*has_changes*/ true)
    );
    assert_eq!(
        guardian_turn["client_metadata"]["x-openai-subagent"].as_str(),
        Some("guardian")
    );
    assert!(turn_metadata(&guardian_turn)?.get("workspaces").is_none());

    test.codex.shutdown_and_wait().await?;
    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ephemeral_system_thread_prewarm_skips_and_turn_observes_fresh_state() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let (repo, head) = create_git_repo()?;
    let server = start_websocket_server(vec![
        vec![vec![ev_response_created("warm-1"), ev_completed("warm-1")]],
        vec![
            vec![ev_response_created("warm-2"), ev_completed("warm-2")],
            vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    "wait-for-git",
                    "test_sync_tool",
                    r#"{"sleep_after_ms":1000}"#,
                ),
                ev_completed("resp-1"),
            ],
            vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "done"),
                ev_completed("resp-2"),
            ],
        ],
    ])
    .await;
    let cwd = repo.path().to_path_buf();
    let mut builder = test_codex()
        .with_model("test-gpt-5.1-codex")
        .with_config(move |config| {
            config.cwd = cwd.abs();
        });
    let test = builder.build_with_websocket_server(&server).await?;
    tokio::time::timeout(
        Duration::from_secs(5),
        server.wait_for_request(/*connection_index*/ 0, /*request_index*/ 0),
    )
    .await?;

    let mut config = test.config.clone();
    config.ephemeral = true;
    let environments = test
        .thread_manager
        .default_environment_selections(&config.cwd, &config.workspace_roots);
    let system_thread = test
        .thread_manager
        .start_thread_with_options(StartThreadOptions {
            config,
            allow_provider_model_fallback: false,
            initial_history: InitialHistory::New,
            history_mode: None,
            session_source: None,
            thread_source: Some(ThreadSource::Feature("system".to_string())),
            dynamic_tools: Vec::new(),
            metrics_service_name: None,
            parent_trace: None,
            environments,
            thread_extension_init: Default::default(),
            supports_openai_form_elicitation: false,
        })
        .await?;
    let prewarm = tokio::time::timeout(
        Duration::from_secs(5),
        server.wait_for_request(/*connection_index*/ 1, /*request_index*/ 0),
    )
    .await?
    .body_json();
    assert!(turn_metadata(&prewarm)?.get("workspaces").is_none());

    std::fs::write(repo.path().join("untracked.txt"), "dirty\n")?;
    system_thread
        .thread
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "generate a thread title".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;
    wait_for_event(system_thread.thread.as_ref(), |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let turn = server
        .connections()
        .get(1)
        .and_then(|connection| connection.get(2))
        .context("system turn follow-up request")?
        .body_json();
    assert_eq!(
        turn_metadata(&turn)?["workspaces"],
        expected_workspace(repo.path(), &head, /*has_changes*/ true)
    );

    system_thread.thread.shutdown_and_wait().await?;
    test.codex.shutdown_and_wait().await?;
    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_turns_keep_distinct_worktree_and_repository_metadata() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let (repo, repo_head) = create_git_repo()?;
    let worktree_parent = TempDir::new()?;
    let worktree = worktree_parent.path().join("linked-worktree");
    run_git(
        repo.path(),
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "feature/cxa5426",
            worktree.to_str().context("worktree path")?,
        ],
    )?;
    std::fs::write(worktree.join("branch.txt"), "branch commit\n")?;
    run_git(&worktree, &["add", "branch.txt"])?;
    run_git(
        &worktree,
        &[
            "-c",
            "user.name=CXA-5426",
            "-c",
            "user.email=cxa5426@example.invalid",
            "commit",
            "-q",
            "-m",
            "branch commit",
        ],
    )?;
    let worktree_head = run_git(&worktree, &["rev-parse", "HEAD"])?;
    let (other_repo, other_head) = create_git_repo()?;

    let server = start_websocket_server(vec![
        vec![vec![ev_response_created("warm-1"), ev_completed("warm-1")]],
        vec![vec![ev_response_created("warm-2"), ev_completed("warm-2")]],
        vec![vec![ev_response_created("warm-3"), ev_completed("warm-3")]],
        vec![vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]],
        vec![vec![
            ev_response_created("resp-2"),
            ev_assistant_message("msg-2", "done"),
            ev_completed("resp-2"),
        ]],
        vec![vec![
            ev_response_created("resp-3"),
            ev_assistant_message("msg-3", "done"),
            ev_completed("resp-3"),
        ]],
    ])
    .await;

    let repo_cwd = repo.path().to_path_buf();
    let mut builder = test_codex().with_config(move |config| {
        config.cwd = repo_cwd.abs();
    });
    let test = builder.build_with_websocket_server(&server).await?;

    let mut worktree_config = test.config.clone();
    worktree_config.cwd = worktree.abs();
    let worktree_environments = test
        .thread_manager
        .default_environment_selections(&worktree_config.cwd, &worktree_config.workspace_roots);
    let worktree_thread = test
        .thread_manager
        .start_thread_with_options(StartThreadOptions {
            config: worktree_config,
            allow_provider_model_fallback: false,
            initial_history: InitialHistory::New,
            history_mode: None,
            session_source: None,
            thread_source: Some(ThreadSource::User),
            dynamic_tools: Vec::new(),
            metrics_service_name: None,
            parent_trace: None,
            environments: worktree_environments,
            thread_extension_init: Default::default(),
            supports_openai_form_elicitation: false,
        })
        .await?;

    let mut other_config = test.config.clone();
    other_config.cwd = other_repo.path().to_path_buf().abs();
    let other_environments = test
        .thread_manager
        .default_environment_selections(&other_config.cwd, &other_config.workspace_roots);
    let other_thread = test
        .thread_manager
        .start_thread_with_options(StartThreadOptions {
            config: other_config,
            allow_provider_model_fallback: false,
            initial_history: InitialHistory::New,
            history_mode: None,
            session_source: None,
            thread_source: Some(ThreadSource::User),
            dynamic_tools: Vec::new(),
            metrics_service_name: None,
            parent_trace: None,
            environments: other_environments,
            thread_extension_init: Default::default(),
            supports_openai_form_elicitation: false,
        })
        .await?;

    tokio::time::timeout(Duration::from_secs(5), async {
        tokio::join!(
            server.wait_for_request(/*connection_index*/ 0, /*request_index*/ 0),
            server.wait_for_request(/*connection_index*/ 1, /*request_index*/ 0),
            server.wait_for_request(/*connection_index*/ 2, /*request_index*/ 0)
        )
    })
    .await?;

    std::fs::write(repo.path().join("untracked.txt"), "dirty\n")?;
    std::fs::write(other_repo.path().join("untracked.txt"), "dirty\n")?;
    let user_turn = |prompt: &str| Op::UserInput {
        items: vec![UserInput::Text {
            text: prompt.to_string(),
            text_elements: Vec::new(),
        }],
        final_output_json_schema: None,
        responsesapi_client_metadata: None,
        additional_context: Default::default(),
        thread_settings: Default::default(),
    };
    tokio::try_join!(
        test.codex.submit(user_turn("inspect the main worktree")),
        worktree_thread
            .thread
            .submit(user_turn("inspect the linked worktree")),
        other_thread
            .thread
            .submit(user_turn("inspect the other repository"))
    )?;
    tokio::join!(
        wait_for_event(test.codex.as_ref(), |event| matches!(
            event,
            EventMsg::TurnComplete(_)
        )),
        wait_for_event(worktree_thread.thread.as_ref(), |event| matches!(
            event,
            EventMsg::TurnComplete(_)
        )),
        wait_for_event(other_thread.thread.as_ref(), |event| matches!(
            event,
            EventMsg::TurnComplete(_)
        ))
    );

    let mut actual_workspaces = server
        .connections()
        .into_iter()
        .skip(3)
        .map(|connection| {
            let request = connection.first().context("turn request")?.body_json();
            Ok(turn_metadata(&request)?["workspaces"].clone())
        })
        .collect::<Result<Vec<_>>>()?;
    let mut expected_workspaces = vec![
        expected_workspace(repo.path(), &repo_head, /*has_changes*/ true),
        expected_workspace(&worktree, &worktree_head, /*has_changes*/ false),
        expected_workspace(other_repo.path(), &other_head, /*has_changes*/ true),
    ];
    actual_workspaces.sort_by_key(Value::to_string);
    expected_workspaces.sort_by_key(Value::to_string);
    assert_eq!(actual_workspaces, expected_workspaces);

    worktree_thread.thread.shutdown_and_wait().await?;
    other_thread.thread.shutdown_and_wait().await?;
    test.codex.shutdown_and_wait().await?;
    server.shutdown().await;
    Ok(())
}
