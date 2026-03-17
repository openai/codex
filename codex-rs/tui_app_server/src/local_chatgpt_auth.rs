use std::path::Path;

use codex_app_server_protocol::ChatgptAuthTokensRefreshResponse;
use codex_core::auth::AuthCredentialsStoreMode;
use codex_core::auth::CodexAuth;
use codex_protocol::account::PlanType as AccountPlanType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalChatgptAuth {
    pub(crate) access_token: String,
    pub(crate) chatgpt_account_id: String,
    pub(crate) chatgpt_plan_type: Option<String>,
}

impl LocalChatgptAuth {
    pub(crate) fn to_refresh_response(&self) -> ChatgptAuthTokensRefreshResponse {
        ChatgptAuthTokensRefreshResponse {
            access_token: self.access_token.clone(),
            chatgpt_account_id: self.chatgpt_account_id.clone(),
            chatgpt_plan_type: self.chatgpt_plan_type.clone(),
        }
    }
}

pub(crate) fn load_local_chatgpt_auth(
    codex_home: &Path,
    auth_credentials_store_mode: AuthCredentialsStoreMode,
    forced_chatgpt_workspace_id: Option<&str>,
) -> Result<LocalChatgptAuth, String> {
    let auth = CodexAuth::from_auth_storage(codex_home, auth_credentials_store_mode)
        .map_err(|err| format!("failed to load local auth: {err}"))?
        .ok_or_else(|| "no local auth available".to_string())?;
    if !auth.is_chatgpt_auth() {
        return Err("local auth is not a ChatGPT login".to_string());
    }

    let access_token = auth
        .get_token()
        .map_err(|err| format!("failed to read local ChatGPT access token: {err}"))?;
    let chatgpt_account_id = auth
        .get_account_id()
        .ok_or_else(|| "local ChatGPT auth is missing chatgpt account id".to_string())?;
    if let Some(expected_workspace) = forced_chatgpt_workspace_id
        && chatgpt_account_id != expected_workspace
    {
        return Err(format!(
            "local ChatGPT auth must use workspace {expected_workspace}, but found {chatgpt_account_id:?}"
        ));
    }

    let chatgpt_plan_type = auth.account_plan_type().map(|plan_type| match plan_type {
        AccountPlanType::Free => "free".to_string(),
        AccountPlanType::Go => "go".to_string(),
        AccountPlanType::Plus => "plus".to_string(),
        AccountPlanType::Pro => "pro".to_string(),
        AccountPlanType::Team => "team".to_string(),
        AccountPlanType::Business => "business".to_string(),
        AccountPlanType::Enterprise => "enterprise".to_string(),
        AccountPlanType::Edu => "edu".to_string(),
        AccountPlanType::Unknown => "unknown".to_string(),
    });

    Ok(LocalChatgptAuth {
        access_token,
        chatgpt_account_id,
        chatgpt_plan_type,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use base64::Engine;
    use chrono::Utc;
    use codex_app_server_protocol::AuthMode;
    use codex_core::auth::AuthDotJson;
    use codex_core::auth::save_auth;
    use codex_core::token_data::TokenData;
    use pretty_assertions::assert_eq;
    use serde::Serialize;
    use serde_json::json;
    use tempfile::TempDir;

    fn fake_jwt(email: &str, account_id: &str, plan_type: &str) -> String {
        #[derive(Serialize)]
        struct Header {
            alg: &'static str,
            typ: &'static str,
        }

        let header = Header {
            alg: "none",
            typ: "JWT",
        };
        let payload = json!({
            "email": email,
            "https://api.openai.com/auth": {
                "chatgpt_account_id": account_id,
                "chatgpt_plan_type": plan_type,
            },
        });
        let encode = |bytes: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        let header_b64 = encode(&serde_json::to_vec(&header).expect("serialize header"));
        let payload_b64 = encode(&serde_json::to_vec(&payload).expect("serialize payload"));
        let signature_b64 = encode(b"sig");
        format!("{header_b64}.{payload_b64}.{signature_b64}")
    }

    fn write_chatgpt_auth(codex_home: &Path) {
        let id_token = fake_jwt("user@example.com", "workspace-1", "business");
        let access_token = fake_jwt("user@example.com", "workspace-1", "business");
        let auth = AuthDotJson {
            auth_mode: Some(AuthMode::Chatgpt),
            openai_api_key: None,
            tokens: Some(TokenData {
                id_token: codex_core::token_data::parse_chatgpt_jwt_claims(&id_token)
                    .expect("id token should parse"),
                access_token,
                refresh_token: "refresh-token".to_string(),
                account_id: Some("workspace-1".to_string()),
            }),
            last_refresh: Some(Utc::now()),
        };
        save_auth(codex_home, &auth, AuthCredentialsStoreMode::File)
            .expect("chatgpt auth should save");
    }

    #[test]
    fn loads_local_chatgpt_auth_from_managed_auth() {
        let codex_home = TempDir::new().expect("tempdir");
        write_chatgpt_auth(codex_home.path());

        let auth = load_local_chatgpt_auth(
            codex_home.path(),
            AuthCredentialsStoreMode::File,
            Some("workspace-1"),
        )
        .expect("chatgpt auth should load");

        assert_eq!(auth.chatgpt_account_id, "workspace-1");
        assert_eq!(auth.chatgpt_plan_type.as_deref(), Some("business"));
        assert!(!auth.access_token.is_empty());
    }

    #[test]
    fn rejects_missing_local_auth() {
        let codex_home = TempDir::new().expect("tempdir");

        let err = load_local_chatgpt_auth(codex_home.path(), AuthCredentialsStoreMode::File, None)
            .expect_err("missing auth should fail");

        assert_eq!(err, "no local auth available");
    }

    #[test]
    fn rejects_api_key_auth() {
        let codex_home = TempDir::new().expect("tempdir");
        save_auth(
            codex_home.path(),
            &AuthDotJson {
                auth_mode: Some(AuthMode::ApiKey),
                openai_api_key: Some("sk-test".to_string()),
                tokens: None,
                last_refresh: None,
            },
            AuthCredentialsStoreMode::File,
        )
        .expect("api key auth should save");

        let err = load_local_chatgpt_auth(codex_home.path(), AuthCredentialsStoreMode::File, None)
            .expect_err("api key auth should fail");

        assert_eq!(err, "local auth is not a ChatGPT login");
    }
}
