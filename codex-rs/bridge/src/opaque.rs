use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::BridgeError;
use crate::BridgeResult;
use crate::OpaqueFrame;

const MSGPACK_CONTENT_TYPE: &str = "application/vnd.codex.msgpack";

/// Encode a typed Rust value into an opaque MsgPack frame for Python to treat as bytes.
pub fn encode_opaque_msgpack<T>(field: &str, codec: &str, value: &T) -> BridgeResult<OpaqueFrame>
where
    T: Serialize,
{
    let bytes = rmp_serde::to_vec_named(value).map_err(|err| BridgeError::Codec {
        message: format!("failed to encode opaque `{field}` payload: {err}"),
    })?;
    Ok(OpaqueFrame {
        field: field.to_string(),
        codec: codec.to_string(),
        content_type: MSGPACK_CONTENT_TYPE.to_string(),
        bytes,
    })
}

/// Decode a typed Rust value from an opaque MsgPack frame returned by Python.
pub fn decode_opaque_msgpack<T>(frames: &[OpaqueFrame], field: &str, codec: &str) -> BridgeResult<T>
where
    T: DeserializeOwned,
{
    let Some(frame) = frames.iter().find(|frame| frame.field == field) else {
        return Err(BridgeError::InvalidResponse {
            message: format!("bridge response did not include `{field}` opaque frame"),
        });
    };
    if frame.codec != codec {
        return Err(BridgeError::InvalidResponse {
            message: format!(
                "bridge response used unsupported `{field}` codec `{}`; expected `{codec}`",
                frame.codec
            ),
        });
    }
    rmp_serde::from_slice(frame.bytes.as_slice()).map_err(|err| BridgeError::Codec {
        message: format!("failed to decode opaque `{field}` payload: {err}"),
    })
}
