pub mod auth;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

use async_channel::unbounded;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::McpListToolsResponseEvent;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::request_user_input::RequestUserInputArgs;
use codex_protocol::request_user_input::RequestUserInputQuestion;
use codex_protocol::request_user_input::RequestUserInputQuestionOption;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_rmcp_client::perform_oauth_login;
use mcp_types::Tool as McpTool;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::AuthManager;
use crate::CodexAuth;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::config::Config;
use crate::config::edit::ConfigEditsBuilder;
use crate::config::load_global_mcp_servers;
use crate::config::types::McpServerConfig;
use crate::config::types::McpServerTransportConfig;
use crate::default_client::is_first_party_originator;
use crate::default_client::originator;
use crate::features::Feature;
use crate::mcp::auth::McpOAuthLoginSupport;
use crate::mcp::auth::compute_auth_statuses;
use crate::mcp::auth::oauth_login_support;
use crate::mcp_connection_manager::McpConnectionManager;
use crate::mcp_connection_manager::SandboxState;
use crate::skills::SkillMetadata;
use crate::skills::model::SkillToolDependency;

const MCP_TOOL_NAME_PREFIX: &str = "mcp";
const MCP_TOOL_NAME_DELIMITER: &str = "__";
pub(crate) const CODEX_APPS_MCP_SERVER_NAME: &str = "codex_apps_mcp";
const CODEX_CONNECTORS_TOKEN_ENV_VAR: &str = "CODEX_CONNECTORS_TOKEN";

fn codex_apps_mcp_bearer_token_env_var() -> Option<String> {
    match env::var(CODEX_CONNECTORS_TOKEN_ENV_VAR) {
        Ok(value) if !value.trim().is_empty() => Some(CODEX_CONNECTORS_TOKEN_ENV_VAR.to_string()),
        Ok(_) => None,
        Err(env::VarError::NotPresent) => None,
        Err(env::VarError::NotUnicode(_)) => Some(CODEX_CONNECTORS_TOKEN_ENV_VAR.to_string()),
    }
}

