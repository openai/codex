use std::collections::BTreeMap;
use std::fmt;

/// Backend authentication supplied programmatically by the caller.
///
/// The caller owns credential validation, rotation, and persistence.
/// Codex keeps this snapshot in memory and uses its headers for backend requests.
#[derive(Clone, PartialEq, Eq)]
pub struct CallerProvidedAuth {
    headers: BTreeMap<String, String>,
    account_id: Option<String>,
    user_id: String,
    capabilities: CallerProvidedAuthCapabilities,
}

/// Behavior explicitly enabled by the caller that supplies
/// [CallerProvidedAuth].
///
/// All capabilities default to disabled. Callers should only enable behavior
/// that is supported by the credentials they provide.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CallerProvidedAuthCapabilities {
    /// Whether these credentials can authenticate requests to Codex backend
    /// services.
    pub uses_codex_backend: bool,
    /// Whether these credentials represent an authenticated human ChatGPT
    /// account.
    pub has_chatgpt_account: bool,
}

impl fmt::Debug for CallerProvidedAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CallerProvidedAuth")
            .field("headers", &"<redacted>")
            .field("account_id", &self.account_id)
            .field("user_id", &self.user_id)
            .field("capabilities", &self.capabilities)
            .finish()
    }
}

impl CallerProvidedAuth {
    /// Creates caller-provided auth with backend request headers and a stable user identity.
    pub fn new(
        headers: impl IntoIterator<Item = (String, String)>,
        user_id: impl Into<String>,
    ) -> Self {
        Self {
            headers: headers.into_iter().collect(),
            account_id: None,
            user_id: user_id.into(),
            capabilities: CallerProvidedAuthCapabilities::default(),
        }
    }

    /// Adds the account selected by the caller.
    pub fn with_account_id(mut self, account_id: impl Into<String>) -> Self {
        self.account_id = Some(account_id.into());
        self
    }

    /// Sets the behavior supported by these caller-provided credentials.
    pub fn with_capabilities(mut self, capabilities: CallerProvidedAuthCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }

    /// Returns the request headers supplied by the caller.
    pub fn headers(&self) -> &BTreeMap<String, String> {
        &self.headers
    }

    /// Returns the selected account ID, when the caller supplied one.
    pub fn account_id(&self) -> Option<&str> {
        self.account_id.as_deref()
    }

    /// Returns the stable user ID supplied by the caller.
    pub fn user_id(&self) -> &str {
        &self.user_id
    }

    /// Returns the behavior supported by these caller-provided credentials.
    pub fn capabilities(&self) -> CallerProvidedAuthCapabilities {
        self.capabilities
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_are_disabled_until_the_caller_enables_them() {
        let auth = CallerProvidedAuth::new([], "user-123");
        assert_eq!(
            auth.capabilities(),
            CallerProvidedAuthCapabilities::default()
        );

        let auth = auth.with_capabilities(CallerProvidedAuthCapabilities {
            uses_codex_backend: true,
            has_chatgpt_account: true,
        });
        assert!(auth.capabilities().uses_codex_backend);
        assert!(auth.capabilities().has_chatgpt_account);
    }
}
