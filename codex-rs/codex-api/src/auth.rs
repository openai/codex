use codex_client::Request;
use codex_client::TransportError;
use codex_http_client::NetworkPolicyDenied;
use http::HeaderMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Error returned while applying authentication to an outbound request.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error(transparent)]
    Policy(#[from] NetworkPolicyDenied),
    #[error("request auth build error: {0}")]
    Build(String),
    #[error("transient auth error: {0}")]
    Transient(String),
}

impl From<AuthError> for TransportError {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::Policy(denied) => TransportError::Policy(denied),
            AuthError::Build(message) => TransportError::Build(message),
            AuthError::Transient(message) => TransportError::Network(message),
        }
    }
}

/// Applies authentication to API requests.
///
/// Header-only providers can implement `add_auth_headers`; providers that sign
/// complete requests can override `apply_auth`.
pub trait AuthProvider: Send + Sync {
    /// Adds any auth headers that are available without request body access.
    ///
    /// Implementations should be cheap and non-blocking. This method is also
    /// used by telemetry and non-HTTP request paths. Providers that require async
    /// work must return an error here and override `apply_auth_headers`.
    /// Callers must not send the request if this returns an error.
    fn add_auth_headers(&self, headers: &mut HeaderMap) -> Result<(), AuthError>;

    /// Returns any auth headers that are available without request body access.
    fn to_auth_headers(&self) -> Result<HeaderMap, AuthError> {
        let mut headers = HeaderMap::new();
        self.add_auth_headers(&mut headers)?;
        Ok(headers)
    }

    /// Describes headers expected on a successful request without refreshing or registering auth.
    /// Async providers should override this rather than resolving credentials for telemetry.
    fn auth_header_telemetry(&self) -> Result<AuthHeaderTelemetry, AuthError> {
        let headers = self.to_auth_headers()?;
        let name = headers
            .contains_key(http::header::AUTHORIZATION)
            .then_some("authorization");
        Ok(AuthHeaderTelemetry {
            attached: name.is_some(),
            name,
        })
    }

    /// Refreshes credentials after an explicit HTTP 401. Returns whether the caller
    /// may retry with new credentials. Callers must bound retries and pass the headers
    /// of the rejected request so concurrent failures can reuse an earlier refresh.
    fn refresh_after_unauthorized(&self, _rejected_headers: HeaderMap) -> AuthRefreshFuture<'_> {
        Box::pin(async { false })
    }

    /// Authenticates a header-only request, including removing credentials it supersedes.
    ///
    /// Implementations may refresh credentials asynchronously. Providers with static credentials
    /// can rely on the default implementation.
    /// Callers must use the returned map rather than overlaying resolved headers.
    fn apply_auth_headers(&self, mut headers: HeaderMap) -> AuthHeadersFuture<'_> {
        Box::pin(async move {
            headers.extend(self.to_auth_headers()?);
            Ok(headers)
        })
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
    fn apply_auth(&self, request: Request) -> AuthProviderFuture<'_> {
        Box::pin(async move {
            let mut request = request;
            request.headers = self.apply_auth_headers(request.headers).await?;
            Ok(request)
        })
    }
}

pub type AuthProviderFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Request, AuthError>> + Send + 'a>>;

pub type AuthHeadersFuture<'a> =
    Pin<Box<dyn Future<Output = Result<HeaderMap, AuthError>> + Send + 'a>>;

pub type AuthRefreshFuture<'a> = Pin<Box<dyn Future<Output = bool> + Send + 'a>>;

/// Shared auth handle passed through API clients.
pub type SharedAuthProvider = Arc<dyn AuthProvider>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentIdentityTelemetry {
    pub agent_id: String,
    pub task_id: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AuthHeaderTelemetry {
    pub attached: bool,
    pub name: Option<&'static str>,
}

pub fn auth_header_telemetry(auth: &dyn AuthProvider) -> Result<AuthHeaderTelemetry, AuthError> {
    auth.auth_header_telemetry()
}
