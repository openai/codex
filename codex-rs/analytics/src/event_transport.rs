//! Let a host select analytics events by wire type after the existing eligibility checks.

use crate::events::TrackEventRequest;
use std::future::Future;
use std::pin::Pin;

/// One source event in its HTTP wire format, before backend normalization.
/// The transport must preserve the product attribution and supply its own fresh actor context.
/// Canonical delivery must also preserve any backend side effects for the selected type.
#[derive(Debug, PartialEq)]
pub struct AnalyticsTransportEvent {
    pub event_type: &'static str,
    /// The complete event, including any type-specific top-level fields.
    pub payload: serde_json::Value,
    pub product_sku: Option<String>,
}

/// Whether the original event still needs the existing HTTP delivery path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalyticsEventDelivery {
    /// Preserve HTTP delivery, including after publishing a shadow copy.
    Http,
    /// The host took responsibility for canonical delivery; do not also send HTTP.
    /// This does not imply durable persistence or successful delivery.
    Handled,
}

/// Host-owned transport for explicitly selected event types.
///
/// Implementations own policy checks, actor refresh, schema/envelope normalization,
/// and bounded delivery. Return `Http` for legacy or shadow mode, or when declining
/// before a canonical publish. After attempting canonical delivery, return `Handled`
/// even if its outcome is uncertain, to avoid an automatic duplicate through HTTP.
/// The queue awaits this method, so implementations must enforce their own deadline
/// or enqueue into a bounded host queue; stalled delivery also delays unrelated facts.
///
/// The boxed future permits different host implementations behind a shared client.
pub trait AnalyticsEventTransport: Send + Sync {
    /// Select the wire types this host supports before preparing a payload.
    /// Return false for unknown types and for events using ordinary HTTP delivery.
    fn selects(&self, event_type: &str) -> bool;

    fn publish(
        &self,
        event: AnalyticsTransportEvent,
    ) -> Pin<Box<dyn Future<Output = AnalyticsEventDelivery> + Send + '_>>;
}

pub(crate) async fn retain_http_events(
    events: Vec<TrackEventRequest>,
    product_sku: Option<&str>,
    transport: &dyn AnalyticsEventTransport,
) -> Vec<TrackEventRequest> {
    let mut http_events = Vec::with_capacity(events.len());
    for event in events {
        let event_type = event.event_type();
        if !transport.selects(event_type) {
            http_events.push(event);
            continue;
        }
        let payload = match serde_json::to_value(&event) {
            Ok(payload) => payload,
            Err(_) => {
                tracing::warn!(
                    event_type,
                    "failed to prepare event for host analytics transport"
                );
                http_events.push(event);
                continue;
            }
        };
        let delivery = transport
            .publish(AnalyticsTransportEvent {
                event_type,
                payload,
                product_sku: product_sku.map(str::to_owned),
            })
            .await;
        if delivery == AnalyticsEventDelivery::Http {
            http_events.push(event);
        }
    }
    http_events
}
