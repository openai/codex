use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use codex_core::StartThreadOptions;
use codex_core::TurnInputRequest;
#[cfg(not(target_os = "windows"))]
use codex_core::config::Constrained;
#[cfg(not(target_os = "windows"))]
use codex_core::sandboxing::SandboxPermissions;
#[cfg(not(target_os = "windows"))]
use codex_protocol::config_types::ApprovalsReviewer;
#[cfg(not(target_os = "windows"))]
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ThreadSource;
use codex_protocol::user_input::UserInput;
use core_test_support::PathBufExt;
use core_test_support::responses::assert_root_turn;
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
use test_case::test_case;

const ORIGIN_URL: &str = "https://example.invalid/cxa5426/repo.git";

pub(super) fn run_git(cwd: &Path, args: &[&str]) -> Result<String> {
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
    run_git(repo.path(), &["config", "user.name", "CXA-5426"])?;
    run_git(
        repo.path(),
        &["config", "user.email", "cxa5426@example.invalid"],
    )?;
    std::fs::write(repo.path().join("README.md"), "initial\n")?;
    run_git(repo.path(), &["add", "README.md"])?;
    run_git(repo.path(), &["commit", "-qm", "initial"])?;
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

#[cfg(unix)]
fn assert_request_workspace(
    request: &core_test_support::responses::ResponsesRequest,
    expected: Value,
) -> Result<()> {
    let header: Value = serde_json::from_str(
        &request
            .header("x-codex-turn-metadata")
            .context("turn metadata header")?,
    )?;
    assert_eq!(header["workspaces"], expected);
    assert_eq!(turn_metadata(&request.body_json())?["workspaces"], expected);
    Ok(())
}

/// Requests remain prompt while status is blocked, then carry the fresh complete snapshot.
#[cfg(unix)]
#[tokio::test(flavor = "current_thread")]
async fn requests_refresh_git_refs_after_model_lookup_without_waiting_for_status() -> Result<()> {
    use super::wake_reservation::GatedModelsManager;
    use core_test_support::responses;
    use std::io::Write;
    use std::sync::Arc;
    use std::time::UNIX_EPOCH;

    struct NotifyRequest(Arc<tokio::sync::Notify>);

    impl wiremock::Match for NotifyRequest {
        fn matches(&self, _request: &wiremock::Request) -> bool {
            self.0.notify_one();
            true
        }
    }

    skip_if_no_network!(Ok(()));
    let (repo, old_head) = create_git_repo()?;
    let root = repo.path();
    let next_origin = "https://example.invalid/cxa5426/next.git";
    let next = run_git(
        root,
        &["commit-tree", "HEAD^{tree}", "-p", "HEAD", "-m", "next"],
    )?;
    let server = responses::start_mock_server().await;
    let first_arrived = Arc::new(tokio::sync::Notify::new());
    let first_requests = responses::mount_sse_once_match(
        &server,
        NotifyRequest(Arc::clone(&first_arrived)),
        responses::sse(vec![
            ev_response_created("initial"),
            ev_function_call(
                "wait",
                "test_sync_tool",
                r#"{"wait_for_git_enrichment":true}"#,
            ),
            ev_completed("initial"),
        ]),
    )
    .await;
    let mut catalog = codex_models_manager::bundled_models_response()?;
    catalog
        .models
        .iter_mut()
        .find(|model| model.slug == "gpt-5.5")
        .context("bundled gpt-5.5 model")?
        .experimental_supported_tools
        .push("test_sync_tool".to_string());
    let models = Arc::new(GatedModelsManager::with_catalog(catalog));
    let cwd = root.to_path_buf().abs();
    let test = test_codex()
        .with_model("gpt-5.5")
        .with_models_manager(models.clone())
        .with_config(move |config| config.cwd = cwd)
        .build(&server)
        .await?;

    let tracked = root.join("README.md");
    std::fs::write(&tracked, "changed\n")?;
    std::fs::File::open(&tracked)?.set_modified(UNIX_EPOCH + Duration::from_secs(1))?;
    std::fs::create_dir_all(root.join(".git/info"))?;
    std::fs::write(
        root.join(".git/info/attributes"),
        "README.md filter=hold-status\n",
    )?;
    let fifo = root.join(".git/release-status");
    ensure!(
        Command::new("mkfifo").arg(&fifo).status()?.success(),
        "create status gate"
    );
    let mut gate = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&fifo)?;
    run_git(
        root,
        &[
            "config",
            "filter.hold-status.clean",
            "read -r line < .git/release-status && cat",
        ],
    )?;

    let model_lookup = models.gate_next_lookup();
    let release_model = async {
        tokio::time::timeout(Duration::from_secs(5), model_lookup.entered.notified()).await?;
        let switched = run_git(root, &["update-ref", "HEAD", &next])
            .and_then(|_| run_git(root, &["remote", "set-url", "origin", next_origin]));
        model_lookup.release.notify_one();
        switched?;
        Ok(())
    };
    let release_status = async {
        // The Git command timeout is five seconds. This request must arrive before status
        // could time out, while the clean filter is still blocked on the FIFO.
        tokio::time::timeout(Duration::from_secs(4), first_arrived.notified()).await?;
        let first = first_requests.single_request();
        let metadata = turn_metadata(&first.body_json())?;
        if let Some(workspace) = metadata["workspaces"].get(root.to_string_lossy().as_ref()) {
            assert_ne!(
                workspace["latest_git_commit_hash"].as_str(),
                Some(old_head.as_str())
            );
            assert_ne!(
                workspace["associated_remote_urls"]["origin"].as_str(),
                Some(ORIGIN_URL)
            );
            assert!(workspace.get("has_changes").is_none());
        }
        let following =
            responses::mount_sse_once(&server, responses::sse_completed("follow-up")).await;
        gate.write_all(b"release\n")?;
        Ok(following)
    };
    let ((), (), following) = tokio::time::timeout(Duration::from_secs(12), async {
        tokio::try_join!(
            test.submit_text_turn("inspect the workspace"),
            release_model,
            release_status,
        )
    })
    .await??;

    let follow_up = following.single_request();
    assert_eq!(
        follow_up.function_call_output_text("wait").as_deref(),
        Some("ok")
    );
    assert_request_workspace(
        &follow_up,
        json!({
            root.to_string_lossy().as_ref(): {
                "associated_remote_urls": {"origin": next_origin},
                "latest_git_commit_hash": next,
                "has_changes": true,
            },
        }),
    )?;
    let input = follow_up.body_json()["input"].to_string();
    assert!(!input.contains(&old_head));
    assert!(!input.contains(&next));
    assert!(!input.contains(ORIGIN_URL));
    assert!(!input.contains(next_origin));
    assert!(!input.contains("codex_git_state"));
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

enum GitMetadataAtStartup {
    Present,
    RestoredAfterPrewarm,
}

#[test_case(GitMetadataAtStartup::Present; "git present at startup")]
#[test_case(GitMetadataAtStartup::RestoredAfterPrewarm; "git restored after prewarm")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn startup_prewarm_skips_git_enrichment_and_user_turn_observes_fresh_state(
    git_metadata: GitMetadataAtStartup,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let (repo, head) = create_git_repo()?;
    let saved_git = TempDir::new()?;
    if matches!(git_metadata, GitMetadataAtStartup::RestoredAfterPrewarm) {
        std::fs::rename(repo.path().join(".git"), saved_git.path().join("git"))?;
    }
    let server = start_websocket_server(vec![vec![
        vec![ev_response_created("warm-1"), ev_completed("warm-1")],
        vec![
            ev_response_created("resp-1"),
            ev_function_call(
                "wait-for-git",
                "test_sync_tool",
                r#"{"wait_for_git_enrichment":true}"#,
            ),
            ev_completed("resp-1"),
        ],
        vec![
            ev_response_created("resp-2"),
            ev_assistant_message("msg-2", "done"),
            ev_completed("resp-2"),
        ],
    ]])
    .await;
    let cwd = repo.path().to_path_buf();
    let mut builder = test_codex()
        .with_model("test-gpt-5.1-codex")
        .with_config(move |config| {
            config.cwd = cwd.abs();
        });
    let test = builder.build_with_websocket_server(&server).await?;

    let prewarm = tokio::time::timeout(
        Duration::from_secs(5),
        server.wait_for_request(/*connection_index*/ 0, /*request_index*/ 0),
    )
    .await?
    .body_json();
    assert_eq!(prewarm["generate"], json!(false));
    assert!(turn_metadata(&prewarm)?.get("workspaces").is_none());

    if matches!(git_metadata, GitMetadataAtStartup::RestoredAfterPrewarm) {
        std::fs::rename(saved_git.path().join("git"), repo.path().join(".git"))?;
    }
    std::fs::write(repo.path().join("untracked.txt"), "dirty\n")?;
    test.submit_turn("inspect the workspace").await?;
    let turn = server
        .single_connection()
        .get(2)
        .context("turn follow-up request")?
        .body_json();
    assert_eq!(
        turn_metadata(&turn)?["workspaces"],
        expected_workspace(repo.path(), &head, /*has_changes*/ true)
    );

    test.codex.shutdown_and_wait().await?;
    server.shutdown().await;
    Ok(())
}

