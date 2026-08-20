use anyhow::Error;
use rmcp::service::ClientInitializeError;
use rmcp::service::ServiceError;
use rmcp::transport::DynamicTransportError;
use rmcp::transport::auth::AuthError;
use rmcp::transport::streamable_http_client::StreamableHttpError;

use crate::http_client_adapter::StreamableHttpClientAdapterError;
use crate::rmcp_client::ClientOperationError;

/// Returns whether an RMCP client error indicates that authentication is required.
///
/// This does not distinguish first-time login from reauthentication.
/// Streamable HTTP initialization errors are stored inside RMCP's dynamic
/// transport error, which is not part of the standard error source chain.
pub fn is_authentication_required_error(error: &Error) -> bool {
    error.chain().any(|source| {
        source
            .downcast_ref::<AuthError>()
            .is_some_and(auth_error_requires_authentication)
            || source
                .downcast_ref::<ClientInitializeError>()
                .is_some_and(|mut error| {
                    while let ClientInitializeError::LegacyFallbackFailed { fallback, .. } = error {
                        error = fallback;
                    }
                    matches!(
                        error,
                        ClientInitializeError::TransportError { error, .. }
                            if transport_error_requires_authentication(error)
                    )
                })
            || source
                .downcast_ref::<ClientOperationError>()
                .is_some_and(|error| {
                    matches!(
                        error,
                        ClientOperationError::Service(ServiceError::TransportSend(error))
                            if transport_error_requires_authentication(error)
                    )
                })
    })
}

fn transport_error_requires_authentication(error: &DynamicTransportError) -> bool {
    error
        .error
        .downcast_ref::<StreamableHttpError<StreamableHttpClientAdapterError>>()
        .is_some_and(|error| match error {
            StreamableHttpError::AuthRequired(_) => true,
            StreamableHttpError::Auth(auth_error) => auth_error_requires_authentication(auth_error),
            _ => false,
        })
}

fn auth_error_requires_authentication(error: &AuthError) -> bool {
    matches!(
        error,
        AuthError::AuthorizationRequired | AuthError::TokenExpired
    )
}
