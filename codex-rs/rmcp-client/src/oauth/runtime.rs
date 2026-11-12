//! Keeps one OAuth refresh and persistence owner for the lifetime of a connection.
//!
//! Both modes use cached expiry to prepare credentials before the MCP request budget.
//! Reactive exchanges also own their tasks so request cancellation cannot interrupt persistence.
//! Coordinated mode keeps RMCP's manager-before-credential lock order and commits through its
//! pinned store. Legacy retains Codex's persistor. Cleanup reads only cached credentials.

use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use futures::TryFutureExt;
use oauth2::TokenResponse;
use rmcp::transport::auth::AuthError;
use rmcp::transport::auth::AuthorizationManager;
use tokio::sync::Mutex;
use tracing::warn;

use super::OAuthPersistor;
use super::credential_store::OAuthCredentialStore;
use super::refresh_transaction::REFRESH_REQUEST_TIMEOUT;
use super::token_needs_refresh;
use crate::oauth_http_client::PROACTIVE_REFRESH_TIMEOUT;

#[derive(Clone)]
pub(crate) enum OAuthRuntime {
    Legacy(OAuthPersistor),
    Coordinated {
        auth_manager: Arc<Mutex<AuthorizationManager>>,
        store: OAuthCredentialStore,
    },
}

enum AccessTokenOperation {
    Get,
    Refresh { rejected_access_token: String },
}

impl OAuthRuntime {
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "cleanup must follow an in-flight refresh under the manager lock"
    )]
    pub(crate) async fn cleanup_access_token(&self) -> Option<String> {
        let _manager = self.authorization_manager().lock().await;
        let credentials = match self {
            Self::Legacy(persistor) => persistor.stored_credentials().await,
            Self::Coordinated { store, .. } => store.stored_credentials().await,
        };
        credentials.map(|tokens| tokens.token_response.0.access_token().secret().to_string())
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "AuthorizationManager access follows RMCP's manager-before-credential lock order"
    )]
    pub(crate) async fn refresh_if_needed(&self) -> Result<()> {
        match self {
            Self::Legacy(persistor) => persistor.refresh_if_needed().await,
            Self::Coordinated {
                auth_manager,
                store,
            } => {
                let expires_at = store
                    .stored_credentials()
                    .await
                    .and_then(|tokens| tokens.expires_at);
                if !token_needs_refresh(expires_at) {
                    return Ok(());
                }
                let auth_manager = Arc::clone(auth_manager);
                let store = store.clone();
                tokio::spawn(async move {
                    let mut manager = auth_manager.lock().await;
                    let result = store.refresh_if_needed(&mut manager).await;
                    if result.is_err() {
                        // Keep the summary in the owned task without logging credential/provider data.
                        warn!("MCP OAuth preparation failed");
                    }
                    result
                })
                .await
                .context("OAuth refresh task failed")?
            }
        }
    }
    pub(crate) async fn get_access_token(&self) -> Result<String, AuthError> {
        self.access_token(AccessTokenOperation::Get).await
    }

    pub(crate) async fn refresh_access_token(
        &self,
        rejected_access_token: &str,
    ) -> Result<String, AuthError> {
        self.access_token(AccessTokenOperation::Refresh {
            rejected_access_token: rejected_access_token.to_string(),
        })
        .await
    }

    async fn access_token(&self, operation: AccessTokenOperation) -> Result<String, AuthError> {
        let (mut manager, credential_guard) = match self {
            Self::Legacy(persistor) => {
                if matches!(&operation, AccessTokenOperation::Get) {
                    let manager = Arc::clone(self.authorization_manager()).lock_owned().await;
                    if let Some(token) = persistor.fresh_access_token(&manager).await? {
                        return Ok(token);
                    }
                }

                let credential_guard = persistor.acquire_persistence_guard().await?;
                let manager = Arc::clone(self.authorization_manager()).lock_owned().await;
                (manager, Some(credential_guard))
            }
            Self::Coordinated { .. } => (
                Arc::clone(self.authorization_manager()).lock_owned().await,
                None,
            ),
        };
        let runtime = self.clone();
        let access_token = async move {
            let result = match operation {
                AccessTokenOperation::Get => manager.get_access_token().await,
                AccessTokenOperation::Refresh {
                    rejected_access_token,
                } => {
                    let (_client_id, credentials) = manager.get_credentials().await?;
                    let current_access_token = credentials
                        .ok_or(AuthError::AuthorizationRequired)?
                        .access_token()
                        .secret()
                        .to_string();
                    if current_access_token != rejected_access_token {
                        match &runtime {
                            Self::Coordinated { store, .. } => {
                                store
                                    .reuse_access_token(&mut manager, &current_access_token)
                                    .await
                            }
                            Self::Legacy(_) => manager.get_access_token().await,
                        }
                    } else {
                        manager
                            .refresh_token()
                            .await
                            .map(|tokens| tokens.access_token().secret().to_string())
                    }
                }
            };
            let token = match result {
                Err(AuthError::TokenRefreshRejected(_)) => {
                    return Err(AuthError::AuthorizationRequired);
                }
                result => result?,
            };
            // Keep the manager locked until Legacy's in-memory result is durable, so the next
            // exchange cannot consume a replacement refresh token before it has been saved.
            if let (Self::Legacy(persistor), Some(credential_guard)) =
                (&runtime, credential_guard.as_ref())
            {
                persistor
                    .persist_with_manager(&manager, credential_guard)
                    .await
                    .map_err(|_| {
                        AuthError::CredentialStoreError(
                            "failed to persist MCP OAuth credentials".to_string(),
                        )
                    })?;
            }
            Ok(token)
        };
        tokio::spawn(PROACTIVE_REFRESH_TIMEOUT.scope(
            REFRESH_REQUEST_TIMEOUT,
            access_token.inspect_err(|_| warn!("MCP OAuth access-token operation failed")),
        ))
        .await
        .map_err(|_| AuthError::InternalError("MCP OAuth task failed".to_string()))?
    }

    fn authorization_manager(&self) -> &Arc<Mutex<AuthorizationManager>> {
        match self {
            Self::Legacy(persistor) => &persistor.inner.authorization_manager,
            Self::Coordinated { auth_manager, .. } => auth_manager,
        }
    }
}
