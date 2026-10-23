#![cfg(test)]

//! Unit tests for the crate and its related test suites.

use super::*;
use crate::legacy_core::config::ConfigBuilder;
use crate::legacy_core::config::ConfigOverrides;
use clap::Parser;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_utils_absolute_path::test_support::PathExt;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

#[test]
fn tui_startup_future_stays_bounded() {
    let future = run_main(
        Cli::parse_from(["codex"]),
        Arg0DispatchPaths::default(),
        LoaderOverrides::default(),
        /*explicit_remote_endpoint*/ None,
    );
    let size = std::mem::size_of_val(&future);

    assert!(size < 64 * 1024, "TUI startup future is {size} bytes");
}

async fn build_config(temp_dir: &TempDir) -> std::io::Result<Config> {
    ConfigBuilder::default()
        .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
        .codex_home(temp_dir.path().to_path_buf())
        .build()
        .await
}

#[tokio::test]
async fn server_account_requirement_controls_login_screen() -> color_eyre::Result<()> {
    for requires_openai_auth in [false, true] {
        let home = TempDir::new()?;
        std::fs::write(
            home.path().join("config.toml"),
            format!(
                r#"
model_provider = "test-provider"
[model_providers.test-provider]
name = "Test provider"
base_url = "http://example.test/v1"
wire_api = "responses"
requires_openai_auth = {requires_openai_auth}
"#
            ),
        )?;
        let server_config = build_config(&home).await?;
        let mut server = AppServerSession::new(
            crate::backend_id::BackendId::new(),
            AppServerClient::InProcess(start_test_embedded_app_server(server_config).await?),
            ThreadParamsMode::Embedded,
        );
        let (login_status, account) = get_login_status(&mut server).await?;
        assert_eq!(account.requires_openai_auth, requires_openai_auth);
        assert_eq!(
            should_show_login_screen(login_status, account.requires_openai_auth),
            requires_openai_auth
        );
        assert!(!should_show_login_screen(
            LoginStatus::AuthMode(AuthMode::Chatgpt),
            account.requires_openai_auth
        ));
        server.shutdown().await?;
    }
    Ok(())
}

#[tokio::test]
async fn bedrock_setup_wizard_requires_eligible_onboarding() -> color_eyre::Result<()> {
    let shared_endpoint = RemoteAppServerEndpoint::WebSocket {
        websocket_url: "ws://127.0.0.1:4500/".to_string(),
        auth_token: None,
    };
    let enabled = "[features]\nbedrock_setup_wizard = true\n";

    for (label, config_toml, login_status, target, expected) in [
        (
            "enabled by default",
            "",
            LoginStatus::NotAuthenticated,
            AppServerTarget::Embedded,
            true,
        ),
        (
            "explicitly disabled",
            "[features]\nbedrock_setup_wizard = false\n",
            LoginStatus::NotAuthenticated,
            AppServerTarget::Embedded,
            false,
        ),
        (
            "enabled for the default provider",
            enabled,
            LoginStatus::NotAuthenticated,
            AppServerTarget::Embedded,
            true,
        ),
        (
            "explicit provider",
            "model_provider = \"openai\"\n[features]\nbedrock_setup_wizard = true\n",
            LoginStatus::NotAuthenticated,
            AppServerTarget::Embedded,
            false,
        ),
        (
            "forced ChatGPT login",
            "forced_login_method = \"chatgpt\"\n[features]\nbedrock_setup_wizard = true\n",
            LoginStatus::NotAuthenticated,
            AppServerTarget::Embedded,
            false,
        ),
        (
            "existing authentication",
            enabled,
            LoginStatus::AuthMode(AuthMode::Chatgpt),
            AppServerTarget::Embedded,
            false,
        ),
        (
            "shared local daemon",
            enabled,
            LoginStatus::NotAuthenticated,
            AppServerTarget::LocalDaemon {
                allow_embedded_fallback: true,
                endpoint: shared_endpoint.clone(),
            },
            false,
        ),
        (
            "remote app server",
            enabled,
            LoginStatus::NotAuthenticated,
            AppServerTarget::Remote {
                endpoint: shared_endpoint,
            },
            false,
        ),
    ] {
        let codex_home = TempDir::new()?;
        std::fs::write(codex_home.path().join("config.toml"), config_toml)?;
        let config = build_config(&codex_home).await?;

        assert_eq!(
            should_show_bedrock_setup_wizard(
                login_status,
                /*requires_openai_auth*/ true,
                &config,
                &target
            ),
            expected,
            "{label}"
        );
    }

    Ok(())
}

pub(crate) fn write_session_rollout(
    codex_home: &Path,
    filename_ts: &str,
    meta_rfc3339: &str,
    preview: &str,
    model_provider: &str,
    cwd: &Path,
) -> color_eyre::Result<ThreadId> {
    let uuid = Uuid::new_v4();
    let uuid_str = uuid.to_string();
    let thread_id = ThreadId::from_string(&uuid_str)?;
    let year = &filename_ts[0..4];
    let month = &filename_ts[5..7];
    let day = &filename_ts[8..10];
    let rollout_path = codex_home
        .join("sessions")
        .join(year)
        .join(month)
        .join(day)
        .join(format!("rollout-{filename_ts}-{uuid_str}.jsonl"));
    let parent = rollout_path
        .parent()
        .ok_or_else(|| color_eyre::eyre::eyre!("rollout path is missing a parent directory"))?;
    std::fs::create_dir_all(parent)?;

    let session_meta = codex_protocol::protocol::SessionMeta {
        session_id: thread_id.into(),
        id: thread_id,
        timestamp: meta_rfc3339.to_string(),
        cwd: cwd.to_path_buf(),
        originator: "codex".to_string(),
        cli_version: "0.0.0".to_string(),
        source: codex_protocol::protocol::SessionSource::Cli,
        model_provider: Some(model_provider.to_string()),
        ..Default::default()
    };
    let session_meta = serde_json::to_value(codex_protocol::protocol::SessionMetaLine {
        meta: session_meta,
        git: None,
    })?;
    let lines = [
        serde_json::json!({
            "timestamp": meta_rfc3339,
            "type": "session_meta",
            "payload": session_meta,
        })
        .to_string(),
        serde_json::json!({
            "timestamp": meta_rfc3339,
            "type": "response_item",
            "payload": {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": preview}],
            },
        })
        .to_string(),
        serde_json::json!({
            "timestamp": meta_rfc3339,
            "type": "event_msg",
            "payload": {
                "type": "user_message",
                "message": preview,
                "kind": "plain",
            },
        })
        .to_string(),
    ];
    std::fs::write(&rollout_path, lines.join("\n") + "\n")?;
    let updated_at =
        chrono::DateTime::parse_from_rfc3339(meta_rfc3339)?.with_timezone(&chrono::Utc);
    let times = std::fs::FileTimes::new().set_modified(updated_at.into());
    std::fs::OpenOptions::new()
        .append(true)
        .open(rollout_path)?
        .set_times(times)?;

    Ok(thread_id)
}

