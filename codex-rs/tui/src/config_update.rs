//! App-server-backed config update helpers for the TUI.
//!
//! This module centralizes the small typed update helpers the TUI uses
//! when a config mutation must be owned by the app server rather than written
//! to the local `config.toml` directly.

use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ConfigBatchWriteParams;
use codex_app_server_protocol::ConfigEdit;
use codex_app_server_protocol::ConfigWriteResponse;
use codex_app_server_protocol::MergeStrategy;
use codex_app_server_protocol::RequestId;
use color_eyre::eyre::Result;
use color_eyre::eyre::WrapErr;
use serde_json::Value as JsonValue;
use uuid::Uuid;

pub(crate) fn replace_config_value(key_path: impl Into<String>, value: JsonValue) -> ConfigEdit {
    ConfigEdit {
        key_path: key_path.into(),
        value,
        merge_strategy: MergeStrategy::Replace,
    }
}

pub(crate) fn clear_config_value(key_path: impl Into<String>) -> ConfigEdit {
    replace_config_value(key_path, JsonValue::Null)
}

pub(crate) fn profile_scoped_key_path(profile: Option<&str>, key_path: &str) -> String {
    if let Some(profile) = profile {
        let profile = serde_json::Value::String(profile.to_string()).to_string();
        format!("profiles.{profile}.{key_path}")
    } else {
        key_path.to_string()
    }
}

pub(crate) fn build_model_selection_edits(
    profile: Option<&str>,
    model: &str,
    effort: Option<impl ToString>,
) -> Vec<ConfigEdit> {
    let effort_edit = effort.map_or_else(
        || clear_config_value(profile_scoped_key_path(profile, "model_reasoning_effort")),
        |effort| {
            replace_config_value(
                profile_scoped_key_path(profile, "model_reasoning_effort"),
                serde_json::json!(effort.to_string()),
            )
        },
    );
    vec![
        replace_config_value(
            profile_scoped_key_path(profile, "model"),
            serde_json::json!(model),
        ),
        effort_edit,
    ]
}

pub(crate) fn build_service_tier_selection_edits(
    profile: Option<&str>,
    service_tier: Option<&str>,
) -> Vec<ConfigEdit> {
    let service_tier_edit = service_tier.map_or_else(
        || clear_config_value(profile_scoped_key_path(profile, "service_tier")),
        |service_tier| {
            let config_value =
                match codex_protocol::config_types::ServiceTier::from_request_value(service_tier) {
                    Some(codex_protocol::config_types::ServiceTier::Fast) => "fast",
                    Some(codex_protocol::config_types::ServiceTier::Flex) => "flex",
                    None => service_tier,
                };
            replace_config_value(
                profile_scoped_key_path(profile, "service_tier"),
                serde_json::json!(config_value),
            )
        },
    );
    let mut edits = vec![service_tier_edit];
    if service_tier.is_none() {
        edits.push(replace_config_value(
            "notice.fast_default_opt_out",
            serde_json::json!(true),
        ));
    }
    edits
}

pub(crate) async fn write_config_batch(
    request_handle: AppServerRequestHandle,
    edits: Vec<ConfigEdit>,
) -> Result<()> {
    let request_id = RequestId::String(format!("tui-config-write-{}", Uuid::new_v4()));
    let _: ConfigWriteResponse = request_handle
        .request_typed(ClientRequest::ConfigBatchWrite {
            request_id,
            params: ConfigBatchWriteParams {
                edits,
                file_path: None,
                expected_version: None,
                reload_user_config: true,
            },
        })
        .await
        .wrap_err("config/batchWrite failed in TUI")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn profile_scoped_key_path_quotes_dotted_profile_names() {
        assert_eq!(
            profile_scoped_key_path(Some("team.prod"), "model"),
            "profiles.\"team.prod\".model"
        );
    }
}
