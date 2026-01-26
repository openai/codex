use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::time::Duration;

use codex_protocol::approvals::SkillDependenciesApprovalOption;
use codex_protocol::approvals::SkillDependenciesApprovalRequestEvent;
use codex_protocol::protocol::McpAuthStatus;
use codex_rmcp_client::perform_oauth_login;
use codex_rmcp_client::supports_oauth_login;
use futures::future::join_all;
use shlex::split as shlex_split;
use tokio::time::timeout;
use tracing::warn;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::config::Config;
use crate::config::edit::ConfigEditsBuilder;
use crate::config::find_codex_home;
use crate::config::load_global_mcp_servers;
use crate::config::types::McpServerConfig;
use crate::config::types::McpServerTransportConfig;
use crate::mcp::auth::McpAuthStatusEntry;
use crate::mcp::auth::compute_auth_statuses;
use crate::mcp::effective_mcp_servers;
use crate::mcp_connection_manager::SandboxState;
use crate::skills::SkillMetadata;
use crate::skills::model::SkillDependencies;
use crate::skills::model::SkillToolDependency;

pub(crate) async fn handle_skill_dependencies(
    session: &Session,
    turn: &TurnContext,
    call_id: String,
    skill: &SkillMetadata,
) -> Option<String> {
    let Some(dependencies) = skill.dependencies.as_ref() else {
        return None;
    };

    let missing_mcps = collect_missing_mcp_dependencies(session, dependencies).await;
    if !missing_mcps.is_empty() {
        let install = prompt_missing_mcp_dependencies(session, turn, call_id, skill, &missing_mcps)
            .await
            .unwrap_or(false);
        if install {
            install_missing_mcp_dependencies(session, turn, skill, &missing_mcps).await;
        }
    }

    None
}

async fn collect_missing_mcp_dependencies(
    session: &Session,
    dependencies: &SkillDependencies,
) -> Vec<SkillToolDependency> {
    let mut seen = HashSet::new();
    let mcp_dependency_names = collect_mcp_dependency_names(&dependencies.tools);
    let auth_entries = collect_mcp_auth_entries(session, &mcp_dependency_names).await;
    let mcp_connection_manager = session.services.mcp_connection_manager.read().await;
    dependencies
        .tools
        .iter()
        .filter(|dependency| dependency.tool_type.eq_ignore_ascii_case("mcp"))
        .filter_map(|dependency| {
            let name = dependency.value.trim();
            if name.is_empty() || !seen.insert(name.to_string()) {
                return None;
            }
            if mcp_connection_manager.has_server(name) {
                if auth_entries.get(name).is_some_and(mcp_auth_missing) {
                    Some(dependency.clone())
                } else {
                    None
                }
            } else {
                Some(dependency.clone())
            }
        })
        .collect()
}

fn collect_mcp_dependency_names(dependencies: &[SkillToolDependency]) -> HashSet<String> {
    dependencies
        .iter()
        .filter_map(|dependency| {
            if !dependency.tool_type.eq_ignore_ascii_case("mcp") {
                return None;
            }
            let name = dependency.value.trim();
            if name.is_empty() {
                None
            } else {
                Some(name.to_string())
            }
        })
        .collect()
}

async fn collect_mcp_auth_entries(
    session: &Session,
    mcp_dependency_names: &HashSet<String>,
) -> HashMap<String, McpAuthStatusEntry> {
    if mcp_dependency_names.is_empty() {
        return HashMap::new();
    }

    let config = session.get_config().await;
    let auth = session.services.auth_manager.auth().await;
    let mcp_servers = effective_mcp_servers(&config, auth.as_ref())
        .into_iter()
        .filter(|(name, _)| mcp_dependency_names.contains(name))
        .collect::<HashMap<_, _>>();
    if mcp_servers.is_empty() {
        return HashMap::new();
    }

    compute_auth_statuses(mcp_servers.iter(), config.mcp_oauth_credentials_store_mode).await
}

