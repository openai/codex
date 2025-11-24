use crate::enums::{ContentType, HandshakeType};
use crate::error::Error;
use crate::log::warn;
use crate::msgs::message::MessagePayload;

/// For a Message $m, and a HandshakePayload enum member $payload_type,
/// return Ok(payload) if $m is both a handshake message and one that
/// has the given $payload_type.  If not, return Err(rustls::Error) quoting
/// $handshake_type as the expected handshake type.
macro_rules! require_handshake_msg(
  ( $m:expr, $handshake_type:path, $payload_type:path ) => (
    match &$m.payload {
        MessagePayload::Handshake { parsed: $crate::msgs::handshake::HandshakeMessagePayload(
            $payload_type(hm),
        ), .. } => Ok(hm),
        payload => Err($crate::check::inappropriate_handshake_message(
            payload,
            &[$crate::ContentType::Handshake],
            &[$handshake_type]))
    }
  )
);

/// Like require_handshake_msg, but moves the payload out of $m.
macro_rules! require_handshake_msg_move(
  ( $m:expr, $handshake_type:path, $payload_type:path ) => (
    match $m.payload {
        MessagePayload::Handshake { parsed: $crate::msgs::handshake::HandshakeMessagePayload(
            $payload_type(hm),
        ), .. } => Ok(hm),
        payload =>
            Err($crate::check::inappropriate_handshake_message(
                &payload,
                &[$crate::ContentType::Handshake],
                &[$handshake_type]))
    }
  )
);

pub(crate) fn inappropriate_message(
    payload: &MessagePayload<'_>,
    content_types: &[ContentType],
) -> Error {
    warn!(
        "Received a {:?} message while expecting {content_types:?}",
        payload.content_type(),
    );
    Error::InappropriateMessage {
        expect_types: content_types.to_vec(),
        got_type: payload.content_type(),
    }
}

pub(crate) fn inappropriate_handshake_message(
    payload: &MessagePayload<'_>,
    content_types: &[ContentType],
    handshake_types: &[HandshakeType],
) -> Error {
    match payload {
        MessagePayload::Handshake { parsed, .. } => {
            let got_type = parsed.0.handshake_type();
            warn!("Received a {got_type:?} handshake message while expecting {handshake_types:?}",);
            Error::InappropriateHandshakeMessage {
                expect_types: handshake_types.to_vec(),
                got_type,
            }
        }
        payload => inappropriate_message(payload, content_types),
    }
}
