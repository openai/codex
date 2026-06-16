use codex_client::CodexHttpClient;
use codex_protocol::account::PlanType as AccountPlanType;
use codex_protocol::auth::PlanType as InternalPlanType;
use serde::Deserialize;
use std::env;
use std::fmt;

use crate::default_client::create_client;

const PROD_AUTHAPI_BASE_URL: &str = "https://auth.openai.com/api/accounts";
const CODEX_AUTHAPI_BASE_URL_ENV_VAR: &str = "CODEX_AUTHAPI_BASE_URL";
const WHOAMI_PATH: &str = "/v1/user-auth-credential/whoami";

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct PersonalAccessTokenMetadata {
    email: String,
    chatgpt_user_id: String,
    chatgpt_account_id: String,
    chatgpt_plan_type: String,
    chatgpt_account_is_fedramp: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub struct PersonalAccessTokenAuth {
    access_token: String,
    metadata: PersonalAccessTokenMetadata,
}

impl fmt::Debug for PersonalAccessTokenAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PersonalAccessTokenAuth")
            .field("access_token", &"<redacted>")
            .field("metadata", &self.metadata)
            .finish()
    }
}

impl PersonalAccessTokenAuth {
    pub(super) async fn load(access_token: &str) -> std::io::Result<Self> {
        let authapi_base_url = env::var(CODEX_AUTHAPI_BASE_URL_ENV_VAR)
            .ok()
            .map(|base_url| base_url.trim().trim_end_matches('/').to_string())
            .filter(|base_url| !base_url.is_empty())
            .unwrap_or_else(|| PROD_AUTHAPI_BASE_URL.to_string());
        hydrate_personal_access_token(&create_client(), &authapi_base_url, access_token).await
    }

    pub fn access_token(&self) -> &str {
        &self.access_token
    }

    pub fn account_id(&self) -> &str {
        &self.metadata.chatgpt_account_id
    }

    pub fn chatgpt_user_id(&self) -> &str {
        &self.metadata.chatgpt_user_id
    }

    pub fn email(&self) -> &str {
        &self.metadata.email
    }

    pub fn plan_type(&self) -> AccountPlanType {
        InternalPlanType::from_raw_value(&self.metadata.chatgpt_plan_type).into()
    }

    pub fn is_fedramp_account(&self) -> bool {
        self.metadata.chatgpt_account_is_fedramp
    }
}

async fn hydrate_personal_access_token(
    client: &CodexHttpClient,
    authapi_base_url: &str,
    access_token: &str,
) -> std::io::Result<PersonalAccessTokenAuth> {
    let endpoint = format!("{}{WHOAMI_PATH}", authapi_base_url.trim_end_matches('/'));
    let response = client
        .get(&endpoint)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|err| {
            std::io::Error::other(format!(
                "failed to request personal access token metadata: {err}"
            ))
        })?;
    if !response.status().is_success() {
        return Err(std::io::Error::other(format!(
            "personal access token metadata request failed with status {}",
            response.status()
        )));
    }

    let metadata = response
        .json::<PersonalAccessTokenMetadata>()
        .await
        .map_err(|err| {
            std::io::Error::other(format!(
                "failed to decode personal access token metadata: {err}"
            ))
        })?;
    Ok(PersonalAccessTokenAuth {
        access_token: access_token.to_string(),
        metadata,
    })
}

#[cfg(test)]
#[path = "personal_access_token_tests.rs"]
mod tests;
