//! Fetches and validates the signing keys returned by the authenticated Plugin Service endpoint.

use crate::ArchiveVerificationFailure;
use codex_http_client::RequestBuilder;
use http::HeaderMap;
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::time::Instant;

const MARKETPLACE_ARCHIVE_PURPOSE: &str = "PLUGIN_RELEASE_ARCHIVE_SIGNING";
const MAX_SIGNING_KEYS: usize = 64;
const MAX_SIGNING_KEYS_RESPONSE_BYTES: usize = 128 * 1024;
const MAX_PUBLIC_KEY_BASE64_BYTES: usize = 16 * 1024;

pub(super) async fn signing_key<Fut>(
    service_base_url: &str,
    key_id: &str,
    key_request: impl FnOnce(String) -> Fut,
) -> Result<SigningKey, ArchiveVerificationFailure>
where
    Fut: Future<Output = Result<(RequestBuilder, HeaderMap), ArchiveVerificationFailure>>,
{
    let endpoint = service_base_url.trim_end_matches('/');
    let key_url = format!("{endpoint}/ps/plugins/signing-keys");
    let (request, headers) = key_request(key_url.clone()).await?;
    let keys = fetch_signing_keys(request.headers(headers), &key_url).await?;
    keys.get(key_id)
        .cloned()
        .unwrap_or(Err(ArchiveVerificationFailure::UnknownSigningKey))
}

#[derive(Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct SigningKey {
    key_id: String,
    purpose: String,
    pub(crate) algorithm: String,
    pub(crate) public_key_spki_der_base64: String,
}

impl SigningKey {
    pub(crate) fn archive_kind(&self) -> &'static str {
        match self.purpose.as_str() {
            MARKETPLACE_ARCHIVE_PURPOSE => "release",
            _ => "unknown",
        }
    }
}

impl fmt::Debug for SigningKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SigningKey { material: redacted }")
    }
}

#[derive(Deserialize)]
struct SigningKeysResponse {
    keys: Vec<SigningKey>,
    #[serde(default)]
    eval_keys: Vec<SigningKey>,
    #[serde(rename = "cache_ttl_seconds")]
    _cache_ttl_seconds: u64,
}

type SigningKeys = HashMap<String, Result<SigningKey, ArchiveVerificationFailure>>;

async fn fetch_signing_keys(
    request: RequestBuilder,
    key_url: &str,
) -> Result<SigningKeys, ArchiveVerificationFailure> {
    let started = Instant::now();
    let result = async {
        let mut response = request
            .send()
            .await
            .map_err(|_| ArchiveVerificationFailure::SigningKeysUnavailable)?;
        // The HTTP client can follow a redirect after dropping the original credentials.
        let expected_url = key_url
            .parse()
            .map_err(|_| ArchiveVerificationFailure::SigningKeysUnavailable)?;
        if !response.status().is_success() || response.url() != &expected_url {
            return Err(ArchiveVerificationFailure::SigningKeysUnavailable);
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_SIGNING_KEYS_RESPONSE_BYTES as u64)
        {
            return Err(ArchiveVerificationFailure::InvalidSigningKeysResponse);
        }
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| ArchiveVerificationFailure::SigningKeysUnavailable)?
        {
            if body.len().saturating_add(chunk.len()) > MAX_SIGNING_KEYS_RESPONSE_BYTES {
                return Err(ArchiveVerificationFailure::InvalidSigningKeysResponse);
            }
            body.extend_from_slice(&chunk);
        }
        let response: SigningKeysResponse = serde_json::from_slice(&body)
            .map_err(|_| ArchiveVerificationFailure::InvalidSigningKeysResponse)?;
        let key_count = response.keys.len() + response.eval_keys.len();
        if key_count == 0 || key_count > MAX_SIGNING_KEYS {
            return Err(ArchiveVerificationFailure::InvalidSigningKeysResponse);
        }
        let mut keys = HashMap::with_capacity(key_count);
        // Older servers published eval keys to all callers in `keys`.
        for key in response.keys {
            if key.key_id.is_empty()
                || key.public_key_spki_der_base64.len() > MAX_PUBLIC_KEY_BASE64_BYTES
                || keys.contains_key(&key.key_id)
            {
                return Err(ArchiveVerificationFailure::InvalidSigningKeysResponse);
            }
            keys.insert(
                key.key_id.clone(),
                (key.purpose == MARKETPLACE_ARCHIVE_PURPOSE)
                    .then_some(key)
                    .ok_or(ArchiveVerificationFailure::UnsupportedKeyPurpose),
            );
        }
        Ok(keys)
    }
    .await;
    if let Some(metrics) = codex_otel::global() {
        let outcome = match &result {
            Ok(_) => "success",
            Err(failure) => failure.as_str(),
        };
        let tags = [("outcome", outcome)];
        let _ = metrics.counter(
            "codex.plugins.archive_signing_keys.fetch",
            /*inc*/ 1,
            &tags,
        );
        let _ = metrics.record_duration(
            "codex.plugins.archive_signing_keys.fetch.duration_ms",
            started.elapsed(),
            &tags,
        );
    }
    result
}
