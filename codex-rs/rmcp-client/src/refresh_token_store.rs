use futures::future::BoxFuture;
use oauth2::RefreshToken;
use oauth2::TokenResponse;
use rmcp::transport::auth::AuthError;
use rmcp::transport::auth::CredentialStore;
use rmcp::transport::auth::StoredCredentials;
use tokio::sync::RwLock;

#[derive(Default)]
pub(crate) struct RefreshTokenStore {
    credentials: RwLock<Option<StoredCredentials>>,
}

impl CredentialStore for RefreshTokenStore {
    fn load<'life0, 'async_trait>(
        &'life0 self,
    ) -> BoxFuture<'async_trait, Result<Option<StoredCredentials>, AuthError>>
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async { Ok(self.credentials.read().await.clone()) })
    }

    fn save<'life0, 'async_trait>(
        &'life0 self,
        mut credentials: StoredCredentials,
    ) -> BoxFuture<'async_trait, Result<(), AuthError>>
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            let mut stored = self.credentials.write().await;
            let previous_refresh_token = stored
                .as_ref()
                .and_then(|credentials| credentials.token_response.as_ref())
                .and_then(TokenResponse::refresh_token)
                .map(|token| token.secret().to_string());

            if let Some(token_response) = credentials.token_response.as_mut()
                && token_response.refresh_token().is_none()
                && let Some(previous_refresh_token) = previous_refresh_token
            {
                token_response.set_refresh_token(Some(RefreshToken::new(previous_refresh_token)));
            }

            *stored = Some(credentials);
            Ok(())
        })
    }

    fn clear<'life0, 'async_trait>(&'life0 self) -> BoxFuture<'async_trait, Result<(), AuthError>>
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async {
            *self.credentials.write().await = None;
            Ok(())
        })
    }
}
