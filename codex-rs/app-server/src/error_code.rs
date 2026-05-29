use codex_app_server_protocol::JSONRPCErrorError;

pub(crate) const INVALID_REQUEST_ERROR_CODE: i64 = -32600;
pub(crate) const METHOD_NOT_FOUND_ERROR_CODE: i64 = -32601;
pub const INVALID_PARAMS_ERROR_CODE: i64 = -32602;
pub(crate) const INTERNAL_ERROR_CODE: i64 = -32603;
pub(crate) const OVERLOADED_ERROR_CODE: i64 = -32001;
pub const INPUT_TOO_LARGE_ERROR_CODE: &str = "input_too_large";

pub(crate) fn invalid_request(message: impl Into<String>) -> JSONRPCErrorError {
    error(INVALID_REQUEST_ERROR_CODE, message)
}

pub(crate) fn method_not_found(message: impl Into<String>) -> JSONRPCErrorError {
    error(METHOD_NOT_FOUND_ERROR_CODE, message)
}

pub(crate) fn invalid_params(message: impl Into<String>) -> JSONRPCErrorError {
    error(INVALID_PARAMS_ERROR_CODE, message)
}

pub(crate) fn internal_error(message: impl Into<String>) -> JSONRPCErrorError {
    error(INTERNAL_ERROR_CODE, message)
}

fn error(code: i64, message: impl Into<String>) -> JSONRPCErrorError {
    JSONRPCErrorError {
        code,
        message: message.into(),
        data: None,
    }
}
