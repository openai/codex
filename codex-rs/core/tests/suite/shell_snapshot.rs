use anyhow::Result;
use codex_core::TurnInputRequest;
#[cfg(unix)]
use codex_core::config::Constrained;
#[cfg(unix)]
use codex_core::config::NetworkProxySpec;
#[cfg(unix)]
use codex_core::shell::get_shell_by_model_provided_path;
#[cfg(unix)]
use codex_core::windows_sandbox::WindowsSandboxLevelExt;
use codex_features::Feature;
#[cfg(unix)]
use codex_network_proxy::NetworkProxyConfig;
#[cfg(unix)]
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
#[cfg(unix)]
use codex_protocol::config_types::TrustLevel;
#[cfg(unix)]
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
#[cfg(unix)]
use codex_protocol::models::PermissionProfileSnapshot;
#[cfg(unix)]
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
#[cfg(unix)]
use codex_protocol::protocol::EnvironmentConfig;
#[cfg(unix)]
use codex_protocol::protocol::EnvironmentConfigState;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecCommandBeginEvent;
use codex_protocol::protocol::ExecCommandEndEvent;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
#[cfg(unix)]
use core_test_support::hooks::trust_discovered_hooks;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
#[cfg(unix)]
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
#[cfg(unix)]
use core_test_support::skip_if_remote;
#[cfg(unix)]
use core_test_support::test_codex::TestCodexBuilder;
use core_test_support::test_codex::TestCodexHarness;
use core_test_support::test_codex::local_selections;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashMap;
#[cfg(target_os = "macos")]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use tokio::fs;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio::time::sleep;

#[derive(Debug)]
struct SnapshotRun {
    begin: ExecCommandBeginEvent,
    end: ExecCommandEndEvent,
    snapshot_path: PathBuf,
    snapshot_content: String,
    codex_home: PathBuf,
}

const POLICY_PATH_FOR_TEST: &str = "/codex/policy/path";
const SNAPSHOT_PATH_FOR_TEST: &str = "/codex/snapshot/path";
const SNAPSHOT_MARKER_VAR: &str = "CODEX_SNAPSHOT_POLICY_MARKER";
const SNAPSHOT_MARKER_VALUE: &str = "from_snapshot";
const POLICY_SUCCESS_OUTPUT: &str = "policy-after-snapshot";

#[derive(Debug, Default)]
struct SnapshotRunOptions {
    shell_environment_set: HashMap<String, String>,
}

async fn wait_for_snapshot(codex_home: &Path) -> Result<PathBuf> {
    let snapshot_dir = codex_home.join("shell_snapshots");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(mut entries) = fs::read_dir(&snapshot_dir).await {
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                let Some(extension) = path.extension().and_then(|ext| ext.to_str()) else {
                    continue;
                };
                if extension == "sh" || extension == "ps1" {
                    return Ok(path);
                }
            }
        }

        if Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for shell snapshot");
        }

        sleep(Duration::from_millis(25)).await;
    }
}

async fn wait_for_file_contents(path: &Path, expected: &str) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match fs::read_to_string(path).await {
            Ok(contents) if contents == expected => return Ok(()),
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }

        if Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for file {}", path.display());
        }

        sleep(Duration::from_millis(25)).await;
    }
}

fn policy_set_path_for_test() -> HashMap<String, String> {
    HashMap::from([("PATH".to_string(), POLICY_PATH_FOR_TEST.to_string())])
}

fn snapshot_override_content_for_policy_test() -> String {
    format!(
        "# Snapshot file\nexport PATH='{SNAPSHOT_PATH_FOR_TEST}'\nexport {SNAPSHOT_MARKER_VAR}='{SNAPSHOT_MARKER_VALUE}'\n"
    )
}

fn command_asserting_policy_after_snapshot() -> String {
    format!(
        "if [ \"${{{SNAPSHOT_MARKER_VAR}:-}}\" = \"{SNAPSHOT_MARKER_VALUE}\" ] && [ \"$PATH\" != \"{SNAPSHOT_PATH_FOR_TEST}\" ]; then case \":$PATH:\" in *\":{POLICY_PATH_FOR_TEST}:\"*) printf \"{POLICY_SUCCESS_OUTPUT}\" ;; *) printf \"path=%s marker=%s\" \"$PATH\" \"${{{SNAPSHOT_MARKER_VAR}:-missing}}\" ;; esac; else printf \"path=%s marker=%s\" \"$PATH\" \"${{{SNAPSHOT_MARKER_VAR}:-missing}}\"; fi"
    )
}

async fn run_snapshot_command(command: &str) -> Result<SnapshotRun> {
    run_snapshot_command_with_options(command, SnapshotRunOptions::default()).await
}

