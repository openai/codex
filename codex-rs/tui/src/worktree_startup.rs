//! Resolves and allocates the destination before telemetry and login policy are initialized.
//! Source distrust is never upgraded; ownership is bound before exposing the new thread.
//! Failed startup retains the checkout and reports manual recovery until ownership is bound.
use super::*;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

#[derive(Clone)]
pub(crate) struct ManagedTuiWorktree {
    manager: codex_worktree::WorktreeManager,
    checkout: codex_worktree::ManagedWorktree,
    recovery: Arc<StartupRecovery>,
}

impl ManagedTuiWorktree {
    pub(crate) fn bind(&self, thread_id: ThreadId) -> color_eyre::Result<()> {
        self.manager
            .bind_thread(&self.checkout.root, &thread_id.to_string())
            .map_err(std::io::Error::other)
            .wrap_err("failed to bind managed worktree thread")?;
        self.recovery.finished.store(true, Ordering::Relaxed);
        Ok(())
    }

    pub(crate) async fn check_source_policy(
        &self,
        cli_overrides: &[(String, toml::Value)],
        overrides: &ConfigOverrides,
        loader_overrides: &LoaderOverrides,
        bundle: &CloudConfigBundleLoader,
        strict_config: bool,
    ) -> std::io::Result<()> {
        let mut source_overrides = overrides.clone();
        source_overrides.cwd = Some(self.checkout.source_cwd.clone());
        let source = ConfigBuilder::default()
            .cli_overrides(cli_overrides.to_vec())
            .harness_overrides(source_overrides)
            .loader_overrides(LoaderOverrides {
                ignore_project_config: true,
                ..loader_overrides.clone()
            })
            .cloud_config_bundle(bundle.clone())
            .strict_config(strict_config)
            .build()
            .await
            .map_err(std::io::Error::other)?;
        if source.active_project.is_untrusted() {
            return Err(std::io::Error::other(
                "`--worktree` cannot create a checkout from an explicitly untrusted source",
            ));
        }
        Ok(())
    }
}

/// Shared by startup and its background thread request; reports at most once.
struct StartupRecovery {
    root: PathBuf,
    finished: AtomicBool,
}

impl StartupRecovery {
    fn message(&self) -> String {
        format!(
            "Startup did not finish binding a thread to this worktree: {:?}\n\
             The checkout was kept. Inspect it and confirm no session is using it.\n\
             To remove it, run `git worktree remove <checkout-path>` from the source repository,\n\
             replacing <checkout-path> with the path above. Do not use --force.",
            self.root
        )
    }

    fn report(&self) {
        if !self.finished.swap(true, Ordering::Relaxed) {
            restore_terminal_before_fatal_exit();
            #[allow(clippy::print_stderr)]
            {
                eprintln!("{}", self.message());
            }
        }
    }
}

impl Drop for StartupRecovery {
    fn drop(&mut self) {
        self.report();
    }
}

impl ManagedTuiWorktree {
    pub(crate) fn report_startup_failure(&self) {
        self.recovery.report();
    }
}

