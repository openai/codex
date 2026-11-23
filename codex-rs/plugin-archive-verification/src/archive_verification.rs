//! Verifies Plugin Service archive bytes before extraction when the release includes signing
//! metadata. Missing signing metadata preserves legacy behavior, while malformed metadata fails at
//! the download boundary. Diagnostics contain no signature or key material.

use crate::SigningKeyCache;
use aws_lc_rs::digest::SHA256;
use aws_lc_rs::digest::digest;
use aws_lc_rs::signature::ParsedPublicKey;
use aws_lc_rs::signature::RSA_PKCS1_2048_8192_SHA256;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde_json::Value;
use std::fmt;
use std::time::Instant;

use codex_http_client::RequestBuilder;
use http::HeaderMap;
use std::future::Future;

const ARCHIVE_SIGNING_ALGORITHM: &str = "RSA_PKCS1_SHA256";
const MAX_SIGNATURE_BASE64_BYTES: usize = 2048;
const MAX_KEY_ID_BYTES: usize = 512;
pub(super) const ARCHIVE_VERIFICATION_EVENT: &str = "codex.plugins.archive_verification";

/// Download metadata supplied with an authorized Plugin Service archive URL.
/// Signing metadata is deliberately omitted when catalog responses are persisted.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct RemotePluginArchiveDownload {
    #[serde(default)]
    pub bundle_download_url: Option<String>,
    #[serde(default, skip_serializing)]
    signing: ArchiveVerification,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
enum ArchiveVerification {
    #[default]
    Absent,
    Present(SigningMetadata),
    Malformed,
}

#[derive(Clone, PartialEq, Eq, Deserialize)]
struct SigningMetadata {
    sha256: String,
    signature: String,
    key_id: String,
    algorithm: String,
}

impl fmt::Debug for SigningMetadata {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SigningMetadata { .. }")
    }
}

// Defer malformed signing metadata until download so unchanged cached versions remain reusable.
impl<'de> Deserialize<'de> for ArchiveVerification {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        let verification = match value {
            Value::Null => Self::Absent,
            value @ Value::Object(_) => match serde_json::from_value::<SigningMetadata>(value) {
                Ok(metadata) => Self::Present(metadata),
                Err(_) => Self::Malformed,
            },
            Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => {
                Self::Malformed
            }
        };
        Ok(verification)
    }
}

/// Result of checking a downloaded remote plugin archive before it can be installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginArchiveVerification {
    Verified,
    NotRequired,
}

impl PluginArchiveVerification {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::NotRequired => "not_required",
        }
    }
}

/// Bounded failure categories safe for install diagnostics and metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ArchiveVerificationFailure {
    #[error("archive signature metadata is malformed")]
    MalformedMetadata,
    #[error("archive signature algorithm is unsupported")]
    UnsupportedArchiveAlgorithm,
    #[error("archive digest does not match its signed metadata")]
    DigestMismatch,
    #[error("archive digest computation failed")]
    DigestComputationFailed,
    #[error("authentication is required to fetch archive signing keys")]
    AuthenticationRequired,
    #[error("archive signing keys are unavailable")]
    SigningKeysUnavailable,
    #[error("archive signing-key response is invalid")]
    InvalidSigningKeysResponse,
    #[error("archive signing key is unknown")]
    UnknownSigningKey,
    #[error("archive signing key has an unsupported purpose")]
    UnsupportedKeyPurpose,
    #[error("archive signing key has an unsupported algorithm")]
    UnsupportedKeyAlgorithm,
    #[error("archive signing public key is invalid")]
    InvalidPublicKey,
    #[error("archive signature does not verify")]
    SignatureMismatch,
}

impl ArchiveVerificationFailure {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MalformedMetadata => "malformed_metadata",
            Self::UnsupportedArchiveAlgorithm => "unsupported_archive_algorithm",
            Self::DigestMismatch => "digest_mismatch",
            Self::DigestComputationFailed => "digest_computation_failed",
            Self::AuthenticationRequired => "authentication_required",
            Self::SigningKeysUnavailable => "signing_keys_unavailable",
            Self::InvalidSigningKeysResponse => "invalid_signing_keys_response",
            Self::UnknownSigningKey => "unknown_signing_key",
            Self::UnsupportedKeyPurpose => "unsupported_key_purpose",
            Self::UnsupportedKeyAlgorithm => "unsupported_key_algorithm",
            Self::InvalidPublicKey => "invalid_public_key",
            Self::SignatureMismatch => "signature_mismatch",
        }
    }
}

struct DetachedSignature<'a> {
    expected_digest: [u8; 32],
    bytes: Vec<u8>,
    key_id: &'a str,
}