#[test]
fn startup_removes_legacy_tui_log_file() -> std::io::Result<()> {
    let temp_dir = TempDir::new()?;
    let legacy_log_dir = temp_dir.path().join("log");
    std::fs::create_dir_all(&legacy_log_dir)?;
    let legacy_log = legacy_log_dir.join(TUI_LOG_FILE_NAME);
    std::fs::write(&legacy_log, "legacy log")?;

    remove_legacy_tui_log_file(temp_dir.path());

    assert!(!legacy_log.exists());
    Ok(())
}

#[tokio::test]
async fn startup_services_use_final_cloud_managed_http_policy() -> color_eyre::Result<()> {
    for (configured_respect_system_proxy, managed_respect_system_proxy) in
        [(true, false), (false, true)]
    {
        let codex_home = TempDir::new()?;
        std::fs::write(
            codex_home.path().join("config.toml"),
            format!("[features]\nrespect_system_proxy = {configured_respect_system_proxy}\n"),
        )?;
        let prepared_environment_manager =
            EnvironmentManager::prepare_from_codex_home(codex_home.path()).await?;
        let loader_overrides = LoaderOverrides::without_managed_config_for_tests();
        let bootstrap_config = load_config_toml_with_layer_stack(
            codex_home.path(),
            /*cwd*/ None,
            Vec::new(),
            codex_config::ConfigLoadOptions {
                loader_overrides: loader_overrides.clone(),
                ..Default::default()
            },
        )
        .await?;
        let bootstrap_http_client_factory = resolve_bootstrap_http_client_factory(
            &bootstrap_config.config_toml,
            bootstrap_config
                .config_layer_stack
                .requirements()
                .feature_requirements
                .as_ref(),
        )?;
        let cloud_config_bundle =
                codex_config::test_support::CloudConfigBundleFixture::loader_with_enterprise_requirement(
                    format!("[features]\nrespect_system_proxy = {managed_respect_system_proxy}\n"),
                );
        let config = ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .loader_overrides(loader_overrides)
            .cloud_config_bundle(cloud_config_bundle)
            .build()
            .await?;
        let runtime_paths = ExecServerRuntimeOptions::new(
            std::env::current_exe()?,
            /*codex_linux_sandbox_exe*/ None,
        )?;
        let environment_manager = prepared_environment_manager
            .build(Some(runtime_paths), config.http_client_factory())?;

        assert_ne!(
            bootstrap_http_client_factory.outbound_proxy_policy(),
            config.http_client_factory().outbound_proxy_policy()
        );
        assert_eq!(
            environment_manager
                .http_client_factory()
                .outbound_proxy_policy(),
            config.http_client_factory().outbound_proxy_policy()
        );
        assert_eq!(
            config
                .auth_route_config()
                .http_client_factory()
                .outbound_proxy_policy(),
            config.http_client_factory().outbound_proxy_policy()
        );
        assert_eq!(config.respect_system_proxy, managed_respect_system_proxy);
    }

    Ok(())
}

pub(crate) async fn start_test_embedded_app_server(
    config: Config,
) -> color_eyre::Result<InProcessAppServerClient> {
    let state_db = init_state_db_for_app_server_target(&config, &AppServerTarget::Embedded).await?;
    start_embedded_app_server(
        Arg0DispatchPaths::default(),
        config,
        Vec::new(),
        LoaderOverrides::default(),
        /*strict_config*/ false,
        CloudConfigBundleLoader::default(),
        codex_feedback::CodexFeedback::new(),
        /*log_db*/ None,
        state_db,
        Arc::new(EnvironmentManager::default_for_tests()),
        Default::default(),
    )
    .await
}