fn codex_apps_mcp_bearer_token(auth: Option<&CodexAuth>) -> Option<String> {
    let token = auth.and_then(|auth| auth.get_token().ok())?;
    let token = token.trim();
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

fn codex_apps_mcp_http_headers(auth: Option<&CodexAuth>) -> Option<HashMap<String, String>> {
    let mut headers = HashMap::new();
    if let Some(token) = codex_apps_mcp_bearer_token(auth) {
        headers.insert("Authorization".to_string(), format!("Bearer {token}"));
    }
    if let Some(account_id) = auth.and_then(CodexAuth::get_account_id) {
        headers.insert("ChatGPT-Account-ID".to_string(), account_id);
    }
    if headers.is_empty() {
        None
    } else {
        Some(headers)
    }
}

fn codex_apps_mcp_url(base_url: &str) -> String {
    let mut base_url = base_url.trim_end_matches('/').to_string();
    if (base_url.starts_with("https://chatgpt.com")
        || base_url.starts_with("https://chat.openai.com"))
        && !base_url.contains("/backend-api")
    {
        base_url = format!("{base_url}/backend-api");
    }
    if base_url.contains("/backend-api") {
        format!("{base_url}/wham/apps")
    } else if base_url.contains("/api/codex") {
        format!("{base_url}/apps")
    } else {
        format!("{base_url}/api/codex/apps")
    }
}

fn codex_apps_mcp_server_config(config: &Config, auth: Option<&CodexAuth>) -> McpServerConfig {
    let bearer_token_env_var = codex_apps_mcp_bearer_token_env_var();
    let http_headers = if bearer_token_env_var.is_some() {
        None
    } else {
        codex_apps_mcp_http_headers(auth)
    };
    let url = codex_apps_mcp_url(&config.chatgpt_base_url);

    McpServerConfig {
        transport: McpServerTransportConfig::StreamableHttp {
            url,
            bearer_token_env_var,
            http_headers,
            env_http_headers: None,
        },
        enabled: true,
        disabled_reason: None,
        startup_timeout_sec: None,
        tool_timeout_sec: None,
        enabled_tools: None,
        disabled_tools: None,
        scopes: None,
    }
}

pub(crate) fn with_codex_apps_mcp(
    mut servers: HashMap<String, McpServerConfig>,
    connectors_enabled: bool,
    auth: Option<&CodexAuth>,
    config: &Config,
) -> HashMap<String, McpServerConfig> {
    if connectors_enabled {
        servers.insert(
            CODEX_APPS_MCP_SERVER_NAME.to_string(),
            codex_apps_mcp_server_config(config, auth),
        );
    } else {
        servers.remove(CODEX_APPS_MCP_SERVER_NAME);
    }
    servers
}

pub(crate) fn effective_mcp_servers(
    config: &Config,
    auth: Option<&CodexAuth>,
) -> HashMap<String, McpServerConfig> {
    with_codex_apps_mcp(
        config.mcp_servers.get().clone(),
        config.features.enabled(Feature::Connectors),
        auth,
        config,
    )
}

pub async fn collect_mcp_snapshot(config: &Config) -> McpListToolsResponseEvent {
    let auth_manager = AuthManager::shared(
        config.codex_home.clone(),
        false,
        config.cli_auth_credentials_store_mode,
    );
    let auth = auth_manager.auth().await;
    let mcp_servers = effective_mcp_servers(config, auth.as_ref());
    if mcp_servers.is_empty() {
        return McpListToolsResponseEvent {
            tools: HashMap::new(),
            resources: HashMap::new(),
            resource_templates: HashMap::new(),
            auth_statuses: HashMap::new(),
        };
    }

    let auth_status_entries =
        compute_auth_statuses(mcp_servers.iter(), config.mcp_oauth_credentials_store_mode).await;

    let mut mcp_connection_manager = McpConnectionManager::default();
    let (tx_event, rx_event) = unbounded();
    drop(rx_event);
    let cancel_token = CancellationToken::new();

    // Use ReadOnly sandbox policy for MCP snapshot collection (safest default)
    let sandbox_state = SandboxState {
        sandbox_policy: SandboxPolicy::ReadOnly,
        codex_linux_sandbox_exe: config.codex_linux_sandbox_exe.clone(),
        sandbox_cwd: env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
    };

    mcp_connection_manager
        .initialize(
            &mcp_servers,
            config.mcp_oauth_credentials_store_mode,
            auth_status_entries.clone(),
            tx_event,
            cancel_token.clone(),
            sandbox_state,
        )
        .await;

    let snapshot =
        collect_mcp_snapshot_from_manager(&mcp_connection_manager, auth_status_entries).await;

    cancel_token.cancel();

    snapshot
}

const SKILL_MCP_DEPENDENCY_PROMPT_ID: &str = "skill_mcp_dependency_install";
const MCP_DEPENDENCY_OPTION_INSTALL: &str = "Install";
const MCP_DEPENDENCY_OPTION_SKIP: &str = "Continue anyway";

fn is_full_access_mode(turn_context: &TurnContext) -> bool {
    matches!(turn_context.approval_policy, AskForApproval::Never)
        && matches!(
            turn_context.sandbox_policy,
            SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. }
        )
}

fn format_missing_mcp_dependencies(missing: &HashMap<String, McpServerConfig>) -> String {
    let mut names = missing.keys().cloned().collect::<Vec<_>>();
    names.sort();
    names.join(", ")
}

async fn filter_prompted_mcp_dependencies(
    sess: &Session,
    missing: &HashMap<String, McpServerConfig>,
) -> HashMap<String, McpServerConfig> {
    let prompted = sess.mcp_dependency_prompted().await;
    if prompted.is_empty() {
        return missing.clone();
    }

    missing
        .iter()
        .filter(|(name, _)| !prompted.contains(*name))
        .map(|(name, config)| (name.clone(), config.clone()))
        .collect()
}

