//! Shared implementation for `codex archive`, `codex delete`, and `codex unarchive`.
//!
//! The CLI commands are thin app-server clients: resolve a user-provided UUID or exact session
//! name, then call the corresponding app-server RPC.

use std::io::IsTerminal;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use crate::Cli;
use crate::app_server_session::AppServerSession;
use crate::legacy_core::config::ConfigBuilder;
use crate::legacy_core::config::ConfigOverrides;
use crate::legacy_core::config::load_config_toml_with_layer_stack;
use crate::legacy_core::config::resolve_oss_provider;
use crate::legacy_core::config::resolve_profile_v2_config_path;
use crate::named_session_lookup::NamedSessionCandidates;
use crate::named_session_lookup::SessionCollection;
use crate::named_session_lookup::SessionNameLookupMode;
use crate::named_session_lookup::current_name_is_compatible;
use codex_app_server_protocol::Thread as AppServerThread;
use codex_arg0::Arg0DispatchPaths;
use codex_config::CloudConfigBundleLoader;
use codex_config::ConfigLoadOptions;
use codex_config::LoaderOverrides;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecServerRuntimePaths;
use codex_protocol::ThreadId;
use codex_utils_cli::CliConfigOverrides;
use codex_utils_home_dir::find_codex_home;
use codex_utils_oss::get_default_model_for_oss_provider;
use color_eyre::eyre::Result;
use color_eyre::eyre::WrapErr;
use color_eyre::eyre::eyre;

