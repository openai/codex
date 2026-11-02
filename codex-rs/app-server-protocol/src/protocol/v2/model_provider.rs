//! Model provider discovery and the configuration fields visible to clients.

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