async fn should_install_mcp_dependencies(
    sess: &Session,
    turn_context: &TurnContext,
    missing: &HashMap<String, McpServerConfig>,
    cancellation_token: &CancellationToken,
) -> bool {
    if is_full_access_mode(turn_context) {
        return true;
    }

    let server_list = format_missing_mcp_dependencies(missing);
    let question = RequestUserInputQuestion {
        id: SKILL_MCP_DEPENDENCY_PROMPT_ID.to_string(),
        header: "Install MCP servers?".to_string(),
        question: format!(
            "The following MCP servers are required by the selected skills but are not installed yet: {server_list}. Install them now?"
        ),
        is_other: false,
        options: Some(vec![
            RequestUserInputQuestionOption {
                label: MCP_DEPENDENCY_OPTION_INSTALL.to_string(),
                description: "Install and enable the missing MCP servers in your global config."
                    .to_string(),
            },
            RequestUserInputQuestionOption {
                label: MCP_DEPENDENCY_OPTION_SKIP.to_string(),
                description: "Skip installation for now and do not show again for these MCP servers in this session."
                    .to_string(),
            },
        ]),
    };
    let args = RequestUserInputArgs {
        questions: vec![question],
    };
    let sub_id = &turn_context.sub_id;
    let call_id = format!("mcp-deps-{sub_id}");
    let response_fut = sess.request_user_input(turn_context, call_id, args);
    let response = tokio::select! {
        biased;
        _ = cancellation_token.cancelled() => {
            let empty = RequestUserInputResponse {
                answers: HashMap::new(),
            };
            sess.notify_user_input_response(sub_id, empty.clone()).await;
            empty
        }
        response = response_fut => response.unwrap_or_else(|| RequestUserInputResponse {
            answers: HashMap::new(),
        }),
    };

    let install = response
        .answers
        .get(SKILL_MCP_DEPENDENCY_PROMPT_ID)
        .is_some_and(|answer| {
            answer
                .answers
                .iter()
                .any(|entry| entry == MCP_DEPENDENCY_OPTION_INSTALL)
        });

    sess.record_mcp_dependency_prompted(missing.keys().cloned())
        .await;

    install
}

pub(crate) async fn maybe_prompt_and_install_mcp_dependencies(
    sess: &Session,
    turn_context: &TurnContext,
    cancellation_token: &CancellationToken,
    mentioned_skills: &[SkillMetadata],
) {
    let originator_value = originator().value;
    if !is_first_party_originator(originator_value.as_str()) {
        // Only support first-party clients for now.
        return;
    }

    let config = turn_context.client.config();
    if mentioned_skills.is_empty() || !config.features.enabled(Feature::SkillMcpDependencyInstall) {
        return;
    }

    let installed = config.mcp_servers.get().clone();
    let missing = collect_missing_mcp_dependencies(mentioned_skills, &installed);
    if missing.is_empty() {
        return;
    }

    let unprompted_missing = filter_prompted_mcp_dependencies(sess, &missing).await;
    if unprompted_missing.is_empty() {
        return;
    }

    if should_install_mcp_dependencies(sess, turn_context, &unprompted_missing, cancellation_token)
        .await
    {
        maybe_install_mcp_dependencies(sess, turn_context, config.as_ref(), mentioned_skills).await;
    }
}

