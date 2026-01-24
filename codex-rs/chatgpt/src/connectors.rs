use std::collections::HashMap;

use codex_core::AuthManager;
use codex_core::CodexAuth;
use codex_core::config::Config;
use codex_core::features::Feature;
use serde::Deserialize;

use crate::chatgpt_client::chatgpt_get_request;

pub use codex_core::connectors::AppInfo;
pub use codex_core::connectors::connector_display_label;
use codex_core::connectors::connector_install_url;
pub use codex_core::connectors::list_accessible_connectors_from_mcp_tools;
use codex_core::connectors::merge_connectors;

#[derive(Debug, Deserialize)]
struct DirectoryListResponse {
    apps: Vec<DirectoryApp>,
    #[serde(default)]
    next_token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct DirectoryApp {
    id: String,
    name: String,
    description: Option<String>,
    logo_url: Option<String>,
    logo_url_dark: Option<String>,
    distribution_channel: Option<String>,
}

const ECOSYSTEM_DIRECTORY_DISTRIBUTION_CHANNEL: &str = "ECOSYSTEM_DIRECTORY";

async fn chatgpt_connectors_auth(config: &Config) -> Option<CodexAuth> {
    if !config.features.enabled(Feature::Connectors) {
        return None;
    }
    let auth_manager = AuthManager::new(
        config.codex_home.clone(),
        false,
        config.cli_auth_credentials_store_mode,
    );
    let auth = auth_manager.auth().await?;
    if auth.is_api_key() {
        return None;
    }
    Some(auth)
}

pub async fn list_connectors(config: &Config) -> anyhow::Result<Vec<AppInfo>> {
    let auth = match chatgpt_connectors_auth(config).await {
        Some(auth) => auth,
        None => return Ok(Vec::new()),
    };
    let include_workspace = auth.get_account_id().is_some();
    let (connectors_result, accessible_result) = tokio::join!(
        list_all_connectors_with_workspace(config, include_workspace),
        list_accessible_connectors_from_mcp_tools(config),
    );
    let connectors = connectors_result?;
    let accessible = accessible_result?;
    Ok(merge_connectors(connectors, accessible))
}

pub async fn list_all_connectors(config: &Config) -> anyhow::Result<Vec<AppInfo>> {
    let auth = match chatgpt_connectors_auth(config).await {
        Some(auth) => auth,
        None => return Ok(Vec::new()),
    };
    list_all_connectors_with_workspace(config, auth.get_account_id().is_some()).await
}

async fn list_all_connectors_with_workspace(
    config: &Config,
    include_workspace: bool,
) -> anyhow::Result<Vec<AppInfo>> {
    let mut apps = list_directory_connectors(config).await?;
    if include_workspace {
        let workspace_apps = list_workspace_connectors(config).await?;
        apps.extend(workspace_apps);
    }
    let apps = apps
        .into_iter()
        .filter(|app| {
            app.distribution_channel.as_deref() != Some(ECOSYSTEM_DIRECTORY_DISTRIBUTION_CHANNEL)
        })
        .collect::<Vec<_>>();
    let apps = merge_directory_apps(apps);
    let mut connectors: Vec<AppInfo> = apps
        .into_iter()
        .map(|app| AppInfo {
            id: app.id,
            name: app.name,
            description: app.description,
            logo_url: app.logo_url,
            logo_url_dark: app.logo_url_dark,
            install_url: None,
            distribution_channel: app.distribution_channel,
            is_accessible: false,
        })
        .collect();
    for connector in &mut connectors {
        let install_url = connector_install_url(&connector.name, &connector.id);
        connector.name = normalize_connector_name(&connector.name, &connector.id);
        connector.description = normalize_connector_value(connector.description.as_deref());
        connector.install_url = Some(install_url);
    }
    connectors.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(connectors)
}

async fn list_directory_connectors(config: &Config) -> anyhow::Result<Vec<DirectoryApp>> {
    let mut apps = Vec::new();
    let mut next_token: Option<String> = None;
    loop {
        let path = match next_token.as_deref() {
            Some(token) => {
                let encoded_token = urlencoding::encode(token);
                format!("/connectors/directory/list?tier=categorized&token={encoded_token}")
            }
            None => "/connectors/directory/list?tier=categorized".to_string(),
        };
        let response: DirectoryListResponse = chatgpt_get_request(config, path).await?;
        apps.extend(response.apps);
        next_token = response
            .next_token
            .map(|token| token.trim().to_string())
            .filter(|token| !token.is_empty());
        if next_token.is_none() {
            break;
        }
    }
    Ok(apps)
}

async fn list_workspace_connectors(config: &Config) -> anyhow::Result<Vec<DirectoryApp>> {
    let response: anyhow::Result<DirectoryListResponse> =
        chatgpt_get_request(config, "/connectors/directory/list_workspace".to_string()).await;
    match response {
        Ok(response) => Ok(response.apps),
        Err(_) => Ok(Vec::new()),
    }
}

fn merge_directory_apps(apps: Vec<DirectoryApp>) -> Vec<DirectoryApp> {
    let mut merged: HashMap<String, DirectoryApp> = HashMap::new();
    for app in apps {
        if let Some(existing) = merged.get_mut(&app.id) {
            merge_directory_app(existing, app);
        } else {
            merged.insert(app.id.clone(), app);
        }
    }
    merged.into_values().collect()
}

fn merge_directory_app(existing: &mut DirectoryApp, incoming: DirectoryApp) {
    let DirectoryApp {
        id: _,
        name,
        description,
        logo_url,
        logo_url_dark,
        distribution_channel,
    } = incoming;

    let incoming_name_is_empty = name.trim().is_empty();
    if existing.name.trim().is_empty() && !incoming_name_is_empty {
        existing.name = name;
    }

    let incoming_description_present = description
        .as_deref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    let existing_description_present = existing
        .description
        .as_deref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    if !existing_description_present && incoming_description_present {
        existing.description = description;
    }

    if existing.logo_url.is_none() && logo_url.is_some() {
        existing.logo_url = logo_url;
    }
    if existing.logo_url_dark.is_none() && logo_url_dark.is_some() {
        existing.logo_url_dark = logo_url_dark;
    }
    if existing.distribution_channel.is_none() && distribution_channel.is_some() {
        existing.distribution_channel = distribution_channel;
    }
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
