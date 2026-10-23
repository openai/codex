//! Resolves ChatGPT backend defaults from the credentials used for a request.
//! Explicit endpoints remain authoritative across credential changes.

use codex_login::CodexAuth;

pub const DEFAULT_CHATGPT_BASE_URL: &str = "https://chatgpt.com/backend-api/";
const FEDRAMP_CHATGPT_BASE_URL: &str = "https://gov.chatgpt.com/backend-api/";

/// A backend endpoint that retains whether configuration supplied an override.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ChatgptBaseUrl {
    #[default]
    Default,
    Configured(String),
}

impl ChatgptBaseUrl {
    /// Resolve with the same captured credentials that will authenticate the request.
    pub fn resolve(&self, auth: &CodexAuth) -> &str {
        match self {
            Self::Configured(url) => url,
            Self::Default if auth.is_api_key_auth() && auth.is_fedramp_account() => {
                FEDRAMP_CHATGPT_BASE_URL
            }
            Self::Default => DEFAULT_CHATGPT_BASE_URL,
        }
    }
}

impl From<String> for ChatgptBaseUrl {
    fn from(url: String) -> Self {
        Self::Configured(url)
    }
}

impl From<&str> for ChatgptBaseUrl {
    fn from(url: &str) -> Self {
        Self::Configured(url.to_owned())
    }
}
