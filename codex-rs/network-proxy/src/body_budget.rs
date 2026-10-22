//! Shared leases bound buffers across quota checks, secret substitution, and signing.
//! Eight requests each reserve space for a 16 MiB input and 16 MiB transformed output.

use anyhow::Result;
use anyhow::anyhow;
use rama_core::bytes::Bytes;
use rama_core::extensions::ExtensionsMut;
use rama_core::extensions::ExtensionsRef;
use rama_http::Body;
use rama_http::HeaderMap;
use rama_http::Request;
use rama_http::body::Frame;
use rama_http::body::SizeHint;
use rama_http::body::StreamingBody;
use rama_http::body::util::BodyExt;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Weak;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;

const BUFFER_SLOT_COUNT: usize = 8;
static BUFFER_SLOTS: LazyLock<Arc<Semaphore>> =
    LazyLock::new(|| Arc::new(Semaphore::new(BUFFER_SLOT_COUNT)));

// Rama clones request metadata through response headers. Keep ownership in the transform or body.
#[derive(Debug, Clone)]
struct BufferLease(Weak<OwnedSemaphorePermit>);

pub(crate) async fn reserve(req: &mut Request) -> Result<Arc<OwnedSemaphorePermit>> {
    reserve_in(req, &BUFFER_SLOTS).await
}

async fn reserve_in(
    req: &mut Request,
    slots: &Arc<Semaphore>,
) -> Result<Arc<OwnedSemaphorePermit>> {
    if let Some(permit) = req
        .extensions()
        .get::<BufferLease>()
        .and_then(|lease| lease.0.upgrade())
    {
        return Ok(permit);
    }
    let permit = tokio::time::timeout(Duration::from_secs(5), slots.clone().acquire_owned())
        .await
        .map_err(|_| anyhow!("request buffer capacity unavailable"))?
        .map_err(|_| anyhow!("request buffer budget closed"))?;
    let permit = Arc::new(permit);
    req.extensions_mut()
        .insert(BufferLease(Arc::downgrade(&permit)));
    Ok(permit)
}

pub(crate) fn is_amazon_event_stream(headers: &HeaderMap) -> bool {
    headers.get_all("content-type").iter().any(|value| {
        value.to_str().is_ok_and(|value| {
            value.split(';').next().is_some_and(|mime| {
                mime.trim()
                    .eq_ignore_ascii_case("application/vnd.amazon.eventstream")
            })
        })
    })
}

pub(crate) fn supported_transfer_framing(headers: &HeaderMap) -> bool {
    let codings = headers.get_all("transfer-encoding");
    let mut codings = codings.iter();
    match codings.next() {
        None => true,
        Some(coding) => {
            !headers.contains_key("content-length")
                && codings.next().is_none()
                && coding
                    .to_str()
                    .is_ok_and(|value| value.trim().eq_ignore_ascii_case("chunked"))
        }
    }
}

pub(crate) fn content_length_matches(headers: &HeaderMap, size: usize) -> bool {
    let lengths = headers.get_all("content-length");
    let mut lengths = lengths.iter();
    match lengths.next() {
        None => true,
        Some(length) => {
            lengths.next().is_none()
                && length.to_str().is_ok_and(|value| {
                    !value.is_empty()
                        && value.bytes().all(|byte| byte.is_ascii_digit())
                        && value.parse::<usize>() == Ok(size)
                })
        }
    }
}

/// Copy frames as they arrive so fragmented bodies cannot retain per-frame allocations.
pub(crate) async fn collect_body(
    mut body: Body,
    max_bytes: usize,
) -> Result<(Bytes, Option<HeaderMap>)> {
    let mut bytes = Vec::new();
    let mut trailers: Option<HeaderMap> = None;
    while let Some(frame) = body.frame().await {
        let frame = frame.map_err(|_| anyhow!("cannot read request body"))?;
        if trailers.is_some() {
            return Err(anyhow!("unexpected request frame after trailers"));
        }
        match frame.into_data() {
            Ok(data) => {
                if data.len() > max_bytes - bytes.len() {
                    return Err(anyhow!("request body exceeds configured limit"));
                }
                let len = bytes.len() + data.len();
                if len > bytes.capacity() {
                    // Grow geometrically without reserving beyond the configured byte limit.
                    let capacity = max_bytes.min(bytes.capacity().saturating_mul(2).max(len));
                    bytes
                        .try_reserve_exact(capacity - bytes.len())
                        .map_err(|_| anyhow!("request body buffer allocation failed"))?;
                }
                bytes.extend_from_slice(&data);
            }
            Err(frame) => trailers = frame.into_trailers().ok(),
        }
    }
    Ok((Bytes::from(bytes), trailers))
}

struct RetainedBody {
    body: Body,
    lease: Option<Arc<OwnedSemaphorePermit>>,
}

struct LeasedBytes {
    bytes: Bytes,
    _lease: Arc<OwnedSemaphorePermit>,
}

impl AsRef<[u8]> for LeasedBytes {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

impl StreamingBody for RetainedBody {
    type Data = Bytes;
    type Error = <Body as StreamingBody>::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        match Pin::new(&mut self.body).poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                let lease = self.lease.clone();
                Poll::Ready(Some(Ok(frame.map_data(|bytes| match lease {
                    Some(lease) => Bytes::from_owner(LeasedBytes {
                        bytes,
                        _lease: lease,
                    }),
                    None => bytes,
                }))))
            }
            other => other,
        }
    }

    fn is_end_stream(&self) -> bool {
        self.body.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.body.size_hint()
    }
}

pub(crate) fn retain(req: Request) -> Request {
    let (parts, body) = req.into_parts();
    let lease = parts
        .extensions
        .get::<BufferLease>()
        .and_then(|lease| lease.0.upgrade());
    // Keep exact sizing, and retain the lease in data frames queued after the body is dropped.
    let body = Body::new(RetainedBody { body, lease });
    Request::from_parts(parts, body)
}

#[cfg(test)]
#[path = "body_budget_tests.rs"]
mod tests;
