//! Interceptors can transform incremental events and retain upstream cancellation.
use super::*;
use crate::client_common::ResponseStream;
use codex_extension_api::ResponseEvent;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

struct Append(&'static str);
impl ModelResponseInterceptor for Append {
    fn intercept(self: Box<Self>, stream: ModelResponseStream) -> ModelResponseStream {
        Box::pin(stream.map(move |event| {
            event.map(|event| match event {
                ResponseEvent::OutputTextDelta(text) => {
                    ResponseEvent::OutputTextDelta(format!("{text}{}", self.0))
                }
                other => other,
            })
        }))
    }
}

#[tokio::test]
async fn interceptors_forward_before_completion_in_order_and_cancel_upstream() {
    let (tx_event, rx_event) = mpsc::channel(1);
    let consumer_dropped = CancellationToken::new();
    let upstream_cancelled = consumer_dropped.clone();
    let mut stream = intercept_stream(
        Box::pin(
            ResponseStream {
                rx_event,
                consumer_dropped,
            }
            .map(|event| {
                event.map_err(|error| {
                    codex_extension_api::ModelResponseError::Stream(error.to_string())
                })
            }),
        ),
        vec![Box::new(Append("a")), Box::new(Append("b"))],
    );
    tx_event
        .send(Ok(ResponseEvent::OutputTextDelta("text".into())))
        .await
        .unwrap();
    // Upstream remains open and has not sent Completed.
    let event = tokio::time::timeout(Duration::from_secs(1), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let ResponseEvent::OutputTextDelta(text) = event else {
        panic!("expected text delta");
    };
    assert_eq!(text, "textab");
    drop(stream);
    tokio::time::timeout(Duration::from_secs(1), upstream_cancelled.cancelled())
        .await
        .unwrap();
}

#[derive(Debug)]
struct MetadataContributor;
impl ModelRequestContributor for MetadataContributor {
    fn request(&self, input: ModelRequestInput<'_>) -> Option<Box<dyn ModelResponseInterceptor>> {
        *input.client_metadata = Some(HashMap::from([
            ("thread_id".into(), "replaced".into()),
            ("guardian_credits_requested".into(), "true".into()),
            ("parent_response_id".into(), "forged".into()),
            ("existing".into(), "replaced".into()),
            ("x-request-id".into(), "request".into()),
        ]));
        None
    }
}

#[test]
fn contributor_metadata_preserves_core_fields_and_existing_values() {
    let mut metadata = Some(HashMap::from([
        ("thread_id".into(), "thread".into()),
        ("existing".into(), "original".into()),
    ]));
    let contributors: Vec<Arc<dyn ModelRequestContributor>> = vec![Arc::new(MetadataContributor)];
    assert!(
        prepare(
            &contributors,
            "thread",
            "model",
            ModelRequestKind::Generation,
            &mut metadata
        )
        .is_empty()
    );
    assert_eq!(
        metadata,
        Some(HashMap::from([
            ("thread_id".into(), "thread".into()),
            ("existing".into(), "original".into()),
            ("x-request-id".into(), "request".into()),
        ]))
    );
}
