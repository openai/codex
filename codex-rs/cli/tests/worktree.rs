//! Exercises interactive destination policy, startup and fork, and ownership before the first turn.

use anyhow::Context as _;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::Write as _;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;

fn git(cwd: &Path, args: &[&str]) -> anyhow::Result<String> {
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
    anyhow::ensure!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

// Retain only the unfinished terminal query, never the full output stream.
#[derive(Default)]
struct TerminalQueries(Vec<u8>);

impl TerminalQueries {
    fn feed(&mut self, bytes: &[u8]) -> Vec<u8> {
        let mut replies = Vec::new();
        for &byte in bytes {
            self.0.push(byte);
            if self.0 == b"\x1b[6n" {
                replies.extend_from_slice(b"\x1b[1;1R");
                self.0.clear();
            } else if self.0 == b"\x1b[c" {
                replies.extend_from_slice(b"\x1b[?1;2c");
                self.0.clear();
            } else {
                while !b"\x1b[6n".starts_with(&self.0) && !b"\x1b[c".starts_with(&self.0) {
                    self.0.remove(0);
                }
            }
        }
        replies
    }
}

#[test]
fn terminal_queries_reply_once_across_every_split() {
    let input = b"noise\x1b[6n\x1b[c\x1b[6n";
    let expected = b"\x1b[1;1R\x1b[?1;2c\x1b[1;1R";
    for split in 0..=input.len() {
        let mut queries = TerminalQueries::default();
        let mut replies = queries.feed(&input[..split]);
        replies.extend(queries.feed(&input[split..]));
        assert_eq!(replies, expected);
        assert_eq!(queries.feed(b"more output"), Vec::<u8>::new());
    }
}

async fn rejected_start(
    program: &Path,
    args: &[String],
    cwd: &Path,
    env: &HashMap<String, String>,
    expected: &str,
) -> anyhow::Result<String> {
    let spawned = codex_utils_pty::spawn_pty_process(
        &program.to_string_lossy(),
        args,
        cwd,
        env,
        /*arg0*/ &None,
        codex_utils_pty::TerminalSize {
            rows: 40,
            cols: 120,
        },
        &[],
    )
    .await?;
    let mut stdout = spawned.stdout_rx;
    let mut exit = spawned.exit_rx;
    let mut output = String::new();
    let mut queries = TerminalQueries::default();
    let result = tokio::time::timeout(Duration::from_secs(/*secs*/ 45), async {
        loop {
            tokio::select! {
                status = &mut exit, if output.contains(expected) => return Ok::<_, anyhow::Error>(status?),
                bytes = stdout.recv() => {
                    let Some(bytes) = bytes else {
                        anyhow::ensure!(output.contains(expected), "TUI exited before rejection: {output}");
                        return Ok(exit.await?);
                    };
                    output.push_str(&String::from_utf8_lossy(&bytes));
                    let replies = queries.feed(&bytes);
                    if !replies.is_empty() { spawned.session.writer_sender().send(replies).await?; }
                }
            }
        }
    }).await;
    if !matches!(&result, Ok(Ok(code)) if *code != 0) {
        let _ = writeln!(
            std::io::stderr(),
            "worktree rejection failed (timeout={}): expected {:?}; output prefix: {:?}",
            result.is_err(),
            expected.chars().take(/*n*/ 200).collect::<String>(),
            output.chars().take(/*n*/ 4000).collect::<String>()
        );
    }
    if result.is_err() {
        spawned.session.terminate();
    }
    assert_ne!(
        result.with_context(|| format!("rejection timed out: {output}"))??,
        0,
        "{output}"
    );
    Ok(output)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interactive_worktree_start_and_fork_bind_owner_before_turn() -> anyhow::Result<()> {
    let root = TempDir::new()?;
    let root = codex_utils_absolute_path::AbsolutePathBuf::from_absolute_path(root.path())?
        .canonicalize()?
        .into_path_buf();
    let home = root.join("home");
    let source = root.join("source");
    let launcher = root.join("launcher");
    for path in [&home, &source, &launcher, &launcher.join("extra")] {
        fs::create_dir(path)?;
    }
    fs::write(
        source.join("AGENTS.md"),
        "committed destination instructions",
    )?;
    let server = MockServer::start().await;
    fs::create_dir(source.join(".codex"))?;
    fs::write(
        source.join(".codex/config.toml"),
        "model = \"destination-model\"\nanalytics.enabled = false\ncli_auth_credentials_store = \"file\"\n",
    )?;
    git(&source, &["init", "--quiet"])?;
    git(&source, &["add", "."])?;
    git(
        &source,
        &["commit", "--quiet", "--no-gpg-sign", "-m", "initial"],
    )?;
    fs::write(
        home.join("config.toml"),
        format!(
            r#"
cli_auth_credentials_store = "file"
chatgpt_base_url = "{}/source/backend-api"
features.worktrees = true
check_for_update_on_startup = false
model_provider = "local"
model = "test-model"
sandbox_mode = "workspace-write"
windows.sandbox = "unelevated"
tui.disable_paste_burst = true
[otel]
metrics_exporter = {{ otlp-http = {{ endpoint = "{}/metrics", protocol = "json" }} }}
[model_providers.local]
name = "local test"
base_url = "{}/v1"
wire_api = "responses"
[projects.{}]
trust_level = "trusted"
[projects.{}]
trust_level = "trusted"
"#,
            server.uri(),
            server.uri(),
            server.uri(),
            serde_json::to_string(&source)?,
            serde_json::to_string(&launcher)?
        ),
    )?;
    fs::write(
        source.join("AGENTS.md"),
        "uncommitted launcher instructions",
    )?;
    fs::write(
        source.join(".codex/config.toml"),
        "model = \"source-model\"\nanalytics.enabled = true\n",
    )?;
    app_test_support::write_chatgpt_auth(
        &home,
        app_test_support::ChatGptAuthFixture::new("test-token")
            .account_id("workspace-123")
            .chatgpt_account_id("workspace-123")
            .chatgpt_user_id("user-123")
            .plan_type("enterprise"),
        codex_config::types::AuthCredentialsStoreMode::File,
    )?;
    let program = codex_utils_cargo_bin::cargo_bin("codex")?;
    let mut env: HashMap<String, String> = std::env::vars().collect();
    env.insert("CODEX_HOME".into(), home.display().to_string());
    env.insert("CODEX_SQLITE_HOME".into(), home.display().to_string());
    env.insert("NO_PROXY".into(), "127.0.0.1,localhost".into());
    env.insert("no_proxy".into(), "127.0.0.1,localhost".into());
    env.insert("TERM".into(), "xterm-256color".into());
    env.insert("OTEL_METRIC_EXPORT_INTERVAL".into(), "100".into());
    for key in [
        "CODEX_EXEC_SERVER_URL",
        "CODEX_ACCESS_TOKEN",
        "OPENAI_API_KEY",
        "CODEX_API_KEY",
    ] {
        env.remove(key);
    }
    let mut owner = None;
    let mut previous: Vec<String> = Vec::new();
    for (fork, explicit_cd, analytics, auth_failure) in [
        (false, false, false, false),
        (true, false, false, false),
        (true, true, false, false),
        (false, false, true, false),
        (false, false, false, true),
    ] {
        if auth_failure {
            fs::write(
                source.join(".codex/config.toml"),
                "forced_login_method = \"api\"\n",
            )?;
            git(
                &source,
                &[
                    "commit",
                    "--quiet",
                    "--no-gpg-sign",
                    "-am",
                    "require API login",
                ],
            )?;
            fs::write(source.join(".codex/config.toml"), "")?;
        }
        let (metric_tx, mut metric_rx) = tokio::sync::mpsc::unbounded_channel();
        Mock::given(wiremock::matchers::path("/metrics"))
            .respond_with(move |_: &wiremock::Request| {
                let _ = metric_tx.send(());
                ResponseTemplate::new(200).set_body_json(serde_json::json!({}))
            })
            .mount(&server)
            .await;
        Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path(
                "/source/backend-api/wham/config/bundle",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "config_toml": { "enterprise_managed": [{ "id": "test", "name": "test",
                    "contents": "developer_instructions = \"managed cloud instructions\"" }] }
            })))
            .mount(&server)
            .await;
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let response = app_test_support::create_final_assistant_message_sse_response("done")?;
        let observed_source = source.clone();
        let observed_pool = home.join("worktrees");
        let observed_previous = previous.clone();
        Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/v1/responses"))
            .respond_with(move |request: &wiremock::Request| {
                let observed = (|| {
                    let checkouts = git(&observed_source, &["worktree", "list", "--porcelain"])?;
                    let checkout = checkouts
                        .lines()
                        .filter_map(|line| line.strip_prefix("worktree "))
                        .find(|path| {
                            Path::new(path).starts_with(&observed_pool)
                                && !observed_previous.contains(&path.to_string())
                        })
                        .context("new managed checkout")?
                        .to_owned();
                    let metadata = git(
                        Path::new(&checkout),
                        &["rev-parse", "--git-path", "codex-thread.json"],
                    )?;
                    let metadata: Value =
                        serde_json::from_slice(&fs::read(Path::new(&checkout).join(metadata))?)?;
                    Ok::<_, anyhow::Error>((request.body.clone(), checkout, metadata))
                })();
                let _ = tx.send(observed);
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(response.clone())
            })
            .mount(&server)
            .await;
        let mut args = vec!["--worktree".to_owned(), "--no-alt-screen".to_owned()];
        if fork {
            args.extend([
                "--add-dir".into(),
                "extra".into(),
                "fork".into(),
                "managed-source".to_owned(),
            ]);
        } else {
            args.extend(["--cd".into(), source.display().to_string()]);
        }
        if explicit_cd {
            args.extend(["--cd".into(), source.join(".codex").display().to_string()]);
        }
        if analytics {
            args.extend(["-c".into(), "analytics.enabled=true".into()]);
        }
        args.push("describe checkout".into());
        if previous.is_empty() {
            let mut untrusted_args = args.clone();
            untrusted_args.extend([
                "-c".into(),
                format!(
                    "projects={{{}={{trust_level=\"untrusted\"}}}}",
                    serde_json::to_string(&source)?
                ),
            ]);
            rejected_start(
                &program,
                &untrusted_args,
                &launcher,
                &env,
                "cannot create a checkout from an explicitly untrusted source",
            )
            .await?;
            assert!(!home.join("worktrees").exists());
        }
        let spawned = codex_utils_pty::spawn_pty_process(
            &program.to_string_lossy(),
            &args,
            &launcher,
            &env,
            /*arg0*/ &None,
            codex_utils_pty::TerminalSize {
                rows: 40,
                cols: 120,
            },
            &[],
        )
        .await?;
        let session = spawned.session;
        let mut stdout = spawned.stdout_rx;
        let mut output = String::new();
        let mut queries = TerminalQueries::default();
        let observed = tokio::time::timeout(Duration::from_secs(/*secs*/ 45), async {
            loop {
                tokio::select! {
                    body = rx.recv() => {
                        let (body, checkout, metadata) = body.context("model request")??;
                        let body: Value = serde_json::from_slice(&body)?;
                        return Ok::<_, anyhow::Error>((body, checkout, metadata));
                    }
                    bytes = stdout.recv() => {
                        let bytes = bytes.context("TUI exited before model request")?;
                        let text = String::from_utf8_lossy(&bytes);
                        output.push_str(&text);
                        if auth_failure && output.contains("API key login is required") && output.contains("Do not use --force.") {
                            anyhow::bail!("observed required API login rejection");
                        }
                        let replies = queries.feed(&bytes);
                        if !replies.is_empty() { session.writer_sender().send(replies).await?; }
                    }
                }
            }
        }).await;
        if auth_failure {
            assert!(observed.is_ok_and(|result| result.is_err()), "{output}");
            let exit =
                tokio::time::timeout(Duration::from_secs(/*secs*/ 10), spawned.exit_rx).await??;
            assert_ne!(exit, 0);
            assert!(output.contains("API key login is required"), "{output}");
            assert!(!home.join("auth.json").exists());
            assert!(output.contains("The checkout was kept"), "{output}");
            continue;
        }
        let renamed = if !fork && matches!(observed, Ok(Ok(_))) {
            tokio::time::timeout(Duration::from_secs(/*secs*/ 10), async {
                session
                    .writer_sender()
                    .send(b"/rename managed-source\r".to_vec())
                    .await?;
                let (_, _, metadata) = observed.as_ref().unwrap().as_ref().unwrap();
                let thread_id = codex_protocol::ThreadId::from_string(
                    metadata["ownerThreadId"].as_str().context("owner id")?,
                )?;
                loop {
                    if codex_rollout::find_thread_name_by_id(&home, &thread_id)
                        .await?
                        .as_deref()
                        == Some("managed-source")
                    {
                        return Ok::<_, anyhow::Error>(());
                    }
                    tokio::select! {
                        bytes = stdout.recv() => {
                            let bytes = bytes.context("TUI exited before rename confirmation")?;
                            output.push_str(&String::from_utf8_lossy(&bytes));
                            let replies = queries.feed(&bytes);
                            if !replies.is_empty() { session.writer_sender().send(replies).await?; }
                        }
                        _ = tokio::time::sleep(Duration::from_millis(/*millis*/ 25)) => {}
                    }
                }
            })
            .await
            .context("rename timed out")
            .and_then(std::convert::identity)
        } else {
            Ok(())
        };
        if analytics {
            tokio::time::timeout(Duration::from_secs(/*secs*/ 10), metric_rx.recv())
                .await?
                .context("metrics export while the TUI is running")?;
        }
        let startup_result = observed.as_ref().map(|result| result.as_ref().map(|_| ()));
        if !matches!(startup_result, Ok(Ok(()))) || renamed.is_err() {
            // Bypass libtest capture and report before tearing down the Windows PTY.
            let _ = writeln!(
                std::io::stderr(),
                "worktree failure (fork={fork}, explicit_cd={explicit_cd}): startup={startup_result:?}, rename={renamed:?}\nTUI output: {:?}",
                output.chars().take(/*n*/ 4000).collect::<String>()
            );
        }
        if matches!(startup_result, Ok(Ok(()))) && renamed.is_ok() {
            session.writer_sender().send(b"/quit\r".to_vec()).await?;
        } else {
            session.terminate();
        }
        let exit =
            tokio::time::timeout(Duration::from_secs(/*secs*/ 10), spawned.exit_rx).await??;
        // Include shutdown output: ConPTY can retain the channel after process exit.
        let _ = tokio::time::timeout(Duration::from_secs(/*secs*/ 1), async {
            while let Some(bytes) = stdout.recv().await {
                output.push_str(&String::from_utf8_lossy(&bytes));
            }
        })
        .await;
        assert_eq!(exit, 0, "{output}");
        assert!(!output.contains("The checkout was kept"), "{output}");
        let metrics = server
            .received_requests()
            .await
            .context("requests")?
            .into_iter()
            .filter(|request| request.url.path() == "/metrics")
            .flat_map(|request| request.body)
            .collect::<Vec<_>>();
        assert!(
            if analytics {
                String::from_utf8_lossy(&metrics).contains("codex.tui.start")
            } else {
                metrics.is_empty()
            },
            "analytics={analytics}, metrics_bytes={}",
            metrics.len()
        );
        let (body, checkout, metadata) = observed
            .with_context(|| {
                format!(
                    "startup timed out: {}",
                    output.chars().take(/*n*/ 4000).collect::<String>()
                )
            })?
            .with_context(|| {
                format!(
                    "startup output: {}",
                    output.chars().take(/*n*/ 4000).collect::<String>()
                )
            })?;
        renamed.with_context(|| {
            format!(
                "rename output: {}",
                output.chars().take(/*n*/ 4000).collect::<String>()
            )
        })?;
        let next_owner = metadata["ownerThreadId"]
            .as_str()
            .context("owner bound before request")?
            .to_owned();
        assert_ne!(owner.as_ref(), Some(&next_owner));
        assert_eq!(
            git(Path::new(&checkout), &["show", "HEAD:AGENTS.md"])?,
            "committed destination instructions"
        );
        let context = body["input"]
            .as_array()
            .context("message input")?
            .iter()
            .filter_map(|item| item["content"].as_array())
            .flatten()
            .filter_map(|part| part["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if explicit_cd {
            let expected = format!("{checkout}/.codex").replace('\\', "/");
            assert!(context.replace('\\', "/").contains(&expected), "{context}");
        }
        assert!(
            context.contains("committed destination instructions"),
            "{context}"
        );
        assert!(
            !context.contains("uncommitted launcher instructions"),
            "{context}"
        );
        if fork {
            assert!(
                context.contains(&launcher.join("extra").display().to_string()),
                "{context}"
            );
        }
        assert_eq!(body["model"], "destination-model");
        assert!(context.contains("managed cloud instructions"), "{context}");
        if previous.is_empty() {
            // A legacy summary still says launcher A, but persisted turns use checkout B.
            let sqlite = codex_state::SqliteConfig::from_sqlite_home(
                codex_utils_absolute_path::AbsolutePathBuf::from_absolute_path(&home)?,
            );
            let db = sqlite.open_read_write_pool(&sqlite.state_db_path()).await?;
            sqlx::query("UPDATE threads SET cwd = ? WHERE id = ?")
                .bind(launcher.to_string_lossy().as_ref())
                .bind(&next_owner)
                .execute(&db)
                .await?;
            db.close().await;
            // B inherits trust from its primary repository; no exact trusted entry masks cloud distrust.
            let home_config = home.join("config.toml");
            let launcher_config = fs::read_to_string(&home_config)?;
            fs::write(
                &home_config,
                launcher_config.replace(
                    "cli_auth_credentials_store = \"file\"",
                    "cli_auth_credentials_store = \"ephemeral\"",
                ),
            )?;
            let cache = home.join("cloud-config-bundle-cache.json");
            if cache.exists() {
                fs::remove_file(&cache)?;
            }
            let request_count = server.received_requests().await.context("requests")?.len();
            Mock::given(wiremock::matchers::path("/source/backend-api/wham/config/bundle"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "config_toml": {"enterprise_managed": [{"id":"distrust", "name":"distrust",
                        "contents": format!("[projects.{}]\ntrust_level = \"untrusted\"\n", serde_json::to_string(&codex_config::loader::project_trust_key(Path::new(&checkout)))?)}]}
                }))).with_priority(/*priority*/ 1).mount(&server).await;
            let before = git(&source, &["worktree", "list", "--porcelain"])?;
            rejected_start(
                &program,
                &[
                    "--worktree".into(),
                    "--no-alt-screen".into(),
                    "fork".into(),
                    next_owner.clone(),
                ],
                &launcher,
                &env,
                "cannot create a checkout from an explicitly untrusted source",
            )
            .await?;
            assert!(
                server
                    .received_requests()
                    .await
                    .context("requests")?
                    .iter()
                    .skip(request_count)
                    .any(|request| request.url.path() == "/source/backend-api/wham/config/bundle")
            );
            assert_eq!(git(&source, &["worktree", "list", "--porcelain"])?, before);
            server.reset().await;
            if cache.exists() {
                fs::remove_file(&cache)?;
            }
            let count = server.received_requests().await.context("requests")?.len();
            Mock::given(wiremock::matchers::path("/source/backend-api/wham/config/bundle"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "config_toml": {"enterprise_managed": [{"id":"distrust-source", "name":"distrust-source",
                        "contents": format!("[projects.{}]\ntrust_level = \"untrusted\"\n", serde_json::to_string(&codex_config::loader::project_trust_key(&source.join(".codex")))?)}]}
                }))).mount(&server).await;
            let output = rejected_start(
                &program,
                &[
                    "--worktree".into(),
                    "--no-alt-screen".into(),
                    "--cd".into(),
                    source.join(".codex").display().to_string(),
                ],
                &launcher,
                &env,
                "cannot create a checkout from an explicitly untrusted source",
            )
            .await?;
            let requests = server.received_requests().await.context("requests")?;
            let paths = requests
                .iter()
                .skip(count)
                .map(|r| r.url.path())
                .collect::<Vec<_>>();
            assert!(paths.contains(&"/source/backend-api/wham/config/bundle"));
            assert!(!paths.contains(&"/v1/responses"));
            let after = git(&source, &["worktree", "list", "--porcelain"])?;
            let retained = after
                .lines()
                .filter_map(|line| line.strip_prefix("worktree "))
                .find(|path| {
                    !before
                        .lines()
                        .any(|line| line.strip_prefix("worktree ") == Some(*path))
                })
                .context("retained untrusted checkout")?;
            assert!(output.contains("The checkout was kept"), "{output}");
            let metadata = git(
                Path::new(retained),
                &["rev-parse", "--git-path", "codex-thread.json"],
            )?;
            assert!(!Path::new(retained).join(metadata).exists());
            previous.push(retained.to_owned());
            fs::write(&home_config, launcher_config)?;
            if cache.exists() {
                fs::remove_file(&cache)?;
            }
        }
        previous.push(checkout);
        owner = Some(next_owner);
        server.reset().await;
    }
    // Source loads its healthy uncommitted config, but the new checkout loads malformed HEAD.
    fs::write(source.join(".codex/config.toml"), "not = [valid toml")?;
    git(
        &source,
        &[
            "commit",
            "--quiet",
            "--no-gpg-sign",
            "-am",
            "invalid destination",
        ],
    )?;
    fs::write(source.join(".codex/config.toml"), "")?;
    let before = git(&source, &["worktree", "list", "--porcelain"])?;
    let output = rejected_start(
        &program,
        &[
            "--worktree".into(),
            "--no-alt-screen".into(),
            "--cd".into(),
            source.display().to_string(),
        ],
        &launcher,
        &env,
        "Do not use --force.",
    )
    .await?;
    let after = git(&source, &["worktree", "list", "--porcelain"])?;
    let retained = after
        .lines()
        .filter_map(|line| line.strip_prefix("worktree "))
        .find(|path| {
            !before
                .lines()
                .any(|line| line.strip_prefix("worktree ") == Some(*path))
        })
        .context("retained failed checkout")?;
    assert!(
        output.contains(&format!(
            "{:?}",
            Path::new(&retained.replace('/', std::path::MAIN_SEPARATOR_STR))
        )),
        "{output}"
    );
    assert!(output.contains("The checkout was kept"), "{output}");
    assert!(
        output.contains("worktree remove <checkout-path>"),
        "{output}"
    );
    Ok(())
}
