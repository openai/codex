//! Supplies sensitive request headers without copying the locked API key.

use std::fmt;

use anyhow::Result;
use reqwest::header::HeaderValue;

pub(crate) enum Auth {
    ApiKey(&'static str),
}

impl fmt::Debug for Auth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ApiKey(_) => f.debug_tuple("ApiKey").field(&"[REDACTED]").finish(),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct AuthHeader {
    pub(crate) value: HeaderValue,
}

impl Auth {
    pub(crate) fn redirect_policy(&self) -> reqwest::redirect::Policy {
        match self {
            Self::ApiKey(_) => reqwest::redirect::Policy::default(),
        }
    }

    /// Returns a sensitive authorization header. API keys are validated when read,
    /// so this currently always succeeds. The fallible return type allows later
    /// credential providers to report acquisition failures.
    pub(crate) fn header(&self) -> Result<AuthHeader> {
        match self {
            Self::ApiKey(value) => {
                // Keep the existing locked, static API key allocation.
                let mut header = HeaderValue::from_static(value);
                header.set_sensitive(true);
                Ok(AuthHeader { value: header })
            }
        }
    }
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
