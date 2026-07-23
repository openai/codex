use super::*;
use codex_protocol::error::CodexErrorDetails;

pub(super) fn environment_selection_error(err: CodexErr) -> JSONRPCErrorError {
    match err.details() {
        CodexErrorDetails::InvalidRequest(message) => invalid_request(message.clone()),
        _ => internal_error(format!("failed to validate environment selections: {err}")),
    }
}