#[tokio::test]
async fn startup_resume_rejects_guardian_thread() -> color_eyre::Result<()> {
    let temp_dir = TempDir::new()?;
    let config = build_config(&temp_dir).await?;
    let thread_id = write_session_rollout(
        temp_dir.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "Guardian review",
        &config.model_provider_id,
        config.cwd.as_path(),
    )?;
    let rollout_path = temp_dir
        .path()
        .join("sessions/2025/01/05")
        .join(format!("rollout-2025-01-05T12-00-00-{thread_id}.jsonl"));
    let rollout = std::fs::read_to_string(&rollout_path)?;
    let (meta, history) = rollout.split_once('\n').expect("rollout has metadata");
    let mut meta: serde_json::Value = serde_json::from_str(meta)?;
    meta["payload"]["source"] = serde_json::json!({"subagent": {"other": "guardian"}});
    meta["payload"]["parent_thread_id"] = serde_json::json!("00000000-0000-0000-0000-000000000001");
    std::fs::write(&rollout_path, format!("{meta}\n{history}"))?;

    let state_db = init_state_db_for_app_server_target(&config, &AppServerTarget::Embedded).await?;
    let mut app_server = start_app_server_for_picker(
        crate::backend_id::BackendId::new(),
        &config,
        &AppServerTarget::Embedded,
        Vec::new(),
        LoaderOverrides::without_managed_config_for_tests(),
        state_db,
        Arc::new(EnvironmentManager::default_for_tests()),
    )
    .await?;
    let error = session_resume::validate_resume_target(&mut app_server, thread_id)
        .await
        .expect_err("Guardian resume should be rejected");
    insta::assert_snapshot!(error.to_string(), @"
        Cannot resume a Guardian review thread.
        Resume its parent session instead: codex resume 00000000-0000-0000-0000-000000000001
        ");
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn startup_resume_and_fork_use_configured_or_explicit_cwd() -> color_eyre::Result<()> {
    let backend_id = crate::backend_id::BackendId::new();
    for (action, configured_mode, has_explicit_cwd, expected_directory) in [
        (CwdPromptAction::Resume, "current", false, "launch"),
        (CwdPromptAction::Resume, "session", false, "session"),
        (CwdPromptAction::Resume, "session", true, "explicit"),
        (CwdPromptAction::Fork, "current", false, "launch"),
        (CwdPromptAction::Fork, "session", false, "session"),
        (CwdPromptAction::Fork, "session", true, "explicit"),
    ] {
        let temp_dir = TempDir::new()?;
        let codex_home = temp_dir.path().join("codex-home");
        let launch_cwd = temp_dir.path().join("launch");
        let session_cwd = temp_dir.path().join("session");
        let explicit_cwd = temp_dir.path().join("explicit");
        std::fs::create_dir_all(&codex_home)?;
        std::fs::create_dir_all(&launch_cwd)?;
        std::fs::create_dir_all(&session_cwd)?;
        std::fs::create_dir_all(&explicit_cwd)?;
        std::fs::write(
            codex_home.join("config.toml"),
            format!("[tui]\nresume_cwd = \"{configured_mode}\"\n"),
        )?;
        let cwd_override = has_explicit_cwd.then_some(explicit_cwd.as_path());
        let config = ConfigBuilder::default()
            .codex_home(codex_home.clone())
            .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
            .harness_overrides(ConfigOverrides {
                cwd: Some(cwd_override.unwrap_or(launch_cwd.as_path()).to_path_buf()),
                ..Default::default()
            })
            .build()
            .await?;
        let filename_timestamp = "2025-01-05T12-00-00";
        let thread_id = write_session_rollout(
            &codex_home,
            filename_timestamp,
            "2025-01-05T12:00:00Z",
            "Saved user message",
            &config.model_provider_id,
            &session_cwd,
        )?;
        let rollout_path = codex_home
            .join("sessions/2025/01/05")
            .join(format!("rollout-{filename_timestamp}-{thread_id}.jsonl"));
        let state_db =
            init_state_db_for_app_server_target(&config, &AppServerTarget::Embedded).await?;
        let target_session = resume_picker::SessionTarget {
            backend_id,
            path: Some(rollout_path),
            thread_id,
            cwd: Some(session_cwd.clone()),
            history_mode: None,
        };
        let session_selection = match action {
            CwdPromptAction::Resume => resume_picker::SessionSelection::Resume(target_session),
            CwdPromptAction::Fork => resume_picker::SessionSelection::Fork(target_session),
        };
        let mut tui = tui::test_support::make_test_tui()?;

        let fallback_cwd = match resolve_startup_resume_or_fork_cwd(
            backend_id,
            &mut tui,
            &config,
            /*app_server*/ None,
            &session_selection,
            cwd_override,
            /*uses_remote_workspace*/ false,
            /*uses_remote_workspace_or_environment*/ false,
        )
        .await?
        {
            ResolveCwdOutcome::Continue(cwd) => cwd,
            ResolveCwdOutcome::ContinueAfterPrompt(_) => {
                panic!("configured cwd should not prompt during startup")
            }
            ResolveCwdOutcome::Exit => panic!("configured cwd should not exit startup"),
        };
        let final_config = ConfigBuilder::default()
            .codex_home(codex_home)
            .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
            .harness_overrides(ConfigOverrides {
                cwd: cwd_override.map(Path::to_path_buf),
                ..Default::default()
            })
            .fallback_cwd(fallback_cwd)
            .build()
            .await?;
        let expected_cwd = temp_dir.path().join(expected_directory);
        assert!(!session_resume::cwds_differ(
            final_config.cwd.as_path(),
            &expected_cwd,
        ));
        let mut app_server = start_app_server_for_picker(
            crate::backend_id::BackendId::new(),
            &final_config,
            &AppServerTarget::Embedded,
            Vec::new(),
            LoaderOverrides::without_managed_config_for_tests(),
            state_db,
            Arc::new(EnvironmentManager::default_for_tests()),
        )
        .await?;
        let started = match action {
            CwdPromptAction::Resume => {
                app_server
                    .resume_thread(
                        &crate::local_settings::LocalSettings::from(&final_config),
                        final_config,
                        thread_id,
                        app_server_session::ResumeModelSettings::RestoreFromThread,
                    )
                    .await?
            }
            CwdPromptAction::Fork => {
                app_server
                    .fork_thread(
                        &crate::local_settings::LocalSettings::from(&final_config),
                        final_config,
                        thread_id,
                    )
                    .await?
            }
        };

        assert!(!session_resume::cwds_differ(
            started.session.cwd.as_path(),
            &expected_cwd,
        ));
        app_server.shutdown().await?;
    }

    Ok(())
}

#[tokio::test]
async fn startup_remote_current_cwd_without_override_is_rejected() -> color_eyre::Result<()> {
    let backend_id = crate::backend_id::BackendId::new();
    let temp_dir = TempDir::new()?;
    std::fs::write(
        temp_dir.path().join("config.toml"),
        "[tui]\nresume_cwd = \"current\"\n",
    )?;
    let config = ConfigBuilder::default()
        .codex_home(temp_dir.path().to_path_buf())
        .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
        .build()
        .await?;
    let mut tui = tui::test_support::make_test_tui()?;

    let error = resolve_startup_resume_or_fork_cwd(
        backend_id,
        &mut tui,
        &config,
        /*app_server*/ None,
        &resume_picker::SessionSelection::Resume(resume_picker::SessionTarget {
            backend_id,
            path: None,
            thread_id: ThreadId::new(),
            cwd: None,
            history_mode: None,
        }),
        /*cwd_override*/ None,
        /*uses_remote_workspace*/ false,
        /*uses_remote_workspace_or_environment*/ true,
    )
    .await
    .expect_err("remote current cwd should require an explicit override");

    assert_eq!(
        error.to_string(),
        "`tui.resume_cwd = \"current\"` requires `--cd` when using a remote workspace"
    );
    let explicit = resolve_startup_resume_or_fork_cwd(
        backend_id,
        &mut tui,
        &config,
        /*app_server*/ None,
        &resume_picker::SessionSelection::Resume(resume_picker::SessionTarget {
            backend_id,
            path: None,
            thread_id: ThreadId::new(),
            cwd: None,
            history_mode: None,
        }),
        Some(Path::new("/remote-only/project")),
        /*uses_remote_workspace*/ true,
        /*uses_remote_workspace_or_environment*/ true,
    )
    .await?;
    assert!(
        matches!(explicit, ResolveCwdOutcome::Continue(Some(cwd)) if cwd == config.cwd.as_path())
    );
    Ok(())
}

#[tokio::test]
async fn startup_session_cwd_without_metadata_is_rejected() -> color_eyre::Result<()> {
    let backend_id = crate::backend_id::BackendId::new();
    let temp_dir = TempDir::new()?;
    std::fs::write(
        temp_dir.path().join("config.toml"),
        "[tui]\nresume_cwd = \"session\"\n",
    )?;
    let config = ConfigBuilder::default()
        .codex_home(temp_dir.path().to_path_buf())
        .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
        .build()
        .await?;
    let mut tui = tui::test_support::make_test_tui()?;

    let error = resolve_startup_resume_or_fork_cwd(
        backend_id,
        &mut tui,
        &config,
        /*app_server*/ None,
        &resume_picker::SessionSelection::Resume(resume_picker::SessionTarget {
            backend_id,
            path: None,
            thread_id: ThreadId::new(),
            cwd: None,
            history_mode: None,
        }),
        /*cwd_override*/ None,
        /*uses_remote_workspace*/ false,
        /*uses_remote_workspace_or_environment*/ false,
    )
    .await
    .expect_err("session cwd should require saved metadata");

    assert_eq!(
        error.to_string(),
        "failed to determine the working directory recorded for the selected session"
    );
    Ok(())
}

#[test]
fn alternate_screen_respects_terminal_compatibility_and_overrides() {
    for terminal_app_over_ssh in [false, true] {
        for (mode, expected) in [
            (AltScreenMode::Auto, !terminal_app_over_ssh),
            (AltScreenMode::Always, true),
            (AltScreenMode::Never, false),
        ] {
            assert_eq!(
                determine_alt_screen_mode(
                    /*no_alt_screen*/ false,
                    mode,
                    terminal_app_over_ssh
                ),
                expected,
            );
            assert!(!determine_alt_screen_mode(
                /*no_alt_screen*/ true,
                mode,
                terminal_app_over_ssh
            ));
        }
    }
}

#[test]
fn session_target_display_label_falls_back_to_thread_id() {
    let thread_id = ThreadId::new();
    let target = crate::resume_picker::SessionTarget {
        backend_id: crate::backend_id::BackendId::new(),
        path: None,
        thread_id,
        cwd: None,
        history_mode: None,
    };

    assert_eq!(target.display_label(), format!("thread {thread_id}"));
}

#[test]
fn resolve_remote_addr_accepts_websocket_url() {
    assert_eq!(
        resolve_remote_addr("ws://127.0.0.1:4500").expect("ws URL should normalize"),
        RemoteAppServerEndpoint::WebSocket {
            websocket_url: "ws://127.0.0.1:4500/".to_string(),
            auth_token: None,
        }
    );
}

#[test]
fn resolve_remote_addr_accepts_secure_websocket_url() {
    assert_eq!(
        resolve_remote_addr("wss://example.com:443").expect("wss URL should normalize"),
        RemoteAppServerEndpoint::WebSocket {
            websocket_url: "wss://example.com/".to_string(),
            auth_token: None,
        }
    );
}

#[test]
fn resolve_remote_addr_accepts_default_socket() -> color_eyre::Result<()> {
    let codex_home = find_codex_home().wrap_err("failed to resolve CODEX_HOME")?;
    assert_eq!(
        resolve_remote_addr("unix://")?,
        RemoteAppServerEndpoint::UnixSocket {
            socket_path: codex_app_server_client::app_server_control_socket_path(&codex_home)?,
        }
    );
    Ok(())
}

#[test]
fn resolve_remote_addr_accepts_relative_socket_path() -> color_eyre::Result<()> {
    assert_eq!(
        resolve_remote_addr("unix://codex.sock")?,
        RemoteAppServerEndpoint::UnixSocket {
            socket_path: AbsolutePathBuf::relative_to_current_dir("codex.sock")?,
        }
    );
    Ok(())
}

#[test]
fn resolve_remote_addr_accepts_absolute_socket_path() -> color_eyre::Result<()> {
    let temp_dir = TempDir::new()?;
    let socket_path = temp_dir.path().join("codex.sock");
    assert_eq!(
        resolve_remote_addr(&format!("unix://{}", socket_path.display()))?,
        RemoteAppServerEndpoint::UnixSocket {
            socket_path: AbsolutePathBuf::from_absolute_path(&socket_path)?,
        }
    );
    Ok(())
}

#[test]
fn resolve_remote_addr_rejects_invalid_remote_addresses() {
    for addr in [
        "ws://127.0.0.1",
        "wss://example.com",
        "127.0.0.1:4500",
        "https://127.0.0.1:4500",
    ] {
        let err = resolve_remote_addr(addr).expect_err("invalid remote addresses should fail");
        assert!(
            err.to_string().contains(
                "expected `ws://host:port`, `wss://host:port`, `unix://`, or `unix://PATH`"
            )
        );
    }
}

#[tokio::test]
async fn default_daemon_auto_connect_skips_missing_socket() -> color_eyre::Result<()> {
    let codex_home = TempDir::new()?;
    assert!(
        maybe_probe_default_daemon_socket(codex_home.path())
            .await
            .is_none()
    );
    Ok(())
}

#[tokio::test]
async fn default_daemon_auto_connect_probes_socket_only() -> color_eyre::Result<()> {
    let codex_home = TempDir::new()?;
    let socket_path = codex_app_server_client::app_server_control_socket_path(codex_home.path())?;
    #[cfg(windows)]
    {
        let parent = socket_path.as_path().parent().expect("socket parent");
        std::fs::create_dir_all(parent)?;
        let listener = codex_uds::UnixListener::bind(socket_path.as_path()).await?;
        assert!(
            maybe_probe_default_daemon_socket(codex_home.path())
                .await
                .is_none()
        );
        drop(listener);
        std::fs::remove_dir_all(parent)?;
    }
    codex_uds::prepare_private_socket_directory(
        socket_path.as_path().parent().expect("socket parent"),
    )
    .await?;
    let _listener = codex_uds::UnixListener::bind(socket_path.as_path()).await?;

    let expected = Some(socket_path);
    #[cfg(windows)]
    let expected = {
        // Existing Windows CI may run elevated; an elevated listener must
        // never be selected implicitly, even when its directory is private.
        let output = std::process::Command::new("powershell.exe")
                .args(["-NoProfile", "-NonInteractive", "-Command", "([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)"])
                .output()?;
        assert!(output.status.success());
        match String::from_utf8(output.stdout)?.trim() {
            "True" => None,
            "False" => expected,
            other => panic!("unexpected elevation result: {other}"),
        }
    };
    assert_eq!(
        maybe_probe_default_daemon_socket(codex_home.path()).await,
        expected
    );
    Ok(())
}

#[test]
fn app_server_target_for_launch_uses_local_daemon_for_default_socket() -> color_eyre::Result<()> {
    let socket_path = AbsolutePathBuf::relative_to_current_dir("codex.sock")?;
    let endpoint = RemoteAppServerEndpoint::UnixSocket {
        socket_path: socket_path.clone(),
    };
    for explicit in [false, true] {
        let target = app_server_target_for_launch(
            explicit.then_some(endpoint.clone()),
            Some(socket_path.clone()),
            /*can_reuse_implicit_local_daemon*/ !explicit,
            /*workload_identity_selected*/ false,
            /*exec_server_url*/ None,
        )?;
        assert_eq!(
            target,
            AppServerTarget::LocalDaemon {
                allow_embedded_fallback: !explicit,
                endpoint: endpoint.clone(),
            }
        );
        assert!(!target.uses_remote_workspace());
        assert_eq!(target.thread_params_mode(), ThreadParamsMode::Embedded);
    }
    Ok(())
}

#[test]
fn app_server_target_for_launch_preserves_executor_selection() -> color_eyre::Result<()> {
    let socket_path = AbsolutePathBuf::relative_to_current_dir("codex.sock")?;
    for executor in ["none", "ws://127.0.0.1:4501"] {
        assert_eq!(
            app_server_target_for_launch(
                /*explicit_remote_endpoint*/ None,
                Some(socket_path.clone()),
                /*can_reuse_implicit_local_daemon*/ true,
                /*workload_identity_selected*/ false,
                Some(std::ffi::OsStr::new(executor)),
            )?,
            AppServerTarget::Embedded,
        );
    }
    Ok(())
}

#[test]
fn app_server_target_for_launch_prefers_explicit_remote_endpoint() -> color_eyre::Result<()> {
    let explicit_endpoint = RemoteAppServerEndpoint::UnixSocket {
        socket_path: AbsolutePathBuf::relative_to_current_dir("explicit.sock")?,
    };
    let target = app_server_target_for_launch(
        Some(explicit_endpoint.clone()),
        Some(AbsolutePathBuf::relative_to_current_dir("default.sock")?),
        /*can_reuse_implicit_local_daemon*/ false,
        /*workload_identity_selected*/ false,
        Some(std::ffi::OsStr::new("none")),
    )?;

    assert_eq!(
        target,
        AppServerTarget::Remote {
            endpoint: explicit_endpoint,
        }
    );
    assert!(target.uses_remote_workspace());
    assert_eq!(target.thread_params_mode(), ThreadParamsMode::Remote);
    Ok(())
}

#[test]
fn app_server_target_for_launch_skips_local_daemon_when_launch_config_is_not_replayable()
-> color_eyre::Result<()> {
    let socket_path = AbsolutePathBuf::relative_to_current_dir("codex.sock")?;
    let target = app_server_target_for_launch(
        /*explicit_remote_endpoint*/ None,
        Some(socket_path),
        /*can_reuse_implicit_local_daemon*/ false,
        /*workload_identity_selected*/ false,
        /*exec_server_url*/ None,
    )?;

    assert_eq!(target, AppServerTarget::Embedded);
    Ok(())
}

#[test]
fn workload_identity_requires_an_embedded_app_server() -> color_eyre::Result<()> {
    let default_socket = AbsolutePathBuf::relative_to_current_dir("default.sock")?;
    assert_eq!(
        app_server_target_for_launch(
            /*explicit_remote_endpoint*/ None,
            Some(default_socket),
            /*can_reuse_implicit_local_daemon*/ true,
            /*workload_identity_selected*/ true,
            /*exec_server_url*/ None,
        )?,
        AppServerTarget::Embedded
    );

    let explicit_endpoint = RemoteAppServerEndpoint::UnixSocket {
        socket_path: AbsolutePathBuf::relative_to_current_dir("explicit.sock")?,
    };
    let error = app_server_target_for_launch(
        Some(explicit_endpoint),
        /*default_daemon_socket*/ None,
        /*can_reuse_implicit_local_daemon*/ false,
        /*workload_identity_selected*/ true,
        /*exec_server_url*/ None,
    )
    .expect_err("remote hosts must own workload identity");
    assert_eq!(
        error.to_string(),
        "workload identity must be configured on the remote app-server host"
    );
    Ok(())
}

#[test]
fn should_load_configured_environments_for_local_daemon() -> color_eyre::Result<()> {
    let target = AppServerTarget::LocalDaemon {
        allow_embedded_fallback: true,
        endpoint: RemoteAppServerEndpoint::UnixSocket {
            socket_path: AbsolutePathBuf::relative_to_current_dir("codex.sock")?,
        },
    };

    assert!(should_load_configured_environments(
        &LoaderOverrides::default(),
        &target,
    ));
    Ok(())
}

#[tokio::test]
async fn latest_session_lookup_params_keep_local_filters_for_embedded_sessions()
-> std::io::Result<()> {
    let temp_dir = TempDir::new()?;
    let cwd = temp_dir.path().join("project");

    let params = latest_session_lookup_params(
        /*model_provider*/ None,
        Some(cwd.as_path()),
        /*include_non_interactive*/ false,
        LatestSessionLookupMode::StateDbOnly,
    );

    assert_eq!(params.model_providers, None);
    assert_eq!(
        params.cwd,
        Some(ThreadListCwdFilter::One(cwd.to_string_lossy().to_string()))
    );
    assert!(params.use_state_db_only);

    let scan_params = latest_session_lookup_params(
        /*model_provider*/ None,
        Some(cwd.as_path()),
        /*include_non_interactive*/ false,
        LatestSessionLookupMode::ScanAndRepair,
    );
    assert!(!scan_params.use_state_db_only);
    Ok(())
}

#[tokio::test]
async fn latest_session_lookup_params_honor_explicit_provider() -> color_eyre::Result<()> {
    let temp_dir = TempDir::new()?;
    let cwd = temp_dir.path().join("project");
    let params = latest_session_lookup_params(
        Some("selected-provider".to_string()),
        Some(cwd.as_path()),
        /*include_non_interactive*/ false,
        LatestSessionLookupMode::StateDbOnly,
    );

    assert_eq!(
        params.model_providers,
        Some(vec!["selected-provider".to_string()])
    );
    assert_eq!(
        params.cwd,
        Some(ThreadListCwdFilter::One(cwd.to_string_lossy().to_string()))
    );
    Ok(())
}

#[tokio::test]
async fn latest_session_lookup_params_omit_local_filters_for_remote_sessions() -> std::io::Result<()>
{
    let params = latest_session_lookup_params(
        /*model_provider*/ None,
        /*cwd_filter*/ None,
        /*include_non_interactive*/ false,
        LatestSessionLookupMode::StateDbOnly,
    );

    assert_eq!(params.model_providers, None);
    assert_eq!(params.cwd, None);
    Ok(())
}

#[tokio::test]
async fn latest_session_lookup_params_can_include_non_interactive_sources() -> std::io::Result<()> {
    let params = latest_session_lookup_params(
        /*model_provider*/ None,
        /*cwd_filter*/ None,
        /*include_non_interactive*/ true,
        LatestSessionLookupMode::StateDbOnly,
    );

    assert_eq!(
        params.source_kinds,
        Some(vec![
            ThreadSourceKind::Cli,
            ThreadSourceKind::VsCode,
            ThreadSourceKind::Exec,
            ThreadSourceKind::AppServer,
        ])
    );
    Ok(())
}

#[tokio::test]
async fn latest_session_lookup_params_keep_explicit_cwd_filter_for_remote_sessions()
-> std::io::Result<()> {
    let cwd = Path::new("repo/on/server");

    let params = latest_session_lookup_params(
        /*model_provider*/ None,
        Some(cwd),
        /*include_non_interactive*/ false,
        LatestSessionLookupMode::StateDbOnly,
    );

    assert_eq!(params.model_providers, None);
    assert_eq!(
        params.cwd,
        Some(ThreadListCwdFilter::One(String::from("repo/on/server")))
    );
    Ok(())
}

#[tokio::test]
async fn latest_session_cwd_filter_respects_scope_options() -> std::io::Result<()> {
    let temp_dir = TempDir::new()?;
    let config = build_config(&temp_dir).await?;
    let remote_cwd = Path::new("repo/on/server");

    let local_filter = latest_session_cwd_filter(
        /*uses_remote_workspace*/ false, /*remote_cwd_override*/ None, &config,
        /*show_all*/ false,
    );
    let show_all_filter = latest_session_cwd_filter(
        /*uses_remote_workspace*/ false, /*remote_cwd_override*/ None, &config,
        /*show_all*/ true,
    );
    let remote_filter = latest_session_cwd_filter(
        /*uses_remote_workspace*/ true,
        Some(remote_cwd),
        &config,
        /*show_all*/ false,
    );

    assert_eq!(local_filter, Some(config.cwd.as_path()));
    assert_eq!(show_all_filter, None);
    assert_eq!(remote_filter, Some(remote_cwd));
    Ok(())
}

#[tokio::test]
async fn fork_last_filters_latest_session_by_cwd_unless_show_all() -> color_eyre::Result<()> {
    let temp_dir = TempDir::new()?;
    let project_cwd = temp_dir.path().join("project");
    let linked_cwd = temp_dir.path().join("linked-project");
    let other_cwd = temp_dir.path().join("other-project");
    let admin = project_cwd.join(".git/worktrees/linked");
    std::fs::create_dir_all(&admin)?;
    std::fs::create_dir_all(&linked_cwd)?;
    std::fs::create_dir_all(&other_cwd)?;
    std::fs::write(project_cwd.join(".git/HEAD"), "ref: refs/heads/main\n")?;
    std::fs::write(admin.join("commondir"), "../..\n")?;
    std::fs::write(
        admin.join("gitdir"),
        linked_cwd.join(".git").display().to_string(),
    )?;
    std::fs::write(
        linked_cwd.join(".git"),
        format!("gitdir: {}", admin.display()),
    )?;

    let mut config = ConfigBuilder::default()
        .codex_home(temp_dir.path().to_path_buf())
        .harness_overrides(ConfigOverrides {
            cwd: Some(project_cwd.clone()),
            ..Default::default()
        })
        .build()
        .await?;
    config
        .features
        .enable(codex_features::Feature::LocalThreadStoreCompression)?;
    let model_provider = config.model_provider_id.as_str();
    let project_thread_id = write_session_rollout(
        temp_dir.path(),
        "2025-01-02T10-00-00",
        "2025-01-02T10:00:00Z",
        "older project session",
        model_provider,
        &project_cwd,
    )?;
    let linked_thread_id = write_session_rollout(
        temp_dir.path(),
        "2025-01-02T11-00-00",
        "2025-01-02T11:00:00Z",
        "newer linked-worktree session",
        model_provider,
        &linked_cwd,
    )?;
    let other_thread_id = write_session_rollout(
        temp_dir.path(),
        "2025-01-02T12-00-00",
        "2025-01-02T12:00:00Z",
        "newer other project session",
        model_provider,
        &other_cwd,
    )?;
    let rollout_paths = [
        ("2025-01-02T10-00-00", project_thread_id),
        ("2025-01-02T11-00-00", linked_thread_id),
        ("2025-01-02T12-00-00", other_thread_id),
    ]
    .map(|(timestamp, thread_id)| {
        app_test_support::rollout_path(temp_dir.path(), timestamp, &thread_id.to_string())
    });

    let mut app_server = AppServerSession::new(
        crate::backend_id::BackendId::new(),
        codex_app_server_client::AppServerClient::InProcess(
            start_test_embedded_app_server(config.clone()).await?,
        ),
        ThreadParamsMode::Embedded,
    );
    // Exercise the logical .jsonl paths returned by thread/list after startup compression.
    tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 30), async {
        while rollout_paths
            .iter()
            .any(|path| path.exists() || !path.with_extension("jsonl.zst").exists())
        {
            tokio::time::sleep(std::time::Duration::from_millis(/*millis*/ 10)).await;
        }
    })
    .await?;
    let filter_cwd = latest_session_cwd_filter(
        /*uses_remote_workspace*/ false, /*remote_cwd_override*/ None, &config,
        /*show_all*/ false,
    );
    let scoped_target = lookup_latest_session_target_with_app_server(
        &mut app_server,
        &config,
        filter_cwd,
        /*include_non_interactive*/ false,
    )
    .await?
    .expect("expected project-scoped fork --last target");
    let show_all_filter_cwd = latest_session_cwd_filter(
        /*uses_remote_workspace*/ false, /*remote_cwd_override*/ None, &config,
        /*show_all*/ true,
    );
    let show_all_target = lookup_latest_session_target_with_app_server(
        &mut app_server,
        &config,
        show_all_filter_cwd,
        /*include_non_interactive*/ false,
    )
    .await?
    .expect("expected global fork --last target");
    app_server.shutdown().await?;

    assert_eq!(scoped_target.thread_id, project_thread_id);
    assert_eq!(show_all_target.thread_id, other_thread_id);
    Ok(())
}

