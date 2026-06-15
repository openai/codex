use codex_app_server_protocol::JSONRPCErrorError;
use codex_exec_server::ExecServerError;

const INVALID_PARAMS_ERROR_CODE: i64 = -32602;
const INTERNAL_ERROR_CODE: i64 = -32603;

pub(crate) fn invalid_params(message: impl Into<String>) -> JSONRPCErrorError {
    error(INVALID_PARAMS_ERROR_CODE, message)
}

pub(crate) fn internal_error(message: impl Into<String>) -> JSONRPCErrorError {
    error(INTERNAL_ERROR_CODE, message)
}

pub(crate) fn exec_server_error_to_jsonrpc(err: ExecServerError) -> JSONRPCErrorError {
    match err {
        ExecServerError::Server { code, message } => JSONRPCErrorError {
            code,
            message,
            data: None,
        },
        _ => internal_error(err.to_string()),
    }
}

fn error(code: i64, message: impl Into<String>) -> JSONRPCErrorError {
    JSONRPCErrorError {
        code,
        message: message.into(),
        data: None,
    }
}