pub(crate) async fn maybe_install_mcp_dependencies(
    sess: &Session,
    turn_context: &TurnContext,
    config: &Config,
    mentioned_skills: &[SkillMetadata],
) {
    if mentioned_skills.is_empty() || !config.features.enabled(Feature::SkillMcpDependencyInstall) {
        return;
    }

    let codex_home = config.codex_home.clone();
    let installed = config.mcp_servers.get().clone();
    let missing = collect_missing_mcp_dependencies(mentioned_skills, &installed);
    if missing.is_empty() {
        return;
    }

    let mut servers = match load_global_mcp_servers(&codex_home).await {
        Ok(servers) => servers,
        Err(err) => {
            warn!("failed to load MCP servers while installing skill dependencies: {err}");
            return;
        }
    };

    let mut updated = false;
    let mut added = Vec::new();
    for (name, config) in missing {
        if servers.contains_key(&name) {
            continue;
        }
        servers.insert(name.clone(), config.clone());
        added.push((name, config));
        updated = true;
    }

    if !updated {
        return;
    }

    if let Err(err) = ConfigEditsBuilder::new(&codex_home)
        .replace_mcp_servers(&servers)
        .apply()
        .await
    {
        warn!("failed to persist MCP dependencies for mentioned skills: {err}");
        return;
    }

    for (name, server_config) in added {
        let oauth_config = match oauth_login_support(&server_config.transport).await {
            McpOAuthLoginSupport::Supported(config) => config,
            McpOAuthLoginSupport::Unsupported => continue,
            McpOAuthLoginSupport::Unknown(err) => {
                warn!("MCP server may or may not require login for dependency {name}: {err}");
                continue;
            }
        };

        sess.notify_background_event(
            turn_context,
            format!(
                "Authenticating MCP {name}... Follow instructions in your browser if prompted."
            ),
        )
        .await;

        if let Err(err) = perform_oauth_login(
            &name,
            &oauth_config.url,
            config.mcp_oauth_credentials_store_mode,
            oauth_config.http_headers,
            oauth_config.env_http_headers,
            &[],
            config.mcp_oauth_callback_port,
        )
        .await
        {
            warn!("failed to login to MCP dependency {name}: {err}");
        }
    }

    let refresh_servers = servers
        .iter()
        .map(|(name, config)| (name.clone(), config.clone()))
        .collect();
    sess.refresh_mcp_servers_now(
        turn_context,
        refresh_servers,
        config.mcp_oauth_credentials_store_mode,
    )
    .await;
}

pub(crate) fn collect_missing_mcp_dependencies(
    mentioned_skills: &[SkillMetadata],
    installed: &HashMap<String, McpServerConfig>,
) -> HashMap<String, McpServerConfig> {
    let mut missing = HashMap::new();

    for skill in mentioned_skills {
        let Some(dependencies) = skill.dependencies.as_ref() else {
            continue;
        };

        for tool in &dependencies.tools {
            if !tool.r#type.eq_ignore_ascii_case("mcp") {
                continue;
            }
            if installed.contains_key(&tool.value) || missing.contains_key(&tool.value) {
                continue;
            }

            let config = match mcp_dependency_to_server_config(tool) {
                Ok(config) => config,
                Err(err) => {
                    warn!(
                        "unable to auto-install MCP dependency {dependency} for skill {skill}: {err}",
                        dependency = tool.value.as_str(),
                        skill = skill.name.as_str()
                    );
                    continue;
                }
            };

            missing.insert(tool.value.clone(), config);
        }
    }

    missing
}

fn mcp_dependency_to_server_config(
    dependency: &SkillToolDependency,
) -> Result<McpServerConfig, String> {
    let transport = dependency.transport.as_deref().unwrap_or("streamable_http");
    if transport.eq_ignore_ascii_case("streamable_http") {
        let url = dependency
            .url
            .as_ref()
            .ok_or_else(|| "missing url for streamable_http dependency".to_string())?;
        return Ok(McpServerConfig {
            transport: McpServerTransportConfig::StreamableHttp {
                url: url.clone(),
                bearer_token_env_var: None,
                http_headers: None,
                env_http_headers: None,
            },
            enabled: true,
            disabled_reason: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
        });
    }

    if transport.eq_ignore_ascii_case("stdio") {
        let command = dependency
            .command
            .as_ref()
            .ok_or_else(|| "missing command for stdio dependency".to_string())?;
        return Ok(McpServerConfig {
            transport: McpServerTransportConfig::Stdio {
                command: command.clone(),
                args: Vec::new(),
                env: None,
                env_vars: Vec::new(),
                cwd: None,
            },
            enabled: true,
            disabled_reason: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
        });
    }

    Err(format!("unsupported transport {transport}"))
}

