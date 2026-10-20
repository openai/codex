//! Read-only profile statistics shared with the desktop account summary.
//! Optional fields retain absence; unknown invocation kinds do not invalidate other statistics.

use super::Client;
use super::RequestError;
use crate::TokenUsageProfileStats;
use http::Method;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct AccountProfile {
    pub profile: Option<ProfileIdentity>,
    pub metadata: Option<ProfileMetadata>,
    pub stats: ProfileStats,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ProfileIdentity {
    pub display_name: Option<String>,
    pub username: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ProfileMetadata {
    pub stats_as_of: Option<String>,
    pub stats_error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ProfileStats {
    #[serde(flatten)]
    pub tokens: TokenUsageProfileStats,
    pub fast_mode_usage_percentage: Option<serde_json::Number>,
    pub most_used_reasoning_effort: Option<String>,
    pub most_used_reasoning_effort_percentage: Option<serde_json::Number>,
    pub unique_skills_used: Option<u64>,
    pub total_skills_used: Option<u64>,
    pub total_threads: Option<u64>,
    pub top_invocations: Option<Vec<ProfileInvocation>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ProfileInvocation {
    #[serde(rename = "type")]
    pub kind: ProfileInvocationKind,
    pub plugin_name: Option<String>,
    pub skill_name: Option<String>,
    pub usage_count: Option<u64>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProfileInvocationKind {
    Plugin,
    Skill,
    #[serde(other)]
    Unknown,
}

impl Client {
    /// Read the full profile using the same route as account token usage.
    pub async fn get_account_profile(&self) -> Result<AccountProfile, RequestError> {
        let url = self.token_usage_profile_url();
        let request = self.request(Method::GET, &url).headers(self.headers()?);
        let (body, _) = self.exec_request_detailed(request, "GET", &url).await?;
        serde_json::from_str(&body)
            .map_err(|_| RequestError::Other(anyhow::anyhow!("Invalid account profile response.")))
    }
}

#[cfg(test)]
#[path = "profile_tests.rs"]
mod tests;
