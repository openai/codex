use std::collections::HashMap;
use std::time::Duration;

use anyhow::Context;
use codex_core::config::Config;
use codex_login::CodexAuth;
use serde::Deserialize;

use crate::chatgpt_client::chatgpt_get_request_with_timeout;

const WORKSPACE_SETTINGS_TIMEOUT: Duration = Duration::from_secs(10);
const CODEX_PLUGINS_BETA_SETTING: &str = "plugins";

#[derive(Debug, Deserialize)]
struct WorkspaceSettingsResponse {
    #[serde(default)]
    beta_settings: HashMap<String, bool>,
}

pub async fn codex_plugins_enabled_for_workspace(
    config: &Config,
    auth: Option<&CodexAuth>,
) -> anyhow::Result<bool> {
    let Some(auth) = auth else {
        return Ok(true);
    };
    if !auth.is_chatgpt_auth() {
        return Ok(true);
    }

    let token_data = auth
        .get_token_data()
        .context("ChatGPT token data is not available")?;
    if !token_data.id_token.is_workspace_account() {
        return Ok(true);
    }

    let Some(account_id) = token_data.account_id.as_deref().filter(|id| !id.is_empty()) else {
        return Ok(true);
    };

    let settings: WorkspaceSettingsResponse = chatgpt_get_request_with_timeout(
        config,
        format!("/accounts/{account_id}/settings"),
        Some(WORKSPACE_SETTINGS_TIMEOUT),
    )
    .await?;

    Ok(settings
        .beta_settings
        .get(CODEX_PLUGINS_BETA_SETTING)
        .copied()
        .unwrap_or(true))
}
