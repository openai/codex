use std::collections::BTreeMap;
use std::fmt;

use codex_protocol::account::PlanType as AccountPlanType;

/// Backend authentication supplied programmatically by the caller.
///
/// The caller owns credential validation, rotation, and persistence.
/// Codex keeps this snapshot in memory and uses its headers for backend requests.
#[derive(Clone, PartialEq, Eq)]
pub struct ExternalProvidedAuth {
    headers: BTreeMap<String, String>,
    account_id: Option<String>,
    user_id: String,
    account_email: Option<String>,
    account_plan_type: Option<AccountPlanType>,
    is_fedramp_account: bool,
    capabilities: ExternalProvidedAuthCapabilities,
}

/// Behavior explicitly enabled by the caller that supplies
/// [ExternalProvidedAuth].
///
/// All capabilities default to disabled. Callers should only enable behavior
/// that is supported by the credentials they provide.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExternalProvidedAuthCapabilities {
    /// Whether these credentials can authenticate requests to Codex backend
    /// services.
    pub uses_codex_backend: bool,
    /// Whether these credentials represent an authenticated human ChatGPT
    /// account.
    pub has_chatgpt_account: bool,
}

impl fmt::Debug for ExternalProvidedAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExternalProvidedAuth")
            .field("headers", &"<redacted>")
            .field("account_id", &self.account_id)
            .field("user_id", &self.user_id)
            .field(
                "account_email",
                &self.account_email.as_ref().map(|_| "<redacted>"),
            )
            .field("account_plan_type", &self.account_plan_type)
            .field("is_fedramp_account", &self.is_fedramp_account)
            .field("capabilities", &self.capabilities)
            .finish()
    }
}

impl ExternalProvidedAuth {
    /// Creates externally provided auth with backend request headers and a stable user identity.
    pub fn new(
        headers: impl IntoIterator<Item = (String, String)>,
        user_id: impl Into<String>,
    ) -> Self {
        Self {
            headers: headers.into_iter().collect(),
            account_id: None,
            user_id: user_id.into(),
            account_email: None,
            account_plan_type: Some(AccountPlanType::Unknown),
            is_fedramp_account: false,
            capabilities: ExternalProvidedAuthCapabilities::default(),
        }
    }

    /// Adds the account selected by the caller.
    pub fn with_account_id(mut self, account_id: impl Into<String>) -> Self {
        self.account_id = Some(account_id.into());
        self
    }

    /// Adds the account email supplied by the caller.
    pub fn with_account_email(mut self, account_email: impl Into<String>) -> Self {
        self.account_email = Some(account_email.into());
        self
    }

    /// Adds the account plan classification supplied by the caller.
    pub fn with_account_plan_type(mut self, account_plan_type: AccountPlanType) -> Self {
        self.account_plan_type = Some(account_plan_type);
        self
    }

    /// Sets whether the supplied account is a FedRAMP account.
    pub fn with_fedramp_account(mut self, is_fedramp_account: bool) -> Self {
        self.is_fedramp_account = is_fedramp_account;
        self
    }

    /// Sets the behavior supported by these externally provided credentials.
    pub fn with_capabilities(mut self, capabilities: ExternalProvidedAuthCapabilities) -> Self {
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

    /// Returns the account email supplied by the caller.
    pub fn account_email(&self) -> Option<&str> {
        self.account_email.as_deref()
    }

    /// Returns the account plan classification supplied by the caller.
    pub fn account_plan_type(&self) -> Option<AccountPlanType> {
        self.account_plan_type.clone()
    }

    /// Returns whether the supplied account is a FedRAMP account.
    pub fn is_fedramp_account(&self) -> bool {
        self.is_fedramp_account
    }

    /// Returns the stable user ID supplied by the caller.
    pub fn user_id(&self) -> &str {
        &self.user_id
    }

    /// Returns the behavior supported by these externally provided credentials.
    pub fn capabilities(&self) -> ExternalProvidedAuthCapabilities {
        self.capabilities
    }

    /// Returns whether these credentials represent an authenticated human ChatGPT account.
    pub fn has_chatgpt_account(&self) -> bool {
        self.capabilities.has_chatgpt_account
    }

    /// Returns whether these credentials can authenticate requests to Codex backend services.
    pub fn uses_codex_backend(&self) -> bool {
        self.capabilities.uses_codex_backend
    }

    /// Externally provided header auth does not expose an API key.
    pub fn api_key(&self) -> Option<&str> {
        None
    }

    /// Externally provided header auth does not expose a bearer token.
    pub fn bearer_token(&self) -> Result<String, std::io::Error> {
        Err(std::io::Error::other(
            "externally provided auth does not expose a bearer token",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_are_disabled_until_the_caller_enables_them() {
        let auth = ExternalProvidedAuth::new([], "user-123");
        assert_eq!(
            auth.capabilities(),
            ExternalProvidedAuthCapabilities::default()
        );

        let auth = auth.with_capabilities(ExternalProvidedAuthCapabilities {
            uses_codex_backend: true,
            has_chatgpt_account: true,
        });
        assert!(auth.capabilities().uses_codex_backend);
        assert!(auth.capabilities().has_chatgpt_account);
    }
}
