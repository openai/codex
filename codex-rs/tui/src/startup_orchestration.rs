//! Orchestrates startup while the provisional composer owns terminal input.
//!
//! Lightweight validation runs before acquiring the terminal. Once the draft is visible, slow
//! configuration and app-server initialization remain responsive to safe local editing.

use super::*;
use codex_terminal_detection::Multiplexer;
use codex_terminal_detection::TerminalName;

pub(super) async fn run_main_inner(
    mut cli: Cli,
    arg0_paths: Arg0DispatchPaths,
    loader_overrides: LoaderOverrides,
    explicit_remote_endpoint: Option<RemoteAppServerEndpoint>,
) -> std::io::Result<AppExitInfo> {
    if cli.no_daemon && explicit_remote_endpoint.is_some() {
        return Err(std::io::Error::other(
            "--no-daemon cannot be used with --remote.",
        ));
    }
    if explicit_remote_endpoint.is_some() && !cli.add_dir.is_empty() {
        return Err(std::io::Error::other(
            "--add-dir is not supported with --remote. Configure additional workspace roots on the server.",
        ));
    }
    let strict_config = cli.strict_config;
    if cli.shared.worktree {
        if explicit_remote_endpoint.is_some() {
            return Err(std::io::Error::other(
                "`--worktree` is only supported for local sessions",
            ));
        }
        if cli.fork_picker || cli.fork_last {
            return Err(std::io::Error::other(
                "`codex fork --worktree` requires an explicit session ID",
            ));
        }
    }
    let (sandbox_mode, approval_policy) = if cli.dangerously_bypass_approvals_and_sandbox {
        (
            Some(SandboxMode::DangerFullAccess),
            Some(AskForApproval::Never.to_core()),
        )
    } else {
        (
            cli.sandbox_mode.map(Into::<SandboxMode>::into),
            cli.approval_policy.map(Into::into),
        )
    };

    cli.shared
        .take_auto_review_config_overrides(&mut cli.config_overrides);

    // Map the legacy --search flag to the canonical web_search mode.
    if cli.web_search {
        cli.config_overrides
            .raw_overrides
            .push("web_search=\"live\"".to_string());
    }

    // When using `--oss`, let the bootstrapper pick the model (defaulting to
    // gpt-oss:20b) and ensure it is present locally. Also, force the built‑in
    let raw_overrides = cli.config_overrides.raw_overrides.clone();
    // `oss` model provider.
    let overrides_cli = codex_utils_cli::CliConfigOverrides { raw_overrides };
    let cli_kv_overrides = match overrides_cli.parse_overrides() {
        // Parse `-c` overrides from the CLI.
        Ok(v) => v,
        #[allow(clippy::print_stderr)]
        Err(e) => {
            eprintln!("Error parsing -c overrides: {e}");
            std::process::exit(1);
        }
    };
    if explicit_remote_endpoint.is_some()
        && cli_kv_overrides.iter().any(|(key, value)| {
            key == "sandbox_workspace_write.writable_roots"
                || (key == "sandbox_workspace_write" && value.get("writable_roots").is_some())
        })
    {
        return Err(std::io::Error::other(
            "sandbox_workspace_write.writable_roots overrides are not supported with --remote. Configure additional workspace roots on the server.",
        ));
    }

    // we load config.toml here to determine project state.
    #[allow(clippy::print_stderr)]
    let codex_home = match find_codex_home() {
        Ok(codex_home) => codex_home.to_path_buf(),
        Err(err) => {
            eprintln!("Error finding codex home: {err}");
            std::process::exit(1);
        }
    };

    let mut launch_loader_overrides = loader_overrides.clone();
    if let Some(profile_v2) = cli.config_profile_v2.as_ref() {
        let user_config_path = resolve_profile_v2_config_path(&codex_home, profile_v2);
        launch_loader_overrides.user_config_path = Some(user_config_path);
        launch_loader_overrides.user_config_profile = Some(profile_v2.clone());
    }
    let embedded_network_policy =
        codex_app_server_client::EmbeddedNetworkPolicy::load(&launch_loader_overrides).await;
    let workload_identity_selected = is_workload_identity_selected();

    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        let validation_target = app_server_target_for_launch(
            explicit_remote_endpoint.clone(),
            /*default_daemon_socket*/ None,
            /*can_reuse_implicit_local_daemon*/ false,
            workload_identity_selected,
            std::env::var_os(codex_exec_server::CODEX_EXEC_SERVER_URL_ENV_VAR).as_deref(),
        )?;
        let validation_environment_manager =
            if should_load_configured_environments(&loader_overrides, &validation_target) {
                EnvironmentManager::prepare_from_codex_home(&codex_home).await
            } else {
                EnvironmentManager::prepare_from_env().await
            }
            .map_err(std::io::Error::other)?;
        let validation_cwd = config_cwd_for_app_server_target(
            cli.cwd.as_deref(),
            &validation_target,
            validation_environment_manager.default_environment_is_remote(),
        )?;
        let mut validation_loader_overrides = launch_loader_overrides.clone();
        validation_loader_overrides.ignore_login_requirements =
            validation_target.uses_remote_workspace();
        let validation_bootstrap = load_bootstrap_config_or_exit(
            &codex_home,
            validation_cwd.as_ref(),
            cli_kv_overrides.clone(),
            validation_loader_overrides.clone(),
            strict_config,
            CloudConfigBundleLoader::default(),
        )
        .await;
        let validation_cloud_config_bundle = if workload_identity_selected {
            cloud_config_bundle_for_app_server_target(
                &validation_target,
                &validation_bootstrap,
                &codex_home,
                &embedded_network_policy,
            )
            .await?
        } else {
            CloudConfigBundleLoader::default()
        };
        load_config_or_exit(
            cli_kv_overrides.clone(),
            ConfigOverrides {
                model: cli.model.clone(),
                approval_policy,
                sandbox_mode,
                cwd: validation_cwd.map(AbsolutePathBuf::into_path_buf),
                model_provider: cli
                    .oss
                    .then(|| {
                        resolve_oss_provider(
                            cli.oss_provider.as_deref(),
                            &validation_bootstrap.config_toml,
                        )
                    })
                    .flatten(),
                bypass_hook_trust: cli.bypass_hook_trust.then_some(true),
                additional_writable_roots: cli.add_dir.clone(),
                ..Default::default()
            },
            validation_loader_overrides,
            validation_cloud_config_bundle,
            strict_config,
        )
        .await;
    }

    let mut daemon_exclusion = daemon_startup::exclusion(
        &cli,
        &cli_kv_overrides,
        &launch_loader_overrides,
        workload_identity_selected,
        std::env::var_os(codex_exec_server::CODEX_EXEC_SERVER_URL_ENV_VAR).as_deref(),
    );
    let reuse_implicit_local_daemon = daemon_exclusion.is_none();
    let search_only_config_override = !workload_identity_selected
        && cli.web_search
        && startup_preflight::has_only_search_config_override(&cli_kv_overrides)
        && loader_overrides_are_default(&launch_loader_overrides)
        && !strict_config
        && !cli.bypass_hook_trust;
    let initial_screen = if cli.resume_picker || cli.fork_picker || cli.agents_overview {
        startup_draft::StartupDraftInitialScreen::SessionPicker
    } else if !cli.oss
        && explicit_remote_endpoint.is_none()
        && (reuse_implicit_local_daemon || search_only_config_override)
        && launch_loader_overrides.packaged_defaults_path.is_none()
        && startup_preflight::should_delay_startup_composer_for_first_login(
            &codex_home,
            codex_config::loader::system_config_toml_file(),
            || codex_config::loader::has_local_managed_configuration(&codex_home),
            |name| std::env::var_os(name),
        )
    {
        startup_draft::StartupDraftInitialScreen::Onboarding
    } else {
        startup_draft::StartupDraftInitialScreen::Composer
    };
    let session_action = if cli.fork_picker || cli.fork_last || cli.fork_session_id.is_some() {
        startup_draft::StartupDraftSessionAction::Fork
    } else if cli.resume_picker || cli.resume_last || cli.resume_session_id.is_some() {
        startup_draft::StartupDraftSessionAction::Resume
    } else {
        startup_draft::StartupDraftSessionAction::New
    };
    // Local-daemon discovery does not change client config precedence. Resolve explicit
    // remote selection and the environment without opening a server connection.
    let presentation_target = app_server_target_for_launch(
        explicit_remote_endpoint.clone(),
        /*default_daemon_socket*/ None,
        reuse_implicit_local_daemon,
        workload_identity_selected,
        std::env::var_os(codex_exec_server::CODEX_EXEC_SERVER_URL_ENV_VAR).as_deref(),
    )?;
    let prepared_environment_manager =
        if should_load_configured_environments(&launch_loader_overrides, &presentation_target) {
            EnvironmentManager::prepare_from_codex_home(&codex_home).await
        } else {
            EnvironmentManager::prepare_from_env().await
        }
        .map_err(std::io::Error::other)?;
    if cli.shared.worktree
        && (presentation_target.uses_remote_workspace()
            || prepared_environment_manager.default_environment_is_remote())
    {
        return Err(std::io::Error::other(
            "`--worktree` is only supported for local sessions",
        ));
    }
    let cwd = cli.cwd.clone();
    let config_cwd = config_cwd_for_app_server_target(
        cwd.as_deref(),
        &presentation_target,
        prepared_environment_manager.default_environment_is_remote(),
    )?;
    // Reuse the profile path resolved above; do not reconstruct profile precedence here.
    let mut loader_overrides = launch_loader_overrides;
    loader_overrides.ignore_login_requirements = presentation_target.uses_remote_workspace();
    let presentation = startup_presentation::load(
        &cli,
        &codex_home,
        loader_overrides.clone(),
        cli_kv_overrides.clone(),
        config_cwd,
    )
    .await
    .map_err(|err| {
        if let Some(config_error) = err
            .get_ref()
            .and_then(|inner| inner.downcast_ref::<ConfigLoadError>())
        {
            std::io::Error::other(format!(
                "loading config.toml:\n{}",
                format_config_error_with_source(config_error.config_error())
            ))
        } else {
            err
        }
    })?;
    // Keep normal terminal signals available until local configuration is ready. Once raw
    // mode begins, StartupDraft immediately takes ownership of input.
    let (initialized_terminal, terminal_restore_guard) = tokio::task::spawn_blocking(|| {
        tui::init().map(|terminal| (terminal, TerminalRestoreGuard::new()))
    })
    .await
    .map_err(std::io::Error::other)??;
    let startup_presentation::StartupPresentation {
        bootstrap_config,
        config_cwd,
        mut screen,
    } = presentation;
    screen.use_alt_screen = determine_alt_screen_mode(
        cli.no_alt_screen,
        bootstrap_config
            .config_toml
            .tui
            .as_ref()
            .map(|tui| tui.alternate_screen)
            .unwrap_or_default(),
        initialized_terminal.terminal_app_over_ssh,
    );
    screen.transcript_mode = crate::transcript_mode::TranscriptMode::resolve(
        screen.transcript_mode.is_owned(),
        screen.use_alt_screen,
    );
    let mut startup_draft = startup_draft::StartupDraft::new(
        initialized_terminal,
        terminal_restore_guard,
        initial_screen,
        session_action,
        screen,
    )?;

    let default_daemon = if explicit_remote_endpoint.is_none() && reuse_implicit_local_daemon {
        startup_draft
            .run_until(maybe_probe_default_daemon_socket(&codex_home))
            .await?
    } else {
        None
    };
    let mut app_server_target = app_server_target_for_launch(
        explicit_remote_endpoint,
        default_daemon,
        reuse_implicit_local_daemon,
        workload_identity_selected,
        std::env::var_os(codex_exec_server::CODEX_EXEC_SERVER_URL_ENV_VAR).as_deref(),
    )?;
    let remote_cwd_override = cli
        .cwd
        .clone()
        .filter(|_| app_server_target.uses_remote_workspace());

    let local_runtime_paths = ExecServerRuntimePaths::from_optional_paths(
        arg0_paths.codex_self_exe.clone(),
        arg0_paths.codex_linux_sandbox_exe.clone(),
    )?;
    // The pre-paint bootstrap used these same local/remote config inputs. Reuse it here;
    // implicit daemon discovery above changes transport, not the client configuration cwd.
    let screen_reader_result = if !loader_overrides.ignore_user_config {
        startup_draft
            .run_until(screen_reader::initialize(
                &bootstrap_config.config_layer_stack,
            ))
            .await?
    } else {
        Ok(())
    };
    let cloud_config_bundle = startup_draft
        .run_until(cloud_config_bundle_for_app_server_target(
            &app_server_target,
            &bootstrap_config,
            &codex_home,
            &embedded_network_policy,
        ))
        .await??;
    let bootstrap_config_toml = &bootstrap_config.config_toml;

    let cwd_override = if app_server_target.uses_remote_workspace() {
        None
    } else {
        cwd.clone()
    };

    let mut manually_selected_oss_provider = None;
    let model_provider_override = if cli.oss {
        let bootstrap_config_with_cloud_config;
        let config_toml_for_oss = if cli.oss_provider.is_none() {
            // The first load intentionally skips cloud config so we can read
            // auth/base-url settings needed to fetch the bundle. If OSS mode
            // needs a default provider from config, reload with the bundle.
            bootstrap_config_with_cloud_config = startup_draft
                .run_until(load_bootstrap_config_or_exit(
                    &codex_home,
                    config_cwd.as_ref(),
                    cli_kv_overrides.clone(),
                    loader_overrides.clone(),
                    strict_config,
                    cloud_config_bundle.clone(),
                ))
                .await?;
            &bootstrap_config_with_cloud_config.config_toml
        } else {
            bootstrap_config_toml
        };

        let resolved = resolve_oss_provider(cli.oss_provider.as_deref(), config_toml_for_oss);

        if let Some(provider) = resolved {
            Some(provider)
        } else {
            let selection = match startup_draft
                .run_until(oss_selection::detect_oss_provider())
                .await?
            {
                oss_selection::OssProviderDetection::AutoSelected(selection) => selection,
                oss_selection::OssProviderDetection::NeedsSelection {
                    lmstudio_status,
                    ollama_status,
                } => {
                    startup_draft.flush_pending_events().await?;
                    startup_draft
                        .tui_mut()
                        .with_restored(|| {
                            oss_selection::select_oss_provider(lmstudio_status, ollama_status)
                        })
                        .await?
                }
            };
            let provider = selection.provider;
            if provider == "__CANCELLED__" {
                return Err(std::io::Error::other(
                    "OSS provider selection was cancelled by user",
                ));
            }
            if selection.manually_selected {
                manually_selected_oss_provider = Some(provider.clone());
            }
            Some(provider)
        }
    } else {
        None
    };

    // When using `--oss`, let the bootstrapper pick the model based on selected provider
    let model = if let Some(model) = &cli.model {
        Some(model.clone())
    } else if cli.oss {
        // Use the provider from model_provider_override
        model_provider_override
            .as_ref()
            .and_then(|provider_id| get_default_model_for_oss_provider(provider_id))
            .map(std::borrow::ToOwned::to_owned)
    } else {
        None // No model specified, will use the default.
    };

    let additional_dirs = cli.add_dir.clone();

    let mut overrides = ConfigOverrides {
        model,
        approval_policy,
        sandbox_mode,
        cwd: cwd_override,
        model_provider: model_provider_override.clone(),
        codex_self_exe: arg0_paths.codex_self_exe.clone(),
        codex_linux_sandbox_exe: arg0_paths.codex_linux_sandbox_exe.clone(),
        main_execve_wrapper_exe: arg0_paths.main_execve_wrapper_exe.clone(),
        show_raw_agent_reasoning: cli.oss.then_some(true),
        bypass_hook_trust: cli.bypass_hook_trust.then_some(true),
        additional_writable_roots: additional_dirs,
        ..Default::default()
    };

    let mut config = startup_draft
        .run_until(load_config_or_exit(
            cli_kv_overrides.clone(),
            overrides.clone(),
            loader_overrides.clone(),
            cloud_config_bundle.clone(),
            strict_config,
        ))
        .await?;
    if app_server_target.uses_embedded_network_policy() {
        embedded_network_policy.activate(&mut config);
    }
    startup_draft.apply_config(&config);

    let mut cloud_config_bundle = if workload_identity_selected {
        cloud_config_bundle
    } else {
        startup_draft
            .run_until(cloud_config_bundle_loader_for_storage(
                embedded_network_policy.bind_bootstrap_auth(
                    app_server_target.auth_config_for_cloud_loader(config.auth_config()),
                ),
                /*enable_codex_api_key_env*/ false,
            ))
            .await??
    };
    let managed_worktree = if cli.shared.worktree {
        let (destination, bundle, worktree) = startup_draft
            .run_until(worktree_startup::prepare(
                &mut cli,
                config.clone(),
                &mut overrides,
                cli_kv_overrides.clone(),
                loader_overrides.clone(),
                strict_config,
                &app_server_target,
                &arg0_paths,
                cloud_config_bundle.clone(),
                &embedded_network_policy,
            ))
            .await?
            .map_err(|err| std::io::Error::other(err.to_string()))?;
        config = destination;
        cloud_config_bundle = bundle;
        if app_server_target.uses_embedded_network_policy() {
            embedded_network_policy.activate(&mut config);
        }
        startup_draft.apply_config(&config);
        Some(worktree)
    } else {
        None
    };
    let auto_start_daemon = config.features.enabled(Feature::DaemonAutoStart)
        && !cli.agents_overview
        && !cli.no_daemon
        && !app_server_target.uses_remote_workspace();
    if auto_start_daemon
        && daemon_exclusion.is_none()
        && should_show_bedrock_setup_wizard(
            LoginStatus::NotAuthenticated,
            config.model_provider.requires_openai_auth,
            &config,
            &AppServerTarget::Embedded,
        )
        && startup_draft
            .run_until(
                config
                    .auth_config()
                    .load_auth(/*enable_codex_api_key_env*/ false),
            )
            .await?
            .ok()
            .flatten()
            .is_none()
    {
        // The Bedrock wizard configures its provider through the embedded server.
        daemon_exclusion = Some("Bedrock sign-in");
        app_server_target = AppServerTarget::Embedded;
    }
    let mut daemon_features = daemon_startup::server_features(&cli_kv_overrides);
    // Disabling shared services requires confirmation, even on a fresh auto-start.
    daemon_features.retain(|_, enabled| *enabled);
    let mut managed_daemon = false;
    if auto_start_daemon && daemon_exclusion.is_none() {
        startup_draft.flush_pending_events().await?;
        let output = startup_draft
            .tui_mut()
            .with_restored(|| async {
                // Package installation may print progress; keep ordinary Ctrl+C handling.
                crossterm::terminal::disable_raw_mode()?;
                let result = codex_app_server_daemon::start_with_features(&daemon_features).await;
                daemon_telemetry::record_start(&config, &result).await;
                result.map_err(|err| {
                    std::io::Error::other(format!("{err:#}\n{}", daemon_startup::FAILURE_HINT))
                })
            })
            .await?;
        managed_daemon = output.backend.is_some();
        app_server_target = AppServerTarget::LocalDaemon {
            endpoint: RemoteAppServerEndpoint::UnixSocket {
                socket_path: AbsolutePathBuf::from_absolute_path_checked(output.socket_path)?,
            },
            allow_embedded_fallback: false,
        };
    }
    // The overview must inspect the shared server's agents regardless of local settings.
    let compatibility_warning = if cli.agents_overview {
        None
    } else {
        daemon_recovery::check(
            &mut startup_draft,
            &app_server_target,
            &config,
            managed_daemon,
        )
        .await?
    };
    if compatibility_warning.is_some() {
        app_server_target = AppServerTarget::Embedded;
        daemon_exclusion = Some("daemon feature settings");
    }
    if app_server_target.uses_embedded_network_policy() {
        embedded_network_policy.activate(&mut config);
    }
    let daemon_startup_warning = compatibility_warning.or_else(|| {
        daemon_exclusion
            .filter(|_| auto_start_daemon)
            .map(|reason| {
                format!(
                    "Running without the shared background server: {reason} requires embedded mode."
                )
            })
    });
    #[cfg(target_os = "macos")]
    let local_runtime_paths = local_runtime_paths.with_allowed_symlinked_codex_home(
        codex_config::allowed_symlinked_codex_home(&config.config_layer_stack, &config.codex_home),
    );
    let environment_manager = Arc::new(
        prepared_environment_manager
            .build(
                Some(local_runtime_paths),
                app_server_target
                    .environment_http_client_factory(&config, &embedded_network_policy),
            )
            .map_err(std::io::Error::other)?,
    );

    remove_legacy_tui_log_file(config.codex_home.as_path());

    let otel_originator = originator().value;
    let otel = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        codex_app_server_client::build_otel_provider(
            &config,
            env!("CARGO_PKG_VERSION"),
            /*service_name_override*/ None,
            /*default_analytics_enabled*/ true,
        )
    })) {
        Ok(Ok(otel)) => otel,
        Ok(Err(e)) => {
            startup_draft.flush_pending_events().await?;
            startup_draft
                .tui_mut()
                .with_restored(|| async {
                    #[allow(clippy::print_stderr)]
                    {
                        eprintln!("Could not create otel exporter: {e}");
                    }
                })
                .await;
            None
        }
        Err(_) => {
            #[allow(clippy::print_stderr)]
            {
                eprintln!("Could not create otel exporter: panicked during initialization");
            }
            startup_draft.tui_mut().recover_after_caught_panic()?;
            None
        }
    };
    let metrics = otel
        .as_ref()
        .and_then(codex_otel::OtelProvider::metrics)
        .cloned();
    if let Some(metrics) = &metrics {
        let _ = codex_otel::record_process_start_once(metrics, otel_originator.as_str());
        let telemetry =
            codex_rollout::sqlite_telemetry_recorder(metrics.clone(), otel_originator.as_str());
        let _ = codex_state::install_process_db_telemetry(telemetry);
    }
    let selection_reason = match (&app_server_target, daemon_exclusion) {
        (AppServerTarget::Remote { .. }, _) => "explicit_remote",
        _ if cli.agents_overview => "agents",
        (_, Some("--no-daemon")) => "explicit_no_daemon",
        (_, Some(_)) => "incompatible_option",
        _ if auto_start_daemon => "auto_start",
        (AppServerTarget::LocalDaemon { .. }, _) => "existing_daemon",
        (AppServerTarget::Embedded, None) => "auto_start_disabled",
    };
    let daemon_settings = if metrics.is_some() {
        codex_app_server_daemon::telemetry::settings_tags(&config.codex_home)
            .await
            .to_vec()
    } else {
        Vec::new()
    };
    let mut launch_tags = daemon_settings.to_vec();
    launch_tags.extend([
        ("daemon_selection_reason", selection_reason),
        (
            "daemon_auto_start",
            if config.features.enabled(Feature::DaemonAutoStart) {
                "enabled"
            } else {
                "disabled"
            },
        ),
    ]);
    // Record the first connection attempt's actual mode, including embedded fallback; never reconnects.
    let launch_telemetry = move |target: &AppServerTarget, connected: bool| {
        let Some(metrics) = metrics else { return };
        let app_server_mode = match (connected, target) {
            (false, _) => "unconfirmed",
            (true, AppServerTarget::Embedded) => "in_process",
            (true, AppServerTarget::LocalDaemon { .. }) => "local_daemon",
            (true, AppServerTarget::Remote { .. }) => "remote",
        };
        // Use a fixed category, not the versioned or user-provided terminal identifier.
        let terminal_info = codex_terminal_detection::terminal_info();
        let terminal_name = match terminal_info.name {
            TerminalName::AppleTerminal => "apple_terminal",
            TerminalName::Ghostty => "ghostty",
            TerminalName::Iterm2 => "iterm2",
            TerminalName::WarpTerminal => "warp",
            TerminalName::VsCode => "vscode",
            TerminalName::WezTerm => "wezterm",
            TerminalName::Kitty => "kitty",
            TerminalName::Alacritty => "alacritty",
            TerminalName::Konsole => "konsole",
            TerminalName::GnomeTerminal => "gnome_terminal",
            TerminalName::Vte => "vte",
            TerminalName::WindowsTerminal => "windows_terminal",
            TerminalName::Dumb => "dumb",
            TerminalName::Unknown => "unknown",
        };
        let multiplexer = match terminal_info.multiplexer {
            Some(Multiplexer::Tmux { .. }) => "tmux",
            Some(Multiplexer::Zellij { .. }) => "zellij",
            None => "none",
        };
        launch_tags.extend([
            ("app_server_mode", app_server_mode),
            ("terminal_name", terminal_name),
            ("multiplexer", multiplexer),
        ]);
        let _ = metrics.counter("codex.tui.start", /*inc*/ 1, &launch_tags);
    };
    let launch_telemetry = daemon_telemetry::Launch(Some(launch_telemetry));
    let state_db = startup_draft
        .run_until(init_state_db_for_app_server_target(
            &config,
            &app_server_target,
        ))
        .await??;
    let config_toml_log_dir_configured = config
        .config_layer_stack
        .effective_config()
        .as_table()
        .is_some_and(|table| table.contains_key("log_dir"))
        || config
            .config_layer_stack
            .requirements_toml()
            .log_dir
            .is_some();

    set_default_client_residency_requirement(config.enforce_residency.value());

    if let Some(warning) = add_dir_warning_message(
        &cli.add_dir,
        &config.permissions.effective_permission_profile(),
        config.cwd.as_path(),
    ) {
        #[allow(clippy::print_stderr)]
        {
            restore_terminal_before_fatal_exit();
            eprintln!("Error adding directories: {warning}");
            if let Some(worktree) = managed_worktree.as_ref() {
                worktree.report_startup_failure();
            }
            launch_telemetry.record(&app_server_target, /*connected*/ false);
            if let Some(otel) = otel {
                let _ = otel
                    .shutdown_with_timeout(INTERACTIVE_OTEL_SHUTDOWN_TIMEOUT)
                    .await;
            }
            std::process::exit(1);
        }
    }

    if !app_server_target.uses_remote_workspace() && !workload_identity_selected {
        #[allow(clippy::print_stderr)]
        if let Err(err) = startup_draft
            .run_until(enforce_login_restrictions(&config.auth_config()))
            .await?
        {
            restore_terminal_before_fatal_exit();
            eprintln!("{err}");
            if let Some(worktree) = managed_worktree.as_ref() {
                worktree.report_startup_failure();
            }
            launch_telemetry.record(&app_server_target, /*connected*/ false);
            if let Some(otel) = otel {
                let _ = otel
                    .shutdown_with_timeout(INTERACTIVE_OTEL_SHUTDOWN_TIMEOUT)
                    .await;
            }
            std::process::exit(1);
        }
    }

    let (tui_file_layer, _tui_file_log_guard) = if config_toml_log_dir_configured {
        let log_dir = config.log_dir.clone();
        std::fs::create_dir_all(&log_dir)?;
        let mut log_file_opts = OpenOptions::new();
        log_file_opts.create(true).append(true);

        // Ensure the file is only readable and writable by the current user.
        // Doing the equivalent to `chmod 600` on Windows is quite a bit more
        // code and requires the Windows API crates.
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            log_file_opts.mode(0o600);
        }

        let log_file = log_file_opts.open(log_dir.join(TUI_LOG_FILE_NAME))?;
        let (non_blocking, guard) = non_blocking(log_file);
        let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::new(
                "codex_core=info,codex_tui=info,codex_rmcp_client=info,codex_realtime_webrtc=warn",
            )
        });
        let file_layer = tracing_subscriber::fmt::layer()
            .with_writer(non_blocking)
            .with_target(true)
            .with_ansi(false)
            .with_span_events(
                tracing_subscriber::fmt::format::FmtSpan::NEW
                    | tracing_subscriber::fmt::format::FmtSpan::CLOSE,
            )
            .with_filter(env_filter);
        (Some(file_layer), Some(guard))
    } else {
        (None, None)
    };

    let feedback = codex_feedback::CodexFeedback::new();
    let feedback_layer = feedback.logger_layer();
    let feedback_metadata_layer = feedback.metadata_layer();

    if cli.oss && model_provider_override.is_some() {
        // We're in the oss section, so provider_id should be Some
        // Let's handle None case gracefully though just in case
        let provider_id = match model_provider_override.as_ref() {
            Some(id) => id,
            None => {
                error!("OSS provider unexpectedly not set when oss flag is used");
                return Err(std::io::Error::other(
                    "OSS provider not set but oss flag was used",
                ));
            }
        };
        startup_draft.flush_pending_events().await?;
        startup_draft
            .tui_mut()
            .with_restored(|| async {
                // Provider setup may print progress or block in an external downloader.
                // Restore ordinary signal handling so Ctrl+C can interrupt that process.
                crossterm::terminal::disable_raw_mode()?;
                ensure_oss_provider_ready(provider_id, &config).await
            })
            .await?;
    }

    let otel_logger_layer = otel.as_ref().and_then(|o| o.logger_layer());

    let otel_tracing_layer = otel.as_ref().and_then(|o| o.tracing_layer());

    let log_db = state_db.clone().map(log_db::start);
    let log_db_layer = log_db
        .clone()
        .map(|layer| layer.with_filter(log_db::default_filter()));

    let _ = tracing_subscriber::registry()
        .with(tui_file_layer)
        .with(feedback_layer)
        .with(feedback_metadata_layer)
        .with(log_db_layer)
        .with(otel_logger_layer)
        .with(otel_tracing_layer)
        .try_init();

    if let Err(err) = screen_reader_result {
        tracing::warn!("Could not save screen-reader detection: {err}");
    }

    // Keep the large app future off the enclosing CLI startup stack during transitions.
    let app_result = Box::pin(run_ratatui_app(
        cli,
        arg0_paths,
        loader_overrides,
        strict_config,
        app_server_target,
        remote_cwd_override,
        config,
        manually_selected_oss_provider,
        overrides,
        cli_kv_overrides,
        cloud_config_bundle,
        feedback,
        log_db,
        state_db,
        environment_manager,
        embedded_network_policy,
        managed_worktree.clone(),
        daemon_startup_warning,
        launch_telemetry,
        startup_draft,
    ))
    .await
    .map_err(|err| {
        err.downcast::<std::io::Error>()
            .unwrap_or_else(|err| std::io::Error::other(err.to_string()))
    });

    if let Some(worktree) = managed_worktree.as_ref() {
        worktree.report_startup_failure();
    }

    // The TUI owns this request's consent. The child is silent; installation remains unconfirmed.
    if let Ok(exit) = &app_result
        && let Some(UpdateAction::Daemon(source)) = exit.update_action
        && let Some(metrics) = otel.as_ref().and_then(codex_otel::OtelProvider::metrics)
    {
        let mut tags = daemon_settings.to_vec();
        tags.extend([
            ("initiation_source", "tui_handoff"),
            (
                "update_target",
                match source {
                    DaemonUpdateSource::PublicStable => "public_stable",
                    DaemonUpdateSource::ThisCli => "this_cli",
                },
            ),
            ("outcome", "handoff_requested"),
        ]);
        let _ = metrics.counter("codex.daemon.update", /*inc*/ 1, &tags);
    }

    if let Some(otel) = otel
        && let Err(err) = otel
            .shutdown_with_timeout(INTERACTIVE_OTEL_SHUTDOWN_TIMEOUT)
            .await
    {
        warn!(error = %err, "failed to finish interactive telemetry shutdown");
    }

    app_result
}
