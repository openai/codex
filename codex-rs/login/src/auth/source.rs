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
    let previous_subject = previous.api_workload_identity();
    let current_subject = current.api_workload_identity();
    if previous_subject != current_subject {
        return false;
    }
    if let (CodexAuth::AgentIdentity(previous), CodexAuth::AgentIdentity(current)) =
        (previous, current)
    {
        return previous.record() == current.record();
    }
    let (Ok(previous_token), Ok(current_token)) = (previous.get_token(), current.get_token())
    else {
        return false;
    };
    if previous_token == current_token {
        return true;
    }
    let platform_wif = previous_subject.is_some();
    if !platform_wif
        && !matches!(
            previous,
            CodexAuth::Chatgpt(_) | CodexAuth::ChatgptAuthTokens(_)
        )
    {
        return false;
    }
    match (
        parse_oauth_source(&previous_token),
        parse_oauth_source(&current_token),
    ) {
        (Some(previous), Some(current)) => previous == current,
        _ => false,
    }
}

pub(super) fn has_session_claim(token: &str) -> bool {
    parse_oauth_source(token).is_some()
}

fn parse_oauth_source(token: &str) -> Option<OAuthSource> {
    if token.split('.').count() != 3 {
        return None;
    }
    let source = decode_jwt_payload::<OAuthSource>(token).ok()?;
    (!source.iss.trim().is_empty() && !source.session_id.trim().is_empty()).then_some(source)
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
    wif_runtime_authorization_version: Option<u32>,
    wif_provider_revision: Option<String>,
    wif_mapping_revision: Option<String>,
}
