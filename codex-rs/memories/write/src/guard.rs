use codex_backend_client::Client as BackendClient;
use codex_core::config::Config;
use codex_model_provider::ProviderAuthContext;
use codex_protocol::protocol::RateLimitSnapshot;
use codex_protocol::protocol::RateLimitWindow;
use tracing::info;
use tracing::warn;

pub(crate) async fn rate_limits_ok(auth_context: &ProviderAuthContext, config: &Config) -> bool {
    rate_limits_check(auth_context, config)
        .await
        .unwrap_or(true)
}

async fn rate_limits_check(auth_context: &ProviderAuthContext, config: &Config) -> Option<bool> {
    let (auth, captured_factory) = auth_context
        .credentials
        .auth_with_http_client_factory()
        .await?;
    if !auth.uses_codex_backend() {
        return None;
    }

    let factory = config.http_client_factory();
    let policy = factory
        .network_policy()
        .clone()
        .with_account_binding(captured_factory.network_policy())?;
    let client = BackendClient::from_auth_with_runtime(
        config.chatgpt_base_url.clone(),
        &auth,
        factory.with_network_policy(policy),
        auth_context.runtime_auth.as_ref(),
        /*headers*/ &Default::default(),
    )
    .await
    .map_err(|error| warn!(%error, "failed to prepare memory rate-limit authentication"))
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
