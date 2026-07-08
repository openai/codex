use serde::Deserialize;
use serde::Serialize;
use strum_macros::Display;
use thiserror::Error;

/// Authentication mode for OpenAI-backed providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthMode {
    /// OpenAI API key provided by the caller and stored by Codex.
    ApiKey,
    /// ChatGPT OAuth managed by Codex (tokens persisted and refreshed by Codex).
    Chatgpt,
    /// ChatGPT auth tokens supplied by an external host application.
    #[serde(rename = "chatgptAuthTokens")]
    #[strum(serialize = "chatgptAuthTokens")]
    ChatgptAuthTokens,
    /// Codex backend auth supplied as request headers.
    #[serde(rename = "headers")]
    #[strum(serialize = "headers")]
    Headers,
    /// Programmatic Codex auth backed by a registered Agent Identity.
    #[serde(rename = "agentIdentity")]
    #[strum(serialize = "agentIdentity")]
    AgentIdentity,
    /// Programmatic Codex auth backed by a personal access token.
    #[serde(rename = "personalAccessToken")]
    #[strum(serialize = "personalAccessToken")]
    PersonalAccessToken,
    /// Amazon Bedrock bearer token managed by Codex.
    #[serde(rename = "bedrockApiKey")]
    #[strum(serialize = "bedrockApiKey")]
    BedrockApiKey,
}

impl AuthMode {
    /// Returns whether this mode represents an authenticated human ChatGPT account.
    pub fn has_chatgpt_account(self) -> bool {
        match self {
            Self::Chatgpt | Self::ChatgptAuthTokens | Self::PersonalAccessToken => true,
            Self::ApiKey | Self::Headers | Self::AgentIdentity | Self::BedrockApiKey => false,
        }
    }

    /// Returns whether this mode is backed by Codex services rather than a direct model API.
    pub fn uses_codex_backend(self) -> bool {
        match self {
            Self::Chatgpt
            | Self::ChatgptAuthTokens
            | Self::Headers
            | Self::AgentIdentity
            | Self::PersonalAccessToken => true,
            Self::ApiKey | Self::BedrockApiKey => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PlanType {
    Known(KnownPlan),
    Unknown(String),
}

impl PlanType {
    pub fn from_raw_value(raw: &str) -> Self {
        match raw.to_ascii_lowercase().as_str() {
            "free" => Self::Known(KnownPlan::Free),
            "go" => Self::Known(KnownPlan::Go),
            "plus" => Self::Known(KnownPlan::Plus),
            "pro" => Self::Known(KnownPlan::Pro),
            "prolite" => Self::Known(KnownPlan::ProLite),
            "team" => Self::Known(KnownPlan::Team),
            "self_serve_business_usage_based" => {
                Self::Known(KnownPlan::SelfServeBusinessUsageBased)
            }
            "business" => Self::Known(KnownPlan::Business),
            "enterprise_cbp_usage_based" => Self::Known(KnownPlan::EnterpriseCbpUsageBased),
            "enterprise" | "hc" => Self::Known(KnownPlan::Enterprise),
            "education" | "edu" => Self::Known(KnownPlan::Edu),
            _ => Self::Unknown(raw.to_string()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KnownPlan {
    Free,
    Go,
    Plus,
    Pro,
    ProLite,
    Team,
    #[serde(rename = "self_serve_business_usage_based")]
    SelfServeBusinessUsageBased,
    Business,
    #[serde(rename = "enterprise_cbp_usage_based")]
    EnterpriseCbpUsageBased,
    #[serde(alias = "hc")]
    Enterprise,
    #[serde(alias = "education")]
    Edu,
}

impl KnownPlan {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Free => "Free",
            Self::Go => "Go",
            Self::Plus => "Plus",
            Self::Pro => "Pro",
            Self::ProLite => "Pro Lite",
            Self::Team => "Team",
            Self::SelfServeBusinessUsageBased => "Self Serve Business Usage Based",
            Self::Business => "Business",
            Self::EnterpriseCbpUsageBased => "Enterprise CBP Usage Based",
            Self::Enterprise => "Enterprise",
            Self::Edu => "Edu",
        }
    }

    pub fn raw_value(self) -> &'static str {
        match self {
            Self::Free => "free",
            Self::Go => "go",
            Self::Plus => "plus",
            Self::Pro => "pro",
            Self::ProLite => "prolite",
            Self::Team => "team",
            Self::SelfServeBusinessUsageBased => "self_serve_business_usage_based",
            Self::Business => "business",
            Self::EnterpriseCbpUsageBased => "enterprise_cbp_usage_based",
            Self::Enterprise => "enterprise",
            Self::Edu => "edu",
        }
    }

    pub fn is_workspace_account(self) -> bool {
        matches!(
            self,
            Self::Team
                | Self::SelfServeBusinessUsageBased
                | Self::Business
                | Self::EnterpriseCbpUsageBased
                | Self::Enterprise
                | Self::Edu
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{message}")]
pub struct RefreshTokenFailedError {
    pub reason: RefreshTokenFailedReason,
    pub message: String,
}

impl RefreshTokenFailedError {
    pub fn new(reason: RefreshTokenFailedReason, message: impl Into<String>) -> Self {
        Self {
            reason,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshTokenFailedReason {
    Expired,
    Exhausted,
    Revoked,
    Other,
}

#[cfg(test)]
mod tests {
    use super::KnownPlan;
    use super::PlanType;
    use pretty_assertions::assert_eq;

    #[test]
    fn plan_type_deserializes_raw_aliases() {
        assert_eq!(
            serde_json::from_str::<PlanType>("\"hc\"").expect("hc should deserialize"),
            PlanType::Known(KnownPlan::Enterprise)
        );
        assert_eq!(
            serde_json::from_str::<PlanType>("\"education\"")
                .expect("education should deserialize"),
            PlanType::Known(KnownPlan::Edu)
        );
    }
}
