//! Trusted, destination-scoped AWS signing after network and HTTP hook authorization.
//! Credentials never enter the workload; unsupported streaming signatures fail closed.

use crate::credential_broker::env_value;
use crate::credential_broker::remove_env_value;
use crate::credential_broker::set_env_value;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::ensure;
use aws_credential_types::Credentials;
use aws_sigv4::http_request::PayloadChecksumKind;
use aws_sigv4::http_request::PercentEncodingMode;
use aws_sigv4::http_request::SignableBody;
use aws_sigv4::http_request::SignableRequest;
use aws_sigv4::http_request::SigningSettings;
use aws_sigv4::http_request::UriPathNormalizationMode;
use aws_sigv4::http_request::sign;
use aws_sigv4::sign::v4;
use codex_utils_absolute_path::AbsolutePathBuf;
use rama_core::bytes::Bytes;
use rama_core::extensions::ExtensionsMut;
use rama_core::extensions::ExtensionsRef;
use rama_http::Body;
use rama_http::HeaderMap;
use rama_http::HeaderValue;
use rama_http::Request;
use rama_http::body::StreamingBody as _;
use rama_http::header::HeaderName;
use rama_http::headers::Connection;
use rama_http::headers::HeaderMapExt as _;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest as _;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;

/// A credential source in the trusted proxy process, never in the executor environment.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProxySecretSource {
    EnvVar(String),
    File(String),
}

impl ProxySecretSource {
    pub(crate) fn validate(&self) -> Result<()> {
        match self {
            Self::EnvVar(name) if !name.is_empty() && !name.contains(['=', '\0']) => Ok(()),
            Self::File(path) if std::path::Path::new(path).is_absolute() => Ok(()),
            Self::EnvVar(_) | Self::File(_) => Err(anyhow!("invalid proxy secret source")),
        }
    }

    pub(crate) fn resolve(
        &self,
        env: &impl Fn(&str) -> Option<String>,
        file: &impl Fn(&AbsolutePathBuf) -> Result<String>,
    ) -> Result<String> {
        self.validate()?;
        let value = match self {
            Self::EnvVar(name) => env(name).ok_or_else(|| anyhow!("missing proxy credential"))?,
            Self::File(path) => file(&AbsolutePathBuf::from_absolute_path(path)?)?,
        };
        if value.is_empty() || value.len() > 65_536 {
            return Err(anyhow!("proxy credential is empty or exceeds 64 KiB"));
        }
        Ok(value)
    }
}

/// AWS SigV4 action selected by an exact-host, method, and path MITM hook.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AwsSigV4Config {
    /// Nonsecret access key ID installed in the guest AWS profile.
    pub placeholder_access_key_id: String,
    pub region: String,
    pub service: String,
    pub access_key_id: ProxySecretSource,
    pub secret_access_key: ProxySecretSource,
    pub session_token: Option<ProxySecretSource>,
    #[serde(default = "https_port")]
    pub port: u16,
    #[serde(default = "default_max_body_bytes")]
    pub max_body_bytes: usize,
}

pub(crate) fn https_port() -> u16 {
    443
}
pub(crate) fn default_max_body_bytes() -> usize {
    8 * 1024 * 1024
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct AwsSigV4Signer {
    config: AwsSigV4Config,
    credentials: Credentials,
}

impl std::fmt::Debug for AwsSigV4Signer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AwsSigV4Signer")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl AwsSigV4Config {
    pub(crate) fn validate(&self) -> Result<()> {
        ensure!(
            !self.placeholder_access_key_id.is_empty()
                && self.placeholder_access_key_id.len() <= 128
                && self
                    .placeholder_access_key_id
                    .bytes()
                    .all(|byte| { byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-') }),
            "invalid AWS placeholder access key ID"
        );
        for value in [&self.region, &self.service] {
            if value.is_empty()
                || !value
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-')
            {
                return Err(anyhow!("invalid AWS signing region or service"));
            }
        }
        ensure!(self.port != 0, "invalid AWS signing port");
        ensure!(
            self.max_body_bytes <= 16 * 1024 * 1024,
            "AWS signing body limit exceeds 16 MiB"
        );
        self.access_key_id.validate()?;
        self.secret_access_key.validate()?;
        if let Some(token) = &self.session_token {
            token.validate()?;
        }
        Ok(())
    }

    pub(crate) fn compile(
        &self,
        env: &impl Fn(&str) -> Option<String>,
        file: &impl Fn(&AbsolutePathBuf) -> Result<String>,
    ) -> Result<AwsSigV4Signer> {
        self.validate()?;
        let credentials = Credentials::new(
            self.access_key_id.resolve(env, file)?,
            self.secret_access_key.resolve(env, file)?,
            self.session_token
                .as_ref()
                .map(|source| source.resolve(env, file))
                .transpose()?,
            /*expires_after*/ None,
            "network-proxy",
        );
        Ok(AwsSigV4Signer {
            config: self.clone(),
            credentials,
        })
    }
}

#[derive(Clone)]
struct UnsupportedSigningKeys(Vec<String>);

impl std::fmt::Debug for UnsupportedSigningKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("UnsupportedSigningKeys")
    }
}

