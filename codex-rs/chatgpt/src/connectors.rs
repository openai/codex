use std::collections::HashMap;

use codex_core::config::Config;
use codex_core::features::Feature;
use serde::Deserialize;

use crate::chatgpt_client::chatgpt_get_request;
use crate::chatgpt_token::get_chatgpt_token_data;
use crate::chatgpt_token::init_chatgpt_token_from_auth;

pub use codex_core::connectors::AppInfo;
pub use codex_core::connectors::connector_display_label;
use codex_core::connectors::connector_install_url;
pub use codex_core::connectors::list_accessible_connectors_from_mcp_tools;
use codex_core::connectors::merge_connectors;

#[derive(Debug, Deserialize)]
struct DirectoryListResponse {
    apps: Vec<DirectoryApp>,
    #[serde(alias = "nextToken")]
    next_token: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct DirectoryApp {
    id: String,
    name: String,
    description: Option<String>,
    #[serde(alias = "logoUrl")]
    logo_url: Option<String>,
    #[serde(alias = "logoUrlDark")]
    logo_url_dark: Option<String>,
    #[serde(alias = "distributionChannel")]
    distribution_channel: Option<String>,
}

pub async fn list_connectors(config: &Config) -> anyhow::Result<Vec<AppInfo>> {
    if !config.features.enabled(Feature::Connectors) {
        return Ok(Vec::new());
    }
    let (connectors_result, accessible_result) = tokio::join!(
        list_all_connectors(config),
        list_accessible_connectors_from_mcp_tools(config),
    );
    let connectors = connectors_result?;
    let accessible = accessible_result?;
    let merged = merge_connectors(connectors, accessible);
    Ok(filter_apps_sdk_connectors(merged))
}

pub async fn list_all_connectors(config: &Config) -> anyhow::Result<Vec<AppInfo>> {
    if !config.features.enabled(Feature::Connectors) {
        return Ok(Vec::new());
    }
    init_chatgpt_token_from_auth(&config.codex_home, config.cli_auth_credentials_store_mode)
        .await?;

    let token_data =
        get_chatgpt_token_data().ok_or_else(|| anyhow::anyhow!("ChatGPT token not available"))?;
    let mut apps = list_directory_connectors(config).await?;
    if token_data.id_token.is_workspace_account() {
        apps.extend(list_workspace_connectors(config).await?);
    }
    let mut connectors = merge_directory_apps(apps)
        .into_iter()
        .map(directory_app_to_app_info)
        .collect::<Vec<_>>();
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

fn directory_app_to_app_info(app: DirectoryApp) -> AppInfo {
    AppInfo {
        id: app.id,
        name: app.name,
        description: app.description,
        logo_url: app.logo_url,
        logo_url_dark: app.logo_url_dark,
        distribution_channel: app.distribution_channel,
        install_url: None,
        is_accessible: false,
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

const ALLOWED_APPS_SDK_APPS: &[&str] = &["asdk_app_69781557cc1481919cf5e9824fa2e792"];

fn filter_apps_sdk_connectors(connectors: Vec<AppInfo>) -> Vec<AppInfo> {
    // TODO: Support Apps SDK connectors.
    connectors
        .into_iter()
        .filter(is_apps_sdk_connector_allowed)
        .collect()
}

fn is_apps_sdk_connector_allowed(connector: &AppInfo) -> bool {
    if !connector.id.starts_with("asdk_app_") {
        return true;
    }
    ALLOWED_APPS_SDK_APPS.contains(&connector.id.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn app(id: &str) -> AppInfo {
        AppInfo {
            id: id.to_string(),
            name: id.to_string(),
            description: None,
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            install_url: None,
            is_accessible: false,
        }
    }

    #[test]
    fn filters_internal_asdk_connectors() {
        let filtered = filter_apps_sdk_connectors(vec![app("asdk_app_hidden"), app("alpha")]);
        assert_eq!(filtered, vec![app("alpha")]);
    }

    #[test]
    fn allows_whitelisted_asdk_connectors() {
        let filtered = filter_apps_sdk_connectors(vec![
            app("asdk_app_69781557cc1481919cf5e9824fa2e792"),
            app("beta"),
        ]);
        assert_eq!(
            filtered,
            vec![
                app("asdk_app_69781557cc1481919cf5e9824fa2e792"),
                app("beta")
            ]
        );
    }
}