#[tokio::test]
async fn latest_session_lookup_falls_back_for_rollout_missing_from_state_db()
-> color_eyre::Result<()> {
    let temp_dir = TempDir::new()?;
    let project_cwd = temp_dir.path().join("project");
    std::fs::create_dir_all(&project_cwd)?;
    let config = ConfigBuilder::default()
        .codex_home(temp_dir.path().to_path_buf())
        .harness_overrides(ConfigOverrides {
            cwd: Some(project_cwd.clone()),
            ..Default::default()
        })
        .build()
        .await?;
    let mut app_server = AppServerSession::new(
        crate::backend_id::BackendId::new(),
        codex_app_server_client::AppServerClient::InProcess(
            start_test_embedded_app_server(config.clone()).await?,
        ),
        ThreadParamsMode::Embedded,
    );

    // Simulate a legacy writer creating a rollout after the state DB backfill completed.
    let thread_id = write_session_rollout(
        temp_dir.path(),
        "2025-01-02T10-00-00",
        "2025-01-02T10:00:00Z",
        "legacy writer session",
        config.model_provider_id.as_str(),
        &project_cwd,
    )?;

    let target = lookup_latest_session_target_with_app_server(
        &mut app_server,
        &config,
        Some(project_cwd.as_path()),
        /*include_non_interactive*/ false,
    )
    .await?
    .expect("expected scan-and-repair fallback to find the rollout");
    app_server.shutdown().await?;

    assert_eq!(target.thread_id, thread_id);
    Ok(())
}

