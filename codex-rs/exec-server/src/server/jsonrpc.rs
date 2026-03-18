use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use serde_json::Value;

pub(crate) fn invalid_request(message: String) -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: -32600,
        data: None,
        message,
    }
}

pub(crate) fn invalid_params(message: String) -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: -32602,
        data: None,
        message,
    }
}

pub(crate) fn method_not_found(message: String) -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: -32601,
        data: None,
        message,
    }
}

pub(crate) fn response_message(
    request_id: RequestId,
    result: Result<Value, JSONRPCErrorError>,
) -> JSONRPCMessage {
    match result {
        Ok(result) => JSONRPCMessage::Response(JSONRPCResponse {
            id: request_id,
            result,
        }),
        Err(error) => JSONRPCMessage::Error(JSONRPCError {
            id: request_id,
            error,
        }),
    }
}