async fn run_snapshot_command_with_options(
    command: &str,
    options: SnapshotRunOptions,
) -> Result<SnapshotRun> {
    let SnapshotRunOptions {
        shell_environment_set,
    } = options;
    let builder = test_codex().with_config(move |config| {
        config
            .features
            .enable(Feature::ShellSnapshot)
            .expect("test config should allow feature update");
        config.permissions.shell_environment_policy.r#set = shell_environment_set;
    });
    let harness = TestCodexHarness::with_builder(builder).await?;
    let args = json!({
        "cmd": command,
        "yield_time_ms": 1000,
    });
    let call_id = "shell-snapshot-exec";
    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "exec_command", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_response_created("resp-2"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(harness.server(), responses).await;

    let test = harness.test();
    let codex = test.codex.clone();
    let codex_home = test.home.path().to_path_buf();
    let session_model = test.session_configured.model.clone();
    let cwd = test.config.cwd.clone();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, cwd.as_path());

    codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "run unified exec with shell snapshot".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(local_selections(cwd)),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(CollaborationMode {
                    mode: ModeKind::Default,
                    settings: Settings {
                        model: session_model,
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            }),
        )
        .await?;

    let begin = wait_for_event_match(&codex, |ev| match ev {
        EventMsg::ExecCommandBegin(ev) if ev.call_id == call_id => Some(ev.clone()),
        _ => None,
    })
    .await;
    let snapshot_path = wait_for_snapshot(&codex_home).await?;
    let snapshot_content = fs::read_to_string(&snapshot_path).await?;

    let end = wait_for_event_match(&codex, |ev| match ev {
        EventMsg::ExecCommandEnd(ev) if ev.call_id == call_id => Some(ev.clone()),
        _ => None,
    })
    .await;

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    Ok(SnapshotRun {
        begin,
        end,
        snapshot_path,
        snapshot_content,
        codex_home,
    })
}

async fn run_tool_turn_on_harness(
    harness: &TestCodexHarness,
    prompt: &str,
    call_id: &str,
    tool_name: &str,
    args: serde_json::Value,
) -> Result<ExecCommandEndEvent> {
    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, tool_name, &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_response_created("resp-2"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(harness.server(), responses).await;

    let test = harness.test();
    let codex = test.codex.clone();
    let session_model = test.session_configured.model.clone();
    let cwd = test.config.cwd.clone();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, cwd.as_path());
    codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: prompt.into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(local_selections(cwd)),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(CollaborationMode {
                    mode: ModeKind::Default,
                    settings: Settings {
                        model: session_model,
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            }),
        )
        .await?;

    wait_for_event_match(&codex, |ev| match ev {
        EventMsg::ExecCommandBegin(ev) if ev.call_id == call_id => Some(ev.clone()),
        _ => None,
    })
    .await;
    let end = wait_for_event_match(&codex, |ev| match ev {
        EventMsg::ExecCommandEnd(ev) if ev.call_id == call_id => Some(ev.clone()),
        _ => None,
    })
    .await;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
    Ok(end)
}

fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn assert_posix_snapshot_sections(snapshot: &str) {
    assert!(snapshot.contains("# Snapshot file"));
    assert!(snapshot.contains("aliases "));
    assert!(snapshot.contains("exports "));
    assert!(snapshot.contains("setopts "));
    assert!(
        snapshot.contains("PATH"),
        "snapshot should include PATH exports; snapshot={snapshot:?}"
    );
}

#[cfg(unix)]
fn shell_snapshot_v2_prewarm_builder(profile_home: &Path) -> TestCodexBuilder {
    let configured_home = profile_home.to_string_lossy().into_owned();
    let shell = get_shell_by_model_provided_path(&PathBuf::from("/bin/bash"));
    test_codex()
        .with_user_shell(shell)
        .with_config(move |config| {
            config
                .features
                .enable(Feature::UnifiedExec)
                .expect("test config should enable unified exec");
            config
                .features
                .enable(Feature::ShellSnapshotV2)
                .expect("test config should enable in-memory snapshots");
            config
                .permissions
                .set_permission_profile(PermissionProfile::Disabled)
                .expect("test config should allow unrestricted permissions");
            config
                .permissions
                .shell_environment_policy
                .ignore_default_excludes = false;
            config.permissions.shell_environment_policy.r#set =
                HashMap::from([("HOME".to_string(), configured_home)]);
        })
}