fn mcp_auth_missing(entry: &McpAuthStatusEntry) -> bool {
    entry.auth_status == McpAuthStatus::NotLoggedIn || bearer_token_env_var_missing(&entry.config)
}

fn mcp_needs_oauth_login(entry: &McpAuthStatusEntry) -> bool {
    entry.auth_status == McpAuthStatus::NotLoggedIn
}

fn bearer_token_env_var_missing(config: &McpServerConfig) -> bool {
    let McpServerTransportConfig::StreamableHttp {
        bearer_token_env_var: Some(env_var),
        ..
    } = &config.transport
    else {
        return false;
    };

    match env::var(env_var) {
        Ok(value) => value.is_empty(),
        Err(env::VarError::NotPresent) | Err(env::VarError::NotUnicode(_)) => true,
    }
}

fn bearer_token_env_var_name(config: &McpServerConfig) -> Option<&str> {
    match &config.transport {
        McpServerTransportConfig::StreamableHttp {
            bearer_token_env_var: Some(env_var),
            ..
        } => Some(env_var.as_str()),
        _ => None,
    }
}

async fn prompt_missing_mcp_dependencies(
    session: &Session,
    turn: &TurnContext,
    call_id: String,
    skill: &SkillMetadata,
    missing_mcps: &[SkillToolDependency],
) -> Option<bool> {
    let missing_names = missing_mcps
        .iter()
        .map(|dependency| dependency.value.trim().to_string())
        .collect::<Vec<_>>();
    if missing_names.is_empty() {
        return None;
    }

    let skill_name = skill.name.as_str();
    let question_id = "missing_mcp_dependencies".to_string();
    let missing_list = missing_names.join(", ");
    let run_anyway_label = "Run anyway".to_string();
    let install_label = if missing_names.len() == 1 {
        let install_target = &missing_names[0];
        format!("Install {install_target}")
    } else {
        "Install missing MCPs".to_string()
    };
    let header = if missing_names.len() == 1 {
        "Missing MCP dependency".to_string()
    } else {
        "Missing MCP dependencies".to_string()
    };
    let prompt = format!(
        "The \"{skill_name}\" skill depends on MCP server(s) that are not loaded or missing required authentication: {missing_list}. What would you like to do?"
    );
    let description = if missing_names.len() == 1 {
        let install_target = &missing_names[0];
        format!("Install and configure the {install_target} MCP server.")
    } else {
        "Install and configure the missing MCP servers.".to_string()
    };
    let event = SkillDependenciesApprovalRequestEvent {
        call_id,
        turn_id: turn.sub_id.clone(),
        question_id: question_id.clone(),
        header,
        question: prompt,
        run_anyway: SkillDependenciesApprovalOption {
            label: run_anyway_label.clone(),
            description: "Proceed without installing. The skill may not work as expected."
                .to_string(),
        },
        install: SkillDependenciesApprovalOption {
            label: install_label.clone(),
            description,
        },
    };

    let response = session
        .request_skill_dependencies_approval(turn, event)
        .await;

    let selected = response
        .as_ref()
        .and_then(|response| response.answers.get(&question_id))
        .and_then(|answer| answer.answers.first());
    Some(matches!(selected, Some(answer) if *answer == install_label))
}

