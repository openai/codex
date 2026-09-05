//! Exercises gated shared-pool execution and preserves existing checkout ownership.
//! Observe ownership at the first model request, before the process can finish.

use anyhow::Context;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_worktree::CreateWorktree;
use codex_worktree::WorktreeManager;
use codex_worktree::WorktreeSettings;
use core_test_support::responses;
use core_test_support::test_codex_exec::test_codex_exec;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::process::Output;
use std::sync::Arc;
use std::sync::Mutex;

fn git(cwd: &Path, args: &[&str]) -> anyhow::Result<()> {
    let output = Command::new("git")
        .current_dir(cwd)
        .args([
            "-c",
            "user.name=Worktree Test",
            "-c",
            "user.email=test@example.invalid",
        ])
        .args(args)
        .output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

fn started_thread(output: Output) -> anyhow::Result<String> {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let events = String::from_utf8(output.stdout)?
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    events
        .iter()
        .find(|event| event["type"] == "thread.started")
        .and_then(|event| event["thread_id"].as_str())
        .map(str::to_owned)
        .context("exec should report its thread id")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn worktree_start_and_fork_use_host_pool_and_preserve_legacy_resume() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let home = AbsolutePathBuf::from_absolute_path(test.home_path())?
        .canonicalize()?
        .into_path_buf();
    let source = AbsolutePathBuf::from_absolute_path(test.cwd_path())?
        .canonicalize()?
        .into_path_buf();
    let extra = source.join("extra");
    fs::create_dir(&extra)?;
    fs::write(extra.join("tracked.txt"), "tracked")?;
    fs::write(source.join("AGENTS.md"), "managed checkout instructions")?;
    git(&source, &["init", "--quiet"])?;
    git(&source, &["add", "."])?;
    git(
        &source,
        &["commit", "--quiet", "--no-gpg-sign", "-m", "initial"],
    )?;
    let pool = home.join("shared-worktrees");
    let other_pool = serde_json::to_string(&home.join("session-only-pool"))?;
    fs::write(
        home.join("config.toml"),
        format!(
            "sandbox_mode = \"workspace-write\"\nfeatures.worktrees = true\n[desktop]\ngit-worktree-root = {}\n[projects.{}]\ntrust_level = \"trusted\"\n",
            serde_json::to_string(&pool)?,
            serde_json::to_string(&source)?,
        ),
    )?;
    fs::write(
        home.join("demo.config.toml"),
        format!("features.worktrees = false\n[desktop]\ngit-worktree-root = {other_pool}\n"),
    )?;
    let mut settings = WorktreeSettings::for_cli(&home, /*desktop*/ None)?;
    settings.root = pool;
    let legacy_manager = WorktreeManager::new(WorktreeSettings {
        root: home.join("worktrees-cli"),
        ..settings.clone()
    });
    let legacy = legacy_manager.create(&CreateWorktree {
        source_cwd: source.clone(),
        base: None,
    })?;
    let manager = WorktreeManager::new(settings);
    let body = responses::sse(vec![
        responses::ev_response_created("response_1"),
        responses::ev_assistant_message("message_1", "done"),
        responses::ev_completed("response_1"),
    ]);
    let server = responses::start_mock_server().await;
    let seed_mock = responses::mount_sse_once(&server, body.clone()).await;
    let legacy_id = started_thread(
        test.cmd_with_server(&server)
            .current_dir(legacy.cwd.join("extra"))
            .args(["--json", "legacy seed"])
            .output()?,
    )?;
    seed_mock.single_request();
    legacy_manager.bind_thread(&legacy.root, &legacy_id)?;
    fs::write(source.join("AGENTS.md"), "uncommitted source instructions")?;
    fs::create_dir(source.join(".codex"))?;
    fs::write(source.join(".codex/config.toml"), "[invalid source config")?;

    let launcher = tempfile::tempdir()?;
    fs::create_dir(launcher.path().join("extra"))?;
    let mut known_roots = Vec::new();
    for (fork, overrides) in [
        (false, Vec::new()),
        (
            true,
            vec![
                "--profile".to_owned(),
                "demo".to_owned(),
                "-c".to_owned(),
                "features.worktrees=true".to_owned(),
                "-c".to_owned(),
                format!("desktop.git-worktree-root={other_pool}"),
                "-c".to_owned(),
                format!("desktop={{git-worktree-root={other_pool}}}"),
            ],
        ),
        (true, vec!["--cd".to_owned(), extra.display().to_string()]),
    ] {
        let explicit_cd = overrides.first().is_some_and(|arg| arg == "--cd");
        let server = responses::start_mock_server().await;
        let observed_owner = Arc::new(Mutex::<Option<String>>::default());
        let captured_owner = Arc::clone(&observed_owner);
        let observed_manager = manager.clone();
        let observed_source = source.clone();
        let previous_roots = known_roots.clone();
        // A matcher is needed here to inspect metadata at request time, not after exit.
        let response_mock = responses::mount_sse_once_match(
            &server,
            move |_: &wiremock::Request| {
                let owner = observed_manager
                    .list(&observed_source)
                    .expect("list shared checkouts")
                    .into_iter()
                    .find(|checkout| !previous_roots.contains(&checkout.root))
                    .and_then(|checkout| {
                        observed_manager.owner(&checkout.root).expect("read owner")
                    });
                *captured_owner.lock().expect("capture request owner") = owner;
                true
            },
            body.clone(),
        )
        .await;
        let mut command = test.cmd_with_server(&server);
        command
            .args(["--json", "--worktree", "--add-dir", "extra"])
            .args(overrides);
        if fork {
            command
                .current_dir(launcher.path())
                .arg("fork")
                .arg(&legacy_id);
        }
        let thread_id = started_thread(command.arg("describe checkout").output()?)?;
        assert_eq!(
            *observed_owner.lock().expect("read captured owner"),
            Some(thread_id)
        );
        let checkout = manager
            .list(&source)?
            .into_iter()
            .find(|checkout| !known_roots.contains(&checkout.root))
            .context("one new shared checkout")?;
        let request = response_mock.single_request();
        let context = request
            .message_input_texts("user")
            .into_iter()
            .chain(request.message_input_texts("developer"))
            .collect::<Vec<_>>()
            .join("\n");
        let checkout_cwd = AbsolutePathBuf::from_absolute_path(if explicit_cd {
            checkout.root.join("extra")
        } else {
            checkout.cwd.clone()
        })?
        .canonicalize()?;
        assert!(
            context.contains(checkout_cwd.to_string_lossy().as_ref()),
            "{context}"
        );
        assert!(
            context.contains(
                if fork {
                    launcher.path().join("extra")
                } else {
                    extra.clone()
                }
                .to_string_lossy()
                .as_ref()
            ),
            "{context}"
        );
        assert!(
            context.contains("managed checkout instructions"),
            "{context}"
        );
        assert!(
            !context.contains("uncommitted source instructions"),
            "{context}"
        );
        known_roots.push(checkout.root);
    }

    let server = responses::start_mock_server().await;
    let resume_mock = responses::mount_sse_once(&server, body).await;
    let resumed_id = started_thread(
        test.cmd_with_server(&server)
            .current_dir(legacy.cwd.join("extra"))
            .args(["--json", "resume", &legacy_id, "continue legacy"])
            .output()?,
    )?;
    assert_eq!(resumed_id, legacy_id);
    assert_eq!(legacy_manager.owner(&legacy.root)?, Some(legacy_id.clone()));
    let legacy_cwd =
        AbsolutePathBuf::from_absolute_path(legacy.cwd.join("extra"))?.canonicalize()?;
    assert!(
        resume_mock
            .single_request()
            .message_input_texts("user")
            .iter()
            .any(|text| text.contains(legacy_cwd.to_string_lossy().as_ref()))
    );
    fs::write(
        home.join("config.toml"),
        format!(
            "features.worktrees=true\n[projects.{}]\ntrust_level=\"untrusted\"\n",
            serde_json::to_string(&source)?,
        ),
    )?;
    for args in [vec!["prompt"], vec!["fork", legacy_id.as_str(), "prompt"]] {
        let output = test.cmd().arg("--worktree").args(args).output()?;
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("explicitly untrusted"));
    }
    assert!(!home.join("worktrees").exists());

    // Synthetic enterprise claims let the cloud bundle, not a local project entry,
    // supply the source's distrust decision.
    fs::write(
        home.join("auth.json"),
        serde_json::to_vec(&serde_json::json!({
            "auth_mode": "chatgpt",
            "tokens": {
                "id_token": "e30.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF9wbGFuX3R5cGUiOiJlbnRlcnByaXNlIiwiY2hhdGdwdF9hY2NvdW50X2lkIjoid29ya3NwYWNlLTEyMyIsImNoYXRncHRfdXNlcl9pZCI6InVzZXItMTIzIn19.signature",
                "access_token": "test-access-token",
                "refresh_token": "test-refresh-token",
                "account_id": "workspace-123"
            }
        }))?,
    )?;
    fs::write(
        home.join("config.toml"),
        format!(
            "features.worktrees=true\ncli_auth_credentials_store=\"file\"\nchatgpt_base_url=\"{}/backend-api\"\n",
            server.uri(),
        ),
    )?;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/backend-api/wham/config/bundle"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "config_toml": { "enterprise_managed": [{
                    "id": "source-trust", "name": "Source trust",
                    "contents": format!(
                        "[projects.{}]\ntrust_level=\"untrusted\"\n[projects.{}]\ntrust_level=\"untrusted\"\n",
                        serde_json::to_string(&source)?,
                        serde_json::to_string(&legacy.cwd.join("extra"))?,
                    )
                }] }
            })),
        )
        .mount(&server)
        .await;
    for args in [vec!["prompt"], vec!["fork", legacy_id.as_str(), "prompt"]] {
        let request_count = if args.first() == Some(&"fork") {
            // The saved checkout can choose file auth even when the launcher does not.
            fs::remove_file(source.join(".codex/config.toml"))?;
            fs::create_dir_all(legacy.cwd.join(".codex"))?;
            fs::write(
                legacy.cwd.join(".codex/config.toml"),
                "cli_auth_credentials_store=\"file\"\n",
            )?;
            git(&legacy.cwd, &["add", ".codex/config.toml"])?;
            git(
                &legacy.cwd,
                &["commit", "--quiet", "--no-gpg-sign", "-m", "file auth"],
            )?;
            fs::write(
                home.join("config.toml"),
                format!(
                    "features.worktrees=true\ncli_auth_credentials_store=\"ephemeral\"\nchatgpt_base_url=\"{}/backend-api\"\n[projects.{}]\ntrust_level=\"trusted\"\n",
                    server.uri(),
                    serde_json::to_string(&source)?,
                ),
            )?;
            let cache = home.join("cloud-config-bundle-cache.json");
            if cache.exists() {
                fs::remove_file(&cache)?;
            }
            Some(server.received_requests().await.context("requests")?.len())
        } else {
            None
        };
        let output = test
            .cmd()
            .current_dir(if request_count.is_some() {
                launcher.path()
            } else {
                &source
            })
            .env_remove("CODEX_API_KEY")
            .env_remove("OPENAI_API_KEY")
            .env_remove("CODEX_ACCESS_TOKEN")
            .args(["--json", "--worktree", "--strict-config"])
            .args(args)
            .output()?;
        assert!(
            !output.status.success()
                && String::from_utf8_lossy(&output.stderr).contains("explicitly untrusted"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        if let Some(request_count) = request_count {
            assert!(!String::from_utf8_lossy(&output.stdout).contains("thread.started"));
            let denied_manager =
                WorktreeManager::new(WorktreeSettings::for_cli(&home, /*desktop*/ None)?);
            let checkout = denied_manager
                .list(&legacy.cwd)?
                .into_iter()
                .next()
                .context("denied checkout remains for manual removal")?;
            assert_eq!(denied_manager.owner(&checkout.root)?, None);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains(
                    checkout
                        .root
                        .file_name()
                        .context("checkout name")?
                        .to_string_lossy()
                        .as_ref()
                )
            );
            assert!(stderr.contains("Remove it manually with `git worktree remove`"));
            assert!(
                server
                    .received_requests()
                    .await
                    .context("requests")?
                    .iter()
                    .skip(request_count)
                    .any(|request| request.url.path() == "/backend-api/wham/config/bundle")
            );
        } else {
            assert!(!home.join("worktrees").exists());
        }
    }
    Ok(())
}

