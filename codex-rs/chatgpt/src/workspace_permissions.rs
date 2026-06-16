use std::sync::RwLock;
use std::time::Duration;
use std::time::Instant;

use codex_core::config::Config;
use codex_login::CodexAuth;
use serde::Deserialize;

use crate::chatgpt_client::chatgpt_get_request_with_timeout;

const WORKSPACE_SETTINGS_TIMEOUT: Duration = Duration::from_secs(10);
const WORKSPACE_SETTINGS_CACHE_TTL: Duration = Duration::from_secs(15 * 60);
const USE_PLUGIN_PERMISSION: &str = "chatgpt.workspace_plugin.use";

#[derive(Debug, Deserialize)]
struct WorkspaceSettingsResponse {
    #[serde(default)]
    permissions: Option<Vec<String>>,
}

#[derive(Debug, Default)]
pub struct WorkspacePermissionsCache {
    entry: RwLock<Option<CachedWorkspacePermissions>>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct WorkspacePermissionsCacheKey {
    chatgpt_base_url: String,
    account_id: String,
}

#[derive(Clone, Debug)]
struct CachedWorkspacePermissions {
    key: WorkspacePermissionsCacheKey,
    expires_at: Instant,
    plugin_use_allowed: bool,
}

impl WorkspacePermissionsCache {
    fn get_plugin_use_allowed(&self, key: &WorkspacePermissionsCacheKey) -> Option<bool> {
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
                return Some(cached.plugin_use_allowed);
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

    fn set_plugin_use_allowed(&self, key: WorkspacePermissionsCacheKey, allowed: bool) {
        let mut entry = match self.entry.write() {
            Ok(entry) => entry,
            Err(err) => err.into_inner(),
        };
        *entry = Some(CachedWorkspacePermissions {
            key,
            expires_at: Instant::now() + WORKSPACE_SETTINGS_CACHE_TTL,
            plugin_use_allowed: allowed,
        });
    }
}

pub async fn codex_plugins_allowed_for_workspace(
    config: &Config,
    auth: Option<&CodexAuth>,
    cache: Option<&WorkspacePermissionsCache>,
) -> anyhow::Result<bool> {
    let Some(auth) = auth else {
        return Ok(true);
    };
    if !auth.is_chatgpt_auth() {
        return Ok(true);
    }

    if !auth.is_workspace_account() {
        return Ok(true);
    }

    let Some(account_id) = auth.get_account_id().filter(|id| !id.is_empty()) else {
        return Ok(true);
    };

    let cache_key = WorkspacePermissionsCacheKey {
        chatgpt_base_url: config.chatgpt_base_url.clone(),
        account_id: account_id.clone(),
    };
    if let Some(cache) = cache
        && let Some(allowed) = cache.get_plugin_use_allowed(&cache_key)
    {
        return Ok(allowed);
    }

    let encoded_account_id = encode_path_segment(&account_id);
    let settings: WorkspaceSettingsResponse = chatgpt_get_request_with_timeout(
        config,
        format!("/accounts/{encoded_account_id}/settings"),
        Some(WORKSPACE_SETTINGS_TIMEOUT),
    )
    .await?;

    // Older servers omit RBAC permissions from this response. Allow plugins until the
    // permission is present so client and server deployments do not need to be atomic.
    let plugin_use_allowed = settings.permissions.as_ref().is_none_or(|permissions| {
        permissions
            .iter()
            .any(|permission| permission == USE_PLUGIN_PERMISSION)
    });

    if let Some(cache) = cache {
        cache.set_plugin_use_allowed(cache_key, plugin_use_allowed);
    }

    Ok(plugin_use_allowed)
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
#[path = "workspace_permissions_tests.rs"]
mod tests;
