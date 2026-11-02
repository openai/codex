//! Applies provider field patches without selecting a provider or changing hidden credentials.
//! RPC writes share the config queue and apply field patches with last-write-wins semantics.

use super::ApiConfigEdit;
use super::ConfigManager;
use super::ConfigManagerError;
use super::MergeStrategy;
use super::TomlValue;
use super::model_providers::field_is_writable;
use super::model_providers::has_external_auth;
use super::model_providers::summarize_provider;
use super::model_providers::supported_fields;
use super::model_providers::uses_builtin_auth;
use super::model_providers::valid_endpoint;
use super::model_providers::writable_user_layer;
use codex_app_server_protocol::BedrockProviderId;
use codex_app_server_protocol::ConfigBatchWriteParams;
use codex_app_server_protocol::ConfigWriteErrorCode;
use codex_app_server_protocol::CustomGatewaySettingsParams;
use codex_app_server_protocol::GcpProviderId;
use codex_app_server_protocol::ModelProviderConfigWriteParams;
use codex_app_server_protocol::ModelProviderConfigWriteResponse;
use codex_app_server_protocol::ProviderSettingsPatch;
use codex_config::ConfigLayerStack;
use codex_config::LoaderOverrides;
use codex_model_provider_info::AMAZON_BEDROCK_PROVIDER_ID;
use codex_model_provider_info::AMAZON_BEDROCK_RUNTIME_PROVIDER_ID;
use codex_model_provider_info::GCP_PROVIDER_ID;
use codex_model_provider_info::LEGACY_OLLAMA_CHAT_PROVIDER_ID;
use codex_model_provider_info::ModelProviderInfo;
use codex_model_provider_info::built_in_model_providers;
use std::collections::HashMap;

impl ConfigManager {
    pub(crate) async fn model_provider_config_write(
        &self,
        params: ModelProviderConfigWriteParams,
    ) -> Result<ModelProviderConfigWriteResponse, ConfigManagerError> {
        let layers = self
            .load_thread_agnostic_config()
            .await
            .map_err(|err| ConfigManagerError::io("failed to read providers", err))?;
        let builtins = built_in_model_providers(/*openai_base_url*/ None);
        let (provider_id, edits) =
            provider_edits(&params, &layers, self.loader_overrides(), &builtins)?;
        let response = self
            .batch_write(ConfigBatchWriteParams {
                edits,
                file_path: None,
                expected_version: None,
                reload_user_config: false,
            })
            .await?;
        if let Some(overridden) = response.overridden_metadata {
            return Err(ConfigManagerError::write(
                ConfigWriteErrorCode::ConfigLayerReadonly,
                overridden.message,
            ));
        }
        let saved = self
            .load_thread_agnostic_config()
            .await
            .map_err(|err| ConfigManagerError::io("failed to read saved provider", err))?;
        let effective = saved.effective_config();
        let provider = summarize_provider(
            &provider_id,
            effective
                .get("model_providers")
                .and_then(|providers| providers.get(&provider_id)),
            builtins.get(&provider_id),
            &effective,
            &saved,
            writable_user_layer(&saved, self.loader_overrides()),
        );
        Ok(ModelProviderConfigWriteResponse { provider })
    }
}

