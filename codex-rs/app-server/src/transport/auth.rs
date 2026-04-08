use anyhow::Context;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use clap::Args;
use clap::ValueEnum;
use codex_utils_absolute_path::AbsolutePathBuf;
use constant_time_eq::constant_time_eq_32;
use jsonwebtoken::Algorithm;
use jsonwebtoken::DecodingKey;
use jsonwebtoken::Validation;
use jsonwebtoken::decode;
use serde::Deserialize;
use sha2::Digest;
use sha2::Sha256;
use std::io;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::path::Path;
use std::path::PathBuf;
use time::OffsetDateTime;

const DEFAULT_MAX_CLOCK_SKEW_SECONDS: u64 = 30;
const MIN_SIGNED_BEARER_SECRET_BYTES: usize = 32;
const INVALID_AUTHORIZATION_HEADER_MESSAGE: &str = "invalid authorization header";

#[derive(Debug, Clone, Default, PartialEq, Eq, Args)]
pub struct AppServerWebsocketAuthArgs {
    /// Websocket auth mode for non-loopback listeners.
    #[arg(long = "ws-auth", value_name = "MODE", value_enum)]
    pub ws_auth: Option<WebsocketAuthCliMode>,

    /// Absolute path to the capability-token file.
    #[arg(long = "ws-token-file", value_name = "PATH")]
    pub ws_token_file: Option<PathBuf>,

    /// Absolute path to the shared secret file for signed JWT bearer tokens.
    #[arg(long = "ws-shared-secret-file", value_name = "PATH")]
    pub ws_shared_secret_file: Option<PathBuf>,

    /// Expected issuer for signed JWT bearer tokens.
    #[arg(long = "ws-issuer", value_name = "ISSUER")]
    pub ws_issuer: Option<String>,

    /// Expected audience for signed JWT bearer tokens.
    #[arg(long = "ws-audience", value_name = "AUDIENCE")]
    pub ws_audience: Option<String>,

