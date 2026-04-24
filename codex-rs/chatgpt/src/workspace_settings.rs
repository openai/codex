use std::collections::HashMap;
use std::sync::RwLock;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use codex_core::config::Config;
use codex_login::CodexAuth;
use serde::Deserialize;

use crate::chatgpt_client::chatgpt_get_request_with_timeout;

const WORKSPACE_SETTINGS_TIMEOUT: Duration = Duration::from_secs(10);
const WORKSPACE_SETTINGS_CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const CODEX_PLUGINS_BETA_SETTING: &str = "plugins";

#[derive(Debug, Deserialize)]
struct WorkspaceSettingsResponse {
    #[serde(default)]
    beta_settings: HashMap<String, bool>,
}

#[derive(Debug, Default)]
pub struct WorkspaceSettingsCache {
    entry: RwLock<Option<CachedWorkspaceSettings>>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct WorkspaceSettingsCacheKey {
    chatgpt_base_url: String,
    account_id: String,
}

#[derive(Clone, Debug)]
struct CachedWorkspaceSettings {
    key: WorkspaceSettingsCacheKey,
    expires_at: Instant,
    codex_plugins_enabled: bool,
}

impl WorkspaceSettingsCache {
    fn get_codex_plugins_enabled(&self, key: &WorkspaceSettingsCacheKey) -> Option<bool> {
        {
            let entry = match self.entry.read() {
                Ok(entry) => entry,
                Err(err) => err.into_inner(),
            };
            let now = Instant::now();
            if let Some(cached) = entry.as_ref()
                && now < cached.expires_at
                && cached.key == *key
            {
                return Some(cached.codex_plugins_enabled);
            }
        }

        let mut entry = match self.entry.write() {
            Ok(entry) => entry,
            Err(err) => err.into_inner(),
        };
        let now = Instant::now();
        if entry
            .as_ref()
            .is_some_and(|cached| now >= cached.expires_at || cached.key != *key)
        {
            *entry = None;
        }
        None
    }

    fn set_codex_plugins_enabled(&self, key: WorkspaceSettingsCacheKey, enabled: bool) {
        let mut entry = match self.entry.write() {
            Ok(entry) => entry,
            Err(err) => err.into_inner(),
        };
        *entry = Some(CachedWorkspaceSettings {
            key,
            expires_at: Instant::now() + WORKSPACE_SETTINGS_CACHE_TTL,
            codex_plugins_enabled: enabled,
        });
    }
}

pub async fn codex_plugins_enabled_for_workspace(
    config: &Config,
    auth: Option<&CodexAuth>,
    cache: Option<&WorkspaceSettingsCache>,
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

    let cache_key = WorkspaceSettingsCacheKey {
        chatgpt_base_url: config.chatgpt_base_url.clone(),
        account_id: account_id.to_string(),
    };
    if let Some(cache) = cache
        && let Some(enabled) = cache.get_codex_plugins_enabled(&cache_key)
    {
        return Ok(enabled);
    }

    let encoded_account_id = encode_path_segment(account_id);
    let settings: WorkspaceSettingsResponse = chatgpt_get_request_with_timeout(
        config,
        format!("/accounts/{encoded_account_id}/settings"),
        Some(WORKSPACE_SETTINGS_TIMEOUT),
    )
    .await?;

    let codex_plugins_enabled = settings
        .beta_settings
        .get(CODEX_PLUGINS_BETA_SETTING)
        .copied()
        .unwrap_or(true);

    if let Some(cache) = cache {
        cache.set_codex_plugins_enabled(cache_key, codex_plugins_enabled);
    }

    Ok(codex_plugins_enabled)
}

fn encode_path_segment(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

#[cfg(test)]
#[path = "workspace_settings_tests.rs"]
mod tests;
