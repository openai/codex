use std::collections::HashMap;
use std::collections::HashSet;

use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::request_user_input::RequestUserInputArgs;
use codex_protocol::request_user_input::RequestUserInputQuestion;
use codex_protocol::request_user_input::RequestUserInputQuestionOption;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_rmcp_client::perform_oauth_login;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use super::auth::McpOAuthLoginSupport;
use super::auth::oauth_login_support;
use super::effective_mcp_servers;
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
use crate::skills::SkillMetadata;
use crate::skills::model::SkillToolDependency;

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
        .filter(|(name, config)| !prompted.contains(&canonical_mcp_server_key(name, config)))
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
        is_secret: false,
        options: Some(vec![
            RequestUserInputQuestionOption {
                label: MCP_DEPENDENCY_OPTION_INSTALL.to_string(),
                description:
                    "Install and enable the missing MCP servers in your global config."
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

    let prompted_keys = missing
        .iter()
        .map(|(name, config)| canonical_mcp_server_key(name, config));
    sess.record_mcp_dependency_prompted(prompted_keys).await;

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

    let config = turn_context.config.clone();
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

    // Refresh from the effective merged MCP map (global + repo + managed) and
    // overlay the updated global servers so we don't drop repo-scoped servers.
    let auth = sess.services.auth_manager.auth().await;
    let mut refresh_servers = effective_mcp_servers(config, auth.as_ref());
    for (name, server_config) in &servers {
        refresh_servers
            .entry(name.clone())
            .or_insert_with(|| server_config.clone());
    }
    sess.refresh_mcp_servers_now(
        turn_context,
        refresh_servers,
        config.mcp_oauth_credentials_store_mode,
    )
    .await;
}

fn canonical_mcp_key(transport: &str, identifier: &str, fallback: &str) -> String {
    let identifier = identifier.trim();
    if identifier.is_empty() {
        fallback.to_string()
    } else {
        format!("mcp__{transport}__{identifier}")
    }
}

fn canonical_mcp_server_key(name: &str, config: &McpServerConfig) -> String {
    match &config.transport {
        McpServerTransportConfig::Stdio { command, .. } => {
            canonical_mcp_key("stdio", command, name)
        }
        McpServerTransportConfig::StreamableHttp { url, .. } => {
            canonical_mcp_key("streamable_http", url, name)
        }
    }
}

fn canonical_mcp_dependency_key(dependency: &SkillToolDependency) -> Result<String, String> {
    let transport = dependency.transport.as_deref().unwrap_or("streamable_http");
    if transport.eq_ignore_ascii_case("streamable_http") {
        let url = dependency
            .url
            .as_ref()
            .ok_or_else(|| "missing url for streamable_http dependency".to_string())?;
        return Ok(canonical_mcp_key("streamable_http", url, &dependency.value));
    }
    if transport.eq_ignore_ascii_case("stdio") {
        let command = dependency
            .command
            .as_ref()
            .ok_or_else(|| "missing command for stdio dependency".to_string())?;
        return Ok(canonical_mcp_key("stdio", command, &dependency.value));
    }
    Err(format!("unsupported transport {transport}"))
}

pub(crate) fn collect_missing_mcp_dependencies(
    mentioned_skills: &[SkillMetadata],
    installed: &HashMap<String, McpServerConfig>,
) -> HashMap<String, McpServerConfig> {
    let mut missing = HashMap::new();
    let installed_keys: HashSet<String> = installed
        .iter()
        .map(|(name, config)| canonical_mcp_server_key(name, config))
        .collect();
    let mut seen_canonical_keys = HashSet::new();

    for skill in mentioned_skills {
        let Some(dependencies) = skill.dependencies.as_ref() else {
            continue;
        };

        for tool in &dependencies.tools {
            if !tool.r#type.eq_ignore_ascii_case("mcp") {
                continue;
            }
            let dependency_key = match canonical_mcp_dependency_key(tool) {
                Ok(key) => key,
                Err(err) => {
                    let dependency = tool.value.as_str();
                    let skill_name = skill.name.as_str();
                    warn!(
                        "unable to auto-install MCP dependency {dependency} for skill {skill_name}: {err}",
                    );
                    continue;
                }
            };
            if installed_keys.contains(&dependency_key)
                || seen_canonical_keys.contains(&dependency_key)
            {
                continue;
            }

            let config = match mcp_dependency_to_server_config(tool) {
                Ok(config) => config,
                Err(err) => {
                    let dependency = dependency_key.as_str();
                    let skill_name = skill.name.as_str();
                    warn!(
                        "unable to auto-install MCP dependency {dependency} for skill {skill_name}: {err}",
                    );
                    continue;
                }
            };

            missing.insert(tool.value.clone(), config);
            seen_canonical_keys.insert(dependency_key);
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
            required: false,
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
            required: false,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::model::SkillDependencies;
    use codex_protocol::protocol::SkillScope;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    fn skill_with_tools(tools: Vec<SkillToolDependency>) -> SkillMetadata {
        SkillMetadata {
            name: "skill".to_string(),
            description: "skill".to_string(),
            short_description: None,
            interface: None,
            dependencies: Some(SkillDependencies { tools }),
            policy: None,
            permission_profile: None,
            path: PathBuf::from("skill"),
            scope: SkillScope::User,
        }
    }

    #[test]
    fn collect_missing_respects_canonical_installed_key() {
        let url = "https://example.com/mcp".to_string();
        let skills = vec![skill_with_tools(vec![SkillToolDependency {
            r#type: "mcp".to_string(),
            value: "github".to_string(),
            description: None,
            transport: Some("streamable_http".to_string()),
            command: None,
            url: Some(url.clone()),
        }])];
        let installed = HashMap::from([(
            "alias".to_string(),
            McpServerConfig {
                transport: McpServerTransportConfig::StreamableHttp {
                    url,
                    bearer_token_env_var: None,
                    http_headers: None,
                    env_http_headers: None,
                },
                enabled: true,
                required: false,
                disabled_reason: None,
                startup_timeout_sec: None,
                tool_timeout_sec: None,
                enabled_tools: None,
                disabled_tools: None,
                scopes: None,
            },
        )]);

        assert_eq!(
            collect_missing_mcp_dependencies(&skills, &installed),
            HashMap::new()
        );
    }

    #[test]
    fn collect_missing_dedupes_by_canonical_key_but_preserves_original_name() {
        let url = "https://example.com/one".to_string();
        let skills = vec![skill_with_tools(vec![
            SkillToolDependency {
                r#type: "mcp".to_string(),
                value: "alias-one".to_string(),
                description: None,
                transport: Some("streamable_http".to_string()),
                command: None,
                url: Some(url.clone()),
            },
            SkillToolDependency {
                r#type: "mcp".to_string(),
                value: "alias-two".to_string(),
                description: None,
                transport: Some("streamable_http".to_string()),
                command: None,
                url: Some(url.clone()),
            },
        ])];

        let expected = HashMap::from([(
            "alias-one".to_string(),
            McpServerConfig {
                transport: McpServerTransportConfig::StreamableHttp {
                    url,
                    bearer_token_env_var: None,
                    http_headers: None,
                    env_http_headers: None,
                },
                enabled: true,
                required: false,
                disabled_reason: None,
                startup_timeout_sec: None,
                tool_timeout_sec: None,
                enabled_tools: None,
                disabled_tools: None,
                scopes: None,
            },
        )]);

        assert_eq!(
            collect_missing_mcp_dependencies(&skills, &HashMap::new()),
            expected
        );
    }
}
