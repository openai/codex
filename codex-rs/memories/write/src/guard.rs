use codex_backend_client::Client as BackendClient;
use codex_core::config::Config;
use codex_login::AuthManager;
use codex_protocol::protocol::RateLimitSnapshot;
use codex_protocol::protocol::RateLimitWindow;
use tracing::info;
use tracing::warn;

pub(crate) async fn rate_limits_ok(auth_manager: &AuthManager, config: &Config) -> bool {
    rate_limits_check(auth_manager, config)
        .await
        .unwrap_or(true)
}

async fn rate_limits_check(auth_manager: &AuthManager, config: &Config) -> Option<bool> {
    let auth = auth_manager.auth().await?;
    if !auth.uses_codex_backend() {
        return None;
    }

    let client = BackendClient::from_auth(config.chatgpt_base_url.clone(), &auth)
        .map_err(|err| warn!(%err, "failed to construct backend client"))
        .ok()?;

    let snapshots = client
        .get_rate_limits_many()
        .await
        .map_err(|err| warn!(%err, "failed to fetch rate limits"))
        .ok()?;

    let snapshot = snapshots
        .iter()
        .find(|s| s.limit_id.as_deref() == Some(crate::guard_limits::CODEX_LIMIT_ID))
        .or_else(|| snapshots.first())?;

    let min_remaining_percent = config.memories.min_rate_limit_remaining_percent;
    let allowed = snapshot_allows_startup(snapshot, min_remaining_percent);

    if !allowed {
        info!(
            min_remaining_percent,
            "skipping memories startup because Codex rate limits are below the configured threshold"
        );
    }

    Some(allowed)
}

fn snapshot_allows_startup(snapshot: &RateLimitSnapshot, min_remaining_percent: i64) -> bool {
    if snapshot.rate_limit_reached_type.is_some() {
        return false;
    }

    let max_used_percent = 100.0 - min_remaining_percent.clamp(0, 100) as f64;
    window_allows_startup(snapshot.primary.as_ref(), max_used_percent)
        && window_allows_startup(snapshot.secondary.as_ref(), max_used_percent)
}

fn window_allows_startup(window: Option<&RateLimitWindow>, max_used_percent: f64) -> bool {
    match window {
        Some(window) => window.used_percent <= max_used_percent,
        None => true,
    }
}

#[cfg(test)]
#[path = "guard_tests.rs"]
mod tests;