use super::RemoteAppServerEndpoint;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteConfirmation {
    Prompt,
    Skip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionArchiveAction {
    Archive,
    Delete(DeleteConfirmation),
    Unarchive,
}

pub struct SessionArchiveCommandOptions {
    pub cli: Cli,
    pub arg0_paths: Arg0DispatchPaths,
    pub explicit_remote_endpoint: Option<RemoteAppServerEndpoint>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SessionNameMatch {
    First,
    FirstIncludingNonInteractive,
}

fn success_message(
    action: SessionArchiveAction,
    session_id: ThreadId,
    session_name: Option<&str>,
) -> String {
    let action = match action {
        SessionArchiveAction::Archive => "Archived",
        SessionArchiveAction::Delete(_) => "Deleted",
        SessionArchiveAction::Unarchive => "Unarchived",
    };
    match session_name {
        Some(name) => format!("{action} session {name} ({session_id})."),
        None => format!("{action} session {session_id}."),
    }
}

struct ResolvedSessionTarget {
    session_id: ThreadId,
    session_name: Option<String>,
}

pub async fn run_session_archive_command(
    action: SessionArchiveAction,
    target: String,
    options: SessionArchiveCommandOptions,
) -> Result<String> {
    let codex_home = find_codex_home().wrap_err("failed to find Codex home")?;
    let mut app_server =
        start_app_server_for_session_command(options, codex_home.to_path_buf()).await?;
    run_session_archive_action_with_app_server(
        &mut app_server,
        codex_home.as_path(),
        action,
        &target,
    )
    .await
}

async fn run_session_archive_action_with_app_server(
    app_server: &mut AppServerSession,
    codex_home: &Path,
    action: SessionArchiveAction,
    target: &str,
) -> Result<String> {
    let resolved = resolve_session_target(app_server, codex_home, action, target).await?;
    let session_name = match action {
        SessionArchiveAction::Archive => {
            app_server.thread_archive(resolved.session_id).await?;
            resolved.session_name
        }
        SessionArchiveAction::Delete(confirmation) => {
            if matches!(confirmation, DeleteConfirmation::Prompt)
                && !confirm_session_delete(&resolved)?
            {
                return Ok("Delete cancelled.".to_string());
            }
            app_server.thread_delete(resolved.session_id).await?;
            resolved.session_name
        }
        SessionArchiveAction::Unarchive => {
            let thread = app_server.thread_unarchive(resolved.session_id).await?;
            thread.name.or(resolved.session_name)
        }
    };
    Ok(success_message(
        action,
        resolved.session_id,
        session_name.as_deref(),
    ))
}

async fn resolve_session_target(
    app_server: &mut AppServerSession,
    codex_home: &Path,
    action: SessionArchiveAction,
    target: &str,
) -> Result<ResolvedSessionTarget> {
    if let Ok(session_id) = ThreadId::from_string(target) {
        if matches!(
            action,
            SessionArchiveAction::Delete(DeleteConfirmation::Prompt)
        ) {
            let thread = app_server
                .thread_read(session_id, /*include_turns*/ false)
                .await
                .with_context(|| {
                    format!("No active or archived session found matching '{target}'.")
                })?;
            return Ok(ResolvedSessionTarget {
                session_id,
                session_name: thread.name,
            });
        }
        return Ok(ResolvedSessionTarget {
            session_id,
            session_name: None,
        });
    }

    let (search_scope, archived_values): (&str, &[bool]) = match action {
        SessionArchiveAction::Archive => ("active", &[false]),
        SessionArchiveAction::Delete(_) => ("active or archived", &[false, true]),
        SessionArchiveAction::Unarchive => ("archived", &[true]),
    };
    for &archived in archived_values {
        if let Some(thread) = lookup_session_by_exact_name(
            app_server,
            codex_home,
            target,
            archived,
            SessionNameMatch::First,
        )
        .await?
        {
            return session_target_from_app_server_thread(thread);
        }
    }
    Err(eyre!(
        "No {search_scope} session found matching '{target}'."
    ))
}

pub(super) async fn lookup_session_by_exact_name(
    app_server: &mut AppServerSession,
    codex_home: &Path,
    name: &str,
    archived: bool,
    match_policy: SessionNameMatch,
) -> Result<Option<AppServerThread>> {
    // Remote workspaces stay on their existing server-side path. Local workspaces trust SQLite
    // names, then scan and repair only after a miss or an unusable rollout path.
    let lookup_modes = if app_server.uses_remote_workspace() {
        &[SessionNameLookupMode::ScanAndRepair][..]
    } else {
        &[
            SessionNameLookupMode::StateDbOnly,
            SessionNameLookupMode::ScanAndRepair,
        ][..]
    };
    let source_kind_filters = if match_policy == SessionNameMatch::FirstIncludingNonInteractive {
        // An empty filter includes Atlas/ChatGPT sessions; explicit kinds additionally include exec.
        vec![
            super::resume_source_kinds(/*include_non_interactive*/ true),
            Vec::new(),
        ]
    } else {
        vec![super::resume_source_kinds(
            /*include_non_interactive*/ false,
        )]
    };
    for &lookup_mode in lookup_modes {
        let sort_by_recency = lookup_mode == SessionNameLookupMode::StateDbOnly
            && app_server.uses_embedded_app_server();
        // Search is the fast path, but legacy stores attach renamed titles after filtering.
        for search_term in [Some(name), None] {
            let mut first_match: Option<AppServerThread> = None;
            for source_kinds in &source_kind_filters {
                let mut candidates = NamedSessionCandidates::new(
                    name,
                    codex_home,
                    if archived {
                        SessionCollection::Archived
                    } else {
                        SessionCollection::Active
                    },
                    lookup_mode,
                    search_term,
                    source_kinds.clone(),
                );
                while let Some(candidate) = candidates
                    .next(app_server)
                    .await
                    .wrap_err("failed to list sessions while resolving session name")?
                {
                    let thread = if match_policy == SessionNameMatch::FirstIncludingNonInteractive
                        && lookup_mode == SessionNameLookupMode::ScanAndRepair
                        && !app_server.uses_remote_workspace()
                    {
                        let thread = app_server
                            .thread_read(
                                ThreadId::from_string(&candidate.thread.id)?,
                                /*include_turns*/ false,
                            )
                            .await?;
                        if !current_name_is_compatible(&thread, name) {
                            continue;
                        }
                        thread
                    } else {
                        candidate.thread
                    };
                    if first_match.as_ref().is_none_or(|existing| {
                        if sort_by_recency {
                            thread.recency_at.unwrap_or(thread.updated_at)
                                > existing.recency_at.unwrap_or(existing.updated_at)
                        } else {
                            thread.updated_at > existing.updated_at
                        }
                    }) {
                        first_match = Some(thread);
                    }
                    break;
                }
            }
            if first_match.is_some() {
                return Ok(first_match);
            }
        }
    }
    Ok(None)
}

fn session_target_from_app_server_thread(thread: AppServerThread) -> Result<ResolvedSessionTarget> {
    let session_id = ThreadId::from_string(&thread.id)
        .wrap_err_with(|| format!("app server returned invalid session id `{}`", thread.id))?;
    Ok(ResolvedSessionTarget {
        session_id,
        session_name: thread.name,
    })
}

fn confirm_session_delete(target: &ResolvedSessionTarget) -> Result<bool> {
    if !(std::io::stdin().is_terminal() && std::io::stderr().is_terminal()) {
        return Err(eyre!(
            "cannot confirm session deletion without an interactive terminal; rerun with --force and a session UUID"
        ));
    }

    let mut stderr = std::io::stderr().lock();
    match target.session_name.as_deref() {
        Some(name) => writeln!(
            stderr,
            "Permanently delete session '{name}' ({})?",
            target.session_id
        ),
        None => writeln!(stderr, "Permanently delete session {}?", target.session_id),
    }?;
    writeln!(
        stderr,
        "This cannot be undone. Subagent threads will also be deleted."
    )?;
    write!(stderr, "Continue? [y/N]: ")?;
    stderr.flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let answer = input.trim();
    Ok(answer.eq_ignore_ascii_case("y") || answer.eq_ignore_ascii_case("yes"))
}

pub(super) async fn start_app_server_for_session_command(
    options: SessionArchiveCommandOptions,
    codex_home: PathBuf,
) -> Result<AppServerSession> {
    let SessionArchiveCommandOptions {
        cli,
        arg0_paths,
        explicit_remote_endpoint,
    } = options;
    let loader_overrides = LoaderOverrides::default();
    let strict_config = cli.strict_config;
    let raw_overrides = cli.config_overrides.raw_overrides.clone();
    let overrides_cli = CliConfigOverrides { raw_overrides };
    let cli_kv_overrides = overrides_cli
        .parse_overrides()
        .map_err(|err| eyre!("failed to parse -c overrides: {err}"))?;
    let mut launch_loader_overrides = loader_overrides.clone();
    if let Some(profile_v2) = cli.config_profile_v2.as_ref() {
        launch_loader_overrides.user_config_path = Some(resolve_profile_v2_config_path(
            codex_home.as_path(),
            profile_v2,
        ));
        launch_loader_overrides.user_config_profile = Some(profile_v2.clone());
    }

    let workload_identity_selected = codex_login::is_workload_identity_selected();
    let reuse_implicit_local_daemon = !workload_identity_selected
        && super::can_reuse_implicit_local_daemon(
            &cli_kv_overrides,
            &launch_loader_overrides,
            strict_config,
            cli.bypass_hook_trust,
        );
    let default_daemon = if explicit_remote_endpoint.is_none() && reuse_implicit_local_daemon {
        super::maybe_probe_default_daemon_socket(codex_home.as_path()).await
    } else {
        None
    };
    let app_server_target = super::app_server_target_for_launch(
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
    )
    .wrap_err("failed to resolve local runtime paths")?;
    let prepared_environment_manager = EnvironmentManager::prepare_from_env()
        .await
        .wrap_err("failed to discover execution environments")?;
    let config_cwd = super::config_cwd_for_app_server_target(
        cli.cwd.as_deref(),
        &app_server_target,
        prepared_environment_manager.default_environment_is_remote(),
    )
    .wrap_err("failed to resolve config cwd")?;

    let mut loader_overrides = loader_overrides;
    if let Some(profile_v2) = cli.config_profile_v2.as_ref() {
        loader_overrides.user_config_path = Some(resolve_profile_v2_config_path(
            codex_home.as_path(),
            profile_v2,
        ));
        loader_overrides.user_config_profile = Some(profile_v2.clone());
    }
    loader_overrides.ignore_login_requirements = app_server_target.uses_remote_workspace();

    let bootstrap_config = load_config_toml_with_layer_stack(
        codex_home.as_path(),
        config_cwd.as_ref(),
        cli_kv_overrides.clone(),
        ConfigLoadOptions {
            loader_overrides: loader_overrides.clone(),
            strict_config,
            cloud_config_bundle: CloudConfigBundleLoader::default(),
        },
    )
    .await
    .wrap_err("failed to load config.toml")?;
    let config_toml = &bootstrap_config.config_toml;
    let cloud_config_bundle = super::cloud_config_bundle_for_app_server_target(
        &app_server_target,
        &bootstrap_config,
        codex_home.as_path(),
    )
    .await?;

    let model_provider = if cli.oss {
        resolve_oss_provider(cli.oss_provider.as_deref(), config_toml)
    } else {
        None
    };
    let model = cli.model.clone().or_else(|| {
        model_provider
            .as_deref()
            .and_then(get_default_model_for_oss_provider)
            .map(ToOwned::to_owned)
    });
    let cwd = cli.cwd.clone();
    let config = ConfigBuilder::default()
        .cli_overrides(cli_kv_overrides.clone())
        .harness_overrides(ConfigOverrides {
            model,
            cwd: if app_server_target.uses_remote_workspace() {
                None
            } else {
                cwd
            },
            model_provider,
            codex_self_exe: arg0_paths.codex_self_exe.clone(),
            codex_linux_sandbox_exe: arg0_paths.codex_linux_sandbox_exe.clone(),
            main_execve_wrapper_exe: arg0_paths.main_execve_wrapper_exe.clone(),
            show_raw_agent_reasoning: cli.oss.then_some(true),
            bypass_hook_trust: cli.bypass_hook_trust.then_some(true),
            ..Default::default()
        })
        .loader_overrides(loader_overrides.clone())
        .strict_config(strict_config)
        .cloud_config_bundle(cloud_config_bundle.clone())
        .build()
        .await
        .wrap_err("failed to load configuration")?;
    let environment_manager = Arc::new(
        prepared_environment_manager
            .build(Some(local_runtime_paths), config.http_client_factory())
            .wrap_err("failed to initialize environment manager")?,
    );
    let state_db = super::init_state_db_for_app_server_target(&config, &app_server_target)
        .await
        .wrap_err("failed to initialize state database")?;
    let app_server = super::start_app_server(
        &app_server_target,
        arg0_paths,
        config,
        cli_kv_overrides,
        loader_overrides,
        strict_config,
        cloud_config_bundle,
        codex_feedback::CodexFeedback::new(),
        /*log_db*/ None,
        state_db,
        environment_manager,
    )
    .await?;
    Ok(
        AppServerSession::new(app_server, app_server_target.thread_params_mode())
            .with_remote_cwd_override(remote_cwd_override),
    )
}

#[cfg(test)]
#[path = "session_archive_commands_tests.rs"]
mod tests;
