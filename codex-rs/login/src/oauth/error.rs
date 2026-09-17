//! Parses OAuth rejections and sanitizes diagnostic text without deciding credential recovery.

use std::fmt;

use codex_http_client::HttpError;
use codex_http_client::HttpResponse;
use http::StatusCode;
use serde_json::Value;

use crate::oauth::diagnostics::redact_error_url;
use crate::oauth::diagnostics::redact_request_secrets;

#[derive(thiserror::Error)]
pub(crate) enum OAuthError {
    #[error("OAuth token request failed: {0}")]
    Transport(#[source] HttpError),
    // Decoder errors can contain token values, including through their source chain.
    #[error("OAuth token response is invalid")]
    InvalidResponse,
    #[error("{0}")]
    Rejected(Box<TokenRejection>),
}

impl fmt::Debug for OAuthError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(error) => formatter
                .debug_tuple("Transport")
                .field(&error.to_string())
                .finish(),
            // A JSON decoder's source error can include the offending token value.
            Self::InvalidResponse => formatter.write_str("InvalidResponse"),
            Self::Rejected(rejection) => {
                formatter.debug_tuple("Rejected").field(rejection).finish()
            }
        }
    }
}

/// Preserve each caller's existing diagnostic-read contract.
#[derive(Clone, Copy)]
pub(crate) enum ErrorBodyLimit {
    Unlimited,
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "Used by GatewayAuthManager in the following PR")
    )]
    Bytes(usize),
}

/// Sanitized token rejection. HTTP status survives even when its body cannot be read.
#[derive(Debug)]
pub(crate) struct TokenRejection {
    pub status: StatusCode,
    pub request_id: Option<String>,
    pub detail: TokenErrorDetail,
    pub body_read_error: Option<HttpError>,
}

impl TokenRejection {
    pub(crate) async fn from_response(
        mut response: HttpResponse,
        limit: ErrorBodyLimit,
        secrets: &[&str],
    ) -> Self {
        let status = response.status();
        let request_id = ["x-request-id", "x-openai-request-id", "cf-ray"]
            .iter()
            .find_map(|name| response.headers().get(*name))
            .and_then(|value| value.to_str().ok())
            .map(|value| {
                redact_request_secrets(value, secrets)
                    .chars()
                    .take(/*n*/ 128)
                    .collect()
            });
        let body = match limit {
            ErrorBodyLimit::Unlimited => response.text().await,
            ErrorBodyLimit::Bytes(limit) => {
                let mut body = Vec::new();
                loop {
                    match response.chunk().await {
                        Ok(Some(chunk)) if chunk.len() <= limit.saturating_sub(body.len()) => {
                            body.extend_from_slice(&chunk);
                        }
                        Ok(None) => break Ok(String::from_utf8_lossy(&body).into_owned()),
                        // Omit oversized bodies entirely so an echoed credential cannot be cut
                        // into a prefix that evades the caller's diagnostic redaction.
                        Ok(Some(_)) => break Ok(String::new()),
                        Err(error) => break Err(error),
                    }
                }
            }
        };
        let (detail, body_read_error) = match body {
            Ok(body) => (TokenErrorDetail::parse(&body, secrets), None),
            Err(error) => (
                TokenErrorDetail::parse("", secrets),
                Some(redact_error_url(error)),
            ),
        };
        Self {
            status,
            request_id,
            detail,
            body_read_error,
        }
    }
}

impl fmt::Display for TokenRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "token endpoint returned status {}: {}",
            self.status, self.detail
        )?;
        if let Some(request_id) = &self.request_id {
            write!(formatter, " (request id: {request_id})")?;
        }
        Ok(())
    }
}

/// Parsed standard OAuth fields, including the nested error envelope used by existing issuers.
pub(crate) struct TokenErrorDetail {
    error_code: Option<String>,
    diagnostic_code: Option<String>,
    error_message: Option<String>,
    display_message: String,
}

impl TokenErrorDetail {
    /// The issuer's unmodified code, for recovery classification rather than diagnostics.
    pub(crate) fn error_code(&self) -> Option<&str> {
        self.error_code.as_deref()
    }

    fn parse(body: &str, secrets: &[&str]) -> Self {
        let trimmed = body.trim();
        let parsed = serde_json::from_str::<Value>(trimmed).ok();
        let display_code = parsed.as_ref().and_then(|json| {
            nonempty_text(json.get("error"))
                .or_else(|| nonempty_text(json.get("error").and_then(|error| error.get("code"))))
        });
        // Top-level codes inform refresh recovery without replacing the legacy full-body display.
        let code = display_code.or_else(|| {
            parsed
                .as_ref()
                .and_then(|json| nonempty_text(json.get("code")))
        });
        let message = parsed.as_ref().and_then(|json| {
            nonempty_text(json.get("error_description"))
                .or_else(|| nonempty_text(json.get("error").and_then(|error| error.get("message"))))
        });
        let display = message.or(display_code).unwrap_or(if trimmed.is_empty() {
            "unknown error"
        } else {
            trimmed
        });
        // Redact plain, JSON-escaped, and form-encoded values before applying display limits.
        Self {
            error_code: code.map(str::to_string),
            diagnostic_code: code.map(|value| redact_request_secrets(value, secrets)),
            error_message: message.map(|value| redact_request_secrets(value, secrets)),
            display_message: redact_request_secrets(display, secrets),
        }
    }
}

fn nonempty_text(value: Option<&Value>) -> Option<&str> {
    value
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
}

impl fmt::Debug for TokenErrorDetail {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Plain-text fallback bodies remain available to the caller's UI, not structured logs.
        formatter
            .debug_struct("TokenErrorDetail")
            .field("error_code", &self.diagnostic_code)
            .field("error_message", &self.error_message)
            .finish_non_exhaustive()
    }
}

impl fmt::Display for TokenErrorDetail {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.display_message)
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
