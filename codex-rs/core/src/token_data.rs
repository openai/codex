use base64::Engine;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Default)]
pub struct TokenData {
    /// Flat info parsed from the JWT in auth.json.
    #[serde(
        deserialize_with = "deserialize_id_token",
        serialize_with = "serialize_id_token"
    )]
    pub id_token: IdTokenInfo,

    /// This is a JWT.
    pub access_token: String,

    pub refresh_token: String,

    pub account_id: Option<String>,
}

/// Flat subset of useful claims in id_token from auth.json.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct IdTokenInfo {
    pub email: Option<String>,
    /// The ChatGPT subscription plan type
    /// (e.g., "free", "plus", "pro", "business", "enterprise", "edu").
    /// (Note: values may vary by backend.)
    pub(crate) chatgpt_plan_type: Option<PlanType>,
    /// ChatGPT user identifier associated with the token, if present.
    pub chatgpt_user_id: Option<String>,
    /// Organization/workspace identifier associated with the token, if present.
    pub chatgpt_account_id: Option<String>,
    pub raw_jwt: String,
}

impl IdTokenInfo {
    pub fn get_chatgpt_plan_type(&self) -> Option<String> {
        self.chatgpt_plan_type.as_ref().map(|t| match t {
            PlanType::Known(plan) => format!("{plan:?}"),
            PlanType::Unknown(s) => s.clone(),
        })
    }

