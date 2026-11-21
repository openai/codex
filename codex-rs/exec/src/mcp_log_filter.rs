//! Filters redundant HTTP MCP worker failures from default exec stderr output.
//! Startup and tool-call owners report these failures with server attribution.

use std::fmt::Debug;
use tracing::Event;
use tracing::Metadata;
use tracing::Subscriber;
use tracing::field::Field;
use tracing::field::Visit;
use tracing_subscriber::layer::Context;
use tracing_subscriber::layer::Filter;

pub(super) struct McpWorkerFilter;

impl<S: Subscriber> Filter<S> for McpWorkerFilter {
    fn enabled(&self, _: &Metadata<'_>, _: &Context<'_, S>) -> bool {
        true
    }

    fn event_enabled(&self, event: &Event<'_>, _: &Context<'_, S>) -> bool {
        if event.metadata().target() != "rmcp::transport::worker" {
            return true;
        }
        let mut message = WorkerMessage::default();
        event.record(&mut message);
        !message.redundant_http_failure
    }
}

#[derive(Default)]
struct WorkerMessage {
    redundant_http_failure: bool,
}

impl Visit for WorkerMessage {
    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        if field.name() == "message" {
            let message = format!("{value:?}");
            // The SDK uses Display for notification errors, which omits the adapter variant.
            self.redundant_http_failure = message
                .strip_prefix("worker quit with fatal: Transport channel closed, when ")
                .is_some_and(|error| {
                    error.starts_with("Client(HandshakeHttpRequest(")
                        || error.starts_with("UnexpectedServerResponse(")
                })
                || (message.starts_with("worker quit with fatal: ")
                    && message.ends_with(", when send initialized notification"));
        }
    }
}