#[tokio::test]
async fn config_cwd_for_app_server_target_omits_cwd_for_remote_sessions() -> std::io::Result<()> {
    let remote_only_cwd = if cfg!(windows) {
        Path::new(r"C:\definitely\not\local\to\this\test")
    } else {
        Path::new("/definitely/not/local/to/this/test")
    };
    let target = AppServerTarget::Remote {
        endpoint: RemoteAppServerEndpoint::UnixSocket {
            socket_path: AbsolutePathBuf::relative_to_current_dir("codex.sock")?,
        },
    };
    let environment_manager = EnvironmentManager::default_for_tests();

    let config_cwd = config_cwd_for_app_server_target(
        Some(remote_only_cwd),
        &target,
        environment_manager
            .default_environment()
            .is_some_and(|environment| environment.is_remote()),
    )?;

    assert_eq!(config_cwd, None);
    Ok(())
}

#[tokio::test]
async fn config_cwd_for_app_server_target_canonicalizes_embedded_cli_cwd() -> std::io::Result<()> {
    let temp_dir = TempDir::new()?;
    let target = AppServerTarget::Embedded;
    let environment_manager = EnvironmentManager::default_for_tests();

    let config_cwd = config_cwd_for_app_server_target(
        Some(temp_dir.path()),
        &target,
        environment_manager
            .default_environment()
            .is_some_and(|environment| environment.is_remote()),
    )?;

    assert_eq!(
        config_cwd,
        Some(AbsolutePathBuf::from_absolute_path(dunce::canonicalize(
            temp_dir.path()
        )?)?)
    );
    Ok(())
}