// Keep only the guest key IDs needed to deny a selected request after hop-header removal.
pub(crate) fn preserve_signing_headers(req: &mut Request) {
    let headers = req.headers();
    if headers.contains_key("authorization")
        && (unsupported_signing_headers(headers)
            || headers
                .typed_get::<Connection>()
                .is_some_and(|connection| connection.contains_header("authorization")))
    {
        let keys = headers
            .get_all("authorization")
            .iter()
            .filter_map(|value| value.to_str().ok())
            .flat_map(authorization_keys)
            .map(str::to_owned)
            .collect();
        req.extensions_mut().insert(UnsupportedSigningKeys(keys));
    }
}

fn authorization_keys(auth: &str) -> impl Iterator<Item = &str> {
    let fields = match auth.split_once(' ') {
        Some(("AWS4-HMAC-SHA256" | "AWS4-ECDSA-P256-SHA256", fields)) => fields,
        _ => "",
    };
    fields
        .split(',')
        .filter_map(|field| field.trim().strip_prefix("Credential=")?.split('/').next())
}

fn unsupported_signing_headers(headers: &HeaderMap) -> bool {
    !crate::body_budget::supported_transfer_framing(headers)
        || crate::body_budget::is_amazon_event_stream(headers)
        || headers.contains_key("trailer")
        || headers.contains_key("x-amz-trailer")
        || headers.contains_key("x-amz-decoded-content-length")
        || headers.get_all("content-encoding").iter().any(|value| {
            value.to_str().map_or(true, |value| {
                value
                    .split(',')
                    .any(|coding| coding.trim().eq_ignore_ascii_case("aws-chunked"))
            })
        })
        || headers
            .get_all("x-amz-content-sha256")
            .iter()
            .any(|value| value.as_bytes().starts_with(b"STREAMING-"))
}

/// Client signing inputs captured before any hook changes the request headers.
pub(crate) struct GuestSignature {
    time: SystemTime,
    signed_headers: Vec<HeaderName>,
    unsigned_payload: bool,
}

const DUMMY_SECRET: &str = "codex-local-aws-secret";
const DUMMY_TOKEN: &str = "codex-local-aws-token";
pub(crate) const AWS_CA_BUNDLE: &str = "AWS_CA_BUNDLE";

/// Every value exposed to a child must be safe with respect to every configured pair.
pub(crate) fn validate_guest_placeholders<'a>(
    signers: impl Iterator<Item = &'a AwsSigV4Signer> + Clone,
) -> Result<()> {
    for signer in signers.clone() {
        let credentials = &signer.credentials;
        let mut placeholders = [DUMMY_SECRET, DUMMY_TOKEN].into_iter().chain(
            signers
                .clone()
                .map(|other| other.config.placeholder_access_key_id.as_str()),
        );
        ensure!(
            placeholders.all(|placeholder| {
                [
                    Some(credentials.access_key_id()),
                    Some(credentials.secret_access_key()),
                    credentials.session_token(),
                ]
                .into_iter()
                .flatten()
                .all(|real| !placeholder.contains(real))
            }),
            "AWS guest placeholders overlap a real credential"
        );
    }
    Ok(())
}

pub(crate) fn snapshot_env_keys(env: &HashMap<String, String>) -> Option<[&'static str; 5]> {
    (env_value(env, "AWS_SECRET_ACCESS_KEY") == Some(DUMMY_SECRET)
        || env_value(env, AWS_CA_BUNDLE)
            .is_some_and(crate::certs::is_managed_mitm_ca_trust_bundle_path))
    .then_some([
        "AWS_ACCESS_KEY_ID",
        "AWS_SECRET_ACCESS_KEY",
        "AWS_SESSION_TOKEN",
        "AWS_SECURITY_TOKEN",
        AWS_CA_BUNDLE,
    ])
}

