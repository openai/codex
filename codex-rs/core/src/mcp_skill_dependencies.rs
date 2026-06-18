use std::collections::HashMap;

use codex_config::ConfigEditsBuilder;
use codex_config::McpServerConfig;
use codex_config::load_global_mcp_servers;
use codex_login::default_client::is_first_party_originator;
use codex_login::default_client::originator;
use codex_protocol::request_user_input::RequestUserInputArgs;
use codex_protocol::request_user_input::RequestUserInputQuestion;
use codex_protocol::request_user_input::RequestUserInputQuestionOption;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_rmcp_client::perform_oauth_login;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use codex_mcp::ElicitationReviewerHandle;
use codex_mcp::McpOAuthLoginSupport;
use codex_mcp::McpPermissionPromptAutoApproveContext;
use codex_mcp::McpServerDependencies;
use codex_mcp::canonical_server_key;
use codex_mcp::mcp_permission_prompt_is_auto_approved;
use codex_mcp::oauth_login_support;
use codex_mcp::resolve_oauth_scopes;
use codex_mcp::should_retry_without_scopes;

const SKILL_MCP_DEPENDENCY_PROMPT_ID: &str = "skill_mcp_dependency_install";
const MCP_DEPENDENCY_OPTION_INSTALL: &str = "Install";
const MCP_DEPENDENCY_OPTION_SKIP: &str = "Continue anyway";

pub(crate) async fn maybe_prompt_and_install_mcp_dependencies(
    sess: &Session,
    turn_context: &TurnContext,
    cancellation_token: &CancellationToken,
    dependencies: &McpServerDependencies,
    elicitation_reviewer: Option<ElicitationReviewerHandle>,
) {
    let originator_value = originator().value;
    if !is_first_party_originator(originator_value.as_str()) {
        // Only support first-party clients for now.
        return;
    }

    let config = turn_context.config.clone();
    if dependencies.is_empty()
        || !config
            .features
            .enabled(codex_features::Feature::SkillMcpDependencyInstall)
    {
        return;
    }

    let installed = sess.runtime_mcp_servers(config.as_ref()).await;
    let missing = dependencies.missing_from(&installed);
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
        maybe_install_mcp_dependencies(
            sess,
            turn_context,
            config.as_ref(),
            dependencies,
            elicitation_reviewer,
        )
        .await;
    }
}

pub(crate) async fn maybe_install_mcp_dependencies(
    sess: &Session,
    turn_context: &TurnContext,
    config: &crate::config::Config,
    dependencies: &McpServerDependencies,
    elicitation_reviewer: Option<ElicitationReviewerHandle>,
) {
    if dependencies.is_empty()
        || !config
            .features
            .enabled(codex_features::Feature::SkillMcpDependencyInstall)
    {
        return;
    }

    let codex_home = config.codex_home.clone();
    let installed = sess.runtime_mcp_servers(config).await;
    let missing = dependencies.missing_from(&installed);
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

        let resolved_scopes = resolve_oauth_scopes(
            /*explicit_scopes*/ None,
            server_config.scopes.clone(),
            oauth_config.discovered_scopes.clone(),
        );
        let oauth_client_id = server_config.oauth_client_id();
        let first_attempt = perform_oauth_login(
            &name,
            &oauth_config.url,
            config.mcp_oauth_credentials_store_mode,
            config.auth_keyring_backend_kind(),
            oauth_config.http_headers.clone(),
            oauth_config.env_http_headers.clone(),
            &resolved_scopes.scopes,
            oauth_client_id,
            server_config.oauth_resource.as_deref(),
            config.mcp_oauth_callback_port,
            config.mcp_oauth_callback_url.as_deref(),
        )
        .await;

        if let Err(err) = first_attempt {
            if should_retry_without_scopes(&resolved_scopes, &err) {
                if let Err(err) = perform_oauth_login(
                    &name,
                    &oauth_config.url,
                    config.mcp_oauth_credentials_store_mode,
                    config.auth_keyring_backend_kind(),
                    oauth_config.http_headers,
                    oauth_config.env_http_headers,
                    &[],
                    oauth_client_id,
                    server_config.oauth_resource.as_deref(),
                    config.mcp_oauth_callback_port,
                    config.mcp_oauth_callback_url.as_deref(),
                )
                .await
                {
                    warn!("failed to login to MCP dependency {name}: {err}");
                }
            } else {
                warn!("failed to login to MCP dependency {name}: {err}");
            }
        }
    }

    let mut refresh_config = config.clone();
    let mut configured_servers = config.mcp_servers.get().clone();
    for (name, server_config) in &servers {
        configured_servers
            .entry(name.clone())
            .or_insert_with(|| server_config.clone());
    }
    if let Err(err) = refresh_config.mcp_servers.set(configured_servers) {
        warn!("failed to refresh MCP dependencies for mentioned skills: {err}");
        return;
    }
    let refresh_servers = sess.runtime_mcp_servers(&refresh_config).await;
    sess.refresh_mcp_servers_now(
        turn_context,
        refresh_servers,
        config.mcp_oauth_credentials_store_mode,
        config.auth_keyring_backend_kind(),
        elicitation_reviewer,
    )
    .await;
}

async fn should_install_mcp_dependencies(
    sess: &Session,
    turn_context: &TurnContext,
    missing: &HashMap<String, McpServerConfig>,
    cancellation_token: &CancellationToken,
) -> bool {
    if mcp_permission_prompt_is_auto_approved(
        turn_context.approval_policy.value(),
        &turn_context.permission_profile(),
        McpPermissionPromptAutoApproveContext::default(),
    ) {
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
        auto_resolution_ms: None,
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
        .map(|(name, config)| canonical_server_key(name, config));
    sess.record_mcp_dependency_prompted(prompted_keys).await;

    install
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
        .filter(|(name, config)| !prompted.contains(&canonical_server_key(name, config)))
        .map(|(name, config)| (name.clone(), config.clone()))
        .collect()
}

fn format_missing_mcp_dependencies(missing: &HashMap<String, McpServerConfig>) -> String {
    let mut names = missing.keys().cloned().collect::<Vec<_>>();
    names.sort();
    names.join(", ")
}