async fn install_missing_mcp_dependencies(
    session: &Session,
    turn: &TurnContext,
    skill: &SkillMetadata,
    missing_mcps: &[SkillToolDependency],
) {
    if missing_mcps.is_empty() {
        return;
    }

    let pending = missing_mcps
        .iter()
        .filter(|dependency| !dependency.value.trim().is_empty())
        .collect::<Vec<_>>();
    if pending.is_empty() {
        return;
    }

    let total = pending.len();
    let mcp_dependency_names = collect_mcp_dependency_names(missing_mcps);
    let auth_entries = collect_mcp_auth_entries(session, &mcp_dependency_names).await;
    let skill_name = skill.name.as_str();
    let plural = if total == 1 {
        "dependency"
    } else {
        "dependencies"
    };
    session
        .notify_background_event(
            turn,
            format!("Installing {total} MCP {plural} for \"{skill_name}\"."),
        )
        .await;
    let config = match Config::load_with_cli_overrides(Vec::new()).await {
        Ok(config) => config,
        Err(err) => {
            warn!("Failed to load config for MCP installs (skill={skill_name}): {err}");
            session
                .notify_background_event(
                    turn,
                    format!("Failed to load config for MCP installs (skill={skill_name}): {err}"),
                )
                .await;
            return;
        }
    };

    let codex_home = match find_codex_home() {
        Ok(codex_home) => codex_home,
        Err(err) => {
            warn!("Failed to resolve CODEX_HOME for MCP installs (skill={skill_name}): {err}");
            session
                .notify_background_event(
                    turn,
                    format!(
                        "Failed to resolve CODEX_HOME for MCP installs (skill={skill_name}): {err}"
                    ),
                )
                .await;
            return;
        }
    };

    let mut servers = match load_global_mcp_servers(&codex_home).await {
        Ok(servers) => servers,
        Err(err) => {
            warn!("Failed to load MCP servers for MCP installs (skill={skill_name}): {err}");
            session
                .notify_background_event(
                    turn,
                    format!(
                        "Failed to load MCP servers for MCP installs (skill={skill_name}): {err}"
                    ),
                )
                .await;
            return;
        }
    };

    const OAUTH_LOGIN_TIMEOUT_SECS: u64 = 300;
    let oauth_login_timeout = Duration::from_secs(OAUTH_LOGIN_TIMEOUT_SECS);

    for (index, dependency) in pending.into_iter().enumerate() {
        let name = dependency.value.trim();
        let step = index + 1;
        let progress_prefix = format!("MCP dependency {step}/{total}: {name}");
        session
            .notify_background_event(turn, format!("{progress_prefix} - starting"))
            .await;
        if !is_valid_mcp_server_name(name) {
            warn!("Invalid MCP server name '{name}' for skill '{skill_name}'.");
            session
                .notify_background_event(
                    turn,
                    format!("{progress_prefix} - skipped (invalid name)"),
                )
                .await;
            continue;
        }

        let auth_entry = auth_entries.get(name);
        let auth_missing = auth_entry.is_some_and(mcp_auth_missing);
        let needs_oauth_login =
            auth_entry.is_none() || auth_entry.is_some_and(mcp_needs_oauth_login);

        let mut installed_servers: HashMap<String, McpServerConfig> = HashMap::new();
        let mut oauth_transport: Option<McpServerTransportConfig> = None;
        let mut config_changed = false;
        let mut did_oauth_attempt = false;

        if let Some(existing) = servers.get(name).cloned() {
            if existing.enabled {
                if needs_oauth_login {
                    oauth_transport = Some(existing.transport.clone());
                }
            } else {
                let mut updated = existing.clone();
                updated.enabled = true;
                updated.disabled_reason = None;
                servers.insert(name.to_string(), updated.clone());
                installed_servers.insert(name.to_string(), updated.clone());
                oauth_transport = Some(updated.transport.clone());
                config_changed = true;
            }
        } else {
            let transport = match resolve_mcp_transport(dependency) {
                Ok(transport) => transport,
                Err(err) => {
                    warn!(
                        "Failed to resolve MCP transport for '{name}' (skill={skill_name}): {err}"
                    );
                    session
                        .notify_background_event(
                            turn,
                            format!("{progress_prefix} - failed to resolve transport: {err}"),
                        )
                        .await;
                    continue;
                }
            };
            let new_entry = McpServerConfig {
                transport: transport.clone(),
                enabled: true,
                disabled_reason: None,
                startup_timeout_sec: None,
                tool_timeout_sec: None,
                enabled_tools: None,
                disabled_tools: None,
            };

            servers.insert(name.to_string(), new_entry.clone());
            installed_servers.insert(name.to_string(), new_entry.clone());
            oauth_transport = Some(transport);
            config_changed = true;
        }

        if config_changed
            && let Err(err) = ConfigEditsBuilder::new(&codex_home)
                .replace_mcp_servers(&servers)
                .apply()
                .await
        {
            warn!("Failed to write MCP servers for skill '{skill_name}': {err}");
            session
                .notify_background_event(
                    turn,
                    format!("{progress_prefix} - failed to update config: {err}"),
                )
                .await;
            continue;
        }

        if config_changed {
            session
                .notify_background_event(turn, format!("{progress_prefix} - config updated"))
                .await;
        }

        if needs_oauth_login
            && let Some(McpServerTransportConfig::StreamableHttp {
                url,
                bearer_token_env_var: None,
                http_headers,
                env_http_headers,
            }) = oauth_transport
        {
            did_oauth_attempt = true;
            session
                .notify_background_event(turn, format!("{progress_prefix} - starting OAuth login"))
                .await;
            match supports_oauth_login(&url).await {
                Ok(true) => {
                    match timeout(
                        oauth_login_timeout,
                        perform_oauth_login(
                            name,
                            &url,
                            config.mcp_oauth_credentials_store_mode,
                            http_headers.clone(),
                            env_http_headers.clone(),
                            &[],
                            config.mcp_oauth_callback_port,
                        ),
                    )
                    .await
                    {
                        Ok(Ok(())) => {
                            session
                                .notify_background_event(
                                    turn,
                                    format!("{progress_prefix} - OAuth login complete"),
                                )
                                .await;
                        }
                        Ok(Err(err)) => {
                            warn!(
                                "OAuth login failed for MCP server '{name}' (skill={skill_name}): {err}"
                            );
                            session
                                .notify_background_event(
                                    turn,
                                    format!("{progress_prefix} - OAuth login failed: {err}"),
                                )
                                .await;
                        }
                        Err(_) => {
                            warn!(
                                "OAuth login timed out for MCP server '{name}' (skill={skill_name}) after {OAUTH_LOGIN_TIMEOUT_SECS} seconds"
                            );
                            session
                                .notify_background_event(
                                    turn,
                                    format!(
                                        "{progress_prefix} - OAuth login timed out after {OAUTH_LOGIN_TIMEOUT_SECS} seconds"
                                    ),
                                )
                                .await;
                        }
                    }
                }
                Ok(false) => {
                    session
                        .notify_background_event(
                            turn,
                            format!("{progress_prefix} - OAuth not supported; skipping login"),
                        )
                        .await;
                }
                Err(err) => {
                    warn!(
                        "OAuth support check failed for MCP server '{name}' (skill={skill_name}): {err}"
                    );
                    session
                        .notify_background_event(
                            turn,
                            format!("{progress_prefix} - OAuth check failed: {err}"),
                        )
                        .await;
                }
            }
        }

        if auth_missing && !needs_oauth_login && installed_servers.is_empty() {
            let env_var = auth_entry.and_then(|entry| bearer_token_env_var_name(&entry.config));
            let message = env_var.map_or_else(
                || format!("{progress_prefix} - authentication required; set bearer token env var"),
                |env_var| format!("{progress_prefix} - missing {env_var}; set it and retry"),
            );
            session.notify_background_event(turn, message).await;
            continue;
        }

        if installed_servers.is_empty() {
            let message = if did_oauth_attempt {
                format!("{progress_prefix} - authentication attempt complete")
            } else {
                format!("{progress_prefix} - no install steps required")
            };
            session.notify_background_event(turn, message).await;
            continue;
        }

        let outcomes =
            hot_reload_installed_mcp_servers(session, turn, installed_servers, &config).await;
        if let Some((_, result)) = outcomes
            .into_iter()
            .find(|(server_name, _)| server_name == name)
        {
            match result {
                Ok(()) => {
                    session
                        .notify_background_event(
                            turn,
                            format!("{progress_prefix} - startup complete"),
                        )
                        .await;
                }
                Err(error) => {
                    session
                        .notify_background_event(
                            turn,
                            format!("{progress_prefix} - startup failed: {error}"),
                        )
                        .await;
                }
            }
        }
    }
}