#[tokio::test]
async fn config_cwd_for_app_server_target_canonicalizes_local_daemon_cli_cwd() -> std::io::Result<()>
{
    let temp_dir = TempDir::new()?;
    let target = AppServerTarget::LocalDaemon {
        allow_embedded_fallback: true,
        endpoint: RemoteAppServerEndpoint::UnixSocket {
            socket_path: AbsolutePathBuf::relative_to_current_dir("codex.sock")?,
        },
    };
    let environment_manager = EnvironmentManager::default_for_tests();

    let config_cwd = config_cwd_for_app_server_target(
        Some(temp_dir.path()),
        &target,
        environment_manager
            .default_environment()
            .is_some_and(|environment| environment.is_remote()),
    )?;

    assert_eq!(
        config_cwd,
        Some(AbsolutePathBuf::from_absolute_path(dunce::canonicalize(
            temp_dir.path()
        )?)?)
    );
    Ok(())
}

#[tokio::test]
async fn config_cwd_for_app_server_target_errors_for_missing_embedded_cli_cwd()
-> std::io::Result<()> {
    let temp_dir = TempDir::new()?;
    let missing = temp_dir.path().join("missing");
    let target = AppServerTarget::Embedded;
    let environment_manager = EnvironmentManager::default_for_tests();

    let err = config_cwd_for_app_server_target(
        Some(&missing),
        &target,
        environment_manager
            .default_environment()
            .is_some_and(|environment| environment.is_remote()),
    )
    .expect_err("missing embedded cwd should fail");

    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    Ok(())
}