    /// Maximum clock skew when validating signed JWT bearer tokens.
    #[arg(long = "ws-max-clock-skew-seconds", value_name = "SECONDS")]
    pub ws_max_clock_skew_seconds: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum WebsocketAuthCliMode {
    CapabilityToken,
    SignedBearerToken,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppServerWebsocketAuthSettings {
    pub config: Option<AppServerWebsocketAuthConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppServerWebsocketAuthConfig {
    CapabilityToken {
        token_file: AbsolutePathBuf,
    },
    SignedBearerToken {
        shared_secret_file: AbsolutePathBuf,
        issuer: Option<String>,
        audience: Option<String>,
        max_clock_skew_seconds: u64,
    },
}

#[derive(Clone, Debug, Default)]
pub(crate) struct WebsocketAuthPolicy {
    pub(crate) mode: Option<WebsocketAuthMode>,
}

#[derive(Clone, Debug)]
pub(crate) enum WebsocketAuthMode {
    CapabilityToken {
        token_sha256: [u8; 32],
    },
    SignedBearerToken {
        shared_secret: Vec<u8>,
        issuer: Option<String>,
        audience: Option<String>,
        max_clock_skew_seconds: i64,
    },
}

#[derive(Debug)]
pub(crate) struct WebsocketAuthError {
    status_code: StatusCode,
    message: &'static str,
}

#[derive(Deserialize)]
struct JwtClaims {
    exp: i64,
    nbf: Option<i64>,
    iss: Option<String>,
    aud: Option<JwtAudienceClaim>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum JwtAudienceClaim {
    Single(String),
    Multiple(Vec<String>),
}

impl WebsocketAuthError {
    pub(crate) fn status_code(&self) -> StatusCode {
        self.status_code
    }

    pub(crate) fn message(&self) -> &'static str {
        self.message
    }
}

impl AppServerWebsocketAuthArgs {
    pub fn try_into_settings(self) -> anyhow::Result<AppServerWebsocketAuthSettings> {
        let normalize = |value: Option<String>| {
            value.and_then(|value| {
                let trimmed = value.trim();
                (!trimmed.is_empty()).then(|| trimmed.to_string())
            })
        };

        let config = match self.ws_auth {
            Some(WebsocketAuthCliMode::CapabilityToken) => {
                if self.ws_shared_secret_file.is_some()
                    || self.ws_issuer.is_some()
                    || self.ws_audience.is_some()
                    || self.ws_max_clock_skew_seconds.is_some()
                {
                    anyhow::bail!(
                        "`--ws-shared-secret-file`, `--ws-issuer`, `--ws-audience`, and `--ws-max-clock-skew-seconds` require `--ws-auth signed-bearer-token`"
                    );
                }
                let token_file = self.ws_token_file.context(
                    "`--ws-token-file` is required when `--ws-auth capability-token` is set",
                )?;
                Some(AppServerWebsocketAuthConfig::CapabilityToken {
                    token_file: absolute_path_arg("--ws-token-file", token_file)?,
                })
            }
            Some(WebsocketAuthCliMode::SignedBearerToken) => {
                if self.ws_token_file.is_some() {
                    anyhow::bail!(
                        "`--ws-token-file` requires `--ws-auth capability-token`, not `signed-bearer-token`"
                    );
                }
                let shared_secret_file = self.ws_shared_secret_file.context(
                    "`--ws-shared-secret-file` is required when `--ws-auth signed-bearer-token` is set",
                )?;
                Some(AppServerWebsocketAuthConfig::SignedBearerToken {
                    shared_secret_file: absolute_path_arg(
                        "--ws-shared-secret-file",
                        shared_secret_file,
                    )?,
                    issuer: normalize(self.ws_issuer),
                    audience: normalize(self.ws_audience),
                    max_clock_skew_seconds: self
                        .ws_max_clock_skew_seconds
                        .unwrap_or(DEFAULT_MAX_CLOCK_SKEW_SECONDS),
                })
            }
            None => {
                if self.ws_token_file.is_some()
                    || self.ws_shared_secret_file.is_some()
                    || self.ws_issuer.is_some()
                    || self.ws_audience.is_some()
                    || self.ws_max_clock_skew_seconds.is_some()
                {
                    anyhow::bail!(
                        "websocket auth flags require `--ws-auth capability-token` or `--ws-auth signed-bearer-token`"
                    );
                }
                None
            }
        };

        Ok(AppServerWebsocketAuthSettings { config })
    }
}

pub(crate) fn policy_from_settings(
    settings: &AppServerWebsocketAuthSettings,
) -> io::Result<WebsocketAuthPolicy> {
    let mode = match settings.config.as_ref() {
        Some(AppServerWebsocketAuthConfig::CapabilityToken { token_file }) => {
            let token = read_trimmed_secret(token_file.as_ref())?;
            Some(WebsocketAuthMode::CapabilityToken {
                token_sha256: sha256_digest(token.as_bytes()),
            })
        }
        Some(AppServerWebsocketAuthConfig::SignedBearerToken {
            shared_secret_file,
            issuer,
            audience,
            max_clock_skew_seconds,
        }) => {
            let shared_secret = read_trimmed_secret(shared_secret_file.as_ref())?.into_bytes();
            validate_signed_bearer_secret(shared_secret_file.as_ref(), &shared_secret)?;
            let max_clock_skew_seconds = i64::try_from(*max_clock_skew_seconds).map_err(|_| {
                io::Error::new(
                    ErrorKind::InvalidInput,
                    "websocket auth clock skew must fit in a signed 64-bit integer",
                )
            })?;
            Some(WebsocketAuthMode::SignedBearerToken {
                shared_secret,
                issuer: issuer.clone(),
                audience: audience.clone(),
                max_clock_skew_seconds,
            })
        }
        None => None,
    };

    Ok(WebsocketAuthPolicy { mode })
}

pub(crate) fn should_warn_about_unauthenticated_non_loopback_listener(
    bind_address: SocketAddr,
    policy: &WebsocketAuthPolicy,
) -> bool {
    !bind_address.ip().is_loopback() && policy.mode.is_none()
}

pub(crate) fn authorize_upgrade(
    headers: &HeaderMap,
    policy: &WebsocketAuthPolicy,
) -> Result<(), WebsocketAuthError> {
    let Some(mode) = policy.mode.as_ref() else {
        return Ok(());
    };

    let token = bearer_token_from_headers(headers)?;
    match mode {
        WebsocketAuthMode::CapabilityToken { token_sha256 } => {
            let actual_sha256 = sha256_digest(token.as_bytes());
            if constant_time_eq_32(token_sha256, &actual_sha256) {
                Ok(())
            } else {
                Err(unauthorized("invalid websocket bearer token"))
            }
        }
        WebsocketAuthMode::SignedBearerToken {
            shared_secret,
            issuer,
            audience,
            max_clock_skew_seconds,
        } => verify_signed_bearer_token(
            token,
            shared_secret,
            issuer.as_deref(),
            audience.as_deref(),
            *max_clock_skew_seconds,
        ),
    }
}

fn verify_signed_bearer_token(
    token: &str,
    shared_secret: &[u8],
    issuer: Option<&str>,
    audience: Option<&str>,
    max_clock_skew_seconds: i64,
) -> Result<(), WebsocketAuthError> {
    let claims = decode_jwt_claims(token, shared_secret)?;
    validate_jwt_claims(&claims, issuer, audience, max_clock_skew_seconds)
}

fn decode_jwt_claims(token: &str, shared_secret: &[u8]) -> Result<JwtClaims, WebsocketAuthError> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.required_spec_claims.clear();
    validation.validate_exp = false;
    validation.validate_nbf = false;
    validation.validate_aud = false;

    decode::<JwtClaims>(token, &DecodingKey::from_secret(shared_secret), &validation)
        .map(|token_data| token_data.claims)
        .map_err(|_| unauthorized("invalid websocket jwt"))
}

fn validate_jwt_claims(
    claims: &JwtClaims,
    issuer: Option<&str>,
    audience: Option<&str>,
    max_clock_skew_seconds: i64,
) -> Result<(), WebsocketAuthError> {
    let now = OffsetDateTime::now_utc().unix_timestamp();
    if now > claims.exp.saturating_add(max_clock_skew_seconds) {
        return Err(unauthorized("expired websocket jwt"));
    }
    if let Some(nbf) = claims.nbf
        && now < nbf.saturating_sub(max_clock_skew_seconds)
    {
        return Err(unauthorized("websocket jwt is not valid yet"));
    }
    if let Some(expected_issuer) = issuer
        && claims.iss.as_deref() != Some(expected_issuer)
    {
        return Err(unauthorized("websocket jwt issuer mismatch"));
    }
    if let Some(expected_audience) = audience
        && !audience_matches(claims.aud.as_ref(), expected_audience)
    {
        return Err(unauthorized("websocket jwt audience mismatch"));
    }

    Ok(())
}

fn audience_matches(audience: Option<&JwtAudienceClaim>, expected_audience: &str) -> bool {
    match audience {
        Some(JwtAudienceClaim::Single(actual)) => actual == expected_audience,
        Some(JwtAudienceClaim::Multiple(actual)) => {
            actual.iter().any(|audience| audience == expected_audience)
        }
        None => false,
    }
}

fn bearer_token_from_headers(headers: &HeaderMap) -> Result<&str, WebsocketAuthError> {
    let raw_header = headers
        .get(AUTHORIZATION)
        .ok_or_else(|| unauthorized("missing websocket bearer token"))?;
    let header = raw_header
        .to_str()
        .map_err(|_| unauthorized(INVALID_AUTHORIZATION_HEADER_MESSAGE))?;
    let Some((scheme, token)) = header.split_once(' ') else {
        return Err(unauthorized(INVALID_AUTHORIZATION_HEADER_MESSAGE));
    };
    if !scheme.eq_ignore_ascii_case("Bearer") {
        return Err(unauthorized(INVALID_AUTHORIZATION_HEADER_MESSAGE));
    }
    let token = token.trim();
    if token.is_empty() {
        return Err(unauthorized(INVALID_AUTHORIZATION_HEADER_MESSAGE));
    }
    Ok(token)
}

fn validate_signed_bearer_secret(path: &Path, shared_secret: &[u8]) -> io::Result<()> {
    if shared_secret.len() < MIN_SIGNED_BEARER_SECRET_BYTES {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            format!(
                "signed websocket bearer secret {} must be at least {MIN_SIGNED_BEARER_SECRET_BYTES} bytes",
                path.display()
            ),
        ));
    }
    Ok(())
}

