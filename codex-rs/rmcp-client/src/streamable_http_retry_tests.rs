use std::any::TypeId;

use codex_exec_server::ExecServerError;
use pretty_assertions::assert_eq;
use rmcp::transport::DynamicTransportError;
use rmcp::transport::streamable_http_client::AuthRequiredError;
use rmcp::transport::streamable_http_client::StreamableHttpError;

use crate::http_client_adapter::StreamableHttpClientAdapterError;
use crate::rmcp_client::ClientOperationError;

use super::*;

#[test]
fn retryable_initialize_error_includes_discovery_and_initialized_notification_context() {
    let contexts = [
        "send discover request",
        "send initialize request",
        "send initialized notification",
        "receive initialize response",
    ];

    assert_eq!(
        contexts.map(|context| {
            RmcpClient::is_retryable_client_initialize_error(&retryable_initialize_error(context))
        }),
        [true, true, true, false],
    );
}

#[test]
fn retryable_streamable_http_error_includes_remote_body_stream_failure() {
    let errors = [
        StreamableHttpError::Client(StreamableHttpClientAdapterError::HttpRequest(
            ExecServerError::HttpRequest("error sending request for url".to_string()),
        )),
        StreamableHttpError::Client(StreamableHttpClientAdapterError::HttpRequest(
            ExecServerError::Server {
                code: JSON_RPC_INTERNAL_ERROR_CODE,
                message: "http/request failed: error sending request for url".to_string(),
            },
        )),
        StreamableHttpError::Client(StreamableHttpClientAdapterError::HttpRequest(
            ExecServerError::Protocol(
                "http response stream `http-1` failed: exec-server transport disconnected"
                    .to_string(),
            ),
        )),
        StreamableHttpError::Client(StreamableHttpClientAdapterError::HttpRequest(
            ExecServerError::Protocol(
                "http response stream `http-1` received seq 2, expected 1".to_string(),
            ),
        )),
        StreamableHttpError::UnexpectedServerResponse("HTTP 502: upstream failure".into()),
        StreamableHttpError::UnexpectedServerResponse("HTTP 400: bad request".into()),
    ];

    assert_eq!(
        errors.map(|error| RmcpClient::is_retryable_streamable_http_error(&error)),
        [true, true, true, false, true, false],
    );
}

#[test]
fn startup_http_authentication_challenges_require_reauthorization() {
    let transport_error = || {
        DynamicTransportError::from_parts(
            "streamable_http",
            TypeId::of::<()>(),
            Box::new(
                StreamableHttpError::<StreamableHttpClientAdapterError>::AuthRequired(
                    AuthRequiredError::new("Bearer error=\"invalid_token\"".to_string()),
                ),
            ),
        )
    };
    let errors = [
        anyhow::Error::new(rmcp::service::ClientInitializeError::TransportError {
            error: transport_error(),
            context: "send initialize request".into(),
        }),
        anyhow::Error::new(ClientOperationError::from(
            rmcp::service::ServiceError::TransportSend(transport_error()),
        )),
    ];

    for error in errors {
        assert!(crate::startup_error::is_authentication_required_error(
            &error
        ));
    }
}

fn retryable_initialize_error(context: &'static str) -> rmcp::service::ClientInitializeError {
    rmcp::service::ClientInitializeError::TransportError {
        error: DynamicTransportError::from_parts(
            "streamable_http",
            TypeId::of::<()>(),
            Box::new(StreamableHttpError::Client(
                StreamableHttpClientAdapterError::HttpRequest(ExecServerError::HttpRequest(
                    "error sending request for url".to_string(),
                )),
            )),
        ),
        context: context.into(),
    }
}
