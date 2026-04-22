use std::io;

use bytes::Bytes;
use codex_exec_server::HttpHeader;
use codex_exec_server::HttpResponseBodyStream;
use futures::StreamExt;
use futures::stream;
use futures::stream::BoxStream;
use reqwest::StatusCode;
use rmcp::transport::streamable_http_client::StreamableHttpError;
use sse_stream::Sse;
use sse_stream::SseStream;

use super::HttpBackedStreamableHttpClientError;

pub(super) fn protocol_headers(headers: &reqwest::header::HeaderMap) -> Vec<HttpHeader> {
    headers
        .iter()
        .filter_map(|(name, value)| {
            Some(HttpHeader {
                name: name.as_str().to_string(),
                value: value.to_str().ok()?.to_string(),
            })
        })
        .collect()
}

pub(super) fn response_header(headers: &[HttpHeader], name: impl AsRef<str>) -> Option<String> {
    let name = name.as_ref();
    headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case(name))
        .map(|header| header.value.clone())
}

pub(super) fn status_is_success(status: u16) -> bool {
    StatusCode::from_u16(status).is_ok_and(|status| status.is_success())
}

pub(super) async fn collect_body(
    body_stream: &mut HttpResponseBodyStream,
) -> std::result::Result<Vec<u8>, StreamableHttpError<HttpBackedStreamableHttpClientError>> {
    let mut body = Vec::new();
    while let Some(chunk) = body_stream
        .recv()
        .await
        .map_err(HttpBackedStreamableHttpClientError::from)
        .map_err(StreamableHttpError::Client)?
    {
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

pub(super) fn sse_stream_from_body(
    body_stream: HttpResponseBodyStream,
) -> BoxStream<'static, std::result::Result<Sse, sse_stream::Error>> {
    SseStream::from_byte_stream(stream::unfold(body_stream, |mut body_stream| async move {
        match body_stream.recv().await {
            Ok(Some(bytes)) => Some((Ok(Bytes::from(bytes)), body_stream)),
            Ok(None) => None,
            Err(error) => Some((Err(io::Error::other(error)), body_stream)),
        }
    }))
    .boxed()
}