#[cfg(unix)]
async fn run_no_shell_turn(harness: &TestCodexHarness) -> Result<()> {
    let response = mount_sse_once(
        harness.server(),
        sse(vec![
            ev_response_created("no-shell"),
            ev_assistant_message("done", "done"),
            ev_completed("no-shell"),
        ]),
    )
    .await;
    let codex = &harness.test().codex;
    codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "hello".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(codex, |event| {
        assert!(
            !matches!(event, EventMsg::ExecApprovalRequest(_) | EventMsg::Error(_)),
            "unexpected event: {event:?}"
        );
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    response.single_request();
    Ok(())
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_snapshot_v2_warms_after_hooks_without_blocking_the_model() -> Result<()> {
    skip_if_remote!(Ok(()), "profile fixture uses a host-local HOME directory");
    let profile_home = tempfile::tempdir()?;
    fs::write(
        profile_home.path().join(".bashrc"),
        ". \"$HOME/prepared\"\nprintf x >> \"$HOME/captures\"\nwhile [ ! -f \"$HOME/ready\" ]; do /bin/sleep 0.01; done\nexport PROFILE_SECRET=secret\n",
    )
    .await?;
    let prepared = profile_home.path().join("prepared");
    let builder = shell_snapshot_v2_prewarm_builder(profile_home.path())
        .with_pre_build_hook(move |home| {
            std::fs::write(home.join("hooks.json"), json!({
                "hooks": { "SessionStart": [{ "hooks": [{
                    "type": "command",
                    "command": format!("printf 'profile_helper() {{ printf helper; }}\\n' > {}", shlex::try_quote(prepared.to_str().unwrap()).unwrap()),
                }] }] }
            }).to_string()).expect("write startup hook");
        })
        .with_config(trust_discovered_hooks);
    let harness = TestCodexHarness::with_auto_env_builder(builder).await?;

    // The model can finish a text-only turn while the profile is still blocked.
    run_no_shell_turn(&harness).await?;
    wait_for_file_contents(&profile_home.path().join("captures"), "x").await?;
    fs::write(profile_home.path().join("ready"), "").await?;

    let end = run_tool_turn_on_harness(
        &harness,
        "use the prewarmed in-memory shell snapshot",
        "shell-snapshot-v2-prewarm",
        "exec_command",
        json!({
            "cmd": "profile_helper; printf '|%s' \"${PROFILE_SECRET-missing}\"",
            "yield_time_ms": 1_000,
        }),
    )
    .await?;

    assert_eq!(end.exit_code, 0);
    assert!(
        harness
            .function_call_stdout("shell-snapshot-v2-prewarm")
            .await
            .trim()
            .ends_with("helper|missing")
    );
    assert_eq!(
        fs::read_to_string(profile_home.path().join("captures")).await?,
        "x"
    );
    assert!(!harness.test().home.path().join("shell_snapshots").exists());
    Ok(())
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_snapshot_v2_recovers_after_failed_prewarm() -> Result<()> {
    skip_if_remote!(Ok(()), "prewarming is local-only");
    let profile_home = tempfile::tempdir()?;
    let profile = profile_home.path().join(".bashrc");
    fs::write(&profile, "printf x > \"$HOME/failed\"\nexit 7\n").await?;
    let bash_env = profile.to_string_lossy().into_owned();
    let builder =
        shell_snapshot_v2_prewarm_builder(profile_home.path()).with_config(move |config| {
            // Ordinary login-shell fallback must read the same profile, so the output
            // distinguishes a recovered, filtered snapshot from an unfiltered fallback.
            let policy = &mut config.permissions.shell_environment_policy;
            policy.r#set.insert("BASH_ENV".to_string(), bash_env);
        });
    let harness = TestCodexHarness::with_auto_env_builder(builder).await?;
    run_no_shell_turn(&harness).await?;
    wait_for_file_contents(&profile_home.path().join("failed"), "x").await?;

    // Replace the file rather than modifying the failed capture's open script.
    let repaired = profile_home.path().join("repaired");
    fs::write(
        &repaired,
        "printf x >> \"$HOME/captures\"\nprofile_helper() { printf recovered; }\nexport PROFILE_SECRET=secret\n",
    ).await?;
    fs::rename(repaired, profile).await?;
    for call_id in ["after-failed-prewarm", "reuse-recovered-snapshot"] {
        let end = run_tool_turn_on_harness(
            &harness,
            "use the recovered snapshot",
            call_id,
            "exec_command",
            json!({"cmd": "profile_helper; printf '|%s' \"${PROFILE_SECRET-missing}\""}),
        )
        .await?;
        assert_eq!(end.exit_code, 0);
        let output = harness.function_call_stdout(call_id).await;
        assert!(output.trim().ends_with("recovered|missing"));
    }
    assert_eq!(
        fs::read_to_string(profile_home.path().join("captures")).await?,
        "x"
    );
    harness.test().codex.shutdown_and_wait().await?;
    Ok(())
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_snapshot_v2_prewarm_preserves_the_sandbox() -> Result<()> {
    skip_if_remote!(Ok(()), "profile fixture uses a host-local HOME directory");
    let profile_home = tempfile::tempdir()?;
    let outside = tempfile::tempdir()?;
    let forbidden = outside.path().join("profile-write");
    let writable_home = profile_home.path().to_path_buf().try_into()?;
    let forbidden_path = forbidden.to_string_lossy().into_owned();
    fs::write(
        profile_home.path().join(".bashrc"),
        "(printf escaped > \"$OUTSIDE\") 2>/dev/null\nprintf done > \"$HOME/captures\"\n",
    )
    .await?;
    let builder =
        shell_snapshot_v2_prewarm_builder(profile_home.path()).with_config(move |config| {
            config
                .permissions
                .set_permission_profile(PermissionProfile::workspace_write_with(
                    std::slice::from_ref(&writable_home),
                    NetworkSandboxPolicy::Restricted,
                    /*exclude_tmpdir_env_var*/ true,
                    /*exclude_slash_tmp*/ true,
                ))
                .expect("set sandboxed permissions");
            config
                .permissions
                .shell_environment_policy
                .r#set
                .insert("OUTSIDE".to_string(), forbidden_path);
            let rules = config.codex_home.join("rules");
            std::fs::create_dir_all(&rules).expect("create rules directory");
            std::fs::write(
                rules.join("default.rules"),
                "prefix_rule(pattern=[\"true\"], decision=\"allow\")\n",
            )
            .expect("allow the former warm-up sentinel");
        });
    let harness = TestCodexHarness::with_auto_env_builder(builder).await?;
    run_no_shell_turn(&harness).await?;
    wait_for_file_contents(&profile_home.path().join("captures"), "done").await?;
    harness.test().codex.shutdown_and_wait().await?;
    assert!(
        !forbidden.exists(),
        "profile startup must stay sandboxed even when true is allowed"
    );
    Ok(())
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_snapshot_v2_prewarm_stops_on_shutdown() -> Result<()> {
    skip_if_remote!(Ok(()), "profile fixture uses a host-local HOME directory");
    let profile_home = tempfile::tempdir()?;
    fs::write(
        profile_home.path().join(".bashrc"),
        "printf '%s' \"$$\" > \"$HOME/pid\"\nprintf x > \"$HOME/captures\"\nwhile :; do /bin/sleep 0.01; done\n",
    ).await?;
    let harness = TestCodexHarness::with_auto_env_builder(shell_snapshot_v2_prewarm_builder(
        profile_home.path(),
    ))
    .await?;
    run_no_shell_turn(&harness).await?;
    wait_for_file_contents(&profile_home.path().join("captures"), "x").await?;
    let pid = fs::read_to_string(profile_home.path().join("pid")).await?;

    let codex = &harness.test().codex;
    codex.submit(Op::Shutdown {}).await?;
    wait_for_event(codex, |event| matches!(event, EventMsg::ShutdownComplete)).await;
    tokio::time::timeout(Duration::from_secs(2), async {
        while tokio::process::Command::new("/bin/kill")
            .args(["-0", &pid])
            .output()
            .await?
            .status
            .success()
        {
            sleep(Duration::from_millis(25)).await;
        }
        anyhow::Ok(())
    })
    .await??;
    Ok(())
}

#[cfg(unix)]
#[test_case::test_case(|config| {
    config.active_project.trust_level = Some(TrustLevel::Untrusted);
    config.permissions.approval_policy = Constrained::allow_any(AskForApproval::UnlessTrusted);
}; "untrusted")]
#[test_case::test_case(|config| {
    config.permissions.network = Some(NetworkProxySpec::from_config_and_constraints(
        NetworkProxyConfig { enabled: true, allow_local_binding: Some(true), ..Default::default() },
        /*requirements*/ None,
        config.permissions.permission_profile(),
    ).expect("configure managed network"));
}; "managed network")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_snapshot_v2_prewarm_skips_ineligible_sessions(
    configure: fn(&mut codex_core::config::Config),
) -> Result<()> {
    skip_if_remote!(Ok(()), "profile fixture uses a host-local HOME directory");
    let profile_home = tempfile::tempdir()?;
    fs::write(
        profile_home.path().join(".bashrc"),
        "printf x > \"$HOME/captures\"\n",
    )
    .await?;
    let builder = shell_snapshot_v2_prewarm_builder(profile_home.path()).with_config(configure);
    let harness = TestCodexHarness::with_auto_env_builder(builder).await?;
    run_no_shell_turn(&harness).await?;
    harness.test().codex.shutdown_and_wait().await?;
    assert!(!profile_home.path().join("captures").exists());
    Ok(())
}

#[cfg(unix)]
#[test_case::test_case("SessionStart")]
#[test_case::test_case("UserPromptSubmit")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_snapshot_v2_does_not_warm_a_hook_stopped_turn(hook_event: &str) -> Result<()> {
    skip_if_remote!(Ok(()), "profile fixture uses a host-local HOME directory");
    let profile_home = tempfile::tempdir()?;
    fs::write(
        profile_home.path().join(".bashrc"),
        "printf x > \"$HOME/captures\"\n",
    )
    .await?;
    let hook_event = hook_event.to_string();
    let builder =
        shell_snapshot_v2_prewarm_builder(profile_home.path())
            .with_pre_build_hook(move |home| {
                std::fs::write(home.join("hooks.json"), json!({
                "hooks": { (hook_event): [{ "hooks": [{
                    "type": "command",
                    "command": "printf '%s' '{\"continue\":false,\"stopReason\":\"blocked\"}'",
                }] }] }
            }).to_string()).expect("write stopping hook");
            })
            .with_config(trust_discovered_hooks);
    let harness = TestCodexHarness::with_auto_env_builder(builder).await?;
    harness.submit("do not start the model or shell").await?;
    harness.test().codex.shutdown_and_wait().await?;
    assert!(harness.request_bodies().await.is_empty());
    assert!(!profile_home.path().join("captures").exists());
    Ok(())
}

