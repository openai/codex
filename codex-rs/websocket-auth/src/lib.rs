//! Shared authentication for incoming app-server and exec-server WebSocket upgrades.
//! Policies retain token digests or JWT verification secrets; callers choose when auth is required.

use anyhow::Context;
use clap::Args;
use clap::ValueEnum;
use codex_utils_absolute_path::AbsolutePathBuf;
use constant_time_eq::constant_time_eq_32;
use http::HeaderMap;
use http::StatusCode;
use http::header::AUTHORIZATION;
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
pub struct WebsocketAuthArgs {
    /// Authentication mode for incoming WebSocket connections.
    #[arg(long = "ws-auth", value_name = "MODE", value_enum)]
    pub ws_auth: Option<WebsocketAuthCliMode>,

    /// Absolute path to the capability-token file.
    #[arg(long = "ws-token-file", value_name = "PATH")]
    pub ws_token_file: Option<PathBuf>,

    /// Hex-encoded SHA-256 digest of the capability token.
    #[arg(long = "ws-token-sha256", value_name = "HEX")]
    pub ws_token_sha256: Option<String>,

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
pub struct WebsocketAuthSettings {
    pub config: Option<WebsocketAuthConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebsocketAuthConfig {
    CapabilityToken {
        source: WebsocketCapabilityTokenSource,
    },
    SignedBearerToken {
        shared_secret_file: AbsolutePathBuf,
        issuer: Option<String>,
        audience: Option<String>,
        max_clock_skew_seconds: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebsocketCapabilityTokenSource {
    TokenFile { token_file: AbsolutePathBuf },
    TokenSha256 { token_sha256: [u8; 32] },
}

/// Loaded upgrade policy. The default permits unauthenticated connections; each
/// listener decides whether that policy is allowed for its bind address.
#[derive(Clone, Debug, Default)]
pub struct WebsocketAuthPolicy {
    mode: Option<WebsocketAuthMode>,
}

#[derive(Clone)]
enum WebsocketAuthMode {
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

impl std::fmt::Debug for WebsocketAuthMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CapabilityToken { .. } => {
                f.debug_struct("CapabilityToken").finish_non_exhaustive()
            }
            Self::SignedBearerToken { .. } => {
                f.debug_struct("SignedBearerToken").finish_non_exhaustive()
            }
        }
    }
}

#[derive(Debug)]
pub struct WebsocketAuthError {
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
    pub fn status_code(&self) -> StatusCode {
        self.status_code
    }

    pub fn message(&self) -> &'static str {
        self.message
    }
}

impl WebsocketAuthArgs {
    pub fn try_into_settings(self) -> anyhow::Result<WebsocketAuthSettings> {
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
                let source = match (self.ws_token_file, self.ws_token_sha256) {
                    (Some(_), Some(_)) => {
                        anyhow::bail!(
                            "`--ws-token-file` and `--ws-token-sha256` are mutually exclusive"
                        );
                    }
                    (Some(token_file), None) => WebsocketCapabilityTokenSource::TokenFile {
                        token_file: absolute_path_arg("--ws-token-file", token_file)?,
                    },
                    (None, Some(token_sha256)) => WebsocketCapabilityTokenSource::TokenSha256 {
                        token_sha256: sha256_digest_arg("--ws-token-sha256", &token_sha256)?,
                    },
                    (None, None) => {
                        anyhow::bail!(
                            "`--ws-token-file` or `--ws-token-sha256` is required when `--ws-auth capability-token` is set"
                        );
                    }
                };
                Some(WebsocketAuthConfig::CapabilityToken { source })
            }
            Some(WebsocketAuthCliMode::SignedBearerToken) => {
                if self.ws_token_file.is_some() || self.ws_token_sha256.is_some() {
                    anyhow::bail!(
                        "`--ws-token-file` and `--ws-token-sha256` require `--ws-auth capability-token`, not `signed-bearer-token`"
                    );
                }
                let shared_secret_file = self.ws_shared_secret_file.context(
                    "`--ws-shared-secret-file` is required when `--ws-auth signed-bearer-token` is set",
                )?;
                Some(WebsocketAuthConfig::SignedBearerToken {
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
                    || self.ws_token_sha256.is_some()
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

        Ok(WebsocketAuthSettings { config })
    }
}

/// Loads credentials once; changes to secret files do not update an existing policy.
pub fn policy_from_settings(settings: &WebsocketAuthSettings) -> io::Result<WebsocketAuthPolicy> {
    let mode = match settings.config.as_ref() {
        Some(WebsocketAuthConfig::CapabilityToken { source }) => match source {
            WebsocketCapabilityTokenSource::TokenFile { token_file } => {
                let token = read_trimmed_secret(token_file.as_ref())?;
                Some(WebsocketAuthMode::CapabilityToken {
                    token_sha256: sha256_digest(token.as_bytes()),
                })
            }
            WebsocketCapabilityTokenSource::TokenSha256 { token_sha256 } => {
                Some(WebsocketAuthMode::CapabilityToken {
                    token_sha256: *token_sha256,
                })
            }
        },
        Some(WebsocketAuthConfig::SignedBearerToken {
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

pub fn is_unauthenticated_non_loopback_listener(
    bind_address: SocketAddr,
    policy: &WebsocketAuthPolicy,
) -> bool {
    !bind_address.ip().is_loopback() && policy.mode.is_none()
}

/// Checks the bearer credential before upgrading. This does not enforce expiry
/// or revocation after a WebSocket has been established.
pub fn authorize_upgrade(
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

fn sha256_digest_arg(flag_name: &str, value: &str) -> anyhow::Result<[u8; 32]> {
    let trimmed = value.trim();
    if trimmed.len() != 64 {
        anyhow::bail!("{flag_name} must be a 64-character hex SHA-256 digest");
    }

    let mut digest = [0u8; 32];
    for (index, pair) in trimmed.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_nibble(flag_name, pair[0])?;
        let low = hex_nibble(flag_name, pair[1])?;
        digest[index] = (high << 4) | low;
    }
    Ok(digest)
}

fn hex_nibble(flag_name: &str, byte: u8) -> anyhow::Result<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => anyhow::bail!("{flag_name} must be a 64-character hex SHA-256 digest"),
    }
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
#[path = "auth_tests.rs"]
mod tests;
