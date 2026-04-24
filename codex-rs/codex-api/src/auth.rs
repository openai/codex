use async_trait::async_trait;
use codex_client::Request;
use codex_client::TransportError;
use http::HeaderMap;
use std::sync::Arc;

/// Error returned while applying authentication to an outbound request.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("request auth build error: {0}")]
    Build(String),
    #[error("transient auth error: {0}")]
    Transient(String),
}

impl From<AuthError> for TransportError {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::Build(message) => TransportError::Build(message),
            AuthError::Transient(message) => TransportError::Network(message),
        }
    }
}

/// Applies authentication to API requests.
///
/// Header-only providers can implement `add_auth_headers`; providers that sign
/// complete requests can override `apply_auth`.
#[async_trait]
pub trait AuthProvider: Send + Sync {
    /// Adds any auth headers that are available without request body access.
    ///
    /// Implementations should be cheap and non-blocking. This method is also
    /// used by telemetry and non-HTTP request paths.
    fn add_auth_headers(&self, headers: &mut HeaderMap);

    /// Returns any auth headers that are available without request body access.
    fn to_auth_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        self.add_auth_headers(&mut headers);
        headers
    }

    /// Applies auth to a complete outbound request and returns the request to send.
    ///
    /// The input `request` is moved into this method. Implementations may mutate
    /// the owned request, or replace it entirely, before returning.
    ///
    /// Header-only auth providers can rely on the default implementation.
    /// Request-signing providers can override this to inspect the final URL,
    /// headers, and body bytes before the transport sends the request.
    ///
    /// Callers must always use the returned request as authoritative.
    /// If this returns [`AuthError`], the request should not be sent.
    async fn apply_auth(&self, request: Request) -> Result<Request, AuthError> {
        let mut request = request;
        self.add_auth_headers(&mut request.headers);
        Ok(request)
    }
}

/// Shared auth handle passed through API clients.
pub type SharedAuthProvider = Arc<dyn AuthProvider>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AuthHeaderTelemetry {
    pub attached: bool,
    pub name: Option<&'static str>,
}

pub fn auth_header_telemetry(auth: &dyn AuthProvider) -> AuthHeaderTelemetry {
    let mut headers = HeaderMap::new();
    auth.add_auth_headers(&mut headers);
    let name = headers
        .contains_key(http::header::AUTHORIZATION)
        .then_some("authorization");
    AuthHeaderTelemetry {
        attached: name.is_some(),
        name,
    }
}
