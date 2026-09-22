//! Local daemon launch policy. Explicit embedded launches never discover or start a daemon;
//! optional attachment may fall back to embedded mode, while automatic launches
//! require a compatible shared server and a successful connection.

use super::*;
use std::collections::BTreeMap;

const SERVER_FEATURES: [Feature; 4] = [
    Feature::ApiKeyModelDiscovery,
    Feature::CodeModeHost,
    Feature::AuthElicitation,
    Feature::McpOAuthRefreshCoordination,
];

pub(super) const FAILURE_HINT: &str = "To work without the background server, rerun the same command with --no-daemon (including resume or fork and its arguments).";

#[derive(Debug, thiserror::Error)]
#[error("Cannot use the shared background server: {reason}.\n{FAILURE_HINT}")]
pub(super) struct CompatibilityError {
    pub reason: String,
    pub restart_features: Option<BTreeMap<String, bool>>,
}

pub(super) fn exclusion(
    cli: &Cli,
    cli_kv_overrides: &[(String, toml::Value)],
    loader_overrides: &LoaderOverrides,
    workload_identity_selected: bool,
    exec_server_url: Option<&std::ffi::OsStr>,
) -> Option<&'static str> {
    if cli.no_daemon {
        Some("--no-daemon")
    } else if cli.oss {
        Some("--oss")
    } else if workload_identity_selected {
        Some("workload identity")
    } else if exec_server_url.is_some() {
        Some("executor selection (CODEX_EXEC_SERVER_URL)")
    } else if cli.agents_overview {
        None
    } else if cli.config_profile_v2.is_some() {
        Some("--profile")
    } else {
        config_exclusion(
            cli_kv_overrides,
            loader_overrides,
            cli.strict_config,
            cli.bypass_hook_trust,
        )
    }
}

pub(super) fn config_exclusion(
    cli_kv_overrides: &[(String, toml::Value)],
    loader_overrides: &LoaderOverrides,
    strict_config: bool,
    bypass_hook_trust: bool,
) -> Option<&'static str> {
    if !cli_kv_overrides
        .iter()
        .all(|(key, value)| match key.as_str() {
            "suppress_unstable_features_warning" | "tui.fullscreen_transcript" => value.is_bool(),
            "tui" => value.as_table().is_some_and(|tui| {
                tui.len() == 1
                    && tui
                        .get("fullscreen_transcript")
                        .is_some_and(toml::Value::is_bool)
            }),
            "features" => value.as_table().is_some_and(|features| {
                !features.is_empty()
                    && features
                        .iter()
                        .all(|(name, value)| allowed_feature(name) && value.is_bool())
            }),
            _ => key.strip_prefix("features.").is_some_and(allowed_feature) && value.is_bool(),
        })
    {
        Some("command-line configuration overrides (-c, --enable, --disable, or --search)")
    } else if !loader_overrides_are_default(loader_overrides) {
        Some("custom configuration loader")
    } else if strict_config {
        Some("--strict-config")
    } else if bypass_hook_trust {
        Some("--dangerously-bypass-hook-trust")
    } else {
        None
    }
}

fn allowed_feature(name: &str) -> bool {
    matches!(
        name,
        // Client gates and per-thread settings already forwarded in thread requests.
        "daemon_auto_start" | "worktrees" | "transcript_v2" | "realtime_conversation" | "standalone_web_search"
        // Shared services and threadless MCP operations need daemon compatibility checks.
        | "api_key_model_discovery" | "code_mode_host" | "auth_elicitation"
        | "mcp_oauth_refresh_coordination"
        // Removed flags still passed by older launch scripts.
        | "remote_models" | "request_rule" | "responses_websockets_v2"
        | "workspace_owner_usage_nudge" | "tool_search_always_defer_mcp_tools"
        | "remote_compaction_v2" | "multi_agent_mode"
    )
}

pub(super) fn server_features(overrides: &[(String, toml::Value)]) -> BTreeMap<String, bool> {
    let layer = codex_config::build_cli_overrides_layer(overrides);
    SERVER_FEATURES
        .into_iter()
        .filter_map(|feature| {
            let name = feature.key();
            let enabled = layer.get("features")?.get(name)?.as_bool()?;
            Some((name.to_string(), enabled))
        })
        .collect()
}

/// Best-effort configured readback, not a guarantee about startup-captured service state.
pub(super) async fn compatibility_warning(
    target: &AppServerTarget,
    config: &Config,
) -> Result<Option<String>, CompatibilityError> {
    let AppServerTarget::LocalDaemon {
        allow_embedded_fallback,
        ..
    } = target
    else {
        return Ok(None);
    };
    let mut restart_features = None;
    let check = async {
        // The feature-list RPC cannot report this process-scoped structured setting.
        if !config.features.enabled(Feature::CodeModeHost)
            && config.code_mode.disable_in_process_fallback
        {
            return Err("code-mode host fallback policy requires embedded mode".to_string());
        }
        let client = app_server_connection::connect(target)
            .await
            .map_err(|_| "could not connect to check daemon feature settings".to_string())?;
        let (tx, rx) = tokio::sync::oneshot::channel();
        crate::experimental_features::fetch(
            client.request_handle(),
            /*thread_id*/ None,
            "tui-daemon-features",
            tx,
        );
        let result = rx.await;
        let _ = client.shutdown().await;
        let features = result.map_err(|_| "daemon feature check was interrupted".to_string())??;
        // A previous client may have launched this daemon with overrides, even if
        // this client has none. Check effective values, including defaults.
        for feature in SERVER_FEATURES {
            let name = feature.key();
            let enabled = config.features.enabled(feature);
            if features
                .iter()
                .find(|feature| feature.name == name)
                .is_some_and(|feature| feature.enabled)
                != enabled
            {
                restart_features = Some(
                    SERVER_FEATURES
                        .into_iter()
                        .map(|feature| {
                            (feature.key().to_string(), config.features.enabled(feature))
                        })
                        .collect(),
                );
                let state = if enabled { "enabled" } else { "disabled" };
                return Err(format!("This session requires {name} to be {state}"));
            }
        }
        Ok::<(), String>(())
    }
    .await;
    match check {
        Ok(()) => Ok(None),
        Err(reason) if *allow_embedded_fallback => Ok(Some(format!(
            "Running without the shared background server: {reason}."
        ))),
        Err(reason) => Err(CompatibilityError {
            reason,
            restart_features,
        }),
    }
}
