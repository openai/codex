//! Safe provider discovery and field patches for custom, AWS, and GCP configuration.

use crate::JsonSchema;
use crate::TS;
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ModelProviderListParams {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ModelProviderListResponse {
    pub data: Vec<ModelProviderSummary>,
    pub default_provider_id: String,
    pub can_change_default: bool,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ModelProviderSummary {
    pub id: String,
    pub name: String,
    /// Whether a supported configuration field can be edited in the active user layer.
    pub can_edit: bool,
    /// Null when the endpoint is absent or cannot be safely displayed.
    pub base_url: Option<String>,
    pub settings: ProviderSettings,
}

impl std::fmt::Debug for ModelProviderSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelProviderSummary")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("can_edit", &self.can_edit)
            .field("base_url", &"[REDACTED]")
            .field("settings", &self.settings)
            .finish()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type", rename_all = "camelCase", export_to = "v2/")]
pub enum ProviderSettings {
    #[serde(rename = "openai")]
    #[ts(rename = "openai")]
    OpenAi,
    CustomGateway {
        auth: GatewayAuth,
    },
    Bedrock {
        profile: Option<String>,
        region: Option<String>,
    },
    Gcp {
        #[serde(rename = "projectId")]
        #[ts(rename = "projectId")]
        project_id: Option<String>,
        location: Option<String>,
        /// Path on the app-server host, without credential-file contents.
        #[serde(rename = "credentialFile")]
        #[ts(rename = "credentialFile")]
        credential_file: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(tag = "mode", rename_all = "camelCase")]
#[ts(tag = "mode", rename_all = "camelCase", export_to = "v2/")]
pub enum GatewayAuth {
    None,
    EnvironmentVariable {
        #[serde(rename = "envKey")]
        #[ts(rename = "envKey")]
        env_key: String,
    },
    /// Authentication is configured outside the gateway form. Preserve it on edits.
    ExternallyConfigured {
        #[serde(rename = "envKey")]
        #[ts(rename = "envKey")]
        env_key: Option<String>,
    },
}

/// Omitted patch fields preserve their values. Explicit null clears a nullable field.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
#[ts(tag = "type", rename_all = "camelCase", export_to = "v2/")]
pub enum ProviderSettingsPatch {
    CustomGateway(CustomGatewaySettingsParams),
    Bedrock(BedrockSettingsParams),
    Gcp(GcpSettingsParams),
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(rename_all = "kebab-case", export_to = "v2/")]
pub enum BedrockProviderId {
    AmazonBedrock,
    AmazonBedrockRuntime,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub enum GcpProviderId {
    Gcp,
}

#[derive(Serialize, Deserialize, Clone, Default, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "v2/")]
pub struct CustomGatewaySettingsParams {
    /// Omit or pass null to allocate a new custom provider ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub provider_id: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_patch_name",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "String", skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub name: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::serde_helpers::deserialize_double_option",
        serialize_with = "crate::protocol::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Option<String>", skip_serializing_if = "Option::is_none")]
    #[ts(type = "string | null")]
    #[ts(optional = nullable)]
    pub base_url: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::serde_helpers::deserialize_double_option",
        serialize_with = "crate::protocol::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Option<String>", skip_serializing_if = "Option::is_none")]
    #[ts(type = "string | null")]
    #[ts(optional = nullable)]
    pub env_key: Option<Option<String>>,
}

/// AWS configuration fields for built-in Bedrock providers.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "v2/")]
pub struct BedrockSettingsParams {
    pub provider_id: BedrockProviderId,
    #[serde(
        default,
        deserialize_with = "crate::protocol::serde_helpers::deserialize_double_option",
        serialize_with = "crate::protocol::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Option<String>", skip_serializing_if = "Option::is_none")]
    #[ts(type = "string | null")]
    #[ts(optional = nullable)]
    pub profile: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::serde_helpers::deserialize_double_option",
        serialize_with = "crate::protocol::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Option<String>", skip_serializing_if = "Option::is_none")]
    #[ts(type = "string | null")]
    #[ts(optional = nullable)]
    pub region: Option<Option<String>>,
}

/// GCP configuration fields. Credential-file paths refer to the app-server host.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "v2/")]
pub struct GcpSettingsParams {
    pub provider_id: GcpProviderId,
    #[serde(
        default,
        deserialize_with = "crate::protocol::serde_helpers::deserialize_double_option",
        serialize_with = "crate::protocol::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Option<String>", skip_serializing_if = "Option::is_none")]
    #[ts(type = "string | null")]
    #[ts(optional = nullable)]
    pub project_id: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::serde_helpers::deserialize_double_option",
        serialize_with = "crate::protocol::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Option<String>", skip_serializing_if = "Option::is_none")]
    #[ts(type = "string | null")]
    #[ts(optional = nullable)]
    pub location: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "crate::protocol::serde_helpers::deserialize_double_option",
        serialize_with = "crate::protocol::serde_helpers::serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Option<String>", skip_serializing_if = "Option::is_none")]
    #[ts(type = "string | null")]
    #[ts(optional = nullable)]
    pub credential_file: Option<Option<String>>,
}

// A missing name preserves it, but an explicitly supplied name must be a string.
fn deserialize_patch_name<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    String::deserialize(deserializer).map(Some)
}

impl std::fmt::Debug for CustomGatewaySettingsParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CustomGatewaySettingsParams")
            .field("name", &self.name)
            .field("base_url", &"[REDACTED]")
            .field("env_key", &self.env_key)
            .field("provider_id", &self.provider_id)
            .finish()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "v2/")]
pub struct ModelProviderConfigWriteParams {
    pub settings: ProviderSettingsPatch,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ModelProviderConfigWriteResponse {
    pub provider: ModelProviderSummary,
}