async fn hot_reload_installed_mcp_servers(
    session: &Session,
    turn: &TurnContext,
    installed_servers: HashMap<String, McpServerConfig>,
    config: &Config,
) -> Vec<(String, Result<(), String>)> {
    let server_names = installed_servers.keys().cloned().collect::<Vec<_>>();
    let auth_statuses = compute_auth_statuses(
        installed_servers.iter(),
        config.mcp_oauth_credentials_store_mode,
    )
    .await;
    let sandbox_state = SandboxState {
        sandbox_policy: turn.sandbox_policy.clone(),
        codex_linux_sandbox_exe: turn.codex_linux_sandbox_exe.clone(),
        sandbox_cwd: turn.cwd.clone(),
    };
    let cancel_token = session
        .services
        .mcp_startup_cancellation_token
        .lock()
        .await
        .clone();
    let tx_event = session.get_tx_event();
    session
        .services
        .mcp_connection_manager
        .write()
        .await
        .add_servers(
            &installed_servers,
            config.mcp_oauth_credentials_store_mode,
            auth_statuses,
            tx_event,
            cancel_token,
            sandbox_state,
        )
        .await;

    if server_names.is_empty() {
        return Vec::new();
    }

    let startup_futures = session
        .services
        .mcp_connection_manager
        .read()
        .await
        .startup_futures_for(&server_names);
    let outcomes = join_all(startup_futures).await;
    for (server_name, result) in &outcomes {
        if let Err(error) = result {
            warn!("MCP server '{server_name}' failed to start: {error}");
        }
    }
    outcomes
}

