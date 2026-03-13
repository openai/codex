use crate::config::Config;
use crate::model_provider_info::LMSTUDIO_OSS_PROVIDER_ID;
use crate::model_provider_info::ModelProviderInfo;
use crate::model_provider_info::OLLAMA_OSS_PROVIDER_ID;
use codex_app_server_protocol::ConfigLayerSource;
use codex_protocol::protocol::SessionSource;
use reqwest::Url;

const BASE_URL_ORIGIN_CHATGPT: &str = "chatgpt.com";
const BASE_URL_ORIGIN_OPENAI_API: &str = "api.openai.com";
const BASE_URL_ORIGIN_OPENROUTER: &str = "openrouter.ai";
const BASE_URL_ORIGIN_CUSTOM: &str = "custom";

const HOST_CLASS_OPENAI_CHATGPT: &str = "openai_chatgpt";
const HOST_CLASS_OPENAI_API: &str = "openai_api";
const HOST_CLASS_KNOWN_THIRD_PARTY: &str = "known_third_party";
const HOST_CLASS_CUSTOM_UNKNOWN: &str = "custom_unknown";

const BASE_URL_SOURCE_DEFAULT: &str = "default";
const BASE_URL_SOURCE_ENV: &str = "env";
const BASE_URL_SOURCE_CONFIG_TOML: &str = "config_toml";
const BASE_URL_SOURCE_IDE_SETTINGS: &str = "ide_settings";
const BASE_URL_SOURCE_MANAGED_CONFIG: &str = "managed_config";
const BASE_URL_SOURCE_SESSION_FLAGS: &str = "session_flags";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EndpointConfigTelemetrySource {
    pub(crate) base_url_source: &'static str,
    pub(crate) base_url_is_default: bool,
}

impl EndpointConfigTelemetrySource {
    pub(crate) const fn new(base_url_source: &'static str, base_url_is_default: bool) -> Self {
        Self {
            base_url_source,
            base_url_is_default,
        }
    }

    pub(crate) fn classify(self, base_url: &str) -> EndpointConfigTelemetry {
        let (base_url_origin, host_class) = classify_base_url(base_url);
        EndpointConfigTelemetry {
            base_url_origin,
            host_class,
            base_url_source: self.base_url_source,
            base_url_is_default: self.base_url_is_default,
        }
    }

    pub(crate) const fn redacted_unknown(self) -> EndpointConfigTelemetry {
        EndpointConfigTelemetry {
            base_url_origin: BASE_URL_ORIGIN_CUSTOM,
            host_class: HOST_CLASS_CUSTOM_UNKNOWN,
            base_url_source: self.base_url_source,
            base_url_is_default: self.base_url_is_default,
        }
    }

    pub(crate) fn for_provider(
        provider_id: &str,
        provider: &ModelProviderInfo,
    ) -> EndpointConfigTelemetrySource {
        endpoint_source_from_provider_defaults(provider_id, provider)
    }

