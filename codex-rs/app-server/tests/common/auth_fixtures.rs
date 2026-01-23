use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::DateTime;
use chrono::Utc;
use codex_core::auth::AuthCredentialsStoreMode;
use codex_core::auth::AuthDotJson;
use codex_core::auth::save_auth;
use codex_core::token_data::TokenData;
use codex_core::token_data::parse_id_token;
use serde_json::json;

/// Builder for writing a fake ChatGPT auth.json in tests.
#[derive(Debug, Clone)]
pub struct ChatGptAuthFixture {
    access_token: String,
    refresh_token: String,
    account_id: Option<String>,
    claims: ChatGptIdTokenClaims,
    last_refresh: Option<Option<DateTime<Utc>>>,
}

impl ChatGptAuthFixture {
    pub fn new(access_token: impl Into<String>) -> Self {
        Self {
            access_token: access_token.into(),
            refresh_token: "refresh-token".to_string(),
            account_id: None,
            claims: ChatGptIdTokenClaims::default(),
            last_refresh: None,
        }
    }

    pub fn refresh_token(mut self, refresh_token: impl Into<String>) -> Self {
        self.refresh_token = refresh_token.into();
        self
    }

    pub fn account_id(mut self, account_id: impl Into<String>) -> Self {
        self.account_id = Some(account_id.into());
        self
    }

    pub fn plan_type(mut self, plan_type: impl Into<String>) -> Self {
        self.claims.plan_type = Some(plan_type.into());
        self
    }

    pub fn chatgpt_user_id(mut self, chatgpt_user_id: impl Into<String>) -> Self {
        self.claims.chatgpt_user_id = Some(chatgpt_user_id.into());
        self
    }

    pub fn chatgpt_account_id(mut self, chatgpt_account_id: impl Into<String>) -> Self {
        self.claims.chatgpt_account_id = Some(chatgpt_account_id.into());
        self
    }

    pub fn email(mut self, email: impl Into<String>) -> Self {
        self.claims.email = Some(email.into());
        self
    }

    pub fn last_refresh(mut self, last_refresh: Option<DateTime<Utc>>) -> Self {
        self.last_refresh = Some(last_refresh);
        self
    }

    pub fn claims(mut self, claims: ChatGptIdTokenClaims) -> Self {
        self.claims = claims;
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct ChatGptIdTokenClaims {
    pub email: Option<String>,
    pub plan_type: Option<String>,
    pub chatgpt_user_id: Option<String>,
    pub chatgpt_account_id: Option<String>,
}

impl ChatGptIdTokenClaims {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn email(mut self, email: impl Into<String>) -> Self {
        self.email = Some(email.into());
        self
    }

    pub fn plan_type(mut self, plan_type: impl Into<String>) -> Self {
        self.plan_type = Some(plan_type.into());
        self
    }

    pub fn chatgpt_user_id(mut self, chatgpt_user_id: impl Into<String>) -> Self {
        self.chatgpt_user_id = Some(chatgpt_user_id.into());
        self
    }

    pub fn chatgpt_account_id(mut self, chatgpt_account_id: impl Into<String>) -> Self {
        self.chatgpt_account_id = Some(chatgpt_account_id.into());
        self
    }
}

pub fn encode_id_token(claims: &ChatGptIdTokenClaims) -> Result<String> {
    let header = json!({ "alg": "none", "typ": "JWT" });
    let mut payload = serde_json::Map::new();
    if let Some(email) = &claims.email {
        payload.insert("email".to_string(), json!(email));
    }
    let mut auth_payload = serde_json::Map::new();
    if let Some(plan_type) = &claims.plan_type {
        auth_payload.insert("chatgpt_plan_type".to_string(), json!(plan_type));
    }
    if let Some(chatgpt_user_id) = &claims.chatgpt_user_id {
        auth_payload.insert("chatgpt_user_id".to_string(), json!(chatgpt_user_id));
    }
    if let Some(chatgpt_account_id) = &claims.chatgpt_account_id {
        auth_payload.insert("chatgpt_account_id".to_string(), json!(chatgpt_account_id));
    }
    if !auth_payload.is_empty() {
        payload.insert(
            "https://api.openai.com/auth".to_string(),
            serde_json::Value::Object(auth_payload),
        );
    }
    let payload = serde_json::Value::Object(payload);

    let header_b64 =
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).context("serialize jwt header")?);
    let payload_b64 =
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).context("serialize jwt payload")?);
    let signature_b64 = URL_SAFE_NO_PAD.encode(b"signature");
    Ok(format!("{header_b64}.{payload_b64}.{signature_b64}"))
}

pub fn write_chatgpt_auth(
    codex_home: &Path,
    fixture: ChatGptAuthFixture,
    cli_auth_credentials_store_mode: AuthCredentialsStoreMode,
) -> Result<()> {
    let id_token_raw = encode_id_token(&fixture.claims)?;
    let id_token = parse_id_token(&id_token_raw).context("parse id token")?;
    let tokens = TokenData {
        id_token,
        access_token: fixture.access_token,
        refresh_token: fixture.refresh_token,
        account_id: fixture.account_id,
    };

    let last_refresh = fixture.last_refresh.unwrap_or_else(|| Some(Utc::now()));

    let auth = AuthDotJson {
        openai_api_key: None,
        tokens: Some(tokens),
        last_refresh,
    };

    save_auth(codex_home, &auth, cli_auth_credentials_store_mode).context("write auth.json")
}
