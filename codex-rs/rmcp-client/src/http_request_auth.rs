use codex_api::AuthError;
use codex_api::SharedAuthProvider;
use futures::future::BoxFuture;
use http::HeaderMap;

/// Select request-scoped auth for each HTTP operation on a long-lived MCP connection.
pub trait AuthProviderResolver: Send + Sync {
    fn resolve<'a>(
        &'a self,
        headers: &'a HeaderMap,
    ) -> BoxFuture<'a, Result<SharedAuthProvider, AuthError>>;
}

impl AuthProviderResolver for SharedAuthProvider {
    fn resolve<'a>(
        &'a self,
        _headers: &'a HeaderMap,
    ) -> BoxFuture<'a, Result<SharedAuthProvider, AuthError>> {
        Box::pin(async move { Ok(self.clone()) })
    }
}