#[tokio::test]
async fn config_cwd_for_app_server_target_omits_cwd_for_remote_exec_server() -> std::io::Result<()>
{
    let remote_only_cwd = if cfg!(windows) {
        Path::new(r"C:\definitely\not\local\to\this\test")
    } else {
        Path::new("/definitely/not/local/to/this/test")
    };
    let target = AppServerTarget::Embedded;
    let environment_manager = EnvironmentManager::create_for_tests(
        Some("ws://127.0.0.1:8765".to_string()),
        Some(ExecServerRuntimeOptions::new(
            std::env::current_exe().expect("current exe"),
            /*codex_linux_sandbox_exe*/ None,
        )?),
    )
    .await;

    let config_cwd = config_cwd_for_app_server_target(
        Some(remote_only_cwd),
        &target,
        environment_manager
            .default_environment()
            .is_some_and(|environment| environment.is_remote()),
    )?;

    assert_eq!(config_cwd, None);
    assert!(!projectless::has_only_local_environments(
        &environment_manager
    ));
    assert!(projectless::has_only_local_environments(
        &EnvironmentManager::default_for_tests()
    ));
    assert!(uses_remote_workspace_or_environment(
        &target,
        &environment_manager
    ));
    let local_daemon = AppServerTarget::LocalDaemon {
        allow_embedded_fallback: true,
        endpoint: RemoteAppServerEndpoint::UnixSocket {
            socket_path: AbsolutePathBuf::relative_to_current_dir("codex.sock")?,
        },
    };
    assert!(uses_remote_workspace_or_environment(
        &local_daemon,
        &environment_manager
    ));
    Ok(())
}

