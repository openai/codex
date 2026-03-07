use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use reqwest::cookie::Jar;

use crate::CodexAuth;
use crate::config::Config;
use crate::mcp::codex_apps_mcp_url;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct AppsMcpCookieStoreKey {
    url: String,
    account_id: Option<String>,
    chatgpt_user_id: Option<String>,
    is_workspace_account: bool,
}

#[derive(Clone, Default)]
pub struct AppsMcpCookieStore {
    jars: Arc<StdMutex<HashMap<AppsMcpCookieStoreKey, Arc<Jar>>>>,
}

impl AppsMcpCookieStore {
    pub fn jar_for(&self, config: &Config, auth: Option<&CodexAuth>) -> Arc<Jar> {
        let token_data = auth.and_then(|auth| auth.get_token_data().ok());
        self.jar_for_identity(
            codex_apps_mcp_url(config),
            token_data
                .as_ref()
                .and_then(|token_data| token_data.account_id.clone()),
            token_data
                .as_ref()
                .and_then(|token_data| token_data.id_token.chatgpt_user_id.clone()),
            token_data
                .as_ref()
                .is_some_and(|token_data| token_data.id_token.is_workspace_account()),
        )
    }

    fn jar_for_identity(
        &self,
        url: String,
        account_id: Option<String>,
        chatgpt_user_id: Option<String>,
        is_workspace_account: bool,
    ) -> Arc<Jar> {
        let key = AppsMcpCookieStoreKey {
            url,
            account_id,
            chatgpt_user_id,
            is_workspace_account,
        };
        let mut jars = self
            .jars
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Arc::clone(jars.entry(key).or_insert_with(|| Arc::new(Jar::default())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jar_for_identity_reuses_matching_key() {
        let store = AppsMcpCookieStore::default();
        let left = store.jar_for_identity(
            "https://example.test/apps".to_string(),
            Some("account-1".to_string()),
            Some("user-1".to_string()),
            false,
        );
        let right = store.jar_for_identity(
            "https://example.test/apps".to_string(),
            Some("account-1".to_string()),
            Some("user-1".to_string()),
            false,
        );

        assert!(Arc::ptr_eq(&left, &right));
    }

    #[test]
    fn jar_for_identity_separates_account_boundaries() {
        let store = AppsMcpCookieStore::default();
        let left = store.jar_for_identity(
            "https://example.test/apps".to_string(),
            Some("account-1".to_string()),
            Some("user-1".to_string()),
            false,
        );
        let right = store.jar_for_identity(
            "https://example.test/apps".to_string(),
            Some("account-2".to_string()),
            Some("user-1".to_string()),
            false,
        );

        assert!(!Arc::ptr_eq(&left, &right));
    }

    #[test]
    fn jar_for_identity_separates_workspace_boundaries() {
        let store = AppsMcpCookieStore::default();
        let left = store.jar_for_identity(
            "https://example.test/apps".to_string(),
            Some("account-1".to_string()),
            Some("user-1".to_string()),
            false,
        );
        let right = store.jar_for_identity(
            "https://example.test/apps".to_string(),
            Some("account-1".to_string()),
            Some("user-1".to_string()),
            true,
        );

        assert!(!Arc::ptr_eq(&left, &right));
    }
}
