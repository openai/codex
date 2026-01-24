use codex_core::AuthManager;
use codex_core::CodexAuth;
use codex_core::config::Config;
use codex_core::features::Feature;
use serde::Deserialize;
use serde::Serialize;

use crate::chatgpt_client::chatgpt_post_request;
use crate::chatgpt_token::get_chatgpt_token_data;
use crate::chatgpt_token::init_chatgpt_token_from_auth;

pub use codex_core::connectors::AppInfo;
pub use codex_core::connectors::connector_display_label;
use codex_core::connectors::connector_install_url;
pub use codex_core::connectors::list_accessible_connectors_from_mcp_tools;
use codex_core::connectors::merge_connectors;

#[derive(Debug, Serialize)]
struct ListConnectorsRequest {
    principals: Vec<Principal>,
}

#[derive(Debug, Serialize)]
struct Principal {
    #[serde(rename = "type")]
    principal_type: PrincipalType,
    id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum PrincipalType {
    User,
}

#[derive(Debug, Deserialize)]
struct ListConnectorsResponse {
    connectors: Vec<AppInfo>,
}

async fn connectors_allowed(config: &Config) -> bool {
    if !config.features.enabled(Feature::Connectors) {
        return false;
    }
    let auth_manager = AuthManager::new(
        config.codex_home.clone(),
        false,
        config.cli_auth_credentials_store_mode,
    );
    let auth = auth_manager.auth().await;
    !auth.as_ref().is_some_and(CodexAuth::is_api_key)
}

pub async fn list_connectors(config: &Config) -> anyhow::Result<Vec<AppInfo>> {
    if !connectors_allowed(config).await {
        return Ok(Vec::new());
    }
    let (connectors_result, accessible_result) = tokio::join!(
        list_all_connectors(config),
        list_accessible_connectors_from_mcp_tools(config),
    );
    let connectors = connectors_result?;
    let accessible = accessible_result?;
    Ok(merge_connectors(connectors, accessible))
}

pub async fn list_all_connectors(config: &Config) -> anyhow::Result<Vec<AppInfo>> {
    if !connectors_allowed(config).await {
        return Ok(Vec::new());
    }
    init_chatgpt_token_from_auth(&config.codex_home, config.cli_auth_credentials_store_mode)
        .await?;

    let token_data =
        get_chatgpt_token_data().ok_or_else(|| anyhow::anyhow!("ChatGPT token not available"))?;
    let user_id = token_data
        .id_token
        .chatgpt_user_id
        .as_deref()
        .ok_or_else(|| {
            anyhow::anyhow!("ChatGPT user ID not available, please re-run `codex login`")
        })?;
    let account_id = token_data
        .id_token
        .chatgpt_account_id
        .as_deref()
        .ok_or_else(|| {
            anyhow::anyhow!("ChatGPT account ID not available, please re-run `codex login`")
        })?;
    let principal_id = format!("{user_id}__{account_id}");
    let request = ListConnectorsRequest {
        principals: vec![Principal {
            principal_type: PrincipalType::User,
            id: principal_id,
        }],
    };
    let response: ListConnectorsResponse = chatgpt_post_request(
        config,
        token_data.access_token.as_str(),
        account_id,
        "/aip/connectors/list_accessible?skip_actions=true&external_logos=true",
        &request,
    )
    .await?;
    let mut connectors = response.connectors;
    for connector in &mut connectors {
        let install_url = match connector.install_url.take() {
            Some(install_url) => install_url,
            None => connector_install_url(&connector.name, &connector.id),
        };
        connector.name = normalize_connector_name(&connector.name, &connector.id);
        connector.description = normalize_connector_value(connector.description.as_deref());
        connector.install_url = Some(install_url);
        connector.is_accessible = false;
    }
    connectors.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(connectors)
}

fn normalize_connector_name(name: &str, connector_id: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        connector_id.to_string()
    } else {
        trimmed.to_string()
    }
}

fn normalize_connector_value(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}
