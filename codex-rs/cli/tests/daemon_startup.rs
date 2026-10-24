//! Exercises daemon startup and excluded resume/fork pickers with Cargo's CLI executable.

use anyhow::Context as _;
use anyhow::Result;
use anyhow::ensure;
use std::collections::VecDeque;
use std::fs;
use std::process::Command;
use std::time::Duration;

#[tokio::test]
#[cfg(unix)]
async fn auto_daemon_start_attaches_to_shared_server() -> Result<()> {
    daemon_startup("start").await
}

#[tokio::test]
#[cfg(unix)]
async fn agents_can_offer_restart_for_managed_daemon() -> Result<()> {
    daemon_startup("agents").await
}

#[tokio::test]
#[cfg(unix)]
async fn agents_defers_launch_directory_trust_until_starting_a_task() -> Result<()> {
    daemon_startup("agents-untrusted").await
}

#[tokio::test]
#[cfg(unix)]
async fn agents_checks_trust_when_starting_after_dismissing_overview() -> Result<()> {
    daemon_startup("agents-untrusted-new").await
}

#[tokio::test]
#[cfg(unix)]
async fn discovered_managed_daemon_can_restart_with_auto_start_disabled() -> Result<()> {
    daemon_startup("discovered").await
}

#[tokio::test]
#[cfg(unix)]
async fn incompatible_daemon_can_cancel() -> Result<()> {
    daemon_startup("mismatch-cancel").await
}

#[tokio::test]
#[cfg(unix)]
async fn incompatible_daemon_can_run_embedded() -> Result<()> {
    daemon_startup("mismatch-embedded").await
}

#[tokio::test]
#[cfg(unix)]
async fn incompatible_daemon_can_restart_with_confirmed_settings() -> Result<()> {
    daemon_startup("mismatch-restart").await
}

#[tokio::test]
#[cfg(unix)]
async fn fresh_daemon_requires_confirmation_to_disable_shared_features() -> Result<()> {
    daemon_startup("mismatch-disable").await
}

#[tokio::test]
#[cfg(unix)]
async fn stopped_daemon_requires_confirmation_to_disable_persisted_features() -> Result<()> {
    daemon_startup("mismatch-disable-persisted").await
}

#[tokio::test]
async fn daemon_exclusion_survives_resume_picker() -> Result<()> {
    daemon_startup("resume").await
}

#[tokio::test]
async fn daemon_exclusion_survives_fork_picker() -> Result<()> {
    daemon_startup("fork").await
}

#[tokio::test]
async fn daemon_auto_start_preserves_bedrock_onboarding() -> Result<()> {
    daemon_startup("bedrock").await
}

