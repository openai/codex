use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::BridgeEnvelope;
use crate::BridgeError;
use crate::BridgeFrame;
use crate::BridgeRequest;
use crate::BridgeResult;
use crate::BridgeTransport;
use crate::OpaqueFrame;

/// Typed response returned by [`BridgeClient`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BridgeResponse<T> {
    /// Decoded response body.
    pub body: T,
    /// Opaque response frames associated with body fields.
    pub opaque_frames: Vec<OpaqueFrame>,
}

/// Reusable MsgPack bridge client over a caller-provided transport.
pub struct BridgeClient<T> {
    transport: T,
    next_request_id: AtomicU64,
}

impl<T> BridgeClient<T> {
    /// Create a bridge client over the supplied transport.
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            next_request_id: AtomicU64::new(1),
        }
    }
}

impl<T> BridgeClient<T>
where
    T: BridgeTransport,
{
    /// Call a bridge method using MsgPack for the typed body and raw frames for opaque fields.
    pub async fn call<Req, Resp>(
        &self,
        method: &'static str,
        request: BridgeRequest<Req>,
    ) -> BridgeResult<BridgeResponse<Resp>>
    where
        Req: Serialize + Send + Sync,
        Resp: DeserializeOwned,
    {
        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let body = rmp_serde::to_vec_named(&request.body).map_err(|err| BridgeError::Codec {
            message: err.to_string(),
        })?;
        let frame = BridgeFrame {
            request_id,
            method: method.to_string(),
            body,
            opaque_frames: request.opaque_frames,
        };
        let response = self.transport.call(frame).await?;
        decode_response(request_id, method, response)
    }
}

fn decode_response<Resp>(
    request_id: u64,
    method: &'static str,
    response: BridgeEnvelope,
) -> BridgeResult<BridgeResponse<Resp>>
where
    Resp: DeserializeOwned,
{
    if response.request_id != request_id {
        return Err(BridgeError::InvalidResponse {
            message: format!(
                "expected request id {request_id}, received {}",
                response.request_id
            ),
        });
    }
    if response.method != method {
        return Err(BridgeError::InvalidResponse {
            message: format!("expected method `{method}`, received `{}`", response.method),
        });
    }
    if let Some(code) = response.error_code {
        return Err(BridgeError::Remote {
            method: method.to_string(),
            code,
            message: response.error_message.unwrap_or_default(),
        });
    }
    let Some(body) = response.body else {
        return Err(BridgeError::InvalidResponse {
            message: format!("method `{method}` returned no response body"),
        });
    };
    let body = rmp_serde::from_slice(body.as_slice()).map_err(|err| BridgeError::Codec {
        message: err.to_string(),
    })?;
    Ok(BridgeResponse {
        body,
        opaque_frames: response.opaque_frames,
    })
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use serde::Deserialize;
    use serde::Serialize;

    use super::*;

    #[derive(Clone)]
    struct EchoTransport;

    #[async_trait]
    impl BridgeTransport for EchoTransport {
        async fn call(&self, request: BridgeFrame) -> BridgeResult<BridgeEnvelope> {
            Ok(BridgeEnvelope {
                request_id: request.request_id,
                method: request.method,
                body: Some(request.body),
                opaque_frames: request.opaque_frames,
                error_code: None,
                error_message: None,
            })
        }
    }

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct Payload {
        value: String,
    }

    #[tokio::test]
    async fn call_round_trips_msgpack_body_and_opaque_frames() -> BridgeResult<()> {
        let client = BridgeClient::new(EchoTransport);
        let response: BridgeResponse<Payload> = client
            .call(
                "echo",
                BridgeRequest::with_opaque_frames(
                    Payload {
                        value: "hello".to_string(),
                    },
                    vec![OpaqueFrame {
                        field: "blob".to_string(),
                        codec: "raw".to_string(),
                        content_type: "application/octet-stream".to_string(),
                        bytes: b"world".to_vec(),
                    }],
                ),
            )
            .await?;

        assert_eq!(
            response.body,
            Payload {
                value: "hello".to_string()
            }
        );
        assert_eq!(response.opaque_frames[0].bytes, b"world");
        Ok(())
    }
}
