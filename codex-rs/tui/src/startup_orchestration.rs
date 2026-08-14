//! Orchestrates startup while the provisional composer owns terminal input.
//!
//! Lightweight validation runs before acquiring the terminal. Once the draft is visible, slow
//! configuration and app-server initialization remain responsive to safe local editing.

use super::*;

pub(super) async fn run_main_inner(
    mut cli: Cli,
    arg0_paths: Arg0DispatchPaths,
    loader_overrides: LoaderOverrides,
    explicit_remote_endpoint: Option<RemoteAppServerEndpoint>,
) -> std::io::Result<AppExitInfo> {
    let strict_config = cli.strict_config;
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
    let workload_identity_selected = is_workload_identity_selected();

    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        let validation_target = app_server_target_for_launch(
            explicit_remote_endpoint.clone(),
            /*default_daemon_socket*/ None,
            /*can_reuse_implicit_local_daemon*/ false,
            workload_identity_selected,
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

    let reuse_implicit_local_daemon = !workload_identity_selected
        && can_reuse_implicit_local_daemon(
            &cli_kv_overrides,
            &launch_loader_overrides,
            strict_config,
            cli.bypass_hook_trust,
        );
    let search_only_config_override = !workload_identity_selected
        && cli.web_search
        && startup_preflight::has_only_search_config_override(&cli_kv_overrides)
        && loader_overrides_are_default(&launch_loader_overrides)
        && !strict_config
        && !cli.bypass_hook_trust;
    let initial_screen = if cli.resume_picker || cli.fork_picker {
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
    let mut startup_draft = startup_draft::StartupDraft::new(initial_screen)?;

    let default_daemon = if explicit_remote_endpoint.is_none() && reuse_implicit_local_daemon {
        startup_draft
            .run_until(maybe_probe_default_daemon_socket(&codex_home))
            .await?
    } else {
        None
    };
    let app_server_target = app_server_target_for_launch(
        explicit_remote_endpoint,
        default_daemon,
        reuse_implicit_local_daemon,
        workload_identity_selected,
    )?;
    let remote_cwd_override = cli
        .cwd
        .clone()
        .filter(|_| app_server_target.uses_remote_workspace());

    let local_runtime_paths = ExecServerRuntimePaths::from_optional_paths(
        arg0_paths.codex_self_exe.clone(),
        arg0_paths.codex_linux_sandbox_exe.clone(),
    )?;
    let prepared_environment_manager =
        if should_load_configured_environments(&loader_overrides, &app_server_target) {
            startup_draft
                .run_until(EnvironmentManager::prepare_from_codex_home(&codex_home))
                .await?
        } else {
            startup_draft
                .run_until(EnvironmentManager::prepare_from_env())
                .await?
        }
        .map_err(std::io::Error::other)?;
    let cwd = cli.cwd.clone();
    let config_cwd = config_cwd_for_app_server_target(
        cwd.as_deref(),
        &app_server_target,
        prepared_environment_manager.default_environment_is_remote(),
    )?;
    let mut loader_overrides = loader_overrides;
    if let Some(profile_v2) = cli.config_profile_v2.as_ref() {
        let user_config_path = resolve_profile_v2_config_path(&codex_home, profile_v2);
        loader_overrides.user_config_path = Some(user_config_path);
        loader_overrides.user_config_profile = Some(profile_v2.clone());
    }
    loader_overrides.ignore_login_requirements = app_server_target.uses_remote_workspace();

    let bootstrap_config = startup_draft
        .run_until(load_bootstrap_config_or_exit(
            &codex_home,
            config_cwd.as_ref(),
            cli_kv_overrides.clone(),
            loader_overrides.clone(),
            strict_config,
            CloudConfigBundleLoader::default(),
        ))
        .await?;
    let bootstrap_config_toml = &bootstrap_config.config_toml;
    let cloud_config_bundle = startup_draft
        .run_until(cloud_config_bundle_for_app_server_target(
            &app_server_target,
            &bootstrap_config,
            &codex_home,
        ))
        .await??;

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

    let overrides = ConfigOverrides {
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

    let config = startup_draft
        .run_until(load_config_or_exit(
            cli_kv_overrides.clone(),
            overrides.clone(),
            loader_overrides.clone(),
            cloud_config_bundle.clone(),
            strict_config,
        ))
        .await?;
    startup_draft.apply_config(&config);

    let cloud_config_bundle = if workload_identity_selected {
        cloud_config_bundle
    } else {
        startup_draft
            .run_until(cloud_config_bundle_loader_for_storage(
                app_server_target.auth_config_for_cloud_loader(config.auth_config()),
                /*enable_codex_api_key_env*/ false,
            ))
            .await??
    };
    let environment_manager = Arc::new(
        prepared_environment_manager
            .build(Some(local_runtime_paths), config.http_client_factory())
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
    if let Some(metrics) = otel.as_ref().and_then(codex_otel::OtelProvider::metrics) {
        let _ = codex_otel::record_process_start_once(metrics, otel_originator.as_str());
        let telemetry =
            codex_rollout::sqlite_telemetry_recorder(metrics.clone(), otel_originator.as_str());
        let _ = codex_state::install_process_db_telemetry(telemetry);
    }
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
            EnvFilter::new("codex_core=info,codex_tui=info,codex_rmcp_client=info")
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

    let app_result = run_ratatui_app(
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
        startup_draft,
    )
    .await
    .map_err(|err| {
        err.downcast::<std::io::Error>()
            .unwrap_or_else(|err| std::io::Error::other(err.to_string()))
    });

    if let Some(otel) = otel
        && let Err(err) = otel
            .shutdown_with_timeout(INTERACTIVE_OTEL_SHUTDOWN_TIMEOUT)
            .await
    {
        warn!(error = %err, "failed to finish interactive telemetry shutdown");
    }

    app_result
}
