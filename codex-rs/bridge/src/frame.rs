use serde::Deserialize;
use serde::Serialize;

/// A raw opaque byte frame associated with one field in a typed bridge message.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpaqueFrame {
    /// Field path inside the typed request or response.
    pub field: String,
    /// Stable codec label used by Rust to encode/decode the bytes.
    pub codec: String,
    /// MIME-ish content type for diagnostics and non-Rust storage services.
    pub content_type: String,
    /// Raw bytes that Python should not deserialize as part of normal persistence.
    pub bytes: Vec<u8>,
}

/// A typed bridge request plus any opaque byte frames referenced by that request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BridgeRequest<T> {
    /// Request body encoded in MsgPack inside the envelope.
    pub body: T,
    /// Opaque frames associated with body fields.
    pub opaque_frames: Vec<OpaqueFrame>,
}

impl<T> BridgeRequest<T> {
    /// Create a request without opaque frames.
    pub fn new(body: T) -> Self {
        Self {
            body,
            opaque_frames: Vec::new(),
        }
    }

    /// Create a request with opaque frames.
    pub fn with_opaque_frames(body: T, opaque_frames: Vec<OpaqueFrame>) -> Self {
        Self {
            body,
            opaque_frames,
        }
    }
}

/// A complete bridge frame as it is written to the transport.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeFrame {
    /// Request id chosen by the client.
    pub request_id: u64,
    /// Stable method name.
    pub method: String,
    /// MsgPack-encoded typed body.
    pub body: Vec<u8>,
    /// Opaque byte frames for large fields.
    pub opaque_frames: Vec<OpaqueFrame>,
}

/// Response envelope returned by the bridge service.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeEnvelope {
    /// Request id this response corresponds to.
    pub request_id: u64,
    /// Stable method name.
    pub method: String,
    /// MsgPack-encoded typed success body, if the call succeeded.
    pub body: Option<Vec<u8>>,
    /// Opaque byte frames for large response fields.
    pub opaque_frames: Vec<OpaqueFrame>,
    /// Remote error code, if the call failed.
    pub error_code: Option<String>,
    /// Remote error message, if the call failed.
    pub error_message: Option<String>,
}
