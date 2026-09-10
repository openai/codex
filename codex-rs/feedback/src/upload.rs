use std::io;
use std::io::Write;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;

use anyhow::Context;
use anyhow::Result;
use bytes::Bytes;
use codex_http_client::RouteAwareClientPool;
use codex_http_client::RouteAwareRequestBuilder;
use flate2::Compression;
use flate2::write::GzEncoder;
use http::HeaderMap;
use http::StatusCode;
use sentry::ClientOptions;
use sentry::protocol::Attachment;
use sentry::protocol::Envelope;
use sentry::protocol::EnvelopeHeaders;
use sentry::protocol::EnvelopeItem;
use sentry::types::Dsn;

use crate::MAX_DECODED_UPLOAD_BYTES;

pub(super) const DEFAULT_RATE_LIMIT: Duration = Duration::from_secs(/*secs*/ 60);

pub(super) enum EnvelopeKind {
    Event,
    Attachment,
}

pub(super) fn envelope_request(
    client_pool: &RouteAwareClientPool,
    dsn: &Dsn,
    body: Bytes,
    timeout: Duration,
) -> RouteAwareRequestBuilder {
    let sentry_options = ClientOptions::default();
    let sentry_auth = dsn.to_auth(Some(sentry_options.user_agent.as_ref()));
    let request = client_pool
        .post(dsn.envelope_api_url().as_str())
        .header("X-Sentry-Auth", sentry_auth.to_string());
    // Persisted parts retain their encoding. Gzip's magic bytes cannot begin
    // an uncompressed envelope, which starts with a JSON header.
    let request = if body.starts_with(&[0x1f, 0x8b]) {
        request.header("Content-Encoding", "gzip")
    } else {
        request
    };
    request.body(body).timeout(timeout)
}

/// Send an already-serialized envelope within the report's shared deadline.
pub(super) async fn send_envelope(
    client_pool: &RouteAwareClientPool,
    dsn: &Dsn,
    body: Vec<u8>,
    kind: EnvelopeKind,
    deadline: Instant,
    rate_limited: &mut bool,
) -> Result<StatusCode> {
    let body = Bytes::from(body);
    // Retain the exact request bytes for at most two diagnostic retries. Never replay the core.
    let mut retry_delays = [250, 500].map(Duration::from_millis).into_iter();
    let response = loop {
        let timeout = deadline.saturating_duration_since(Instant::now());
        anyhow::ensure!(!timeout.is_zero(), "feedback upload deadline exceeded");
        let response = envelope_request(client_pool, dsn, body.clone(), timeout)
            .send()
            .await;
        if response.as_ref().is_ok_and(|response| {
            sentry_rate_limit_delay(response.headers()).is_some_and(|delay| !delay.is_zero())
                || (response.status().is_success()
                    && response.headers().get("Retry-After").is_some_and(|value| {
                        parse_retry_after(value.to_str().unwrap_or_default())
                            .is_none_or(|delay| !delay.is_zero())
                    }))
        }) {
            // Even accepted requests can impose a quota on events or attachments.
            // https://develop.sentry.dev/sdk/foundations/transport/rate-limiting/
            *rate_limited = true;
            break response;
        }
        let retryable = match &response {
            Ok(response) => matches!(response.status().as_u16(), 408 | 429 | 500..=599),
            Err(error) => {
                error.is_timeout()
                    || (error.failure_class().is_none()
                        && (error.is_connect() || error.is_request() || error.is_body()))
            }
        };
        if matches!(kind, EnvelopeKind::Event) || !retryable {
            break response;
        }
        let retry_delay = retry_delays.next();
        if response.as_ref().is_ok_and(|response| {
            response.status() == StatusCode::TOO_MANY_REQUESTS
                && (retry_delay.is_none() || !response.headers().contains_key("Retry-After"))
        }) {
            // Never let later files bypass an exhausted rate limit.
            *rate_limited = true;
            break response;
        }
        let Some(retry_delay) = retry_delay else {
            break response;
        };
        let retry_after = response
            .as_ref()
            .ok()
            .and_then(|response| response.headers().get("Retry-After"))
            .map(|value| {
                // An invalid cooldown is not permission to send immediately.
                parse_retry_after(value.to_str().unwrap_or_default())
                    .unwrap_or_else(|| deadline.saturating_duration_since(Instant::now()))
            });
        let delay = retry_delay.max(retry_after.unwrap_or_default());
        if delay >= deadline.saturating_duration_since(Instant::now()) {
            // Leave long cooldowns for a later submission, including its remaining files.
            *rate_limited = true;
            break response;
        }
        tokio::time::sleep(delay).await;
    };
    response
        .map(|response| response.status())
        .context("failed to upload feedback to Sentry")
}

pub(super) fn sentry_rate_limit_delay(headers: &HeaderMap) -> Option<Duration> {
    let mut retry_after = None;
    for value in headers.get_all("X-Sentry-Rate-Limits") {
        for quota in value.to_str().unwrap_or_default().split(',') {
            let mut fields = quota.trim().split(':');
            let seconds = fields.next().unwrap_or_default();
            let categories = fields.next().unwrap_or_default();
            if !categories.is_empty()
                && !categories
                    .split(';')
                    .any(|category| matches!(category, "error" | "attachment"))
            {
                continue;
            }
            let delay = seconds
                .parse::<f64>()
                .ok()
                .and_then(|seconds| Duration::try_from_secs_f64(seconds.ceil()).ok())
                .unwrap_or(DEFAULT_RATE_LIMIT);
            retry_after = retry_after.max(Some(delay));
        }
    }
    retry_after
}

pub(super) fn parse_retry_after(value: &str) -> Option<Duration> {
    value
        .parse::<u64>()
        .map(Duration::from_secs)
        .or_else(|_| {
            httpdate::parse_http_date(value)
                .map(|date| date.duration_since(SystemTime::now()).unwrap_or_default())
        })
        .ok()
}

pub(super) fn encode_envelope(envelope: &Envelope) -> io::Result<(Vec<u8>, usize)> {
    let mut writer = CountingWriter {
        inner: GzEncoder::new(Vec::new(), Compression::fast()),
        bytes: 0,
    };
    envelope.to_writer(&mut writer)?;
    let decoded_bytes = writer.bytes;
    let mut body = writer.inner.finish()?;
    // Already-compressed attachments can grow beyond Sentry's 200 MiB wire limit.
    // Reuse the buffer for the raw envelope when gzip does not make it smaller.
    if body.len() >= decoded_bytes {
        body.clear();
        envelope.to_writer(&mut body)?;
    }
    Ok((body, decoded_bytes))
}

pub(super) fn encode_attachment_envelope(
    headers: &EnvelopeHeaders,
    attachment: Attachment,
) -> Result<Vec<u8>> {
    anyhow::ensure!(
        attachment.buffer.len() <= MAX_DECODED_UPLOAD_BYTES,
        "feedback attachment exceeds the size limit"
    );
    let mut envelope = Envelope::new().with_headers(headers.clone());
    envelope.add_item(EnvelopeItem::Attachment(attachment));
    let (body, decoded_bytes) = encode_envelope(&envelope)?;
    anyhow::ensure!(
        decoded_bytes <= MAX_DECODED_UPLOAD_BYTES,
        "feedback attachment envelope exceeds the size limit"
    );
    Ok(body)
}

struct CountingWriter<W> {
    inner: W,
    bytes: usize,
}

impl<W: Write> Write for CountingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(buf)?;
        self.bytes += written;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