/// Prepare the standard AWS environment for a local process before generic credential discovery.
pub(crate) fn mask_local_aws_env(
    signers: &[Arc<AwsSigV4Signer>],
    env: &mut HashMap<String, String>,
    ca_path: Option<&Path>,
) {
    if signers.is_empty() {
        return;
    }
    if env_value(env, AWS_CA_BUNDLE).is_none_or(str::is_empty)
        && let Some(path) = ca_path
    {
        set_env_value(env, AWS_CA_BUNDLE, path.to_string_lossy().into_owned());
    }
    let access = env_value(env, "AWS_ACCESS_KEY_ID").filter(|id| !id.is_empty());
    let use_default = access.is_none()
        && env_value(env, "AWS_PROFILE").is_none()
        && env_value(env, "AWS_DEFAULT_PROFILE").is_none();
    let mut candidates = signers.iter().filter(|s| {
        use_default
            || access.is_some_and(|id| {
                id == s.credentials.access_key_id() || id == s.config.placeholder_access_key_id
            })
    });
    let first = candidates.next();
    let selected = first.filter(|first| {
        candidates.all(|s| {
            s.config.placeholder_access_key_id == first.config.placeholder_access_key_id
                && s.credentials.session_token().is_some()
                    == first.credentials.session_token().is_some()
        })
    });
    let ambiguous_access = access.is_some() && first.is_some() && selected.is_none();
    // Mask aliases of every configured pair, including pairs the SDK didn't select.
    for value in env.values_mut() {
        mask_local_aws_text(signers, value);
    }
    if ambiguous_access {
        for key in [
            "AWS_ACCESS_KEY_ID",
            "AWS_SECRET_ACCESS_KEY",
            "AWS_SESSION_TOKEN",
            "AWS_SECURITY_TOKEN",
        ] {
            remove_env_value(env, key);
        }
    } else if let Some(signer) = selected {
        set_env_value(
            env,
            "AWS_ACCESS_KEY_ID",
            signer.config.placeholder_access_key_id.clone(),
        );
        set_env_value(env, "AWS_SECRET_ACCESS_KEY", DUMMY_SECRET.to_string());
        remove_env_value(env, "AWS_SECURITY_TOKEN");
        if signer.credentials.session_token().is_some() {
            set_env_value(env, "AWS_SESSION_TOKEN", DUMMY_TOKEN.to_string());
        } else {
            remove_env_value(env, "AWS_SESSION_TOKEN");
        }
    }
}

pub(crate) fn mask_local_aws_text(signers: &[Arc<AwsSigV4Signer>], text: &mut String) {
    for signer in signers {
        for (real, dummy) in [
            (
                Some(signer.credentials.access_key_id()),
                signer.config.placeholder_access_key_id.as_str(),
            ),
            (Some(signer.credentials.secret_access_key()), DUMMY_SECRET),
            (signer.credentials.session_token(), DUMMY_TOKEN),
        ] {
            if let Some(real) = real
                && text.contains(real)
            {
                *text = text.replace(real, dummy);
            }
        }
    }
}