pub fn split_qualified_tool_name(qualified_name: &str) -> Option<(String, String)> {
    let mut parts = qualified_name.split(MCP_TOOL_NAME_DELIMITER);
    let prefix = parts.next()?;
    if prefix != MCP_TOOL_NAME_PREFIX {
        return None;
    }
    let server_name = parts.next()?;
    let tool_name: String = parts.collect::<Vec<_>>().join(MCP_TOOL_NAME_DELIMITER);
    if tool_name.is_empty() {
        return None;
    }
    Some((server_name.to_string(), tool_name))
}

pub fn group_tools_by_server(
    tools: &HashMap<String, McpTool>,
) -> HashMap<String, HashMap<String, McpTool>> {
    let mut grouped = HashMap::new();
    for (qualified_name, tool) in tools {
        if let Some((server_name, tool_name)) = split_qualified_tool_name(qualified_name) {
            grouped
                .entry(server_name)
                .or_insert_with(HashMap::new)
                .insert(tool_name, tool.clone());
        }
    }
    grouped
}

pub(crate) async fn collect_mcp_snapshot_from_manager(
    mcp_connection_manager: &McpConnectionManager,
    auth_status_entries: HashMap<String, crate::mcp::auth::McpAuthStatusEntry>,
) -> McpListToolsResponseEvent {
    let (tools, resources, resource_templates) = tokio::join!(
        mcp_connection_manager.list_all_tools(),
        mcp_connection_manager.list_all_resources(),
        mcp_connection_manager.list_all_resource_templates(),
    );

    let auth_statuses = auth_status_entries
        .iter()
        .map(|(name, entry)| (name.clone(), entry.auth_status))
        .collect();

    McpListToolsResponseEvent {
        tools: tools
            .into_iter()
            .map(|(name, tool)| (name, tool.tool))
            .collect(),
        resources,
        resource_templates,
        auth_statuses,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcp_types::ToolInputSchema;
    use pretty_assertions::assert_eq;

    fn make_tool(name: &str) -> McpTool {
        McpTool {
            annotations: None,
            description: None,
            input_schema: ToolInputSchema {
                properties: None,
                required: None,
                r#type: "object".to_string(),
            },
            name: name.to_string(),
            output_schema: None,
            title: None,
        }
    }

    #[test]
    fn split_qualified_tool_name_returns_server_and_tool() {
        assert_eq!(
            split_qualified_tool_name("mcp__alpha__do_thing"),
            Some(("alpha".to_string(), "do_thing".to_string()))
        );
    }

    #[test]
    fn split_qualified_tool_name_rejects_invalid_names() {
        assert_eq!(split_qualified_tool_name("other__alpha__do_thing"), None);
        assert_eq!(split_qualified_tool_name("mcp__alpha__"), None);
    }

    #[test]
    fn group_tools_by_server_strips_prefix_and_groups() {
        let mut tools = HashMap::new();
        tools.insert("mcp__alpha__do_thing".to_string(), make_tool("do_thing"));
        tools.insert(
            "mcp__alpha__nested__op".to_string(),
            make_tool("nested__op"),
        );
        tools.insert("mcp__beta__do_other".to_string(), make_tool("do_other"));

        let mut expected_alpha = HashMap::new();
        expected_alpha.insert("do_thing".to_string(), make_tool("do_thing"));
        expected_alpha.insert("nested__op".to_string(), make_tool("nested__op"));

        let mut expected_beta = HashMap::new();
        expected_beta.insert("do_other".to_string(), make_tool("do_other"));

        let mut expected = HashMap::new();
        expected.insert("alpha".to_string(), expected_alpha);
        expected.insert("beta".to_string(), expected_beta);

        assert_eq!(group_tools_by_server(&tools), expected);
    }
}