    pub(crate) fn for_provider_without_id(provider: &ModelProviderInfo) -> Self {
        let base_url_is_default = provider.base_url.is_none();
        let base_url_source = if base_url_is_default {
            BASE_URL_SOURCE_DEFAULT
        } else {
            BASE_URL_SOURCE_CONFIG_TOML
        };
        EndpointConfigTelemetrySource::new(base_url_source, base_url_is_default)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EndpointConfigTelemetry {
    pub(crate) base_url_origin: &'static str,
    pub(crate) host_class: &'static str,
    pub(crate) base_url_source: &'static str,
    pub(crate) base_url_is_default: bool,
}

impl Default for EndpointConfigTelemetry {
    fn default() -> Self {
        Self {
            base_url_origin: BASE_URL_ORIGIN_CUSTOM,
            host_class: HOST_CLASS_CUSTOM_UNKNOWN,
            base_url_source: BASE_URL_SOURCE_DEFAULT,
            base_url_is_default: false,
        }
    }
}

pub(crate) fn resolve_endpoint_config_telemetry_source(
    config: &Config,
    session_source: SessionSource,
) -> EndpointConfigTelemetrySource {
    let origins = config.config_layer_stack.origins();
    let key = format!("model_providers.{}.base_url", config.model_provider_id);
    if let Some(origin) = origins.get(&key) {
        return endpoint_source_from_layer(&origin.name, session_source);
    }

    endpoint_source_from_provider_defaults(
        config.model_provider_id.as_str(),
        &config.model_provider,
    )
}

fn endpoint_source_from_layer(
    layer: &ConfigLayerSource,
    session_source: SessionSource,
) -> EndpointConfigTelemetrySource {
    let base_url_source = match layer {
        ConfigLayerSource::SessionFlags => match session_source {
            SessionSource::VSCode | SessionSource::Mcp => BASE_URL_SOURCE_IDE_SETTINGS,
            SessionSource::Cli
            | SessionSource::Exec
            | SessionSource::SubAgent(_)
            | SessionSource::Unknown => BASE_URL_SOURCE_SESSION_FLAGS,
        },
        ConfigLayerSource::User { .. } | ConfigLayerSource::Project { .. } => {
            BASE_URL_SOURCE_CONFIG_TOML
        }
        ConfigLayerSource::System { .. }
        | ConfigLayerSource::Mdm { .. }
        | ConfigLayerSource::LegacyManagedConfigTomlFromFile { .. }
        | ConfigLayerSource::LegacyManagedConfigTomlFromMdm => BASE_URL_SOURCE_MANAGED_CONFIG,
    };

    EndpointConfigTelemetrySource::new(base_url_source, false)
}

fn endpoint_source_from_provider_defaults(
    provider_id: &str,
    provider: &ModelProviderInfo,
) -> EndpointConfigTelemetrySource {
    let env_source = match provider_id {
        "openai" => env_var_present("OPENAI_BASE_URL"),
        OLLAMA_OSS_PROVIDER_ID | LMSTUDIO_OSS_PROVIDER_ID => {
            env_var_present("CODEX_OSS_BASE_URL") || env_var_present("CODEX_OSS_PORT")
        }
        _ => false,
    };
    if env_source {
        return EndpointConfigTelemetrySource::new(BASE_URL_SOURCE_ENV, false);
    }

    let base_url_is_default = match provider_id {
        "openai" => provider.base_url.is_none(),
        OLLAMA_OSS_PROVIDER_ID | LMSTUDIO_OSS_PROVIDER_ID => true,
        _ => provider.base_url.is_none(),
    };
    if base_url_is_default {
        return EndpointConfigTelemetrySource::new(BASE_URL_SOURCE_DEFAULT, true);
    }

    EndpointConfigTelemetrySource::new(BASE_URL_SOURCE_CONFIG_TOML, false)
}

fn env_var_present(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .is_some_and(|value| !value.trim().is_empty())
}

fn classify_base_url(base_url: &str) -> (&'static str, &'static str) {
    let Ok(url) = Url::parse(base_url) else {
        return (BASE_URL_ORIGIN_CUSTOM, HOST_CLASS_CUSTOM_UNKNOWN);
    };
    let Some(host) = url.host_str().map(str::to_ascii_lowercase) else {
        return (BASE_URL_ORIGIN_CUSTOM, HOST_CLASS_CUSTOM_UNKNOWN);
    };

    if matches!(host.as_str(), "chatgpt.com" | "chat.openai.com") {
        if is_chatgpt_codex_path(url.path()) {
            return (BASE_URL_ORIGIN_CHATGPT, HOST_CLASS_OPENAI_CHATGPT);
        }
        return (BASE_URL_ORIGIN_CHATGPT, HOST_CLASS_CUSTOM_UNKNOWN);
    }

    if host == BASE_URL_ORIGIN_OPENAI_API {
        return (BASE_URL_ORIGIN_OPENAI_API, HOST_CLASS_OPENAI_API);
    }

    if host == BASE_URL_ORIGIN_OPENROUTER || host.ends_with(".openrouter.ai") {
        return (BASE_URL_ORIGIN_OPENROUTER, HOST_CLASS_KNOWN_THIRD_PARTY);
    }

    (BASE_URL_ORIGIN_CUSTOM, HOST_CLASS_CUSTOM_UNKNOWN)
}

fn is_chatgpt_codex_path(path: &str) -> bool {
    path == "/backend-api/codex" || path.starts_with("/backend-api/codex/")
}

#[cfg(test)]
mod tests {
    use super::EndpointConfigTelemetry;
    use super::EndpointConfigTelemetrySource;
    use super::endpoint_source_from_layer;
    use super::endpoint_source_from_provider_defaults;
    use crate::model_provider_info::WireApi;
    use crate::model_provider_info::create_oss_provider_with_base_url;
    use codex_app_server_protocol::ConfigLayerSource;
    use codex_protocol::protocol::SessionSource;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;

    fn provider(base_url: Option<&str>) -> crate::ModelProviderInfo {
        crate::ModelProviderInfo {
            name: "test-provider".to_string(),
            base_url: base_url.map(str::to_string),
            env_key: None,
            env_key_instructions: None,
            experimental_bearer_token: None,
            wire_api: crate::WireApi::Responses,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: None,
            stream_max_retries: None,
            stream_idle_timeout_ms: None,
            requires_openai_auth: true,
            supports_websockets: true,
        }
    }

    #[test]
    fn endpoint_config_telemetry_classifies_known_hosts_without_logging_custom_values() {
        let source = EndpointConfigTelemetrySource::new("config_toml", false);

        assert_eq!(
            source.classify("https://chatgpt.com/backend-api/codex"),
            EndpointConfigTelemetry {
                base_url_origin: "chatgpt.com",
                host_class: "openai_chatgpt",
                base_url_source: "config_toml",
                base_url_is_default: false,
            }
        );
        assert_eq!(
            source.classify("https://api.openai.com/v1"),
            EndpointConfigTelemetry {
                base_url_origin: "api.openai.com",
                host_class: "openai_api",
                base_url_source: "config_toml",
                base_url_is_default: false,
            }
        );
        assert_eq!(
            source.classify("https://openrouter.ai/api/v1"),
            EndpointConfigTelemetry {
                base_url_origin: "openrouter.ai",
                host_class: "known_third_party",
                base_url_source: "config_toml",
                base_url_is_default: false,
            }
        );
        assert_eq!(
            source.classify("https://private.example.internal/v1?token=secret"),
            EndpointConfigTelemetry {
                base_url_origin: "custom",
                host_class: "custom_unknown",
                base_url_source: "config_toml",
                base_url_is_default: false,
            }
        );
        assert_eq!(
            source.classify("https://chatgpt.com/api/codex"),
            EndpointConfigTelemetry {
                base_url_origin: "chatgpt.com",
                host_class: "custom_unknown",
                base_url_source: "config_toml",
                base_url_is_default: false,
            }
        );
    }

    #[test]
    fn endpoint_config_telemetry_source_maps_layers_and_defaults() {
        assert_eq!(
            endpoint_source_from_layer(&ConfigLayerSource::SessionFlags, SessionSource::VSCode),
            EndpointConfigTelemetrySource::new("ide_settings", false)
        );
        assert_eq!(
            endpoint_source_from_layer(
                &ConfigLayerSource::Project {
                    dot_codex_folder: AbsolutePathBuf::try_from(std::path::PathBuf::from(
                        "/tmp/project/.codex",
                    ))
                    .expect("absolute path"),
                },
                SessionSource::Cli,
            ),
            EndpointConfigTelemetrySource::new("config_toml", false)
        );
        assert_eq!(
            endpoint_source_from_provider_defaults("openai", &provider(None)),
            EndpointConfigTelemetrySource::new("default", true)
        );
        assert_eq!(
            endpoint_source_from_provider_defaults(
                "custom",
                &provider(Some("https://example.com/v1"))
            ),
            EndpointConfigTelemetrySource::new("config_toml", false)
        );
    }

    #[test]
    fn endpoint_config_telemetry_source_requires_explicit_provider_id_for_builtin_oss_defaults() {
        let provider =
            create_oss_provider_with_base_url("http://localhost:1234/v1", WireApi::Responses);

        assert_eq!(
            EndpointConfigTelemetrySource::for_provider("lmstudio", &provider),
            EndpointConfigTelemetrySource::new("default", true)
        );
        assert_eq!(
            EndpointConfigTelemetrySource::for_provider_without_id(&provider),
            EndpointConfigTelemetrySource::new("config_toml", false)
        );
    }
}