impl AwsSigV4Signer {
    pub(crate) fn plan(&self, req: &Request) -> Result<Option<GuestSignature>> {
        let placeholder = &self.config.placeholder_access_key_id;
        let selects_pair = |scope: &str| scope.split('/').next() == Some(placeholder.as_str());
        let mut query =
            url::form_urlencoded::parse(req.uri().query().unwrap_or_default().as_bytes());
        ensure!(
            !query.clone().any(|(key, value)| {
                key.eq_ignore_ascii_case("x-amz-credential") && selects_pair(&value)
            }),
            "cannot re-sign a presigned AWS URL"
        );
        ensure!(
            req.extensions()
                .get::<UnsupportedSigningKeys>()
                .is_none_or(|keys| !keys.0.contains(placeholder)),
            "unsupported AWS signing headers"
        );
        let auths = req.headers().get_all("authorization");
        let selected = auths
            .iter()
            .filter_map(|value| value.to_str().ok())
            .find(|auth| authorization_keys(auth).any(|key| key == placeholder));
        let Some(auth) = selected else {
            return Ok(None);
        };
        ensure!(
            auths.iter().count() == 1
                && !query.any(|(key, _)| key.eq_ignore_ascii_case("x-amz-signature"))
                && !unsupported_signing_headers(req.headers()),
            "ambiguous or unsupported AWS signing request"
        );
        let fields = auth
            .strip_prefix("AWS4-HMAC-SHA256 ")
            .ok_or_else(|| anyhow!("unsupported AWS signature algorithm"))?;
        let mut scope = None;
        let mut signed = None;
        let mut signature = None;
        for field in fields.split(',') {
            match field.trim().split_once('=') {
                Some(("Credential", value)) if scope.is_none() => scope = Some(value),
                Some(("SignedHeaders", value)) if signed.is_none() => signed = Some(value),
                Some(("Signature", value)) if signature.is_none() => signature = Some(value),
                _ => return Err(anyhow!("invalid AWS authorization field")),
            }
        }
        ensure!(
            signature.is_some_and(|v| v.len() == 64 && v.bytes().all(|b| b.is_ascii_hexdigit())),
            "invalid AWS signature"
        );
        let dates = req.headers().get_all("x-amz-date");
        let mut dates = dates.iter();
        let date = dates
            .next()
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| anyhow!("missing AWS signing date"))?;
        let time = chrono::NaiveDateTime::parse_from_str(date, "%Y%m%dT%H%M%SZ")
            .map_err(|_| anyhow!("invalid AWS signing date"))?;
        ensure!(
            dates.next().is_none()
                && date.len() == 16
                && date.is_ascii()
                && scope.is_some_and(|s| s.split('/').eq([
                    placeholder.as_str(),
                    &date[..8],
                    &self.config.region,
                    &self.config.service,
                    "aws4_request",
                ])),
            "AWS credential scope does not match the configured destination"
        );
        let signed_headers = signed
            .ok_or_else(|| anyhow!("missing AWS signed headers"))?
            .split(';')
            .map(|name| {
                HeaderName::from_lowercase(name.as_bytes())
                    .map_err(|_| anyhow!("invalid AWS signed header"))
            })
            .collect::<Result<Vec<_>>>()?;
        ensure!(
            signed_headers
                .windows(2)
                .all(|w| w[0].as_str() < w[1].as_str())
                && signed_headers.contains(&rama_http::header::HOST)
                && signed_headers.iter().any(|name| name == "x-amz-date")
                && signed_headers.iter().all(|name| {
                    name != "authorization"
                        && (name == "host"
                            || (name == "x-amz-security-token"
                                && self.credentials.session_token().is_some())
                            || req.headers().contains_key(name))
                }),
            "invalid AWS signed header list"
        );
        let hashes = req.headers().get_all("x-amz-content-sha256");
        ensure!(hashes.iter().count() <= 1, "ambiguous AWS payload hash");
        let unsigned_payload = hashes.iter().any(|v| v == "UNSIGNED-PAYLOAD");
        ensure!(
            !unsigned_payload || self.config.service != "aoss",
            "OpenSearch Serverless requires a signed payload"
        );
        Ok(Some(GuestSignature {
            time: time.and_utc().into(),
            signed_headers,
            unsigned_payload,
        }))
    }

    pub(crate) async fn sign(&self, mut req: Request, guest: &GuestSignature) -> Result<Request> {
        ensure!(
            req.uri().scheme_str() == Some("https")
                && req.uri().port_u16().unwrap_or(443) == self.config.port
                && !unsupported_signing_headers(req.headers()),
            "AWS signing requires the configured HTTPS origin and supported framing"
        );
        let had_payload_hash = req.headers().contains_key("x-amz-content-sha256");
        let mut permit = None;
        let (mut parts, body, payload_hash) = if guest.unsigned_payload {
            let (parts, body) = req.into_parts();
            ensure!(
                !parts.headers.contains_key("content-length")
                    || body
                        .size_hint()
                        .exact()
                        .and_then(|n| usize::try_from(n).ok())
                        .is_some_and(|n| crate::body_budget::content_length_matches(
                            &parts.headers,
                            n
                        )),
                "ambiguous AWS unsigned payload length"
            );
            (parts, body, "UNSIGNED-PAYLOAD".to_string())
        } else {
            let (bytes, trailers) = if req.body().is_end_stream() {
                (Bytes::new(), None)
            } else {
                permit = Some(crate::body_budget::reserve(&mut req).await?);
                let body = std::mem::take(req.body_mut());
                tokio::time::timeout(
                    Duration::from_secs(30),
                    crate::body_budget::collect_body(body, self.config.max_body_bytes),
                )
                .await
                .map_err(|_| anyhow!("AWS signing body read timed out"))??
            };
            let (parts, _) = req.into_parts();
            ensure!(
                trailers.is_none()
                    && crate::body_budget::content_length_matches(&parts.headers, bytes.len()),
                "AWS signing requires unambiguous payload framing"
            );
            let digest = format!("{:x}", sha2::Sha256::digest(&bytes));
            (parts, Body::from(bytes), digest)
        };
        if ["connection", "proxy-connection", "upgrade", "te"]
            .iter()
            .any(|name| parts.headers.contains_key(*name))
        {
            return Err(anyhow!(
                "AWS signing cannot forward connection-specific headers"
            ));
        }
        // Rama removes these HTTP/1 fields when the upstream negotiates HTTP/2.
        // They have no meaning for an ordinary signed request.
        for name in ["keep-alive", "sec-websocket-key", "transfer-encoding"] {
            parts.headers.remove(name);
        }
        // Recreate signing headers from the selected pair and the captured guest signature.
        for name in [
            "authorization",
            "x-amz-date",
            "x-amz-security-token",
            "x-amz-content-sha256",
        ] {
            parts.headers.remove(name);
        }
        // HTTP/2-to-HTTP/1.1 forwarding joins Cookie fields with "; ". Sign that
        // representation now so upstream protocol negotiation cannot change it.
        let cookies = parts
            .headers
            .get_all("cookie")
            .iter()
            .map(HeaderValue::to_str)
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|_| anyhow!("AWS signing requires textual headers"))?;
        if cookies.len() > 1 {
            let mut cookie = HeaderValue::from_str(&cookies.join("; "))
                .map_err(|_| anyhow!("invalid AWS signing cookie header"))?;
            cookie.set_sensitive(
                parts
                    .headers
                    .get_all("cookie")
                    .iter()
                    .any(HeaderValue::is_sensitive),
            );
            parts.headers.insert("cookie", cookie);
        }
        let mut settings = SigningSettings::default();
        settings.excluded_headers = None;
        if matches!(
            self.config.service.as_str(),
            "s3" | "s3-object-lambda" | "s3-outposts"
        ) {
            settings.percent_encoding_mode = PercentEncodingMode::Single;
            settings.uri_path_normalization_mode = UriPathNormalizationMode::Disabled;
        }
        // S3 and AOSS need the hash on the wire even when the client did not sign that field.
        if had_payload_hash
            || matches!(
                self.config.service.as_str(),
                "s3" | "s3-object-lambda" | "s3-outposts" | "aoss"
            )
        {
            parts.headers.insert(
                "x-amz-content-sha256",
                HeaderValue::from_str(&payload_hash)?,
            );
        }
        if guest
            .signed_headers
            .iter()
            .any(|h| h == "x-amz-content-sha256")
        {
            settings.payload_checksum_kind = PayloadChecksumKind::XAmzSha256;
        }
        // Error messages are deliberately static: SDK errors can include request or credential data.
        for name in &guest.signed_headers {
            match name.as_str() {
                "host" | "x-amz-date" | "x-amz-content-sha256" => {}
                "x-amz-security-token" if self.credentials.session_token().is_some() => {}
                _ => ensure!(
                    parts.headers.contains_key(name),
                    "AWS signed header was removed"
                ),
            }
        }
        let headers = parts
            .headers
            .iter()
            .filter(|(name, _)| guest.signed_headers.contains(name))
            .map(|(name, value)| {
                value
                    .to_str()
                    .map(|value| (name.as_str(), value))
                    .map_err(|_| anyhow!("AWS signing requires textual headers"))
            })
            .collect::<Result<Vec<_>>>()?;
        let uri = parts.uri.to_string();
        let request = SignableRequest::new(
            parts.method.as_str(),
            &uri,
            headers.into_iter(),
            if guest.unsigned_payload {
                SignableBody::UnsignedPayload
            } else {
                SignableBody::Precomputed(payload_hash)
            },
        )
        .map_err(|_| anyhow!("invalid AWS signing request"))?;
        let identity = self.credentials.clone().into();
        let params = v4::SigningParams::builder()
            .identity(&identity)
            .region(&self.config.region)
            .name(&self.config.service)
            .time(guest.time)
            .settings(settings)
            .build()
            .map_err(|_| anyhow!("invalid AWS signing parameters"))?;
        // The SDK's TRACE events include canonical headers (including session tokens).
        // Signing is synchronous: suppress only those SDK events, without changing proxy audit logs.
        let (instructions, _) =
            tracing::dispatcher::with_default(&tracing::Dispatch::none(), || {
                sign(request, &params.into())
            })
            .map_err(|_| anyhow!("AWS signing failed"))?
            .into_parts();
        for header in instructions.into_parts().0 {
            let mut value = HeaderValue::from_str(header.value())
                .map_err(|_| anyhow!("invalid AWS signed header"))?;
            value.set_sensitive(
                header.sensitive()
                    || matches!(header.name(), "authorization" | "x-amz-security-token"),
            );
            parts.headers.insert(header.name(), value);
        }
        let req = Request::from_parts(parts, body);
        Ok(if permit.is_some() {
            crate::body_budget::retain(req)
        } else {
            req
        })
    }
}

#[cfg(test)]
#[path = "aws_sigv4_tests.rs"]
pub(crate) mod tests;
