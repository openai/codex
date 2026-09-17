//! Redacts credentials in OAuth transport URLs and issuer diagnostics.

const REDACTED_URL_VALUE: &str = "<redacted>";
const SENSITIVE_URL_QUERY_KEYS: &[&str] = &[
    "access_token",
    "api_key",
    "client_secret",
    "code",
    "code_verifier",
    "id_token",
    "key",
    "refresh_token",
    "requested_token",
    "state",
    "subject_token",
    "token",
];

fn redact_sensitive_query_value(key: &str, value: &str) -> String {
    if SENSITIVE_URL_QUERY_KEYS
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(key))
    {
        REDACTED_URL_VALUE.to_string()
    } else {
        value.to_string()
    }
}

/// Redacts URL components that commonly carry auth secrets while preserving the host/path shape.
///
/// This keeps developer-facing logs useful for debugging transport failures without persisting
/// tokens, callback codes, fragments, or embedded credentials.
fn redact_sensitive_url_parts(url: &mut url::Url) {
    let _ = url.set_username("");
    let _ = url.set_password(None);
    url.set_fragment(None);

    let query_pairs = url
        .query_pairs()
        .map(|(key, value)| {
            let key = key.into_owned();
            let value = value.into_owned();
            (key.clone(), redact_sensitive_query_value(&key, &value))
        })
        .collect::<Vec<_>>();

    if query_pairs.is_empty() {
        url.set_query(None);
        return;
    }

    let redacted_query = query_pairs
        .into_iter()
        .fold(
            url::form_urlencoded::Serializer::new(String::new()),
            |mut serializer, (key, value)| {
                serializer.append_pair(&key, &value);
                serializer
            },
        )
        .finish();
    url.set_query(Some(&redacted_query));
}

/// Redacts any URL attached to an HTTP transport error before it is logged or returned.
pub(crate) fn redact_error_url(
    mut err: codex_http_client::HttpError,
) -> codex_http_client::HttpError {
    if let Some(url) = err.url_mut() {
        redact_sensitive_url_parts(url);
    }
    err
}

/// Sanitizes a free-form URL string for structured logging.
///
/// This is used for caller-supplied issuer values, which may contain credentials or query
/// parameters on non-default deployments.
pub(crate) fn sanitize_url_for_logging(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(mut url) => {
            redact_sensitive_url_parts(&mut url);
            url.to_string()
        }
        Err(_) => "<invalid-url>".to_string(),
    }
}

pub(crate) fn redact_request_secrets(text: &str, secrets: &[&str]) -> String {
    let mut redacted = text.to_string();
    for secret in secrets.iter().copied().filter(|secret| !secret.is_empty()) {
        let form_encoded: String =
            url::form_urlencoded::byte_serialize(secret.as_bytes()).collect();
        redacted = redacted.replace(&form_encoded, "[REDACTED]");
        if let Ok(encoded) = serde_json::to_string(secret) {
            redacted = redacted.replace(&encoded[1..encoded.len() - 1], "[REDACTED]");
        }
        redacted = redacted.replace(secret, "[REDACTED]");
    }
    redacted
}

#[cfg(test)]
#[path = "diagnostics_tests.rs"]
mod tests;
