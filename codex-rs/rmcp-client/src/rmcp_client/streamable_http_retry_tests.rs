use std::any::TypeId;

use codex_exec_server::ExecServerError;
use pretty_assertions::assert_eq;
use rmcp::transport::DynamicTransportError;
use rmcp::transport::streamable_http_client::StreamableHttpError;

use crate::http_client_adapter::StreamableHttpClientAdapterError;

use super::*;

#[test]
fn retryable_initialize_error_includes_initialized_notification_context() {
    let contexts = [
        "send initialize request",
        "send initialized notification",
        "receive initialize response",
    ];

    assert_eq!(
        contexts.map(|context| {
            RmcpClient::is_retryable_client_initialize_error(&retryable_initialize_error(context))
        }),
        [true, true, false],
    );
}

#[test]
fn retryable_streamable_http_error_includes_remote_body_stream_failure() {
    let errors = [
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
    ];

    assert_eq!(
        errors.map(|error| RmcpClient::is_retryable_streamable_http_error(&error)),
        [true, false],
    );
}

fn retryable_initialize_error(context: &'static str) -> rmcp::service::ClientInitializeError {
    rmcp::service::ClientInitializeError::TransportError {
        error: DynamicTransportError::from_parts(
            "streamable_http",
            TypeId::of::<()>(),
            Box::new(StreamableHttpError::Client(
                StreamableHttpClientAdapterError::RetryableHttpStatus(
                    reqwest::StatusCode::SERVICE_UNAVAILABLE.as_u16(),
                ),
            )),
        ),
        context: context.into(),
    }
}
