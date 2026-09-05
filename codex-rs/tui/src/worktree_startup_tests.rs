//! Checks saved-directory fallback and the user-facing retained-checkout notice.
use super::*;
use clap::Parser;
use pretty_assertions::assert_eq;

#[test]
fn recovery_message_names_checkout_and_safe_manual_action() {
    let recovery = StartupRecovery {
        root: PathBuf::from("checkout with spaces"),
        // This test inspects rendering without printing or changing terminal state on drop.
        finished: AtomicBool::new(true),
    };
    insta::assert_snapshot!(recovery.message(), @r#"
    Startup did not finish binding a thread to this worktree: "checkout with spaces"
    The checkout was kept. Inspect it and confirm no session is using it.
    To remove it, run `git worktree remove <checkout-path>` from the source repository,
    replacing <checkout-path> with the path above. Do not use --force.
    "#);
}

#[tokio::test]
async fn explicit_remote_worktree_rejection_is_snapshotted() -> anyhow::Result<()> {
    let cli = Cli::parse_from(["codex", "--worktree"]);
    let endpoint = RemoteAppServerEndpoint::UnixSocket {
        socket_path: AbsolutePathBuf::relative_to_current_dir("remote.sock")?,
    };
    let error = crate::startup_orchestration::run_main_inner(
        cli,
        Arg0DispatchPaths::default(),
        LoaderOverrides::default(),
        Some(endpoint),
    )
    .await
    .expect_err("managed worktrees require a local session");
    insta::assert_snapshot!(error.to_string(), @"`--worktree` is only supported for local sessions");
    Ok(())
}

#[tokio::test]
async fn latest_directory_uses_turn_context_and_preserves_fallback() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let fallback = dir.path().join("old");
    let latest = dir.path().join("latest");
    let path = dir.path().join("rollout.jsonl");
    let turn = serde_json::json!({
        "timestamp": "2026-09-04T00:00:00Z", "type": "turn_context", "payload": {
            "cwd": latest, "approval_policy": "never", "sandbox_policy": {"type":"read-only"},
            "model": "test-model", "summary": "auto"
        }
    });
    std::fs::write(&path, format!("{turn}\n"))?;
    assert_eq!(
        latest_thread_cwd(Some(path.clone()), fallback.clone()).await,
        latest
    );
    std::fs::write(&path, "")?;
    assert_eq!(
        latest_thread_cwd(Some(path), fallback.clone()).await,
        fallback
    );
    assert_eq!(
        latest_thread_cwd(/*path*/ None, fallback.clone()).await,
        fallback
    );
    Ok(())
}

#[tokio::test]
async fn refreshed_bundle_rechecks_source_during_config_reload() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let source = dir.path().join("source");
    let nested = source.join(".codex");
    let destination = dir.path().join("checkout");
    for path in [&nested, &destination] {
        std::fs::create_dir_all(path)?;
    }
    let mut loader_overrides = LoaderOverrides::without_managed_config_for_tests();
    loader_overrides.ignore_user_config = true;
    let manager = codex_worktree::WorktreeManager::new(codex_worktree::WorktreeSettings::for_cli(
        dir.path(),
        /*desktop*/ None,
    )?);
    let checkout = codex_worktree::ManagedWorktree {
        root: destination.clone(),
        cwd: destination.clone(),
        source_root: source,
        source_cwd: nested.clone(),
        head_sha: String::new(),
        branch: None,
    };
    let recovery = Arc::new(StartupRecovery {
        root: destination.clone(),
        finished: AtomicBool::new(true),
    });
    let worktree = ManagedTuiWorktree {
        manager,
        checkout,
        recovery,
    };
    let overrides = ConfigOverrides {
        cwd: Some(destination),
        ..Default::default()
    };
    crate::load_config_with_worktree_source_policy(
        Vec::new(),
        overrides.clone(),
        loader_overrides.clone(),
        CloudConfigBundleLoader::default(),
        /*strict_config*/ false,
        /*fallback_cwd*/ None,
        Some(&worktree),
    )
    .await?;
    let refreshed =
        codex_config::test_support::CloudConfigBundleFixture::loader_with_enterprise_config(
            format!(
                "[projects.{}]\ntrust_level = \"untrusted\"\n",
                serde_json::to_string(&codex_config::loader::project_trust_key(&nested))?
            ),
        );
    let err = crate::load_config_with_worktree_source_policy(
        Vec::new(),
        overrides,
        loader_overrides,
        refreshed,
        /*strict_config*/ false,
        /*fallback_cwd*/ None,
        Some(&worktree),
    )
    .await
    .expect_err("refreshed source distrust must reject");
    assert!(err.to_string().contains("explicitly untrusted source"));
    Ok(())
}
