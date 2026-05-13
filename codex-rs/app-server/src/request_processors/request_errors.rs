use super::*;

pub(super) fn environment_selection_error_message(err: CodexErr) -> String {
    match err {
        CodexErr::InvalidRequest(message) => message,
        err => err.to_string(),
    }
}