async fn latest_thread_cwd(path: Option<PathBuf>, fallback: PathBuf) -> PathBuf {
    let Some(path) = path else { return fallback };
    tokio::task::spawn_blocking(move || {
        let reader = codex_rollout::open_rollout_seekable_reader(&path).ok()?;
        let mut scanner = codex_rollout::ReverseJsonlScanner::new(reader).ok()?;
        while let Some(outcome) = scanner.scan_next_rollout_line().ok()? {
            if let codex_rollout::ScanOutcome::Parsed(codex_rollout::RolloutLine {
                item: codex_rollout::RolloutItem::TurnContext(item),
                ..
            }) = outcome
            {
                return Some(item.cwd.into_path_buf());
            }
        }
        None
    })
    .await
    .ok()
    .flatten()
    .unwrap_or(fallback)
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn prepare(
    cli: &mut Cli,
    mut source: Config,
    overrides: &mut ConfigOverrides,
    cli_overrides: Vec<(String, toml::Value)>,
    loader_overrides: LoaderOverrides,
    strict_config: bool,
    target: &AppServerTarget,
    arg0_paths: &Arg0DispatchPaths,
    source_bundle: CloudConfigBundleLoader,
) -> color_eyre::Result<(Config, CloudConfigBundleLoader, ManagedTuiWorktree)> {
    if let Some(id_or_name) = cli.fork_session_id.as_deref() {
        let prepared = if should_load_configured_environments(&loader_overrides, target) {
            EnvironmentManager::prepare_from_codex_home(&source.codex_home).await
        } else {
            EnvironmentManager::prepare_from_env().await
        }
        .map_err(std::io::Error::other)?;
        if prepared.default_environment_is_remote() {
            color_eyre::eyre::bail!("`--worktree` is only supported for local sessions");
        }
        let environment = prepared.build(
            Some(ExecServerRuntimePaths::from_optional_paths(
                arg0_paths.codex_self_exe.clone(),
                arg0_paths.codex_linux_sandbox_exe.clone(),
            )?),
            source.http_client_factory(),
        )?;
        let state =
            init_state_db_for_app_server_target(&source, &AppServerTarget::Embedded).await?;
        let mut lookup_config = source.clone();
        lookup_config.analytics_enabled = Some(false);
        let client = start_embedded_app_server(
            arg0_paths.clone(),
            lookup_config,
            cli_overrides.clone(),
            loader_overrides.clone(),
            strict_config,
            source_bundle.clone(),
            codex_feedback::CodexFeedback::new(),
            /*log_db*/ None,
            state,
            Arc::new(environment),
        )
        .await?;
        let mut lookup = AppServerSession::new(
            AppServerClient::InProcess(client),
            crate::app_server_session::ThreadParamsMode::Embedded,
        );
        let resolved = async {
            let target = lookup_session_target_with_app_server(&mut lookup, &source, id_or_name)
                .await?
                .ok_or_else(|| color_eyre::eyre::eyre!("Session not found: {id_or_name}"))?;
            lookup
                .thread_read(target.thread_id, /*include_turns*/ false)
                .await
        }
        .await;
        let shutdown = lookup.shutdown().await;
        let thread = resolved?;
        shutdown?;
        cli.fork_session_id = Some(thread.id);
        if cli.cwd.is_none() {
            let cwd = latest_thread_cwd(thread.path, thread.cwd.into_path_buf()).await;
            let cwd = AbsolutePathBuf::from_absolute_path(cwd)?;
            let bootstrap = load_bootstrap_config_or_exit(
                &source.codex_home,
                Some(&cwd),
                cli_overrides.clone(),
                loader_overrides.clone(),
                strict_config,
                CloudConfigBundleLoader::default(),
            )
            .await;
            let source_bundle =
                cloud_config_bundle_for_app_server_target(target, &bootstrap, &source.codex_home)
                    .await?;
            overrides.cwd = Some(cwd.into_path_buf());
            source = load_config_or_exit(
                cli_overrides.clone(),
                overrides.clone(),
                loader_overrides.clone(),
                source_bundle,
                strict_config,
            )
            .await;
        }
    }
    if !source.features.enabled(codex_features::Feature::Worktrees) {
        color_eyre::eyre::bail!(
            "`--worktree` requires the worktrees feature; enable it with `--enable worktrees`"
        );
    }
    if source.active_project.is_untrusted() {
        color_eyre::eyre::bail!(
            "`--worktree` cannot create a checkout from an explicitly untrusted source"
        );
    }
    let invocation_cwd = std::env::current_dir()?;
    for path in &mut overrides.additional_writable_roots {
        if path.is_relative() {
            *path = invocation_cwd.join(&*path);
        }
    }
    let host = load_bootstrap_config_or_exit(
        &source.codex_home,
        /*cwd*/ None,
        Vec::new(),
        LoaderOverrides::default(),
        strict_config,
        CloudConfigBundleLoader::default(),
    )
    .await;
    let manager = codex_worktree::WorktreeManager::new(
        codex_worktree::WorktreeSettings::for_cli(
            &source.codex_home,
            host.config_toml.desktop.as_ref(),
        )
        .map_err(std::io::Error::other)?,
    );
    let checkout = manager
        .create(&codex_worktree::CreateWorktree {
            source_cwd: source.cwd.to_path_buf(),
            base: None,
        })
        .map_err(std::io::Error::other)?;
    let recovery = Arc::new(StartupRecovery {
        root: checkout.root.clone(),
        finished: AtomicBool::new(false),
    });
    let managed = ManagedTuiWorktree {
        manager,
        checkout,
        recovery,
    };
    let destination = AbsolutePathBuf::from_absolute_path(managed.checkout.cwd.clone())?;
    let bootstrap = load_config_toml_with_layer_stack(
        &source.codex_home,
        Some(&destination),
        cli_overrides.clone(),
        codex_config::ConfigLoadOptions {
            loader_overrides: loader_overrides.clone(),
            strict_config,
            cloud_config_bundle: CloudConfigBundleLoader::default(),
        },
    )
    .await?;
    let bundle =
        cloud_config_bundle_for_app_server_target(target, &bootstrap, &source.codex_home).await?;
    managed
        .check_source_policy(
            &cli_overrides,
            overrides,
            &loader_overrides,
            &bundle,
            strict_config,
        )
        .await?;
    overrides.cwd = Some(managed.checkout.cwd.clone());
    let config = ConfigBuilder::default()
        .cli_overrides(cli_overrides)
        .harness_overrides(overrides.clone())
        .loader_overrides(loader_overrides)
        .cloud_config_bundle(bundle.clone())
        .strict_config(strict_config)
        .build()
        .await?;
    cli.cwd = Some(managed.checkout.cwd.clone());
    Ok((config, bundle, managed))
}

#[cfg(test)]
#[path = "worktree_startup_tests.rs"]
mod tests;