#[cfg(unix)]
#[test_case::test_case(PermissionProfile::workspace_write(); "read only reviewer")]
#[test_case::test_case(PermissionProfile::External { network: NetworkSandboxPolicy::Enabled }; "reviewer without shell tools")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_snapshot_v2_guardian_uses_its_resolved_permissions_and_tools(
    parent_permissions: PermissionProfile,
) -> Result<()> {
    skip_if_remote!(Ok(()), "profile fixture uses a host-local HOME directory");
    let profile_home = tempfile::tempdir()?;
    fs::write(
        profile_home.path().join(".bashrc"),
        "printf capture > \"$HOME/$CODEX_THREAD_ID\"\n",
    )
    .await?;
    let builder =
        shell_snapshot_v2_prewarm_builder(profile_home.path()).with_config(move |config| {
            config
                .permissions
                .set_permission_profile(parent_permissions)
                .expect("set parent permissions");
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            let rules = config.codex_home.join("rules");
            std::fs::create_dir_all(&rules).expect("create rules directory");
            std::fs::write(
                rules.join("default.rules"),
                "prefix_rule(pattern=[\"printf\", \"parent\"], decision=\"prompt\")\n",
            )
            .expect("require review even for an unrestricted owner environment");
        });
    let harness = TestCodexHarness::with_auto_env_builder(builder).await?;
    let test = harness.test();
    let responses = mount_sse_sequence(
        harness.server(),
        vec![
            sse(vec![
                ev_function_call(
                    "parent",
                    "exec_command",
                    &json!({
                        "cmd": "printf parent",
                        "sandbox_permissions": "require_escalated",
                        "justification": "Exercise Guardian review",
                    })
                    .to_string(),
                ),
                ev_completed("parent"),
            ]),
            sse(vec![
                ev_function_call("reviewer", "exec_command", r#"{"cmd":"printf reviewed"}"#),
                ev_completed("reviewer"),
            ]),
            sse(vec![
                ev_assistant_message(
                    "assessment",
                    r#"{"risk_level":"low","user_authorization":"high","outcome":"allow","rationale":"Harmless output."}"#,
                ),
                ev_completed("assessment"),
            ]),
            sse(vec![ev_completed("done")]),
        ],
    )
    .await;
    let mut environments = local_selections(test.config.cwd.clone());
    environments.environments[0].config = EnvironmentConfigState::Ready(EnvironmentConfig {
        allow_login_shell: true,
        workspace_roots: environments.environments[0].workspace_roots.clone(),
        permission_profile: PermissionProfileSnapshot::legacy(PermissionProfile::Disabled),
        shell_environment_policy: test.config.permissions.shell_environment_policy.clone(),
        windows_sandbox_level: WindowsSandboxLevel::from_config(&test.config),
        windows_sandbox_type: test.config.permissions.windows_sandbox_type,
        windows_sandbox_private_desktop: test.config.permissions.windows_sandbox_private_desktop,
        use_legacy_landlock: test.config.features.use_legacy_landlock(),
        exec_policy: None,
        mcp_policy: None,
        network_policy: None,
        selected_capability_roots: Vec::new(),
    });
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "run the reviewed command".to_string(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(environments),
                ..Default::default()
            }),
        )
        .await?;
    wait_for_event(&test.codex, |event| {
        assert!(
            !matches!(event, EventMsg::ExecApprovalRequest(_) | EventMsg::Error(_)),
            "unexpected event: {event:?}"
        );
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.codex.shutdown_and_wait().await?;
    let requests = responses.requests();
    let guardian_requests = requests
        .iter()
        .filter(|request| request.body_json()["client_metadata"]["x-openai-subagent"] == "guardian")
        .collect::<Vec<_>>();
    assert_eq!(guardian_requests.len(), 2);
    let guardian_id = guardian_requests[0].body_json()["client_metadata"]["thread_id"]
        .as_str()
        .expect("Guardian thread ID")
        .to_string();
    assert_ne!(guardian_id, test.session_configured.thread_id.to_string());
    assert!(
        profile_home
            .path()
            .join(test.session_configured.thread_id.to_string())
            .exists()
    );
    assert!(
        !profile_home.path().join(guardian_id).exists(),
        "Guardian profile must not inherit writable owner permissions"
    );
    Ok(())
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_snapshot_v2_filters_profile_secrets_without_creating_files() -> Result<()> {
    skip_if_remote!(Ok(()), "profile fixture uses a host-local HOME directory");
    let profile_home = tempfile::tempdir()?;
    fs::write(
        profile_home.path().join(".bashrc"),
        "printf x >> \"$HOME/captures\"\nexport PATH=\"$HOME/profile-bin:$PATH\"\nexport PROFILE_ALLOWED=profile\nexport PROFILE_SECRET=secret\nprofile_helper() { printf helper; }\n",
    )
    .await?;
    fs::write(
        profile_home.path().join(".bash_profile"),
        "export PROFILE_SECRET=secret\n",
    )
    .await?;
    let configured_home = profile_home.path().to_string_lossy().into_owned();
    let shell = get_shell_by_model_provided_path(&PathBuf::from("/bin/sh"));
    let builder = test_codex()
        .with_user_shell(shell)
        .with_config(move |config| {
            config
                .features
                .enable(Feature::UnifiedExec)
                .expect("test config should enable unified exec");
            config
                .features
                .enable(Feature::ShellSnapshotV2)
                .expect("test config should enable in-memory snapshots");
            config
                .permissions
                .shell_environment_policy
                .ignore_default_excludes = false;
            config.permissions.shell_environment_policy.r#set = HashMap::from([
                ("HOME".to_string(), configured_home),
                ("PROFILE_ALLOWED".to_string(), "policy".to_string()),
            ]);
        });
    let harness = TestCodexHarness::with_auto_env_builder(builder).await?;

    for attempt in 0..2 {
        let end = run_tool_turn_on_harness(
            &harness,
            "run an in-memory shell snapshot",
            &format!("shell-snapshot-v2-{attempt}"),
            "exec_command",
            json!({
                "cmd": "profile_helper; case \":$PATH:\" in *\":$HOME/profile-bin:\"*) printf '|path';; *) printf '|missing';; esac; printf '|%s|%s' \"$PROFILE_ALLOWED\" \"${PROFILE_SECRET-missing}\"",
                "shell": "/bin/bash",
                "yield_time_ms": 1_000,
            }),
        )
        .await?;

        assert_eq!(end.exit_code, 0);
        assert_eq!(
            normalize_newlines(&end.stdout).trim(),
            "helper|path|policy|missing"
        );
    }

    assert_eq!(
        fs::read_to_string(profile_home.path().join("captures")).await?,
        "x"
    );
    assert!(!harness.test().home.path().join("shell_snapshots").exists());
    Ok(())
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_snapshot_v2_preserves_legacy_snapshots_for_user_shell() -> Result<()> {
    skip_if_remote!(Ok(()), "legacy shell snapshots require a local environment");
    let builder = test_codex().with_config(|config| {
        config
            .features
            .disable(Feature::ShellTool)
            .expect("test config should disable model shell tools");
        config
            .features
            .enable(Feature::ShellSnapshot)
            .expect("test config should enable legacy snapshots");
        config
            .features
            .enable(Feature::ShellSnapshotV2)
            .expect("test config should enable in-memory snapshots");
    });
    let harness = TestCodexHarness::with_auto_env_builder(builder).await?;
    let snapshot_path = wait_for_snapshot(harness.test().home.path()).await?;
    assert_posix_snapshot_sections(&fs::read_to_string(snapshot_path).await?);
    let codex = &harness.test().codex;
    codex
        .submit(Op::RunUserShellCommand {
            command: "printf legacy".to_string(),
            timeout_ms: None,
        })
        .await?;
    let end = wait_for_event_match(codex, |event| match event {
        EventMsg::ExecCommandEnd(event) => Some(event.clone()),
        _ => None,
    })
    .await;

    assert_eq!(end.exit_code, 0);
    assert_eq!(normalize_newlines(&end.stdout).trim(), "legacy");
    Ok(())
}

#[cfg_attr(not(target_os = "linux"), ignore)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn linux_unified_exec_uses_shell_snapshot() -> Result<()> {
    let command = "echo snapshot-linux";
    let run = run_snapshot_command(command).await?;
    let stdout = normalize_newlines(&run.end.stdout);

    assert_eq!(run.begin.command.get(1).map(String::as_str), Some("-lc"));
    assert_eq!(run.begin.command.get(2).map(String::as_str), Some(command));
    assert_eq!(run.begin.command.len(), 3);
    assert!(run.snapshot_path.starts_with(&run.codex_home));
    assert_posix_snapshot_sections(&run.snapshot_content);
    assert_eq!(run.end.exit_code, 0);
    assert!(
        stdout.contains("snapshot-linux"),
        "stdout should contain snapshot marker; stdout={stdout:?}"
    );

    Ok(())
}

