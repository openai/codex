#[derive(Debug, Clone)]
pub(crate) enum StatusAccountDisplay {
    ChatGpt {
        email: Option<String>,
// tui/src/status/account.rs
        plan: Option<String>,
    },
    ApiKey,
}