fn read_trimmed_secret(path: &std::path::Path) -> io::Result<String> {
    let raw = std::fs::read_to_string(path).map_err(|err| {
        io::Error::new(
            err.kind(),
            format!(
                "failed to read websocket auth secret {}: {err}",
                path.display()
            ),
        )
    })?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            format!("websocket auth secret {} must not be empty", path.display()),
        ));
    }
    Ok(trimmed.to_string())
}

fn absolute_path_arg(flag_name: &str, path: PathBuf) -> anyhow::Result<AbsolutePathBuf> {
    AbsolutePathBuf::try_from(path).with_context(|| format!("{flag_name} must be an absolute path"))
}

fn sha256_digest(input: &[u8]) -> [u8; 32] {
    let mut digest = [0u8; 32];
    digest.copy_from_slice(&Sha256::digest(input));
    digest
}

fn unauthorized(message: &'static str) -> WebsocketAuthError {
    WebsocketAuthError {
        status_code: StatusCode::UNAUTHORIZED,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use core_test_support::test_absolute_path;
    use core_test_support::test_path_buf;
    use hmac::Hmac;
    use hmac::Mac;
    use serde_json::json;

    type HmacSha256 = Hmac<Sha256>;

    fn signed_token(shared_secret: &[u8], claims: serde_json::Value) -> String {
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"HS256","typ":"JWT"}"#);
        let claims_segment = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap());
        let payload = format!("{header}.{claims_segment}");
        let mut mac = HmacSha256::new_from_slice(shared_secret).unwrap();
        mac.update(payload.as_bytes());
        let signature = URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());
        format!("{payload}.{signature}")
    }

    #[test]
    fn warns_about_unauthenticated_non_loopback_listener() {
        let policy = WebsocketAuthPolicy::default();
        assert!(should_warn_about_unauthenticated_non_loopback_listener(
            "0.0.0.0:8765".parse().unwrap(),
            &policy,
        ));
        assert!(!should_warn_about_unauthenticated_non_loopback_listener(
            "127.0.0.1:8765".parse().unwrap(),
            &policy,
        ));
        assert!(!should_warn_about_unauthenticated_non_loopback_listener(
            "0.0.0.0:8765".parse().unwrap(),
            &WebsocketAuthPolicy {
                mode: Some(WebsocketAuthMode::CapabilityToken {
                    token_sha256: [0u8; 32],
                }),
            },
        ));
    }

    #[test]
    fn capability_token_args_require_token_file() {
        let err = AppServerWebsocketAuthArgs {
            ws_auth: Some(WebsocketAuthCliMode::CapabilityToken),
            ..Default::default()
        }
        .try_into_settings()
        .expect_err("capability-token mode should require a token file");
        assert!(
            err.to_string().contains("--ws-token-file"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn signed_bearer_args_require_mode_when_mode_specific_flags_are_set() {
        let err = AppServerWebsocketAuthArgs {
            ws_shared_secret_file: Some(test_path_buf("/tmp/secret")),
            ..Default::default()
        }
        .try_into_settings()
        .expect_err("mode-specific flags should require --ws-auth");
        assert!(
            err.to_string().contains("websocket auth flags require"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn signed_bearer_args_default_clock_skew_and_trim_optional_claims() {
        let settings = AppServerWebsocketAuthArgs {
            ws_auth: Some(WebsocketAuthCliMode::SignedBearerToken),
            ws_shared_secret_file: Some(test_path_buf("/tmp/secret")),
            ws_issuer: Some(" issuer ".to_string()),
            ws_audience: Some("   ".to_string()),
            ..Default::default()
        }
        .try_into_settings()
        .expect("signed bearer args should parse");

        assert_eq!(
            settings,
            AppServerWebsocketAuthSettings {
                config: Some(AppServerWebsocketAuthConfig::SignedBearerToken {
                    shared_secret_file: test_absolute_path("/tmp/secret"),
                    issuer: Some("issuer".to_string()),
                    audience: None,
                    max_clock_skew_seconds: DEFAULT_MAX_CLOCK_SKEW_SECONDS,
                }),
            }
        );
    }

    #[test]
    fn signed_bearer_token_verification_rejects_tampering() {
        let shared_secret = b"0123456789abcdef0123456789abcdef";
        let token = signed_token(
            shared_secret,
            json!({
                "exp": OffsetDateTime::now_utc().unix_timestamp() + 60,
            }),
        );
        let tampered = token.replace(".eyJleHAi", ".eyJleHBi");
        let err = verify_signed_bearer_token(
            &tampered,
            shared_secret,
            /*issuer*/ None,
            /*audience*/ None,
            /*max_clock_skew_seconds*/ 30,
        )
        .expect_err("tampered jwt should fail");
        assert_eq!(err.status_code(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn signed_bearer_token_verification_accepts_valid_token() {
        let shared_secret = b"0123456789abcdef0123456789abcdef";
        let token = signed_token(
            shared_secret,
            json!({
                "exp": OffsetDateTime::now_utc().unix_timestamp() + 60,
                "iss": "issuer",
                "aud": "audience",
            }),
        );
        verify_signed_bearer_token(
            &token,
            shared_secret,
            Some("issuer"),
            Some("audience"),
            /*max_clock_skew_seconds*/ 30,
        )
        .expect("valid signed token should verify");
    }

    #[test]
    fn signed_bearer_token_verification_accepts_multiple_audiences() {
        let shared_secret = b"0123456789abcdef0123456789abcdef";
        let token = signed_token(
            shared_secret,
            json!({
                "exp": OffsetDateTime::now_utc().unix_timestamp() + 60,
                "aud": ["other-audience", "audience"],
            }),
        );
        verify_signed_bearer_token(
            &token,
            shared_secret,
            /*issuer*/ None,
            Some("audience"),
            /*max_clock_skew_seconds*/ 30,
        )
        .expect("jwt audience arrays should verify");
    }

    #[test]
    fn signed_bearer_token_verification_rejects_alg_none_tokens() {
        let claims_segment = URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&json!({
                "exp": OffsetDateTime::now_utc().unix_timestamp() + 60,
            }))
            .unwrap(),
        );
        let header_segment = URL_SAFE_NO_PAD.encode(br#"{"alg":"none","typ":"JWT"}"#);
        let token = format!("{header_segment}.{claims_segment}.");
        let err = verify_signed_bearer_token(
            &token,
            b"0123456789abcdef0123456789abcdef",
            /*issuer*/ None,
            /*audience*/ None,
            /*max_clock_skew_seconds*/ 30,
        )
        .expect_err("alg=none jwt should be rejected");
        assert_eq!(err.status_code(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn signed_bearer_token_verification_rejects_missing_exp() {
        let shared_secret = b"0123456789abcdef0123456789abcdef";
        let token = signed_token(
            shared_secret,
            json!({
                "iss": "issuer",
            }),
        );
        let err = verify_signed_bearer_token(
            &token,
            shared_secret,
            /*issuer*/ None,
            /*audience*/ None,
            /*max_clock_skew_seconds*/ 30,
        )
        .expect_err("jwt without exp should be rejected");
        assert_eq!(err.status_code(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn validate_signed_bearer_secret_rejects_short_secret() {
        let err = validate_signed_bearer_secret(Path::new("/tmp/secret"), b"too-short")
            .expect_err("short shared secret should be rejected");
        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert!(
            err.to_string().contains("must be at least 32 bytes"),
            "unexpected error: {err}"
        );
    }
}
