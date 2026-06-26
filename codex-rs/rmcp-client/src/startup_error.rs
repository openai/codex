use anyhow::Error;
use rmcp::service::ClientInitializeError;
use rmcp::transport::auth::AuthError;
use rmcp::transport::streamable_http_client::StreamableHttpError;

use crate::http_client_adapter::StreamableHttpClientAdapterError;

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
                .is_some_and(client_initialize_error_requires_authentication)
    })
}

fn client_initialize_error_requires_authentication(error: &ClientInitializeError) -> bool {
    let ClientInitializeError::TransportError { error, .. } = error else {
        return false;
    };

    error
        .error
        .downcast_ref::<StreamableHttpError<StreamableHttpClientAdapterError>>()
        .is_some_and(|error| {
            matches!(
                error,
                StreamableHttpError::Auth(auth_error)
                    if auth_error_requires_authentication(auth_error)
            )
        })
}

fn auth_error_requires_authentication(error: &AuthError) -> bool {
    matches!(
        error,
        AuthError::AuthorizationRequired | AuthError::TokenExpired
    )
}