#[tokio::test]
async fn embedded_app_server_supports_thread_start_rpc() -> color_eyre::Result<()> {
    let temp_dir = TempDir::new()?;
    let config = build_config(&temp_dir).await?;
    let app_server = start_test_embedded_app_server(config).await?;
    let response: ThreadStartResponse = app_server
        .request_typed(ClientRequest::ThreadStart {
            request_id: RequestId::Integer(1),
            params: ThreadStartParams {
                ephemeral: Some(true),
                ..ThreadStartParams::default()
            },
        })
        .await
        .expect("thread/start should succeed");
    assert!(!response.thread.id.is_empty());

    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn resume_picker_loads_complete_paginated_and_legacy_transcripts() -> color_eyre::Result<()> {
    let temp_dir = TempDir::new()?;
    let mut config = build_config(&temp_dir).await?;
    config.terminal_resize_reflow.max_rows =
        crate::legacy_core::config::TerminalResizeReflowMaxRows::Limit(2);
    let mut app_server = AppServerSession::new(
        crate::backend_id::BackendId::new(),
        AppServerClient::InProcess(start_test_embedded_app_server(config.clone()).await?),
        ThreadParamsMode::Embedded,
    );
    let filename_ts = "2025-01-05T12-00-00";
    let rollout_line = |ordinal: usize, payload: serde_json::Value| {
        serde_json::json!({
            "timestamp": "2025-01-05T12:00:00Z",
            "type": "event_msg",
            "payload": payload,
            "ordinal": ordinal,
        })
    };
    for (history_mode, create_rollout) in [
        app_test_support::create_fake_rollout,
        app_test_support::create_fake_paginated_rollout,
    ]
    .into_iter()
    .enumerate()
    {
        let thread_id = create_rollout(
            temp_dir.path(),
            filename_ts,
            "2025-01-05T12:00:00Z",
            "message 0",
            Some(config.model_provider_id.as_str()),
            /*git_info*/ None,
        )
        .expect("create session rollout");
        let path = app_test_support::rollout_path(temp_dir.path(), filename_ts, &thread_id);
        let mut contents = std::fs::read_to_string(&path)?;
        let started = rollout_line(
            /*ordinal*/ 3,
            serde_json::json!({ "type": "task_started", "turn_id": "history-turn", "model_context_window": null }),
        );
        contents.push_str(&format!("{started}\n"));
        for index in 0..=100 {
            let message = format!("message {index}");
            let payload = if history_mode == 1 {
                serde_json::json!({
                    "type": "item_completed",
                    "thread_id": thread_id,
                    "turn_id": "history-turn",
                    "item": { "type": "UserMessage", "id": format!("user-{index}"),
                        "content": [{ "type": "text", "text": message }] },
                })
            } else {
                serde_json::json!({ "type": "user_message", "message": message })
            };
            let item = rollout_line(index + 4, payload);
            contents.push_str(&format!("{item}\n"));
        }
        if history_mode == 1 {
            for index in 0..125 {
                let item = rollout_line(
                    index + 105,
                    serde_json::json!({
                        "type": "item_completed",
                        "thread_id": thread_id,
                        "turn_id": "history-turn",
                        "item": { "type": "Reasoning", "id": format!("hidden-{index}"),
                            "summary_text": [], "raw_content": [] },
                    }),
                );
                contents.push_str(&format!("{item}\n"));
            }
        }
        std::fs::write(path, contents)?;
        let thread_id = ThreadId::from_string(&thread_id)?;
        let started = app_server
            .resume_thread(
                &crate::local_settings::LocalSettings::from(&config),
                config.clone(),
                thread_id,
                app_server_session::ResumeModelSettings::RestoreFromThread,
            )
            .await?;
        if history_mode == 1 {
            assert!(
                started
                    .turns
                    .iter()
                    .flat_map(|turn| &turn.items)
                    .any(|item| {
                        matches!(
                            item,
                            codex_app_server_protocol::ThreadItem::UserMessage { .. }
                        )
                    })
            );
            let preview = crate::resume_picker::load_transcript_preview(
                &mut app_server,
                thread_id,
                /*config*/ None,
            )
            .await?;
            assert!(!preview.is_empty());
        }
        let cells = crate::thread_transcript::load_session_transcript(
            &mut app_server,
            thread_id,
            crate::thread_transcript::RawReasoningVisibility::Hidden,
            /*config*/ None,
        )
        .await?;
        assert!(cells.len() > 100);
    }
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn embedded_app_server_start_failure_is_returned() -> color_eyre::Result<()> {
    let temp_dir = TempDir::new()?;
    let config = build_config(&temp_dir).await?;
    let result = start_embedded_app_server_with(
        Arg0DispatchPaths::default(),
        config,
        Vec::new(),
        LoaderOverrides::default(),
        /*strict_config*/ false,
        CloudConfigBundleLoader::default(),
        codex_feedback::CodexFeedback::new(),
        /*log_db*/ None,
        /*state_db*/ None,
        Arc::new(EnvironmentManager::default_for_tests()),
        Default::default(),
        |_args| async { Err(std::io::Error::other("boom")) },
    )
    .await;
    let err = match result {
        Ok(_) => panic!("startup failure should be returned"),
        Err(err) => err,
    };

    assert!(
        err.to_string()
            .contains("failed to start embedded app server"),
        "error should preserve the embedded app server startup context"
    );
    Ok(())
}

#[tokio::test]
async fn embedded_state_db_failure_is_typed_for_cli_recovery() -> color_eyre::Result<()> {
    let temp_dir = TempDir::new()?;
    let mut config = build_config(&temp_dir).await?;
    let occupied_sqlite_home = temp_dir.path().join("sqlite-home");
    std::fs::write(&occupied_sqlite_home, "occupied")?;
    let sqlite = codex_state::SqliteConfig::new_for_testing(occupied_sqlite_home.as_path().abs());
    config.sqlite = sqlite.clone();

    let err = match init_state_db_for_app_server_target(&config, &AppServerTarget::Embedded).await {
        Ok(_) => panic!("embedded startup should surface state db init failures"),
        Err(err) => err,
    };
    let startup_error = err
        .get_ref()
        .and_then(|err| err.downcast_ref::<LocalStateDbStartupError>())
        .expect("state db startup failure should retain its typed context");

    assert_eq!(
        startup_error.state_db_path(),
        sqlite.state_db_path().as_path()
    );
    assert!(
        startup_error
            .detail()
            .contains("failed to initialize state runtime"),
        "startup error should preserve the underlying state db failure"
    );
    Ok(())
}

#[tokio::test]
async fn embedded_state_db_corruption_preserves_failed_database_for_cli_recovery()
-> color_eyre::Result<()> {
    let temp_dir = TempDir::new()?;
    let mut config = build_config(&temp_dir).await?;
    let sqlite_home = temp_dir.path().join("sqlite-home");
    std::fs::create_dir_all(&sqlite_home)?;
    let sqlite = codex_state::SqliteConfig::new_for_testing(sqlite_home.as_path().abs());
    let logs_db_path = sqlite.logs_db_path();
    std::fs::write(&logs_db_path, "not a sqlite database")?;
    config.sqlite = sqlite;

    let err = match init_state_db_for_app_server_target(&config, &AppServerTarget::Embedded).await {
        Ok(_) => panic!("embedded startup should surface state db init failures"),
        Err(err) => err,
    };
    let startup_error = err
        .get_ref()
        .and_then(|err| err.downcast_ref::<LocalStateDbStartupError>())
        .expect("state db startup failure should retain its typed context");

    assert_eq!(startup_error.database_path(), logs_db_path.as_path());
    assert!(
        startup_error.is_corruption(),
        "startup error should preserve the SQLite corruption cause, got: {}",
        startup_error.detail()
    );
    Ok(())
}

#[tokio::test]
async fn config_rebuild_changes_trust_defaults_with_cwd() -> std::io::Result<()> {
    let temp_dir = TempDir::new()?;
    let codex_home = temp_dir.path().to_path_buf();
    let trusted = temp_dir.path().join("trusted");
    let untrusted = temp_dir.path().join("untrusted");
    std::fs::create_dir_all(&trusted)?;
    std::fs::create_dir_all(&untrusted)?;

    // TOML keys need escaped backslashes on Windows paths.
    let trusted_display = trusted.display().to_string().replace('\\', "\\\\");
    let untrusted_display = untrusted.display().to_string().replace('\\', "\\\\");
    let config_toml = format!(
        r#"[projects."{trusted_display}"]
trust_level = "trusted"

[projects."{untrusted_display}"]
trust_level = "untrusted"
"#
    );
    std::fs::write(temp_dir.path().join("config.toml"), config_toml)?;

    let trusted_overrides = ConfigOverrides {
        cwd: Some(trusted.clone()),
        ..Default::default()
    };
    let trusted_config = ConfigBuilder::default()
        .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
        .codex_home(codex_home.clone())
        .harness_overrides(trusted_overrides.clone())
        .build()
        .await?;
    assert_eq!(
        AskForApproval::from(trusted_config.permissions.approval_policy.value()),
        AskForApproval::OnRequest
    );

    let untrusted_overrides = ConfigOverrides {
        cwd: Some(untrusted),
        ..trusted_overrides
    };
    let untrusted_config = ConfigBuilder::default()
        .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
        .codex_home(codex_home)
        .harness_overrides(untrusted_overrides)
        .build()
        .await?;
    assert_eq!(
        AskForApproval::from(untrusted_config.permissions.approval_policy.value()),
        AskForApproval::UnlessTrusted
    );
    Ok(())
}

/// Regression: theme must be configured from the *final* config.
///
/// `run_ratatui_app` can reload config during onboarding and again
/// during session resume/fork.  The syntax theme override (stored in
/// a `OnceLock`) must use the final config's `tui_theme`, not the
/// initial one — otherwise users resuming a thread in a project with
/// a different theme get the wrong highlighting.
///
/// We verify the invariant indirectly: `validate_theme_name` (the
/// pure validation core of `set_theme_override`) must be called with
/// the *final* config's theme, and its warning must land in the
/// final config's `startup_warnings`.
#[tokio::test]
async fn theme_warning_uses_final_config() -> std::io::Result<()> {
    use crate::render::highlight::validate_theme_name;

    let temp_dir = TempDir::new()?;

    // initial_config has a valid theme — no warning.
    let initial_config = build_config(&temp_dir).await?;
    assert!(initial_config.tui_theme.is_none());

    // Simulate resume/fork reload: the final config has an invalid theme.
    let mut config = build_config(&temp_dir).await?;
    config.tui_theme = Some("bogus-theme".into());

    // Theme override must use the final config (not initial_config).
    // This mirrors the real call site in run_ratatui_app.
    if let Some(w) = validate_theme_name(config.tui_theme.as_deref(), Some(temp_dir.path())) {
        config.startup_warnings.push(w);
    }

    assert_eq!(
        config.startup_warnings.len(),
        1,
        "warning from final config's invalid theme should be present"
    );
    assert!(
        config.startup_warnings[0].contains("bogus-theme"),
        "warning should reference the final config's theme name"
    );
    Ok(())
}

#[path = "daemon_startup_tests.rs"]
mod daemon_startup_tests;
#[path = "test_backend.rs"]
pub(crate) mod test_backend;
#[path = "test_support.rs"]
pub(crate) mod test_support;
