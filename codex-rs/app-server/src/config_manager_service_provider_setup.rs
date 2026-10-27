//! Previews the effective provider configuration and captures the user layer version for commit.

use super::ConfigManagerError;
use super::MergeError;
use super::PreparedProviderWrite;
use super::apply_merge;
use super::find_effective_layer;
use super::override_message;
use super::parse_value;
use super::value_at_path;
use super::writable_key_path;
use codex_app_server_protocol::ConfigEdit;
use codex_app_server_protocol::ConfigWriteErrorCode;
use codex_config::ConfigLayerStack;
use codex_config::version_for_toml;
use codex_model_provider_info::ModelProviderInfo;
use codex_utils_absolute_path::AbsolutePathBuf;
use toml::Value as TomlValue;

pub(super) fn preview(
    layers: &ConfigLayerStack,
    user_path: &AbsolutePathBuf,
    provider_id: &str,
    edits: Vec<ConfigEdit>,
) -> Result<(PreparedProviderWrite, ModelProviderInfo), ConfigManagerError> {
    let (mut user_config, expected_version) = match layers.get_active_user_layer() {
        Some(layer) => (layer.config.clone(), layer.version.clone()),
        None => {
            let empty = TomlValue::Table(Default::default());
            let version = version_for_toml(&empty);
            (empty, version)
        }
    };
    let mut fields = Vec::with_capacity(edits.len());
    for edit in &edits {
        let path = writable_key_path(layers, &edit.key_path)?;
        let value = parse_value(edit.value.clone())?;
        apply_merge(
            &mut user_config,
            &path,
            value.as_ref(),
            edit.merge_strategy.clone(),
        )
        .map_err(|MergeError::Validation(message)| {
            ConfigManagerError::write(ConfigWriteErrorCode::ConfigValidationError, message)
        })?;
        fields.push((&edit.key_path, path, value));
    }

    let preview = layers
        .with_user_config(user_path, user_config)
        .map_err(|err| {
            ConfigManagerError::write(ConfigWriteErrorCode::ConfigValidationError, err.to_string())
        })?;
    let effective = preview.effective_config();
    for (key, path, expected) in fields {
        if value_at_path(&effective, &path) != expected.as_ref() {
            let source = find_effective_layer(&preview, &path)
                .map(|layer| override_message(&layer.name))
                .unwrap_or_else(|| "Overridden by another configuration layer".to_owned());
            return Err(ConfigManagerError::write(
                ConfigWriteErrorCode::ConfigLayerReadonly,
                format!("Cannot configure `{key}`: {source}"),
            ));
        }
    }
    let provider = effective
        .get("model_providers")
        .and_then(|providers| providers.get(provider_id))
        .cloned()
        .map(TomlValue::try_into)
        .transpose()
        .map_err(|_| {
            ConfigManagerError::write(
                ConfigWriteErrorCode::ConfigValidationError,
                format!("Invalid `{provider_id}` provider configuration."),
            )
        })?
        .unwrap_or_default();
    Ok((
        PreparedProviderWrite {
            edits,
            expected_version,
        },
        provider,
    ))
}
