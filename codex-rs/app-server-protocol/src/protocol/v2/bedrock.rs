use crate::JsonSchema;
use crate::TS;
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct BedrockDiscoverParams {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct BedrockDiscoverResponse {
    pub profiles: Vec<BedrockAwsProfile>,
    pub environment_credentials: Vec<BedrockEnvironmentCredential>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct BedrockAwsProfile {
    pub name: String,
    pub region: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct BedrockEnvironmentCredential {
    #[serde(rename = "type")]
    #[ts(rename = "type")]
    pub credential_type: AwsCredentialType,
    pub region: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum AwsCredentialType {
    AccessKeys,
    BedrockApiKey,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type", rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum BedrockSetupParams {
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    Profile { profile: String, region: String },
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    Environment { region: String },
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    AccessKeys {
        access_key_id: String,
        secret_access_key: String,
        #[ts(optional = nullable)]
        session_token: Option<String>,
        region: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct BedrockSetupResponse {}
