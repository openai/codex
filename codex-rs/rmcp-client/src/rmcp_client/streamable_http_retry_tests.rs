use std::any::TypeId;
use std::collections::BTreeMap;

use pretty_assertions::assert_eq;
use rmcp::transport::DynamicTransportError;
use rmcp::transport::streamable_http_client::StreamableHttpError;

use crate::http_client_adapter::StreamableHttpClientAdapterError;

use super::*;

fn metric_tags_map(tags: Vec<(&'static str, String)>) -> BTreeMap<&'static str, String> {
    tags.into_iter().collect()
}

#[test]
fn initialize_metric_tags_record_success_after_retry() {
    let tags = metric_tags_map(initialize_metric_tags(
        "streamable_http",
        "success",
        /*attempts*/ 2,
        /*retry_exhausted*/ false,
        "none",
    ));

    assert_eq!(
        tags,
        metric_tags_map(vec![
            ("transport", "streamable_http".to_string()),
            ("outcome", "success".to_string()),
            ("retried", "true".to_string()),
            ("attempts", "2".to_string()),
            ("retry_count", "1".to_string()),
            ("retry_exhausted", "false".to_string()),
            ("failure_kind", "none".to_string()),
        ])
    );
}

#[test]
fn initialize_metric_tags_record_retry_exhaustion() {
    let tags = metric_tags_map(initialize_metric_tags(
        "streamable_http",
        "error",
        /*attempts*/ 3,
        /*retry_exhausted*/ true,
        "retry_exhausted",
    ));

    assert_eq!(
        tags,
        metric_tags_map(vec![
            ("transport", "streamable_http".to_string()),
            ("outcome", "error".to_string()),
            ("retried", "true".to_string()),
            ("attempts", "3".to_string()),
            ("retry_count", "2".to_string()),
            ("retry_exhausted", "true".to_string()),
            ("failure_kind", "retry_exhausted".to_string()),
        ])
    );
}

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