impl<'a> TryFrom<&'a SigningMetadata> for DetachedSignature<'a> {
    type Error = ArchiveVerificationFailure;

    fn try_from(value: &'a SigningMetadata) -> Result<Self, Self::Error> {
        let SigningMetadata {
            sha256,
            signature,
            key_id,
            algorithm,
        } = value;
        if algorithm != ARCHIVE_SIGNING_ALGORITHM {
            return Err(ArchiveVerificationFailure::UnsupportedArchiveAlgorithm);
        }
        let mut expected_digest = [0; 32];
        hex::decode_to_slice(sha256, &mut expected_digest)
            .map_err(|_| ArchiveVerificationFailure::MalformedMetadata)?;
        // The decoder accepts uppercase hex; release digests must be lowercase.
        if hex::encode(expected_digest) != *sha256
            || signature.len() > MAX_SIGNATURE_BASE64_BYTES
            || key_id.is_empty()
            || key_id.len() > MAX_KEY_ID_BYTES
        {
            return Err(ArchiveVerificationFailure::MalformedMetadata);
        }
        let signature_bytes = STANDARD
            .decode(signature)
            .map_err(|_| ArchiveVerificationFailure::MalformedMetadata)?;
        if signature_bytes.is_empty() {
            return Err(ArchiveVerificationFailure::MalformedMetadata);
        }
        Ok(Self {
            expected_digest,
            bytes: signature_bytes,
            key_id,
        })
    }
}

impl RemotePluginArchiveDownload {
    pub fn new(bundle_download_url: Option<String>) -> Self {
        Self {
            bundle_download_url,
            signing: ArchiveVerification::Absent,
        }
    }

    /// Checks the downloaded bytes before they can be extracted. The key request must
    /// use the authorized Plugin Service route and return its resolved authentication headers.
    pub async fn verify<Fut>(
        &self,
        archive_bytes: Vec<u8>,
        service_base_url: &str,
        signing_key_cache: &SigningKeyCache,
        key_request: impl FnOnce(String) -> Fut,
    ) -> Result<(Vec<u8>, PluginArchiveVerification), ArchiveVerificationFailure>
    where
        Fut: Future<Output = Result<(RequestBuilder, HeaderMap), ArchiveVerificationFailure>>,
    {
        let started = Instant::now();
        // The authenticated key list determines the kind. Digest and key lookup failures precede it.
        let mut archive_kind = "unknown";
        let result = async {
            let signing = match &self.signing {
                ArchiveVerification::Absent => {
                    return Ok((archive_bytes, PluginArchiveVerification::NotRequired));
                }
                ArchiveVerification::Present(signing) => signing,
                ArchiveVerification::Malformed => {
                    return Err(ArchiveVerificationFailure::MalformedMetadata);
                }
            };
            let signature = DetachedSignature::try_from(signing)?;
            let (archive_bytes, archive_digest) = tokio::task::spawn_blocking(move || {
                let archive_digest = digest(&SHA256, &archive_bytes);
                (archive_bytes, archive_digest)
            })
            .await
            .map_err(|_| ArchiveVerificationFailure::DigestComputationFailed)?;
            if archive_digest.as_ref() != signature.expected_digest {
                return Err(ArchiveVerificationFailure::DigestMismatch);
            }
            let key = signing_key_cache
                .key(service_base_url, signature.key_id, key_request)
                .await?;
            archive_kind = key.archive_kind();
            if key.algorithm != ARCHIVE_SIGNING_ALGORITHM {
                return Err(ArchiveVerificationFailure::UnsupportedKeyAlgorithm);
            }
            let key_bytes = STANDARD
                .decode(&key.public_key_spki_der_base64)
                .map_err(|_| ArchiveVerificationFailure::InvalidPublicKey)?;
            let public_key = ParsedPublicKey::new(&RSA_PKCS1_2048_8192_SHA256, key_bytes)
                .map_err(|_| ArchiveVerificationFailure::InvalidPublicKey)?;
            public_key
                .verify_digest_sig(&archive_digest, &signature.bytes)
                .map_err(|_| ArchiveVerificationFailure::SignatureMismatch)?;
            Ok((archive_bytes, PluginArchiveVerification::Verified))
        }
        .await;
        let outcome = match &result {
            Ok((_, outcome)) => outcome.as_str(),
            Err(failure) => {
                let reason = failure.as_str();
                tracing::warn!(reason, "plugin archive verification failed");
                reason
            }
        };
        if let Some(metrics) = codex_otel::global() {
            let tags = [("outcome", outcome), ("archive_kind", archive_kind)];
            let _ = metrics.counter(ARCHIVE_VERIFICATION_EVENT, /*inc*/ 1, &tags);
            let _ = metrics.record_duration(
                "codex.plugins.archive_verification.duration_ms",
                started.elapsed(),
                &tags,
            );
        }
        result
    }
}

#[cfg(test)]
#[path = "archive_verification_tests.rs"]
mod tests;
