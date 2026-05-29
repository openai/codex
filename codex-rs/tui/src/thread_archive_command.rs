//! Shared implementation for `codex archive` and `codex unarchive`.
//!
//! The CLI commands are thin app-server clients: resolve a user-provided UUID or exact thread
//! name, then call the existing `thread/archive` or `thread/unarchive` RPC.

use std::sync::Arc;

use crate::Cli;
use crate::app_server_session::AppServerSession;
use crate::legacy_core::config::ConfigBuilder;
use crate::legacy_core::config::ConfigOverrides;
use crate::legacy_core::config::load_config_as_toml_with_cli_and_load_options;
use crate::legacy_core::config::resolve_profile_v2_config_path;
use codex_app_server_protocol::Thread as AppServerThread;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadSortKey;
use codex_arg0::Arg0DispatchPaths;
use codex_cloud_requirements::cloud_requirements_loader_for_storage;
use codex_config::ConfigLoadOptions;
use codex_config::LoaderOverrides;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecServerRuntimePaths;
use codex_protocol::ThreadId;
use codex_utils_cli::CliConfigOverrides;
use codex_utils_home_dir::find_codex_home;
use color_eyre::eyre::ContextCompat;
use color_eyre::eyre::Result;
use color_eyre::eyre::WrapErr;
use color_eyre::eyre::eyre;
use uuid::Uuid;

use super::RemoteAppServerEndpoint;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadArchiveAction {
    Archive,
    Unarchive,
}

impl ThreadArchiveAction {
    fn archived_filter(self) -> bool {
        match self {
            Self::Archive => false,
            Self::Unarchive => true,
        }
    }