    pub fn is_workspace_account(&self) -> bool {
        matches!(
            self.chatgpt_plan_type,
            Some(PlanType::Known(
                KnownPlan::Team | KnownPlan::Business | KnownPlan::Enterprise | KnownPlan::Edu
            ))
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum PlanType {
    Known(KnownPlan),
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum KnownPlan {
    Free,
    Go,
    Plus,
    Pro,
    Team,
    Business,
    Enterprise,
    Edu,
}

#[derive(Deserialize)]
struct IdClaims {
    #[serde(default)]
    email: Option<String>,
    #[serde(rename = "https://api.openai.com/profile", default)]
    profile: Option<ProfileClaims>,
    #[serde(rename = "https://api.openai.com/auth", default)]
    auth: Option<AuthClaims>,
}

#[derive(Deserialize)]
struct ProfileClaims {
    #[serde(default)]
    email: Option<String>,
}

#[derive(Deserialize)]
struct AuthClaims {
    #[serde(default)]
    chatgpt_plan_type: Option<PlanType>,
    #[serde(default)]
    chatgpt_user_id: Option<String>,
    #[serde(default)]
    user_id: Option<String>,
    #[serde(default)]
    chatgpt_account_id: Option<String>,
}

#[derive(Debug, Error)]
pub enum IdTokenInfoError {
    #[error("invalid ID token format")]
    InvalidFormat,
    #[error(transparent)]
    Base64(#[from] base64::DecodeError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn parse_id_token(id_token: &str) -> Result<IdTokenInfo, IdTokenInfoError> {
    // JWT format: header.payload.signature
    let mut parts = id_token.split('.');
    let (_header_b64, payload_b64, _sig_b64) = match (parts.next(), parts.next(), parts.next()) {
        (Some(h), Some(p), Some(s)) if !h.is_empty() && !p.is_empty() && !s.is_empty() => (h, p, s),
        _ => return Err(IdTokenInfoError::InvalidFormat),
    };

    let payload_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload_b64)?;
    let claims: IdClaims = serde_json::from_slice(&payload_bytes)?;
    let email = claims
        .email
        .or_else(|| claims.profile.and_then(|profile| profile.email));

    match claims.auth {
        Some(auth) => Ok(IdTokenInfo {
            email,
            raw_jwt: id_token.to_string(),
            chatgpt_plan_type: auth.chatgpt_plan_type,
            chatgpt_user_id: auth.chatgpt_user_id.or(auth.user_id),
            chatgpt_account_id: auth.chatgpt_account_id,
        }),
        None => Ok(IdTokenInfo {
            email,
            raw_jwt: id_token.to_string(),
            chatgpt_plan_type: None,
            chatgpt_user_id: None,
            chatgpt_account_id: None,
        }),
    }
}

fn deserialize_id_token<'de, D>(deserializer: D) -> Result<IdTokenInfo, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    parse_id_token(&s).map_err(serde::de::Error::custom)
}

fn serialize_id_token<S>(id_token: &IdTokenInfo, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&id_token.raw_jwt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde::Serialize;

    #[test]
    fn id_token_info_parses_email_and_plan() {
        #[derive(Serialize)]
        struct Header {
            alg: &'static str,
            typ: &'static str,
        }
        let header = Header {
            alg: "none",
            typ: "JWT",
        };
        let payload = serde_json::json!({
            "email": "user@example.com",
            "https://api.openai.com/auth": {
                "chatgpt_plan_type": "pro"
            }
        });

        fn b64url_no_pad(bytes: &[u8]) -> String {
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
        }

        let header_b64 = b64url_no_pad(&serde_json::to_vec(&header).unwrap());
        let payload_b64 = b64url_no_pad(&serde_json::to_vec(&payload).unwrap());
        let signature_b64 = b64url_no_pad(b"sig");
        let fake_jwt = format!("{header_b64}.{payload_b64}.{signature_b64}");

        let info = parse_id_token(&fake_jwt).expect("should parse");
        assert_eq!(info.email.as_deref(), Some("user@example.com"));
        assert_eq!(info.get_chatgpt_plan_type().as_deref(), Some("Pro"));
    }

    #[test]
    fn id_token_info_parses_go_plan() {
        #[derive(Serialize)]
        struct Header {
            alg: &'static str,
            typ: &'static str,
        }
        let header = Header {
            alg: "none",
            typ: "JWT",
        };
        let payload = serde_json::json!({
            "email": "user@example.com",
            "https://api.openai.com/auth": {
                "chatgpt_plan_type": "go"
            }
        });

        fn b64url_no_pad(bytes: &[u8]) -> String {
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
        }

        let header_b64 = b64url_no_pad(&serde_json::to_vec(&header).unwrap());
        let payload_b64 = b64url_no_pad(&serde_json::to_vec(&payload).unwrap());
        let signature_b64 = b64url_no_pad(b"sig");
        let fake_jwt = format!("{header_b64}.{payload_b64}.{signature_b64}");

        let info = parse_id_token(&fake_jwt).expect("should parse");
        assert_eq!(info.email.as_deref(), Some("user@example.com"));
        assert_eq!(info.get_chatgpt_plan_type().as_deref(), Some("Go"));
    }

    #[test]
    fn id_token_info_handles_missing_fields() {
        #[derive(Serialize)]
        struct Header {
            alg: &'static str,
            typ: &'static str,
        }
        let header = Header {
            alg: "none",
            typ: "JWT",
        };
        let payload = serde_json::json!({ "sub": "123" });

        fn b64url_no_pad(bytes: &[u8]) -> String {
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
        }

        let header_b64 = b64url_no_pad(&serde_json::to_vec(&header).unwrap());
        let payload_b64 = b64url_no_pad(&serde_json::to_vec(&payload).unwrap());
        let signature_b64 = b64url_no_pad(b"sig");
        let fake_jwt = format!("{header_b64}.{payload_b64}.{signature_b64}");

        let info = parse_id_token(&fake_jwt).expect("should parse");
        assert!(info.email.is_none());
        assert!(info.get_chatgpt_plan_type().is_none());
    }

    #[test]
    fn workspace_account_detection_matches_workspace_plans() {
        let workspace = IdTokenInfo {
            chatgpt_plan_type: Some(PlanType::Known(KnownPlan::Business)),
            ..IdTokenInfo::default()
        };
        assert_eq!(workspace.is_workspace_account(), true);

        let personal = IdTokenInfo {
            chatgpt_plan_type: Some(PlanType::Known(KnownPlan::Pro)),
            ..IdTokenInfo::default()
        };
        assert_eq!(personal.is_workspace_account(), false);
    }
}
