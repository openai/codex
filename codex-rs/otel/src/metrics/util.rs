use tracing::error;

pub(crate) fn error_or_panic(message: impl ToString) {
    if cfg!(debug_assertions) {
        panic!("{}", message.to_string());
    } else {
        error!("{}", message.to_string());
    }
}