#[cfg_attr(target_os = "windows", ignore)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_snapshot_preserves_shell_environment_policy_set() -> Result<()> {
    let builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::ShellSnapshot)
            .expect("test config should allow feature update");
        config.permissions.shell_environment_policy.r#set = policy_set_path_for_test();
    });
    let harness = TestCodexHarness::with_builder(builder).await?;
    let codex_home = harness.test().home.path().to_path_buf();
    run_tool_turn_on_harness(
        &harness,
        "warm up unified exec shell snapshot",
        "shell-snapshot-policy-warmup-exec",
        "exec_command",
        json!({
            "cmd": "printf warmup",
            "yield_time_ms": 1_000,
        }),
    )
    .await?;
    let snapshot_path = wait_for_snapshot(&codex_home).await?;
    fs::write(&snapshot_path, snapshot_override_content_for_policy_test()).await?;

    let command = command_asserting_policy_after_snapshot();
    let end = run_tool_turn_on_harness(
        &harness,
        "verify unified exec policy after snapshot",
        "shell-snapshot-policy-assert-exec",
        "exec_command",
        json!({
            "cmd": command,
            "yield_time_ms": 1_000,
        }),
    )
    .await?;

    assert_eq!(
        normalize_newlines(&end.stdout).trim(),
        POLICY_SUCCESS_OUTPUT
    );
    assert_eq!(end.exit_code, 0);
    assert!(snapshot_path.starts_with(codex_home));

    Ok(())
}