#[tokio::test]
#[cfg(windows)]
async fn elevated_local_tui_uses_embedded_without_starting_daemon() -> Result<()> {
    if !codex_app_server_daemon::is_elevated()? {
        eprintln!("requires an elevated Windows runner");
        return Ok(());
    }
    for command in ["elevated", "elevated-resume", "elevated-fork"] {
        daemon_startup(command).await?;
    }
    let codex = codex_utils_cargo_bin::cargo_bin("codex")?;
    let home = tempfile::tempdir()?;
    for operation in ["start", "restart"] {
        let output = Command::new(&codex)
            .env("CODEX_HOME", home.path())
            .args(["app-server", "daemon", operation])
            .output()?;
        ensure!(
            !output.status.success(),
            "{operation} must reject elevation"
        );
        ensure!(
            String::from_utf8_lossy(&output.stderr).contains("non-elevated terminal"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    ensure!(!home.path().join("app-server-daemon").exists());
    Ok(())
}

#[tokio::test]
#[cfg(windows)]
async fn restrictive_launcher_uses_embedded_if_daemon_cannot_start() -> Result<()> {
    const CHILD: &str = "CODEX_TEST_RESTRICTIVE_DAEMON_START";
    if std::env::var_os(CHILD).is_some() {
        // The PTY creates its own breakaway-permitted job. Add Cargo's
        // restriction *inside* the PTY so it is the CLI's innermost job.
        let job = codex_utils_pty::JobObject::create_without_breakaway()?;
        let codex = codex_utils_cargo_bin::cargo_bin("codex")?;
        let mut command = tokio::process::Command::new(codex);
        command
            .arg("--no-alt-screen")
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            // This interactive CLI must inherit the PTY console rather than use
            // the background helper's CREATE_NO_WINDOW launch policy.
            .creation_flags(windows_sys::Win32::System::Threading::CREATE_SUSPENDED)
            .kill_on_drop(true);
        let mut child = command.spawn()?;
        ensure!(
            job.assign_and_resume_process(child.id().context("missing CLI pid")?)?
                .is_some(),
            "CLI job assignment failed"
        );
        let status = child.wait().await?;
        ensure!(status.success(), "CLI exited: {status}");
        return Ok(());
    }
    daemon_startup("restrictive-job").await
}

#[tokio::test]
#[cfg(unix)]
async fn bedrock_onboarding_leaves_a_running_daemon_untouched() -> Result<()> {
    daemon_startup("bedrock-running").await
}

async fn daemon_startup(command: &str) -> Result<()> {
    #[cfg(windows)]
    let elevated = codex_app_server_daemon::is_elevated()?;
    #[cfg(not(windows))]
    let elevated = false;
    let elevated_launch = command.starts_with("elevated");
    let codex = codex_utils_cargo_bin::cargo_bin("codex")?.canonicalize()?;
    let workspace = tempfile::tempdir()?;
    let workspace_path = workspace.path().canonicalize()?;
    #[cfg(unix)]
    let home = tempfile::Builder::new().tempdir_in("/tmp")?;
    #[cfg(not(unix))]
    let home = tempfile::tempdir()?;
    // Keep cold Rosetta translation outside the timed startup assertions.
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    anyhow::ensure!(
        Command::new(&codex)
            .env("CODEX_HOME", home.path())
            .arg("--version")
            .output()?
            .status
            .success(),
        "failed to prepare CLI test executable"
    );
    fs::write(
        home.path().join("config.toml"),
        format!(
            "model = \"gpt-5.6-terra\"\n\
         cli_auth_credentials_store = \"file\"\n\
         suppress_unstable_features_warning = true\nanalytics.enabled = false\n\
         windows.sandbox = \"unelevated\"\n\
         tui.disable_paste_burst = true\n\
         {}\n\
         notice.model_migrations.\"gpt-5.6-terra\" = \"gpt-6-sol\"\n\
         [projects.{}]\ntrust_level = \"{}\"\n",
            if command == "discovered" {
                "tui.status_line=[\"model\",\"thread-id\",\"thread-name\"]\nfeatures.daemon_auto_start=false"
            } else {
                ""
            },
            serde_json::to_string(&workspace_path)?,
            if command.starts_with("agents-untrusted") {
                "untrusted"
            } else {
                "trusted"
            },
        ),
    )?;
    let mismatch = command.starts_with("mismatch-");
    let persisted = command == "mismatch-disable-persisted";
    let disabling = matches!(command, "mismatch-disable" | "mismatch-disable-persisted");
    let restart = command == "mismatch-restart" || disabling;
    let bedrock_onboarding = matches!(command, "bedrock" | "bedrock-running");
    if !bedrock_onboarding {
        fs::write(
            home.path().join("auth.json"),
            r#"{"OPENAI_API_KEY":"daemon-startup-test","tokens":null,"last_refresh":null}"#,
        )?;
    }
    let mut env = std::env::vars().collect::<std::collections::HashMap<_, _>>();
    for key in [
        "CODEX_EXEC_SERVER_URL",
        "CODEX_ACCESS_TOKEN",
        "CODEX_API_KEY",
        "CODEX_CLOUD_TASKS_MODE",
        "OPENAI_API_KEY",
        "OPENAI_FEDERATION_RULE_ID",
        "OPENAI_IDENTITY_TOKEN_FILE",
        "OPENAI_WORKLOAD_IDENTITY_CONTEXT",
    ] {
        env.remove(key);
    }
    env.insert("CODEX_HOME".into(), home.path().display().to_string());
    env.insert(
        "CODEX_SQLITE_HOME".into(),
        home.path().display().to_string(),
    );
    env.insert("TERM".into(), "xterm-256color".into());
    let mut args = vec!["--no-alt-screen".to_string()];
    let mut steps: VecDeque<(&str, &[u8])> = VecDeque::new();
    if matches!(
        command,
        "start"
            | "agents"
            | "agents-untrusted"
            | "agents-untrusted-new"
            | "discovered"
            | "bedrock-running"
            | "restrictive-job"
    ) || mismatch
    {
        // A selected package with a stopped daemon avoids installing a release.
        let managed = home
            .path()
            .join("packages/app-server-daemon/current/bin")
            .join(if cfg!(windows) { "codex.exe" } else { "codex" });
        fs::create_dir_all(home.path().join("packages/app-server-daemon/current/bin"))?;
        // Hard links change the executable's ctime and invalidate Rosetta's translation cache.
        #[cfg(unix)]
        std::os::unix::fs::symlink(&codex, &managed)?;
        #[cfg(not(unix))]
        fs::hard_link(&codex, &managed)
            .or_else(|_| codex_utils_cargo_bin::copy_executable(&codex, &managed))?;
        if command != "restrictive-job" {
            fs::create_dir(home.path().join("app-server-daemon"))?;
            fs::write(
                home.path().join("app-server-daemon/settings.json"),
                if persisted {
                    r#"{"shutdownGraceSeconds":0,"updater":{"autoUpdateEnabled":false},"featureOverrides":{"api_key_model_discovery":true}}"#
                } else {
                    r#"{"shutdownGraceSeconds":0,"updater":{"autoUpdateEnabled":false}}"#
                },
            )?;
        }
    }
    let pid_file = home.path().join("app-server-daemon/daemon.pid");
    let result = async {
        let mut existing_daemon = if matches!(command, "bedrock-running" | "discovered")
            || (mismatch && !disabling)
        {
            let started = Command::new(&codex)
                .env_clear()
                .envs(&env)
                .current_dir(&workspace_path)
                .args(["app-server", "daemon", "start"])
                .output()?;
            ensure!(
                started.status.success(),
                "{}",
                String::from_utf8_lossy(&started.stderr)
            );
            Some(fs::read(&pid_file)?)
        } else {
            None
        };
        let expected = if command == "start" {
            // The draft header is visible before the session's command composer is ready.
            steps.push_back(("GPT-5.6-Terra", b"/status\r"));
            "Server:Localbackgroundserver"
        } else if command == "agents" {
            args.push("agents".into());
            steps.push_back(("Notasksyet", b"\x1b"));
            steps.push_back(("GPT-5.6-Terra", b"/daemon\r"));
            steps.push_back(("Restartbackgroundserver", b"\x1b[F\r"));
            "Restartbackgroundserver?"
        } else if command.starts_with("agents-untrusted") {
            args.push("agents".into());
            if command == "agents-untrusted-new" {
                steps.push_back(("Notasksyet", b"\x1b"));
                steps.push_back(("GPT-5.6-Terra", b"/new\r"));
            } else {
                steps.push_back(("Notasksyet", b"n"));
            }
            "entercontinue·escback"
        } else if command == "discovered" {
            args.extend(["--disable".into(), "daemon_auto_start".into()]);
            // Empty drafts need not survive a daemon restart. Renaming materializes the
            // thread; wait for the confirmed footer name, not the echoed command.
            steps.push_back(("GPT-5.6-Terra", b"/rename daemon-restart-test\r"));
            steps.push_back(("·daemon-restart-test", b"/daemon\r"));
            steps.push_back(("Restartbackgroundserver", b"2"));
            steps.push_back(("Restartbackgroundserver?", b"2\r"));
            // Offline editing preserves text but suppresses Enter. Observe the command
            // popup in a complete connected frame before submitting the retained draft.
            steps.push_back(("GPT-5.6-Terra", b"/status"));
            steps.push_back(("/statusshowcurrentsessionconfigurationandtokenusage", b"\r"));
            "Server:Localbackgroundserver"
        } else if elevated_launch || command == "restrictive-job" && elevated {
            if let Some(action) = command.strip_prefix("elevated-") {
                args.push(action.into());
                steps.push_back(("Nosessionsyet", b"\x1b"));
            }
            steps.push_back(("GPT-5.6-Terra", b"\x14"));
            "Runningasadministrator:sharedbackgroundserverdisabled."
        } else if command == "restrictive-job" {
            steps.push_back(("GPT-5.6-Terra", b"\x14"));
            "Runningwithoutthesharedbackgroundserver:thisWindowslauncher"
        } else if mismatch {
            args.extend(if disabling && !persisted {
                ["--disable".into(), "auth_elicitation".into()]
            } else {
                ["--disable".into(), "api_key_model_discovery".into()]
            });
            let input: &[u8] = match command {
                "mismatch-cancel" => b"\x03",
                "mismatch-embedded" => b"1",
                "mismatch-restart" | "mismatch-disable" | "mismatch-disable-persisted" => b"2\r",
                _ => unreachable!(),
            };
            steps.push_back(("Backgroundserverhasincompatiblefeaturesettings", input));
            if command == "mismatch-cancel" {
                "--no-daemon"
            } else {
                steps.push_back(("GPT-5.6-Terra", b"/status\r"));
                if restart { "Server:Localbackgroundserver" } else { "Model:" }
            }
        } else if bedrock_onboarding {
            "UseAmazonBedrock"
        } else {
            args.extend([command.into(), "--strict-config".into()]);
            if command == "resume" {
                let thread_id = "00000000-0000-0000-0000-000000000001";
                let sessions = home.path().join("sessions/2025/01/05");
                fs::create_dir_all(&sessions)?;
                let metadata = serde_json::json!({
                    "timestamp": "2025-01-05T12:00:00Z",
                    "type": "session_meta",
                    "payload": {
                        "session_id": thread_id, "id": thread_id,
                        "timestamp": "2025-01-05T12:00:00Z", "cwd": workspace_path,
                        "originator": "codex", "cli_version": "0.0.0",
                        "source": "cli", "model_provider": "openai"
                    }
                });
                let message = serde_json::json!({
                    "timestamp": "2025-01-05T12:00:00Z",
                    "type": "event_msg",
                    "payload": {"type": "user_message", "message": "Saved resume session", "kind": "plain"}
                });
                fs::write(
                    sessions.join(format!("rollout-2025-01-05T12-00-00-{thread_id}.jsonl")),
                    format!("{metadata}\n{message}\n"),
                )?;
                steps.push_back(("Savedresumesession", b"\r"));
            } else {
                steps.push_back(("Nosessionsyet", b"\x1b"));
            }
            steps.push_back(("GPT-5.6-Terra", b"\x14"));
            if elevated {
                "Runningasadministrator:sharedbackgroundserverdisabled."
            } else {
                "Runningwithoutthesharedbackgroundserver:--strict-config"
            }
        };
        let program = if cfg!(windows) && command == "restrictive-job" {
            env.insert("CODEX_TEST_RESTRICTIVE_DAEMON_START".into(), "1".into());
            args = vec![
                "--exact".into(),
                "restrictive_launcher_uses_embedded_if_daemon_cannot_start".into(),
                "--nocapture".into(),
            ];
            std::env::current_exe()?
        } else {
            codex.clone()
        };
        let spawned = codex_utils_pty::spawn_pty_process(
            &program.to_string_lossy(),
            &args,
            &workspace_path,
            &env,
            /*arg0*/ &None,
            codex_utils_pty::TerminalSize {
                rows: 40,
                cols: 120,
            },
            codex_utils_pty::ChildFds::Inherited(&[]),
        )
        .await?;
        let exit = spawned.exit_rx;
        let session = spawned.session;
        let writer = session.writer_sender();
        let mut stdout = spawned.stdout_rx;
        let mut output = String::new();
        let mut initial_agents_screen = String::new();
        let mut screen = vt100::Parser::new(
            /*rows*/ 40, /*cols*/ 120, /*scrollback_len*/ 0,
        );
        let mut queries: Vec<(&str, &[u8])> = vec![
            ("\x1b[6n", b"\x1b[1;1R"),
            ("\x1b[?u", b"\x1b[?0u\x1b[?1;2c"),
            ("\x1b[c", b"\x1b[?1;2c"),
            ("\x1b]10;?", b"\x1b]10;rgb:ffff/ffff/ffff\x1b\\"),
            ("\x1b]11;?", b"\x1b]11;rgb:0000/0000/0000\x1b\\"),
        ];
        // A fresh disable request includes both cold daemon startup and its confirmed restart.
        let timeout = Duration::from_secs(if disabling { 90 } else { 45 });
        let result = tokio::time::timeout(timeout, async {
            while let Some(bytes) = stdout.recv().await {
                output.push_str(&String::from_utf8_lossy(&bytes));
                screen.process(&bytes);
                for (query, reply) in &mut queries {
                    if !query.is_empty() && output.contains(*query) {
                        writer.send(reply.to_vec()).await?;
                        *query = "";
                    }
                }
                // PTY reads can split a redraw after the reconnect notice is erased but
                // before it is repainted. Only inspect completed synchronized updates.
                if command == "discovered"
                    && output
                        .rfind("\x1b[?2026l")
                        .is_none_or(|end| output[end..].contains("\x1b[?2026h"))
                {
                    continue;
                }
                let text: String = screen
                    .screen()
                    .contents()
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect();
                if let Some((ready, input)) = steps.front()
                    && text.contains(ready)
                {
                    if command == "agents-untrusted" && *ready == "Notasksyet" {
                        if !text.contains("enteropennnew") {
                            continue;
                        }
                        initial_agents_screen = screen.screen().contents();
                    }
                    if command == "discovered" {
                        if *ready == "Restartbackgroundserver?" {
                            existing_daemon = Some(fs::read(&pid_file)?);
                        } else if *ready == "GPT-5.6-Terra" {
                            // The header renders before thread/start completes. Wait for the
                            // live status line to confirm there is a session to reconnect.
                            if !screen.screen().contents().split_whitespace().any(|part| {
                                codex_protocol::ThreadId::from_string(part).is_ok()
                            }) {
                                continue;
                            }
                        }
                        // A replacement PID can appear before the TUI observes the disconnect.
                        // Wait for reconnect to clear/rebuild the transcript, so the old
                        // composer's command popup cannot make the status probe look ready.
                        if *input == b"/status" && !output.contains("\x1b[3J") {
                            continue;
                        }
                        // The old header can linger while the daemon stops. Check both
                        // the draft and its Enter against the replacement process and UI.
                        if (*input == b"/status" || *input == b"\r")
                            && (!fs::read(&pid_file).is_ok_and(|pid| Some(pid) != existing_daemon)
                                || text.contains("Reconnecting"))
                        {
                            continue;
                        }
                    }
                    if disabling && *ready == "Backgroundserverhasincompatiblefeaturesettings" {
                        existing_daemon = Some(fs::read(&pid_file)?);
                        let settings: serde_json::Value = serde_json::from_slice(&fs::read(home.path().join("app-server-daemon/settings.json"))?)?;
                        if persisted {
                            ensure!(settings["featureOverrides"]["api_key_model_discovery"] == true);
                        } else {
                            ensure!(settings["featureOverrides"]["auth_elicitation"].is_null());
                        }
                    }
                    writer.send(input.to_vec()).await?;
                    steps.pop_front();
                    output.clear();
                } else if steps.is_empty() && text.contains(expected) {
                    if command == "agents-untrusted" {
                        let transition = format!(
                            "{initial_agents_screen}\n\n--- Start a task ---\n\n{}",
                            screen.screen().contents()
                        )
                        .replace(&*workspace_path.to_string_lossy(), "[WORKSPACE]")
                        .replace(&*home.path().to_string_lossy(), "[CODEX_HOME]")
                        .replace("     tab/⇧tab filter", "tab/shift+tab filter");
                        let transition = transition.lines().map(str::trim_end).collect::<Vec<_>>().join("\n");
                        let snapshot = codex_utils_cargo_bin::find_resource!(
                            "tests/snapshots/daemon_startup__agents_overview_before_folder_consent.snap"
                        )?;
                        insta::with_settings!({snapshot_path => snapshot.parent().context("snapshot has no parent")?}, {
                            insta::assert_snapshot!("agents_overview_before_folder_consent", transition);
                        });
                    }
                    if mismatch {
                        let previous_pid = existing_daemon.as_ref().context("missing original daemon PID")?;
                        ensure!((fs::read(&pid_file)? != *previous_pid) == restart);
                        if command == "mismatch-cancel" {
                            ensure!(text.contains("Cannotusethesharedbackgroundserver:Thissessionrequiresapi_key_model_discoverytobedisabled."));
                        } else {
                            ensure!(text.contains("Server:Localbackgroundserver") == restart);
                            if restart {
                                let settings: serde_json::Value = serde_json::from_slice(
                                    &fs::read(home.path().join("app-server-daemon/settings.json"))?
                                )?;
                                let flag = if disabling && !persisted {
                                    "auth_elicitation"
                                } else {
                                    "api_key_model_discovery"
                                };
                                assert_eq!(settings["featureOverrides"], serde_json::json!({(flag): false}));
                            }
                        }
                    } else if command == "start" {
                        ensure!(text.contains("Server:Localbackgroundserver"));
                        ensure!(home.path().join("app-server-daemon/daemon.pid").exists());
                    } else if command == "discovered" {
                        let previous_pid = existing_daemon.as_ref().context("missing original daemon PID")?;
                        ensure!(fs::read(&pid_file)? != *previous_pid, "daemon PID did not change");
                    } else if let Some(existing_daemon) = &existing_daemon {
                        ensure!(fs::read(&pid_file)? == *existing_daemon);
                    } else if elevated_launch {
                        ensure!(!home.path().join("app-server-daemon").exists());
                        ensure!(!home.path().join("packages/app-server-daemon").exists());
                    } else if bedrock_onboarding || command == "restrictive-job" {
                        ensure!(!pid_file.exists());
                        if command == "restrictive-job" && !elevated {
                            let contents = screen.screen().contents();
                            let warning = contents.lines()
                                .find(|line| line.contains("Running without the shared background server:"))
                                .context("missing rendered fallback warning")?;
                            insta::assert_snapshot!("restrictive_launcher_warning", warning.trim());
                        }
                    }
                    return Ok::<_, anyhow::Error>(());
                }
            }
            anyhow::bail!(
                "TUI exited before {expected}: {}",
                screen.screen().contents()
            )
        })
        .await;
        if command == "mismatch-cancel" && matches!(result, Ok(Ok(()))) {
            let status = tokio::time::timeout(Duration::from_secs(/*secs*/ 5), exit).await??;
            ensure!(status != 0, "incompatible daemon launch must fail");
        }
        Ok::<_, anyhow::Error>((
            session,
            result.with_context(|| format!("{command} timed out waiting for {}: {}", steps.front().map_or(expected, |(ready, _)| *ready), screen.screen().contents())),
        ))
    }
    .await;
    // Always stop the detached daemon, including on timeout or failed assertions.
    let stopped = Command::new(&codex)
        .env_clear()
        .envs(&env)
        .args(["app-server", "daemon", "stop"])
        .output()?;
    let (session, result) = result?;
    session.terminate();
    result??;
    ensure!(
        stopped.status.success(),
        "{}",
        String::from_utf8_lossy(&stopped.stderr)
    );
    Ok(())
}
