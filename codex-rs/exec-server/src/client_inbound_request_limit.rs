use codex_exec_server_protocol::EXEC_CLOSED_METHOD;
use codex_exec_server_protocol::EXEC_EXITED_METHOD;
use codex_exec_server_protocol::EXEC_OUTPUT_DELTA_METHOD;
use codex_exec_server_protocol::HTTP_REQUEST_BODY_DELTA_METHOD;
use codex_exec_server_protocol::JSONRPCMessage;
use codex_exec_server_protocol::MAX_HTTP_BODY_DELTA_BYTES;

// A transport may materialize one larger frame before its JSON-RPC kind is known.
pub(crate) const MAX_CLIENT_INBOUND_REQUEST_LEN: usize = 8 * 1024;
// Streamed HTTP bodies carry up to 1 MiB before base64 and JSON-RPC framing.
pub(crate) const MAX_CLIENT_INBOUND_NOTIFICATION_LEN: usize = 2 * MAX_HTTP_BODY_DELTA_BYTES;

pub(crate) fn client_inbound_message_exceeded_limit(
    message: Result<&JSONRPCMessage, &serde_json::Error>,
    encoded_len: usize,
    max_request_len: usize,
) -> Option<usize> {
    let max_len = match message {
        Ok(JSONRPCMessage::Notification(notification))
            if matches!(
                notification.method.as_str(),
                EXEC_OUTPUT_DELTA_METHOD
                    | EXEC_EXITED_METHOD
                    | EXEC_CLOSED_METHOD
                    | HTTP_REQUEST_BODY_DELTA_METHOD
            ) =>
        {
            MAX_CLIENT_INBOUND_NOTIFICATION_LEN
        }
        Ok(JSONRPCMessage::Request(_)) | Ok(JSONRPCMessage::Notification(_)) | Err(_) => {
            max_request_len
        }
        Ok(JSONRPCMessage::Response(_)) | Ok(JSONRPCMessage::Error(_)) => return None,
    };
    (encoded_len > max_len).then_some(max_len)
}

#[cfg(test)]
#[path = "client_inbound_request_limit_tests.rs"]
mod tests;
