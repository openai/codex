//! Account details presented in the status panel.

#[derive(Debug, Clone)]
pub(crate) enum StatusAccountDisplay {
    /// ChatGPT account details sourced from auth.
    ChatGpt {
        /// Email address shown in the status panel when available.
        email: Option<String>,
        /// Plan name shown in the status panel when available.
        plan: Option<String>,
    },
    /// API key auth; no account metadata is shown.
    ApiKey,
}