    fn command_name(self) -> &'static str {
        match self {
            Self::Archive => "archive",
            Self::Unarchive => "unarchive",
        }
    }

    fn past_tense(self) -> &'static str {
        match self {
            Self::Archive => "Archived",
            Self::Unarchive => "Unarchived",
        }
    }

    fn search_scope(self) -> &'static str {
        match self {
            Self::Archive => "active",
            Self::Unarchive => "archived",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ThreadArchiveCommandOptions {
    pub cli: Cli,
    pub arg0_paths: Arg0DispatchPaths,
    pub loader_overrides: LoaderOverrides,
    pub explicit_remote_endpoint: Option<RemoteAppServerEndpoint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadArchiveCommandOutput {
    pub action: ThreadArchiveAction,
    pub thread_id: ThreadId,
    pub thread_name: Option<String>,
}

impl ThreadArchiveCommandOutput {
    pub fn success_message(&self) -> String {
        let action = self.action.past_tense();
        let thread_id = self.thread_id;
        match self.thread_name.as_deref() {
            Some(name) => format!("{action} thread {name} ({thread_id})."),
            None => format!("{action} thread {thread_id}."),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedThreadTarget {
    thread_id: ThreadId,
    thread_name: Option<String>,
}

pub async fn run_thread_archive_command(
    action: ThreadArchiveAction,
    target: String,
    options: ThreadArchiveCommandOptions,
) -> Result<ThreadArchiveCommandOutput> {
    let mut app_server = start_app_server_for_archive_command(options).await?;
    run_thread_archive_action_with_app_server(&mut app_server, action, &target).await
}

async fn run_thread_archive_action_with_app_server(
    app_server: &mut AppServerSession,
    action: ThreadArchiveAction,
    target: &str,
) -> Result<ThreadArchiveCommandOutput> {
    let resolved = resolve_thread_target(app_server, action, target).await?;
    match action {
        ThreadArchiveAction::Archive => {
            app_server.thread_archive(resolved.thread_id).await?;
            Ok(ThreadArchiveCommandOutput {
                action,
                thread_id: resolved.thread_id,
                thread_name: resolved.thread_name,
            })
        }
        ThreadArchiveAction::Unarchive => {
            let thread = app_server.thread_unarchive(resolved.thread_id).await?;
            Ok(ThreadArchiveCommandOutput {
                action,
                thread_id: resolved.thread_id,
                thread_name: thread.name.or(resolved.thread_name),
            })
        }
    }
}

async fn resolve_thread_target(
    app_server: &mut AppServerSession,
    action: ThreadArchiveAction,
    target: &str,
) -> Result<ResolvedThreadTarget> {
    if Uuid::parse_str(target).is_ok() {
        let thread_id = ThreadId::from_string(target)
            .wrap_err_with(|| format!("invalid thread id: {target}"))?;
        return Ok(ResolvedThreadTarget {
            thread_id,
            thread_name: None,
        });
    }

    let resolved = lookup_thread_by_exact_name(app_server, action, target)
        .await?
        .map(thread_target_from_app_server_thread)
        .transpose()?;

    resolved.with_context(|| {
        format!(
            "No {} chat found matching '{}'.",
            action.search_scope(),
            target
        )
    })
}

async fn lookup_thread_by_exact_name(
    app_server: &mut AppServerSession,
    action: ThreadArchiveAction,
    name: &str,
) -> Result<Option<AppServerThread>> {
    let mut cursor = None;
    loop {
        let response = app_server
            .thread_list(ThreadListParams {
                cursor: cursor.clone(),
                limit: Some(100),
                sort_key: Some(ThreadSortKey::UpdatedAt),
                sort_direction: None,
                model_providers: None,
                source_kinds: Some(super::resume_source_kinds(
                    /*include_non_interactive*/ false,
                )),
                archived: Some(action.archived_filter()),
                cwd: None,
                use_state_db_only: false,
                search_term: None,
            })
            .await
            .wrap_err_with(|| {
                format!(
                    "thread/list failed while resolving thread to {}",
                    action.command_name()
                )
            })?;

        if let Some(thread) = response
            .data
            .into_iter()
            .find(|thread| thread.name.as_deref() == Some(name))
        {
            return Ok(Some(thread));
        }
        if response.next_cursor.is_none() {
            return Ok(None);
        }
        cursor = response.next_cursor;
    }
}

fn thread_target_from_app_server_thread(thread: AppServerThread) -> Result<ResolvedThreadTarget> {
    let thread_id = ThreadId::from_string(&thread.id)
        .wrap_err_with(|| format!("app server returned invalid thread id `{}`", thread.id))?;
    Ok(ResolvedThreadTarget {
        thread_id,
        thread_name: thread.name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::SessionSource;
    use codex_app_server_protocol::ThreadStatus;
    use codex_utils_absolute_path::AbsolutePathBuf;

    fn app_server_thread(id: &str) -> AppServerThread {
        AppServerThread {
            id: id.to_string(),
            session_id: id.to_string(),
            forked_from_id: None,
            preview: String::new(),
            ephemeral: false,
            model_provider: "mock_provider".to_string(),
            created_at: 0,
            updated_at: 0,
            status: ThreadStatus::NotLoaded,
            path: None,
            cwd: AbsolutePathBuf::from_absolute_path(std::env::current_dir().expect("cwd"))
                .expect("absolute cwd"),
            cli_version: String::new(),
            source: SessionSource::Cli,
            thread_source: None,
            agent_nickname: None,
            agent_role: None,
            git_info: None,
            name: Some("saved-thread".to_string()),
            turns: Vec::new(),
        }
    }

    #[test]
    fn thread_target_from_app_server_thread_reports_invalid_ids() {
        let err = thread_target_from_app_server_thread(app_server_thread("not-a-thread-id"))
            .expect_err("invalid ids should be reported as normal errors");

        assert!(
            err.to_string()
                .contains("app server returned invalid thread id `not-a-thread-id`"),
            "unexpected error: {err}"
        );
    }
}

async fn start_app_server_for_archive_command(
    options: ThreadArchiveCommandOptions,
) -> Result<AppServerSession> {
    let ThreadArchiveCommandOptions {
        cli,
        arg0_paths,
        loader_overrides,
        explicit_remote_endpoint,
    } = options;
    let strict_config = cli.strict_config;
    let raw_overrides = cli.config_overrides.raw_overrides.clone();
    let overrides_cli = CliConfigOverrides { raw_overrides };
    let cli_kv_overrides = overrides_cli
        .parse_overrides()
        .map_err(|err| eyre!("failed to parse -c overrides: {err}"))?;
    let codex_home = find_codex_home().wrap_err("failed to find Codex home")?;

    let mut launch_loader_overrides = loader_overrides.clone();
    if let Some(profile_v2) = cli.config_profile_v2.as_ref() {
        launch_loader_overrides.user_config_path = Some(resolve_profile_v2_config_path(
            codex_home.as_path(),
            profile_v2,
        ));
        launch_loader_overrides.user_config_profile = Some(profile_v2.clone());
    }

    let reuse_implicit_local_daemon = super::can_reuse_implicit_local_daemon(
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
    );
    let remote_cwd_override = cli
        .cwd
        .clone()
        .filter(|_| app_server_target.uses_remote_workspace());

    let local_runtime_paths = ExecServerRuntimePaths::from_optional_paths(
        arg0_paths.codex_self_exe.clone(),
        arg0_paths.codex_linux_sandbox_exe.clone(),
    )
    .wrap_err("failed to resolve local runtime paths")?;
    let environment_manager = EnvironmentManager::from_env(Some(local_runtime_paths))
        .await
        .map(Arc::new)
        .wrap_err("failed to initialize environment manager")?;
    let config_cwd = super::config_cwd_for_app_server_target(
        cli.cwd.as_deref(),
        &app_server_target,
        &environment_manager,
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

    let config_toml = load_config_as_toml_with_cli_and_load_options(
        codex_home.as_path(),
        config_cwd.as_ref(),
        cli_kv_overrides.clone(),
        ConfigLoadOptions {
            loader_overrides: loader_overrides.clone(),
            strict_config,
        },
    )
    .await
    .wrap_err("failed to load config.toml")?;
    let chatgpt_base_url = config_toml
        .chatgpt_base_url
        .clone()
        .unwrap_or_else(|| "https://chatgpt.com/backend-api/".to_string());
    let cloud_requirements = cloud_requirements_loader_for_storage(
        codex_home.to_path_buf(),
        /*enable_codex_api_key_env*/ false,
        config_toml.cli_auth_credentials_store.unwrap_or_default(),
        chatgpt_base_url,
    )
    .await;

    let cwd = cli.cwd.clone();
    let config = ConfigBuilder::default()
        .cli_overrides(cli_kv_overrides.clone())
        .harness_overrides(ConfigOverrides {
            cwd: if app_server_target.uses_remote_workspace() {
                None
            } else {
                cwd
            },
            codex_self_exe: arg0_paths.codex_self_exe.clone(),
            codex_linux_sandbox_exe: arg0_paths.codex_linux_sandbox_exe.clone(),
            main_execve_wrapper_exe: arg0_paths.main_execve_wrapper_exe.clone(),
            bypass_hook_trust: cli.bypass_hook_trust.then_some(true),
            ..Default::default()
        })
        .loader_overrides(loader_overrides.clone())
        .strict_config(strict_config)
        .cloud_requirements(cloud_requirements.clone())
        .build()
        .await
        .wrap_err("failed to load configuration")?;
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
        cloud_requirements,
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