#[cfg_attr(target_os = "windows", ignore)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_snapshot_still_intercepts_apply_patch() -> Result<()> {
    let builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::ShellSnapshot)
            .expect("test config should allow feature update");
    });
    let harness = TestCodexHarness::with_builder(builder).await?;

    let test = harness.test();
    let codex = test.codex.clone();
    let cwd = test.config.cwd.clone();
    let codex_home = test.home.path().to_path_buf();
    let target = cwd.join("snapshot-apply.txt");

    let script = "apply_patch <<'EOF'\n*** Begin Patch\n*** Add File: snapshot-apply.txt\n+hello from snapshot\n*** End Patch\nEOF\n";
    let args = json!({
        "cmd": script,
        // Keep this above the default because intercepted apply_patch still
        // performs filesystem work that can be slow in Bazel macOS test
        // environments.
        "yield_time_ms": 5_000,
    });
    let call_id = "shell-snapshot-apply-patch";
    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "exec_command", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_response_created("resp-2"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(harness.server(), responses).await;

    let model = test.session_configured.model.clone();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, cwd.as_path());
    codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "apply patch via unified_exec with snapshot".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(local_selections(cwd.clone())),
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(CollaborationMode {
                    mode: ModeKind::Default,
                    settings: Settings {
                        model,
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            }),
        )
        .await?;

    let mut saw_patch_begin = false;
    let mut patch_end = None;
    wait_for_event(&codex, |ev| match ev {
        EventMsg::PatchApplyBegin(begin) if begin.call_id == call_id => {
            saw_patch_begin = true;
            false
        }
        EventMsg::PatchApplyEnd(end) if end.call_id == call_id => {
            patch_end = Some(end.clone());
            false
        }
        EventMsg::TurnComplete(_) => true,
        _ => false,
    })
    .await;

    let snapshot_path = wait_for_snapshot(&codex_home).await?;
    let snapshot_content = fs::read_to_string(&snapshot_path).await?;
    assert_posix_snapshot_sections(&snapshot_content);

    assert!(
        saw_patch_begin,
        "expected apply_patch to emit PatchApplyBegin"
    );
    let patch_end = patch_end.expect("expected apply_patch to emit PatchApplyEnd");
    assert!(
        patch_end.success,
        "expected apply_patch to finish successfully: stdout={:?} stderr={:?}",
        patch_end.stdout, patch_end.stderr,
    );

    wait_for_file_contents(&target, "hello from snapshot\n").await?;

    Ok(())
}