fn provider_edits(
    params: &ModelProviderConfigWriteParams,
    layers: &ConfigLayerStack,
    overrides: &LoaderOverrides,
    builtins: &HashMap<String, ModelProviderInfo>,
) -> Result<(String, Vec<ApiConfigEdit>), ConfigManagerError> {
    let effective = layers.effective_config();
    let configured = effective.get("model_providers");
    let (provider_id, mut fields) = match &params.settings {
        ProviderSettingsPatch::CustomGateway(patch) => {
            let target = patch.provider_id.as_deref();
            if target
                .is_some_and(|id| builtins.contains_key(id) || id == LEGACY_OLLAMA_CHAT_PROVIDER_ID)
            {
                return Err(invalid(
                    "A custom gateway cannot use a built-in provider ID.",
                ));
            }
            let existing = target.and_then(|id| configured?.get(id));
            if target.is_some() && existing.is_none() {
                return Err(ConfigManagerError::write(
                    ConfigWriteErrorCode::ConfigPathNotFound,
                    "Unknown provider ID.",
                ));
            }
            let name = validate_custom_patch(patch, existing)?;
            let id = if let Some(id) = target {
                id.to_owned()
            } else {
                if patch.base_url.as_ref().and_then(Option::as_deref).is_none() {
                    return Err(invalid(
                        "A base URL is required to create a custom provider.",
                    ));
                }
                let default_id = layers
                    .required_model_provider()
                    .or_else(|| effective.get("model_provider")?.as_str());
                allocate_custom_id(name, |id| {
                    builtins.contains_key(id)
                        || configured.is_some_and(|table| table.get(id).is_some())
                        || default_id == Some(id)
                })
            };
            let mut fields =
                patch_fields([("base_url", &patch.base_url), ("env_key", &patch.env_key)]);
            if patch.name.is_some() {
                fields.push(("name", Some(name)));
            }
            (id, fields)
        }
        ProviderSettingsPatch::Bedrock(patch) => {
            let id = match patch.provider_id {
                BedrockProviderId::AmazonBedrock => AMAZON_BEDROCK_PROVIDER_ID,
                BedrockProviderId::AmazonBedrockRuntime => AMAZON_BEDROCK_RUNTIME_PROVIDER_ID,
            };
            (
                id.to_owned(),
                patch_fields([
                    ("aws.profile", &patch.profile),
                    ("aws.region", &patch.region),
                ]),
            )
        }
        ProviderSettingsPatch::Gcp(patch) => {
            let GcpProviderId::Gcp = patch.provider_id;
            (
                GCP_PROVIDER_ID.to_owned(),
                patch_fields([
                    ("project_id", &patch.project_id),
                    ("location", &patch.location),
                    ("credential_file", &patch.credential_file),
                ]),
            )
        }
    };
    if fields.iter().any(|(_, value)| *value == Some("")) {
        return Err(invalid(
            "Configuration values must not be empty; use null to clear a field.",
        ));
    }
    let existing = configured.and_then(|providers| providers.get(&provider_id));
    let builtin = builtins.get(&provider_id);
    let user_layer = writable_user_layer(layers, overrides);
    let supported = supported_fields(&provider_id, existing, builtin);
    for (field, _) in &fields {
        if !supported.contains(field) {
            return Err(invalid(
                "A supplied field is not editable for this provider.",
            ));
        }
        if !field_is_writable(layers, user_layer, &provider_id, field) {
            return Err(ConfigManagerError::write(
                ConfigWriteErrorCode::ConfigLayerReadonly,
                "This configuration field is read-only.",
            ));
        }
    }
    // Custom override tables require a name, but clearing an absent leaf needs no table.
    let user_provider =
        user_layer.and_then(|layer| layer.config.get("model_providers")?.get(&provider_id));
    if builtin.is_none()
        && !fields.is_empty()
        && !fields.iter().any(|(field, _)| *field == "name")
        && (user_provider.is_some() || fields.iter().any(|(_, value)| value.is_some()))
        && user_provider
            .and_then(|provider| provider.get("name"))
            .is_none()
        && let Some(name) = existing.and_then(|provider| provider.get("name")?.as_str())
    {
        fields.push(("name", Some(name)));
    }
    if builtin.is_none() && existing.is_none() {
        fields.push(("wire_api", Some("responses")));
    }
    let escaped_id = provider_id.replace('\\', "\\\\").replace('"', "\\\"");
    let edits = fields
        .into_iter()
        .map(|(field, value)| ApiConfigEdit {
            key_path: format!(r#"model_providers."{escaped_id}".{field}"#),
            value: value.into(),
            merge_strategy: MergeStrategy::Replace,
        })
        .collect();
    Ok((provider_id, edits))
}

// Missing fields are excluded. A field with no value clears its TOML key.
type PatchField<'a> = (&'static str, Option<&'a str>);

fn patch_fields<'a, const N: usize>(
    requested: [(&'static str, &'a Option<Option<String>>); N],
) -> Vec<PatchField<'a>> {
    requested
        .into_iter()
        .filter_map(|(field, value)| {
            value
                .as_ref()
                .map(|value| (field, value.as_deref().map(str::trim)))
        })
        .collect()
}

fn validate_custom_patch<'a>(
    patch: &'a CustomGatewaySettingsParams,
    existing: Option<&'a TomlValue>,
) -> Result<&'a str, ConfigManagerError> {
    if patch
        .base_url
        .as_ref()
        .and_then(Option::as_deref)
        .is_some_and(|url| !valid_endpoint(url.trim()))
    {
        return Err(invalid(
            "Enter an HTTP(S) base URL without credentials, a query, or a fragment.",
        ));
    }
    if let Some(Some(key)) = &patch.env_key {
        let key = key.trim();
        if !key.starts_with(|c: char| c == '_' || c.is_ascii_alphabetic())
            || !key.bytes().all(|c| c == b'_' || c.is_ascii_alphanumeric())
        {
            return Err(invalid("Invalid API key environment variable name."));
        }
    }
    let existing_name = existing.and_then(|config| config.get("name")?.as_str());
    if patch.env_key.is_some()
        && existing.is_some_and(|config| {
            has_external_auth(config)
                || existing_name
                    .or(patch.provider_id.as_deref())
                    .is_some_and(uses_builtin_auth)
        })
    {
        return Err(invalid(
            "Authentication is configured externally. Omit envKey to preserve it.",
        ));
    }
    let name = patch
        .name
        .as_deref()
        .map(str::trim)
        .or(existing_name)
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| invalid("Enter a provider name."))?;
    // Legacy names select built-in authentication even on a custom provider.
    if uses_builtin_auth(name) {
        return Err(invalid(
            "This name is reserved for a built-in provider. Enter a different custom provider name.",
        ));
    }
    Ok(name)
}

fn allocate_custom_id(name: &str, is_taken: impl Fn(&str) -> bool) -> String {
    let slug = name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
        .to_ascii_lowercase();
    let stem = if slug.is_empty() { "custom" } else { &slug };
    let mut id = stem.to_owned();
    let mut suffix = 2;
    while id == LEGACY_OLLAMA_CHAT_PROVIDER_ID || is_taken(&id) {
        id = format!("{stem}-{suffix}");
        suffix += 1;
    }
    id
}

fn invalid(message: &str) -> ConfigManagerError {
    ConfigManagerError::write(ConfigWriteErrorCode::ConfigValidationError, message)
}