#[test]
fn worktree_rejects_remote_defaults_before_allocation() -> anyhow::Result<()> {
    for route in ["environment", "file"] {
        let test = test_codex_exec();
        let mut command = test.cmd();
        if route == "environment" {
            command.env("CODEX_EXEC_SERVER_URL", "ws://127.0.0.1:9");
        } else {
            fs::write(
                test.home_path().join("environments.toml"),
                "default = 'remote'\n[[environments]]\nid = 'remote'\nurl = 'ws://127.0.0.1:9'\n",
            )?;
        }
        let output = command
            .args(["--worktree", "-c", "features.worktrees=true", "prompt"])
            .output()?;
        assert!(!output.status.success(), "{route}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("--worktree requires local execution"),
            "{route}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!test.home_path().join("worktrees").exists(), "{route}");
    }
    Ok(())
}

#[test]
fn worktree_rejects_disabled_features_and_ignored_config_before_allocation() -> anyhow::Result<()> {
    for (config, args, expected) in [
        (
            "",
            vec!["--ignore-user-config", "prompt"],
            "--worktree cannot be combined with --ignore-user-config",
        ),
        ("", vec!["prompt"], "--enable worktrees"),
        ("", vec!["fork", "missing"], "--enable worktrees"),
        (
            "features.worktrees = true",
            vec!["-c", "features.worktrees=false", "prompt"],
            "--enable worktrees",
        ),
    ] {
        let test = test_codex_exec();
        fs::write(test.home_path().join("config.toml"), config)?;
        let output = test.cmd().arg("--worktree").args(args).output()?;
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(expected));
        assert!(!test.home_path().join("worktrees").exists());
    }
    Ok(())
}