/// Repository credentials must never cross the outbound model-request boundary.
#[test_case(false; "ordinary clone")]
#[test_case(true; "partial clone")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_turn_git_enrichment_redacts_remote_credentials(partial_clone: bool) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let (source, head) = create_git_repo()?;
    let repo = if partial_clone {
        run_git(source.path(), &["config", "uploadpack.allowFilter", "true"])?;
        let clone = TempDir::new()?;
        run_git(
            clone.path(),
            &[
                "clone",
                "--no-local",
                "--filter=blob:none",
                source.path().to_str().context("source repo path")?,
                ".",
            ],
        )?;
        clone
    } else {
        source
    };
    run_git(
        repo.path(),
        &[
            "remote",
            "set-url",
            "origin",
            "https://git-user:git-secret-token@example.invalid/cxa5426/repo.git",
        ],
    )?;
    let server = start_websocket_server(vec![vec![
        vec![ev_response_created("warm-1"), ev_completed("warm-1")],
        vec![
            ev_response_created("resp-1"),
            ev_function_call(
                "wait-for-git",
                "test_sync_tool",
                r#"{"wait_for_git_enrichment":true}"#,
            ),
            ev_completed("resp-1"),
        ],
        vec![
            ev_response_created("resp-2"),
            ev_assistant_message("msg-2", "done"),
            ev_completed("resp-2"),
        ],
    ]])
    .await;
    let cwd = repo.path().to_path_buf();
    let mut builder = test_codex()
        .with_model("test-gpt-5.1-codex")
        .with_config(move |config| {
            config.cwd = cwd.abs();
        });
    let test = builder.build_with_websocket_server(&server).await?;

    test.submit_turn("inspect the workspace").await?;
    let turn = server
        .single_connection()
        .get(2)
        .context("turn follow-up request")?
        .body_json();
    let serialized_turn = serde_json::to_string(&turn)?;
    assert!(!serialized_turn.contains("git-user"));
    assert!(!serialized_turn.contains("git-secret-token"));
    assert_eq!(
        turn_metadata(&turn)?["workspaces"],
        expected_workspace(repo.path(), &head, /*has_changes*/ false)
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
            ev_response_created("wait-for-parent-git"),
            ev_function_call(
                "wait-for-parent-git",
                "test_sync_tool",
                r#"{"wait_for_git_enrichment":true}"#,
            ),
            ev_completed("wait-for-parent-git"),
        ]],
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
    let mut builder = test_codex()
        .with_model("test-gpt-5.1-codex")
        .with_config(move |config| {
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
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "run a command that requires Guardian review".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let (initial_user_turn, user_turn, guardian_turn) =
        tokio::time::timeout(Duration::from_secs(15), async {
            tokio::join!(
                server.wait_for_request(/*connection_index*/ 2, /*request_index*/ 0),
                server.wait_for_request(/*connection_index*/ 3, /*request_index*/ 0),
                server.wait_for_request(/*connection_index*/ 4, /*request_index*/ 0)
            )
        })
        .await?;
    let initial_user_turn = initial_user_turn.body_json();
    let user_turn = user_turn.body_json();
    let guardian_turn = guardian_turn.body_json();
    let initial_user_turn_metadata = turn_metadata(&initial_user_turn)?;
    assert_eq!(
        turn_metadata(&user_turn)?["auto_review_enabled"].as_bool(),
        Some(true)
    );
    let dirty_workspace = expected_workspace(repo.path(), &head, /*has_changes*/ true);
    if let Some(workspaces) = initial_user_turn_metadata.get("workspaces") {
        let root = repo.path().to_string_lossy();
        assert_eq!(
            workspaces.as_object().context("initial workspaces")?.len(),
            1
        );
        let fields = workspaces[root.as_ref()]
            .as_object()
            .context("initial workspace")?;
        assert!(!fields.is_empty());
        for (field, value) in fields {
            assert_eq!(Some(value), dirty_workspace[root.as_ref()].get(field));
        }
    }
    assert_eq!(turn_metadata(&user_turn)?["workspaces"], dirty_workspace);
    assert_eq!(
        guardian_turn["client_metadata"]["x-openai-subagent"].as_str(),
        Some("guardian")
    );
    assert!(turn_metadata(&guardian_turn)?.get("workspaces").is_none());

    test.codex.shutdown_and_wait().await?;
    server.shutdown().await;
    Ok(())
}

#[test_case("system"; "system background thread")]
#[test_case("ambient_background"; "ambient background thread")]
#[test_case("ambient_suggestions"; "ambient suggestions")]
#[test_case("ambient_suggestion_safety"; "ambient suggestion safety")]
#[test_case("title"; "title background thread")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ephemeral_system_thread_prewarm_skips_and_turn_observes_fresh_state(
    thread_source: &str,
) -> Result<()> {
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
    let system_thread = test
        .thread_manager
        .start_thread(StartThreadOptions {
            thread_source: Some(ThreadSource::Feature(thread_source.to_string())),
            ..StartThreadOptions::new(config)
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
    let submission = system_thread
        .thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "generate a thread title".into(),
            text_elements: Vec::new(),
        }]))
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
    let codex_core::TurnInputSubmission::Started { turn_id, .. } = submission else {
        panic!("background input should start a turn");
    };
    assert_root_turn(&turn, Some(&turn_id))?;
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
    run_git(&worktree, &["commit", "-qm", "branch commit"])?;
    let worktree_head = run_git(&worktree, &["rev-parse", "HEAD"])?;
    let (other_repo, other_head) = create_git_repo()?;

    let initial_turn = |name: &str| {
        vec![vec![
            ev_response_created(&format!("turn-{name}")),
            ev_function_call(
                &format!("wait-for-git-{name}"),
                "test_sync_tool",
                r#"{"barrier":{"id":"concurrent-git-enrichment","participants":3,"timeout_ms":10000},"wait_for_git_enrichment":true}"#,
            ),
            ev_completed(&format!("turn-{name}")),
        ]]
    };
    let follow_up_turn = |name: &str| {
        vec![vec![
            ev_response_created(&format!("follow-up-{name}")),
            ev_assistant_message(&format!("msg-{name}"), "done"),
            ev_completed(&format!("follow-up-{name}")),
        ]]
    };
    let server = start_websocket_server(vec![
        vec![vec![ev_response_created("warm-1"), ev_completed("warm-1")]],
        vec![vec![ev_response_created("warm-2"), ev_completed("warm-2")]],
        vec![vec![ev_response_created("warm-3"), ev_completed("warm-3")]],
        initial_turn("repo"),
        initial_turn("worktree"),
        initial_turn("other-repo"),
        follow_up_turn("repo"),
        follow_up_turn("worktree"),
        follow_up_turn("other-repo"),
    ])
    .await;

    let repo_cwd = repo.path().to_path_buf();
    let mut builder = test_codex()
        .with_model("test-gpt-5.1-codex")
        .with_config(move |config| {
            config.cwd = repo_cwd.abs();
        });
    let test = builder.build_with_websocket_server(&server).await?;

    let mut worktree_config = test.config.clone();
    worktree_config.cwd = worktree.abs();
    let worktree_thread = test
        .thread_manager
        .start_thread(StartThreadOptions {
            thread_source: Some(ThreadSource::User),
            ..StartThreadOptions::new(worktree_config)
        })
        .await?;

    let mut other_config = test.config.clone();
    other_config.cwd = other_repo.path().to_path_buf().abs();
    let other_thread = test
        .thread_manager
        .start_thread(StartThreadOptions {
            thread_source: Some(ThreadSource::User),
            ..StartThreadOptions::new(other_config)
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
    let user_turn = |prompt: &str| {
        TurnInputRequest::user_input(vec![UserInput::Text {
            text: prompt.to_string(),
            text_elements: Vec::new(),
        }])
    };
    tokio::try_join!(
        test.codex
            .start_or_steer_turn(user_turn("inspect the main worktree")),
        worktree_thread
            .thread
            .start_or_steer_turn(user_turn("inspect the linked worktree")),
        other_thread
            .thread
            .start_or_steer_turn(user_turn("inspect the other repository"))
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
        .skip(6)
        .map(|connection| {
            let request = connection
                .first()
                .context("synchronized turn follow-up request")?
                .body_json();
            let thread_id = request["client_metadata"]["thread_id"]
                .as_str()
                .context("follow-up thread id")?
                .to_string();
            Ok((thread_id, turn_metadata(&request)?["workspaces"].clone()))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut expected_workspaces = vec![
        (
            test.session_configured.thread_id.to_string(),
            expected_workspace(repo.path(), &repo_head, /*has_changes*/ true),
        ),
        (
            worktree_thread.thread_id.to_string(),
            expected_workspace(&worktree, &worktree_head, /*has_changes*/ false),
        ),
        (
            other_thread.thread_id.to_string(),
            expected_workspace(other_repo.path(), &other_head, /*has_changes*/ true),
        ),
    ];
    actual_workspaces.sort_by(|(left, _), (right, _)| left.cmp(right));
    expected_workspaces.sort_by(|(left, _), (right, _)| left.cmp(right));
    assert_eq!(actual_workspaces, expected_workspaces);

    worktree_thread.thread.shutdown_and_wait().await?;
    other_thread.thread.shutdown_and_wait().await?;
    test.codex.shutdown_and_wait().await?;
    server.shutdown().await;
    Ok(())
}