#[cfg_attr(target_os = "windows", ignore)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_snapshot_deleted_after_shutdown_with_skills() -> Result<()> {
    let builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::ShellSnapshot)
            .expect("test config should allow feature update");
    });
    let harness = TestCodexHarness::with_builder(builder).await?;
    let home = harness.test().home.clone();
    let codex_home = home.path().to_path_buf();
    let codex = harness.test().codex.clone();

    let snapshot_path = wait_for_snapshot(&codex_home).await?;
    assert!(snapshot_path.exists());

    codex.submit(Op::Shutdown {}).await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ShutdownComplete)).await;

    drop(codex);
    drop(harness);
    sleep(Duration::from_millis(150)).await;

    assert_eq!(
        snapshot_path.exists(),
        false,
        "snapshot should be removed after shutdown"
    );

    Ok(())
}

#[cfg(target_os = "macos")]
#[test_case::test_case(false; "default")]
#[test_case::test_case(true; "disabled_proxy_with_broker_configured")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn macos_unified_exec_resolves_command_from_tied_path_snapshot(
    disabled_proxy: bool,
) -> Result<()> {
    let builder = test_codex()
        .with_user_shell(get_shell_by_model_provided_path(&PathBuf::from("/bin/zsh")))
        .with_config(move |config| {
            config
                .features
                .enable(Feature::ShellSnapshot)
                .expect("test config should allow feature update");
            if disabled_proxy {
                let mut network = codex_network_proxy::NetworkProxyConfig {
                    enabled: false,
                    ..Default::default()
                };
                network.set_credential_broker_enabled(/*enabled*/ true);
                config.permissions.network = Some(
                    codex_core::config::NetworkProxySpec::from_config_and_constraints(
                        network,
                        Some(Default::default()),
                        config.permissions.permission_profile(),
                    )
                    .expect("disabled proxy configuration"),
                );
            }
        });
    let harness = TestCodexHarness::with_builder(builder).await?;
    let command_dir = harness
        .test()
        .config
        .cwd
        .join("path with spaces")
        .join("bin");
    fs::create_dir_all(&command_dir).await?;
    let command_path = command_dir.join("snapshot-only-command");
    fs::write(&command_path, "#!/bin/sh\nprintf tied-path-command").await?;
    let mut permissions = fs::metadata(&command_path).await?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&command_path, permissions).await?;

    run_tool_turn_on_harness(
        &harness,
        "warm up the tied PATH shell snapshot",
        "shell-snapshot-tied-path-warmup",
        "exec_command",
        json!({
            "cmd": "printf warmup",
            "yield_time_ms": 5_000,
        }),
    )
    .await?;
    let snapshot_path = wait_for_snapshot(harness.test().home.path()).await?;
    fs::write(
        &snapshot_path,
        format!(
            "# Snapshot file\nexport -UT PATH path=('{}' /usr/bin /bin)\n",
            command_dir.display()
        ),
    )
    .await?;

    let end = run_tool_turn_on_harness(
        &harness,
        "resolve a command from the tied PATH snapshot",
        "shell-snapshot-tied-path",
        "exec_command",
        json!({
            "cmd": "snapshot-only-command",
            "yield_time_ms": 5_000,
        }),
    )
    .await?;

    assert_eq!(
        end.exit_code, 0,
        "tied-path command failed: stderr={:?}",
        end.stderr
    );
    assert_eq!(normalize_newlines(&end.stdout).trim(), "tied-path-command");

    Ok(())
}

