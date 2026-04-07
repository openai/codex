#![cfg(test)]

use std::path::Path;

use codex_app_server_protocol::AuthMode;
use codex_login::AuthCredentialsStoreMode;
use codex_login::load_auth_dot_json;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalChatgptAuth {
    pub(crate) access_token: String,
    pub(crate) chatgpt_account_id: String,
    pub(crate) chatgpt_plan_type: Option<String>,
}

pub(crate) fn load_local_chatgpt_auth(
    codex_home: &Path,
    auth_credentials_store_mode: AuthCredentialsStoreMode,
    forced_chatgpt_workspace_id: Option<&str>,
) -> Result<LocalChatgptAuth, String> {
    let auth = load_auth_dot_json(codex_home, auth_credentials_store_mode)
        .map_err(|err| format!("failed to load local auth: {err}"))?
        .ok_or_else(|| "no local auth available".to_string())?;
    if matches!(auth.auth_mode, Some(AuthMode::ApiKey)) || auth.openai_api_key.is_some() {
        return Err("local auth is not a ChatGPT login".to_string());
    }

    let tokens = auth
        .tokens
        .ok_or_else(|| "local ChatGPT auth is missing token data".to_string())?;
    let access_token = tokens.access_token;
    let chatgpt_account_id = tokens
        .account_id
        .or(tokens.id_token.chatgpt_account_id.clone())
        .ok_or_else(|| "local ChatGPT auth is missing chatgpt account id".to_string())?;
    if let Some(expected_workspace) = forced_chatgpt_workspace_id
        && chatgpt_account_id != expected_workspace
    {
        return Err(format!(
            "local ChatGPT auth must use workspace {expected_workspace}, but found {chatgpt_account_id:?}"
        ));
    }

    let chatgpt_plan_type = tokens
        .id_token
        .get_chatgpt_plan_type_raw()
        .map(|plan_type| plan_type.to_ascii_lowercase());

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
    use codex_login::AuthDotJson;
    use codex_login::auth::login_with_chatgpt_auth_tokens;
    use codex_login::save_auth;
    use codex_login::token_data::TokenData;
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

    fn write_chatgpt_auth(codex_home: &Path, plan_type: &str) {
        let id_token = fake_jwt("user@example.com", "workspace-1", plan_type);
        let access_token = fake_jwt("user@example.com", "workspace-1", plan_type);
        let auth = AuthDotJson {
            auth_mode: Some(AuthMode::Chatgpt),
            openai_api_key: None,
            tokens: Some(TokenData {
                id_token: codex_login::token_data::parse_chatgpt_jwt_claims(&id_token)
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
        write_chatgpt_auth(codex_home.path(), "business");

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

        let err = load_local_chatgpt_auth(
            codex_home.path(),
            AuthCredentialsStoreMode::File,
            /*forced_chatgpt_workspace_id*/ None,
        )
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

        let err = load_local_chatgpt_auth(
            codex_home.path(),
            AuthCredentialsStoreMode::File,
            /*forced_chatgpt_workspace_id*/ None,
        )
        .expect_err("api key auth should fail");

        assert_eq!(err, "local auth is not a ChatGPT login");
    }

    #[test]
    fn prefers_managed_auth_over_external_ephemeral_tokens() {
        let codex_home = TempDir::new().expect("tempdir");
        write_chatgpt_auth(codex_home.path(), "business");
        login_with_chatgpt_auth_tokens(
            codex_home.path(),
            &fake_jwt("user@example.com", "workspace-2", "enterprise"),
            "workspace-2",
            Some("enterprise"),
        )
        .expect("external auth should save");

        let auth = load_local_chatgpt_auth(
            codex_home.path(),
            AuthCredentialsStoreMode::File,
            Some("workspace-1"),
        )
        .expect("managed auth should win");

        assert_eq!(auth.chatgpt_account_id, "workspace-1");
        assert_eq!(auth.chatgpt_plan_type.as_deref(), Some("business"));
    }

    #[test]
    fn preserves_usage_based_plan_type_wire_name() {
        let codex_home = TempDir::new().expect("tempdir");
        write_chatgpt_auth(codex_home.path(), "self_serve_business_usage_based");

        let auth = load_local_chatgpt_auth(
            codex_home.path(),
            AuthCredentialsStoreMode::File,
            Some("workspace-1"),
        )
        .expect("chatgpt auth should load");

        assert_eq!(
            auth.chatgpt_plan_type.as_deref(),
            Some("self_serve_business_usage_based")
        );
    }
}
