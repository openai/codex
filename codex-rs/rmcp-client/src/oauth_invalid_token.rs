use anyhow::Error;
use rmcp::service::ClientInitializeError;
use rmcp::transport::DynamicTransportError;
use rmcp::transport::streamable_http_client::StreamableHttpError;

use crate::http_client_adapter::StreamableHttpClientAdapterError;
use crate::http_client_adapter::is_bearer_invalid_token_challenge;

#[derive(Debug, thiserror::Error)]
#[error("MCP OAuth access token was rejected; credentials refreshed, retry the request")]
pub(crate) struct RetryRequired;

pub(crate) fn rejected_initialize_request(error: &Error) -> bool {
    let Some(ClientInitializeError::TransportError { error, context }) = error
        .chain()
        .find_map(|source| source.downcast_ref::<ClientInitializeError>())
    else {
        return false;
    };

    context.as_ref() == "send initialize request" && rejected_transport(error)
}

pub(crate) fn rejected_transport(error: &DynamicTransportError) -> bool {
    error
        .error
        .downcast_ref::<StreamableHttpError<StreamableHttpClientAdapterError>>()
        .is_some_and(|error| {
            matches!(
                error,
                StreamableHttpError::AuthRequired(auth)
                    if is_bearer_invalid_token_challenge(&auth.www_authenticate_header)
            )
        })
}
