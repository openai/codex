//! Projects safe provider summaries and shares field editability policy with provider writes.

use super::ConfigManager;
use super::ConfigManagerError;
use super::TomlValue;
use codex_app_server_protocol::GatewayAuth;
use codex_app_server_protocol::ModelProviderListParams;
use codex_app_server_protocol::ModelProviderListResponse;
use codex_app_server_protocol::ModelProviderSummary;
use codex_app_server_protocol::ProviderSettings;
use codex_config::ConfigLayerEntry;
use codex_config::ConfigLayerStack;
use codex_config::LoaderOverrides;
use codex_model_provider_info::AMAZON_BEDROCK_PROVIDER_ID;
use codex_model_provider_info::AMAZON_BEDROCK_PROVIDER_NAME;
use codex_model_provider_info::AMAZON_BEDROCK_RUNTIME_PROVIDER_ID;
use codex_model_provider_info::AMAZON_BEDROCK_RUNTIME_PROVIDER_NAME;
use codex_model_provider_info::GCP_PROVIDER_ID;
use codex_model_provider_info::GCP_PROVIDER_NAME;
use codex_model_provider_info::LMSTUDIO_OSS_PROVIDER_ID;
use codex_model_provider_info::ModelProviderInfo;
use codex_model_provider_info::OLLAMA_OSS_PROVIDER_ID;
use codex_model_provider_info::OPENAI_PROVIDER_ID;
use codex_model_provider_info::OPENAI_PROVIDER_NAME;
use codex_model_provider_info::built_in_model_providers;
use std::collections::BTreeSet;

fn string_field(config: Option<&TomlValue>, key: &str) -> Option<String> {
    config?.get(key)?.as_str().map(str::to_owned)
}

pub(super) fn has_external_auth(config: &TomlValue) -> bool {
    config
        .get("base_url")
        .and_then(TomlValue::as_str)
        .and_then(|value| url::Url::parse(value).ok())
        .is_some_and(|url| {
            !url.username().is_empty() || url.password().is_some() || url.query().is_some()
        })
        || ["auth", "gateway_oauth", "aws", "experimental_bearer_token"]
            .iter()
            .any(|key| config.get(key).is_some())
        || config
            .get("requires_openai_auth")
            .and_then(TomlValue::as_bool)
            == Some(true)
        || ["http_headers", "env_http_headers", "query_params"]
            .iter()
            .filter_map(|key| config.get(key)?.as_table())
            .any(|headers| !headers.is_empty())
}

pub(super) fn valid_endpoint(value: &str) -> bool {
    url::Url::parse(value).is_ok_and(|url| {
        matches!(url.scheme(), "http" | "https")
            && url.has_host()
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
    })
}

impl ConfigManager {
    pub(crate) async fn model_provider_list(
        &self,
        _params: ModelProviderListParams,
    ) -> Result<ModelProviderListResponse, ConfigManagerError> {
        let layers = self
            .load_thread_agnostic_config()
            .await
            .map_err(|e| ConfigManagerError::io("failed to read providers", e))?;
        let effective = layers.effective_config();
        let builtins = built_in_model_providers(/*openai_base_url*/ None);
        let writable_user_layer = writable_user_layer(&layers, self.loader_overrides());
        let configured = effective
            .get("model_providers")
            .and_then(TomlValue::as_table);
        let default_id = layers
            .required_model_provider()
            .or_else(|| effective.get("model_provider").and_then(TomlValue::as_str))
            .unwrap_or(OPENAI_PROVIDER_ID);
        let selection_layer = layers
            .layers_high_to_low()
            .find(|layer| layer.config.get("model_provider").is_some());
        let can_change_default = layers.required_model_provider().is_none()
            && writable_user_layer.is_some_and(|user| {
                selection_layer
                    .is_none_or(|layer| layer.name.precedence() <= user.name.precedence())
            });
        let ids: BTreeSet<_> = configured
            .into_iter()
            .flat_map(|table| table.keys().map(String::as_str))
            // Local OSS built-ins cannot have configured provider tables.
            .filter(|id| !matches!(*id, OLLAMA_OSS_PROVIDER_ID | LMSTUDIO_OSS_PROVIDER_ID))
            .chain([OPENAI_PROVIDER_ID])
            .collect();
        let data = ids
            .into_iter()
            .map(|id| {
                summarize_provider(
                    id,
                    configured.and_then(|table| table.get(id)),
                    builtins.get(id),
                    &effective,
                    &layers,
                    writable_user_layer,
                )
            })
            .collect();
        Ok(ModelProviderListResponse {
            data,
            default_provider_id: default_id.to_owned(),
            can_change_default,
        })
    }
}

