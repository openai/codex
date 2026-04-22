use std::borrow::Cow;

use reqwest::header::HeaderMap;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use reqwest::header::WWW_AUTHENTICATE;
use rmcp::transport::streamable_http_client::AuthRequiredError;
use rmcp::transport::streamable_http_client::StreamableHttpError;

pub(crate) const EVENT_STREAM_MIME_TYPE: &str = "text/event-stream";
pub(crate) const JSON_MIME_TYPE: &str = "application/json";
pub(crate) const HEADER_LAST_EVENT_ID: &str = "Last-Event-Id";
pub(crate) const HEADER_SESSION_ID: &str = "Mcp-Session-Id";
const NON_JSON_RESPONSE_BODY_PREVIEW_BYTES: usize = 8_192;

pub(crate) fn body_preview(body: impl Into<String>) -> String {
    let mut body_preview = body.into();
    let body_len = body_preview.len();
    if body_len > NON_JSON_RESPONSE_BODY_PREVIEW_BYTES {
        let mut boundary = NON_JSON_RESPONSE_BODY_PREVIEW_BYTES;
        while !body_preview.is_char_boundary(boundary) {
            boundary = boundary.saturating_sub(1);
        }
        body_preview.truncate(boundary);
        body_preview.push_str(&format!(
            "... (truncated {} bytes)",
            body_len.saturating_sub(boundary)
        ));
    }
    body_preview
}

pub(crate) fn is_streamable_http_content_type(content_type: &str) -> bool {
    content_type
        .as_bytes()
        .starts_with(EVENT_STREAM_MIME_TYPE.as_bytes())
        || content_type
            .as_bytes()
            .starts_with(JSON_MIME_TYPE.as_bytes())
}

pub(crate) fn www_authenticate_error<Error>(
    headers: &HeaderMap,
) -> std::result::Result<Option<AuthRequiredError>, StreamableHttpError<Error>>
where
    Error: std::error::Error + Send + Sync + 'static,
{
    let Some(header) = headers.get(WWW_AUTHENTICATE) else {
        return Ok(None);
    };
    let header = header
        .to_str()
        .map_err(|_| {
            StreamableHttpError::UnexpectedServerResponse(Cow::Borrowed(
                "invalid www-authenticate header value",
            ))
        })?
        .to_string();
    Ok(Some(AuthRequiredError {
        www_authenticate_header: header,
    }))
}

pub(crate) fn insert_header<Error>(
    headers: &mut HeaderMap,
    name: HeaderName,
    value: String,
    map_error: impl FnOnce(String) -> Error,
) -> std::result::Result<(), StreamableHttpError<Error>>
where
    Error: std::error::Error + Send + Sync + 'static,
{
    let value = HeaderValue::from_str(&value)
        .map_err(|error| StreamableHttpError::Client(map_error(error.to_string())))?;
    headers.insert(name, value);
    Ok(())
}
