//! Compare actor credential sources for local runtime reuse, not server authentication.
//!
//! OAuth session claims are only continuity hints. The registration service verifies
//! the credential and binds the runtime; these values never grant authority locally.

use super::CodexAuth;
use crate::token_data::decode_jwt_payload;
use serde::Deserialize;

pub(super) fn same_source(previous: Option<&CodexAuth>, current: Option<&CodexAuth>) -> bool {
    let (Some(previous), Some(current)) = (previous, current) else {
        return false;
    };
    if previous.api_auth_mode() != current.api_auth_mode()
        || previous.get_account_id() != current.get_account_id()
        || previous.get_chatgpt_user_id() != current.get_chatgpt_user_id()
    {
        return false;
    }
    let (Ok(previous_token), Ok(current_token)) = (previous.get_token(), current.get_token())
    else {
        return false;
    };
    if previous_token == current_token {
        return true;
    }
    match previous {
        CodexAuth::Chatgpt(_) | CodexAuth::ChatgptAuthTokens(_) => {
            if previous_token.split('.').count() != 3 || current_token.split('.').count() != 3 {
                return false;
            }
            let (Ok(previous), Ok(current)) = (
                decode_jwt_payload::<OAuthSource>(&previous_token),
                decode_jwt_payload::<OAuthSource>(&current_token),
            ) else {
                // Sessionless credentials must refresh during bootstrap before a
                // source-bound runtime can be registered. Never guess continuity.
                return false;
            };
            !previous.iss.trim().is_empty()
                && !previous.session_id.trim().is_empty()
                && previous == current
        }
        CodexAuth::ApiKey(_)
        | CodexAuth::PersonalAccessToken(_)
        | CodexAuth::Headers(_)
        | CodexAuth::AgentIdentity(_)
        | CodexAuth::BedrockApiKey(_)
        | CodexAuth::BedrockAccessKeys(_) => false,
    }
}

#[derive(Deserialize, PartialEq, Eq)]
struct OAuthSource {
    iss: String,
    session_id: String,
    sub: Option<String>,
    client_id: Option<String>,
    azp: Option<String>,
    #[serde(rename = "https://api.openai.com/auth", default)]
    identity: OAuthIdentity,
}

#[derive(Default, Deserialize, PartialEq, Eq)]
struct OAuthIdentity {
    user_id: Option<String>,
    chatgpt_user_id: Option<String>,
    chatgpt_account_id: Option<String>,
    chatgpt_account_user_id: Option<String>,
    chatgpt_backing_organization_id: Option<String>,
    wif_identity_provider_id: Option<String>,
    wif_federation_rule_id: Option<String>,
    wif_tenant_id: Option<String>,
    wif_external_issuer: Option<String>,
    wif_principal_kind: Option<String>,
    wif_product: Option<String>,
}