#[cfg_attr(not(target_os = "macos"), ignore)]
#[cfg_attr(
    target_os = "macos",
    ignore = "requires unrestricted networking on macOS"
)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn macos_unified_exec_uses_shell_snapshot() -> Result<()> {
    let command = "echo snapshot-macos";
    let run = run_snapshot_command(command).await?;

    let shell_path = run
        .begin
        .command
        .first()
        .expect("shell path recorded")
        .clone();
    assert_eq!(run.begin.command.get(1).map(String::as_str), Some("-c"));
    assert_eq!(
        run.begin.command.get(2).map(String::as_str),
        Some(". \"$0\" && exec \"$@\"")
    );
    assert_eq!(run.begin.command.get(4), Some(&shell_path));
    assert_eq!(run.begin.command.get(5).map(String::as_str), Some("-c"));
    assert_eq!(run.begin.command.last(), Some(&command.to_string()));

    assert!(run.snapshot_path.starts_with(&run.codex_home));
    assert_posix_snapshot_sections(&run.snapshot_content);
    assert_eq!(normalize_newlines(&run.end.stdout).trim(), "snapshot-macos");
    assert_eq!(run.end.exit_code, 0);

    Ok(())
}

// #[cfg_attr(not(target_os = "windows"), ignore)]
#[ignore]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn windows_unified_exec_uses_shell_snapshot() -> Result<()> {
    let command = "Write-Output snapshot-windows";
    let run = run_snapshot_command(command).await?;

    let snapshot_index = run
        .begin
        .command
        .iter()
        .position(|arg| arg.contains("shell_snapshots"))
        .expect("snapshot argument exists");
    assert!(run.begin.command.iter().any(|arg| arg == "-NoProfile"));
    assert!(
        run.begin
            .command
            .iter()
            .any(|arg| arg == "param($snapshot) . $snapshot; & @args")
    );
    assert!(snapshot_index > 0);
    assert_eq!(run.begin.command.last(), Some(&command.to_string()));

    assert!(run.snapshot_path.starts_with(&run.codex_home));
    assert!(run.snapshot_content.contains("# Snapshot file"));
    assert!(run.snapshot_content.contains("# aliases "));
    assert!(run.snapshot_content.contains("# exports "));
    assert_eq!(
        normalize_newlines(&run.end.stdout).trim(),
        "snapshot-windows"
    );
    assert_eq!(run.end.exit_code, 0);

    Ok(())
}
