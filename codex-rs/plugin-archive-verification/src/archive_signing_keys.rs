//! Fetches authenticated Plugin Service signing keys. The bounded cache shares keys and in-flight
//! fetches only within the same endpoint and credentials. Each lookup fetches at most once.

use crate::ArchiveVerificationFailure;
use aws_lc_rs::digest::Context;
use aws_lc_rs::digest::SHA256;
use codex_http_client::RequestBuilder;
use http::HeaderMap;
use http::header::AUTHORIZATION;
use serde::Deserialize;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::Semaphore;

const MARKETPLACE_ARCHIVE_PURPOSE: &str = "PLUGIN_RELEASE_ARCHIVE_SIGNING";
const EVAL_ARCHIVE_PURPOSE: &str = "EVAL_PLUGIN_RELEASE_ARCHIVE_SIGNING";
const MAX_CACHED_SCOPES: usize = 8;
const MAX_SIGNING_KEYS: usize = 64;
const MAX_SIGNING_KEYS_RESPONSE_BYTES: usize = 128 * 1024;
const MAX_PUBLIC_KEY_BASE64_BYTES: usize = 16 * 1024;

type SigningKeyCacheScope = (String, aws_lc_rs::digest::Digest);

/// Reuses Plugin Service signing keys within one owner and credential scope.
#[derive(Default)]
pub struct SigningKeyCache {
    scopes: Mutex<VecDeque<(SigningKeyCacheScope, Arc<ScopedSigningKeyCache>)>>,
}

impl fmt::Debug for SigningKeyCache {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SigningKeyCache")
            .finish_non_exhaustive()
    }
}

impl SigningKeyCache {
    fn scope(&self, endpoint: &str, headers: &HeaderMap) -> Arc<ScopedSigningKeyCache> {
        if headers
            .get(AUTHORIZATION)
            .is_none_or(http::HeaderValue::is_empty)
        {
            // External providers can authenticate with custom headers. Always fetch in that case.
            return Arc::default();
        }
        // Bind both the cache entry and its single-flight fetch to the exact outgoing auth,
        // including account and provider-specific identity headers. A fresh runtime proof
        // must select a fresh scope so the key service authorizes that request.
        let mut fields: Vec<_> = headers.iter().collect();
        fields.sort_by_key(|(name, _)| name.as_str());
        let mut digest = Context::new(&SHA256);
        for (name, value) in fields {
            digest.update(name.as_str().as_bytes());
            digest.update(b":");
            digest.update(value.as_bytes());
            digest.update(b"\n");
        }
        let fingerprint = digest.finish();
        let mut scopes = self
            .scopes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let entry = scopes
            .iter()
            .position(|((url, hash), _)| url == endpoint && hash.as_ref() == fingerprint.as_ref())
            .and_then(|position| scopes.remove(position))
            .unwrap_or_else(|| ((endpoint.to_owned(), fingerprint), Arc::default()));
        let scope = Arc::clone(&entry.1);
        scopes.push_front(entry);
        scopes.truncate(MAX_CACHED_SCOPES);
        scope
    }

    pub(super) async fn key<Fut>(
        &self,
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
        self.scope(endpoint, &headers)
            .key(request.headers(headers), &key_url, key_id)
            .await
    }
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
            EVAL_ARCHIVE_PURPOSE => "eval",
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
    cache_ttl_seconds: u64,
}

struct CachedSigningKeys {
    keys: HashMap<String, Result<SigningKey, ArchiveVerificationFailure>>,
    expires_at: Instant,
}

impl CachedSigningKeys {
    fn fresh_key(&self, key_id: &str) -> Option<&Result<SigningKey, ArchiveVerificationFailure>> {
        if self.expires_at <= Instant::now() {
            return None;
        }
        self.keys.get(key_id)
    }

    fn key(&self, key_id: &str) -> Result<SigningKey, ArchiveVerificationFailure> {
        self.keys
            .get(key_id)
            .cloned()
            .unwrap_or(Err(ArchiveVerificationFailure::UnknownSigningKey))
    }
}

struct ScopedSigningKeyCache {
    state: Mutex<SigningKeyCacheState>,
    refresh: Semaphore,
}

#[derive(Default)]
struct SigningKeyCacheState {
    completed_fetches: u64,
    last_fetch: Option<Result<CachedSigningKeys, ArchiveVerificationFailure>>,
}

impl Default for ScopedSigningKeyCache {
    fn default() -> Self {
        Self {
            state: Mutex::default(),
            refresh: Semaphore::new(/*permits*/ 1),
        }
    }
}

impl ScopedSigningKeyCache {
    fn lock_state(&self) -> MutexGuard<'_, SigningKeyCacheState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    async fn key(
        &self,
        request: RequestBuilder,
        key_url: &str,
        key_id: &str,
    ) -> Result<SigningKey, ArchiveVerificationFailure> {
        let observed_fetches = {
            let state = self.lock_state();
            if let Some(Ok(keys)) = &state.last_fetch
                && let Some(key) = keys.fresh_key(key_id)
            {
                return key.clone();
            }
            state.completed_fetches
        };

        // Keep refreshes ordered without locking the cached data during the request.
        // Dropping the permit lets a waiter retry after a cancelled refresh.
        let _permit = self
            .refresh
            .acquire()
            .await
            .map_err(|_| ArchiveVerificationFailure::SigningKeysUnavailable)?;
        {
            let mut state = self.lock_state();
            // Lookups that overlapped a completed refresh share its result, even
            // if the response has already expired or the key is absent.
            if state.completed_fetches != observed_fetches
                && let Some(result) = &state.last_fetch
            {
                return result.as_ref().map_err(Clone::clone)?.key(key_id);
            }
            // A failed refresh can mean that this caller has lost access to the published keys.
            state.last_fetch = None;
        }
        let result = fetch_signing_keys(request, key_url).await;
        let key = result
            .as_ref()
            .map_err(Clone::clone)
            .and_then(|keys| keys.key(key_id));
        let mut state = self.lock_state();
        state.last_fetch = Some(result);
        state.completed_fetches = state.completed_fetches.wrapping_add(1);
        key
    }
}

async fn fetch_signing_keys(
    request: RequestBuilder,
    key_url: &str,
) -> Result<CachedSigningKeys, ArchiveVerificationFailure> {
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
        for (list, expected_purpose) in [
            // Older servers published eval keys to all callers in `keys`. Only `eval_keys` is scoped.
            (response.keys, MARKETPLACE_ARCHIVE_PURPOSE),
            (response.eval_keys, EVAL_ARCHIVE_PURPOSE),
        ] {
            for key in list {
                if key.key_id.is_empty()
                    || key.public_key_spki_der_base64.len() > MAX_PUBLIC_KEY_BASE64_BYTES
                    || keys.contains_key(&key.key_id)
                {
                    return Err(ArchiveVerificationFailure::InvalidSigningKeysResponse);
                }
                keys.insert(
                    key.key_id.clone(),
                    (key.purpose == expected_purpose)
                        .then_some(key)
                        .ok_or(ArchiveVerificationFailure::UnsupportedKeyPurpose),
                );
            }
        }
        let now = Instant::now();
        let expires_at = now
            .checked_add(Duration::from_secs(response.cache_ttl_seconds))
            .unwrap_or(now);
        Ok(CachedSigningKeys { keys, expires_at })
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

#[cfg(test)]
#[path = "archive_signing_keys_tests.rs"]
mod tests;
