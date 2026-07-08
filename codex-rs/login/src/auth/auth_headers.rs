use std::fmt;

use reqwest::header::HeaderMap;

/// Request headers returned by an external auth provider.
///
/// The provider owns credential validation, rotation, and persistence. Codex
/// keeps the resolved headers in memory and attaches them to backend requests.
#[derive(Clone, PartialEq, Eq)]
pub struct AuthHeaders {
    headers: HeaderMap,
}

impl AuthHeaders {
    pub fn new(headers: HeaderMap) -> Self {
        Self { headers }
    }

    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }
}

impl fmt::Debug for AuthHeaders {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthHeaders")
            .field("headers", &"<redacted>")
            .finish()
    }
}