pub(super) fn summarize_provider(
    id: &str,
    config: Option<&TomlValue>,
    builtin: Option<&ModelProviderInfo>,
    effective: &TomlValue,
    layers: &ConfigLayerStack,
    writable_user_layer: Option<&ConfigLayerEntry>,
) -> ModelProviderSummary {
    let display_name = match builtin {
        Some(_) if id == AMAZON_BEDROCK_PROVIDER_ID => "Amazon Bedrock Mantle",
        Some(provider) => &provider.name,
        None => config.and_then(|c| c.get("name")?.as_str()).unwrap_or(id),
    };
    let builtin_auth = uses_builtin_auth(display_name);
    let requires_rename = builtin.is_none() && (display_name.trim().is_empty() || builtin_auth);
    let external_auth = builtin_auth || config.is_some_and(has_external_auth);
    let base_url = if id == OPENAI_PROVIDER_ID {
        effective.get("openai_base_url")
    } else {
        config.and_then(|config| config.get("base_url"))
    }
    .and_then(TomlValue::as_str)
    .filter(|url| valid_endpoint(url))
    .map(str::to_owned);
    let settings = match id {
        OPENAI_PROVIDER_ID => ProviderSettings::OpenAi,
        AMAZON_BEDROCK_PROVIDER_ID | AMAZON_BEDROCK_RUNTIME_PROVIDER_ID => {
            let aws = config.and_then(|provider| provider.get("aws"));
            ProviderSettings::Bedrock {
                profile: string_field(aws, "profile"),
                region: string_field(aws, "region"),
            }
        }
        GCP_PROVIDER_ID => ProviderSettings::Gcp {
            project_id: string_field(config, "project_id"),
            location: string_field(config, "location"),
            credential_file: string_field(config, "credential_file"),
        },
        _ => {
            let env_key = string_field(config, "env_key");
            let auth = if external_auth {
                GatewayAuth::ExternallyConfigured { env_key }
            } else if let Some(env_key) = env_key {
                GatewayAuth::EnvironmentVariable { env_key }
            } else {
                GatewayAuth::None
            };
            ProviderSettings::CustomGateway { auth }
        }
    };
    let can_edit = supported_fields(id, config, builtin).iter().any(|&field| {
        (field == "name" || !requires_rename)
            && (field != "env_key" || !external_auth)
            && field_is_writable(layers, writable_user_layer, id, field)
    });
    ModelProviderSummary {
        id: id.to_owned(),
        name: display_name.to_owned(),
        can_edit,
        base_url,
        settings,
    }
}

pub(super) fn uses_builtin_auth(name: &str) -> bool {
    matches!(
        name,
        OPENAI_PROVIDER_NAME
            | AMAZON_BEDROCK_PROVIDER_NAME
            | AMAZON_BEDROCK_RUNTIME_PROVIDER_NAME
            | GCP_PROVIDER_NAME
    )
}

pub(super) fn writable_user_layer<'a>(
    layers: &'a ConfigLayerStack,
    overrides: &LoaderOverrides,
) -> Option<&'a ConfigLayerEntry> {
    layers
        .get_active_user_layer()
        .filter(|user| !user.is_disabled() && !overrides.ignore_user_config)
}

pub(super) fn supported_fields(
    id: &str,
    config: Option<&TomlValue>,
    builtin: Option<&ModelProviderInfo>,
) -> &'static [&'static str] {
    let aws_config = config.and_then(|config| config.get("aws"));
    match (builtin, id) {
        (Some(_), AMAZON_BEDROCK_PROVIDER_ID | AMAZON_BEDROCK_RUNTIME_PROVIDER_ID) => {
            if config.is_some_and(|config| config.get("auth").is_some()) {
                &[]
            } else if aws_config.is_some_and(|aws| aws.get("credential_export").is_some()) {
                &["aws.region"]
            } else {
                &["aws.profile", "aws.region"]
            }
        }
        (Some(_), GCP_PROVIDER_ID) => &["project_id", "location", "credential_file"],
        (Some(_), _) => &[],
        (None, _) => &["name", "base_url", "env_key"],
    }
}

pub(super) fn field_is_writable(
    layers: &ConfigLayerStack,
    writable_user_layer: Option<&ConfigLayerEntry>,
    id: &str,
    field: &str,
) -> bool {
    let Some(user) = writable_user_layer else {
        return false;
    };
    if id.is_empty()
        || layers
            .requirements_toml()
            .model_providers
            .as_ref()
            .is_some_and(|providers| providers.contains_key(id))
    {
        return false;
    }

    !layers.layers_high_to_low().any(|layer| {
        layer.name.precedence() > user.name.precedence()
            && layer
                .config
                .get("model_providers")
                .and_then(|providers| providers.get(id))
                .and_then(|provider| {
                    field
                        .split('.')
                        .try_fold(provider, |value, key| value.get(key))
                })
                .is_some()
    })
}