fn resolve_mcp_transport(
    dependency: &SkillToolDependency,
) -> Result<McpServerTransportConfig, String> {
    let transport = dependency.transport.as_deref();
    let url = dependency.url.as_deref().map(str::trim);
    let value = dependency.value.trim();

    if transport
        .map(|transport| transport.eq_ignore_ascii_case("streamable_http"))
        .unwrap_or(false)
    {
        let url = url
            .filter(|url| !url.is_empty())
            .ok_or_else(|| "missing URL for streamable_http transport".to_string())?;
        return Ok(McpServerTransportConfig::StreamableHttp {
            url: url.to_string(),
            bearer_token_env_var: None,
            http_headers: None,
            env_http_headers: None,
        });
    }

    if transport
        .map(|transport| transport.eq_ignore_ascii_case("stdio"))
        .unwrap_or(false)
    {
        let command_line = url
            .filter(|command_line| !command_line.is_empty())
            .ok_or_else(|| "missing command for stdio transport".to_string())?;
        let tokens = shlex_split(command_line).unwrap_or_else(|| {
            command_line
                .split_whitespace()
                .map(ToString::to_string)
                .collect()
        });
        let mut iter = tokens.into_iter();
        let command = iter
            .next()
            .ok_or_else(|| "missing command for stdio transport".to_string())?;
        let args = iter.collect::<Vec<_>>();
        return Ok(McpServerTransportConfig::Stdio {
            command,
            args,
            env: None,
            env_vars: Vec::new(),
            cwd: None,
        });
    }

    if transport.is_none()
        && let Some(url) = url.filter(|url| !url.is_empty())
    {
        return Ok(McpServerTransportConfig::StreamableHttp {
            url: url.to_string(),
            bearer_token_env_var: None,
            http_headers: None,
            env_http_headers: None,
        });
    }

    Err(format!(
        "unsupported MCP dependency (name={value}, transport={transport:?}, url={url:?})"
    ))
}

fn is_valid_mcp_server_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}
