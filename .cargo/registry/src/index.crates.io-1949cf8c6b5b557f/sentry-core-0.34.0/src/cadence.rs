//! [`cadence`] integration for Sentry.
//!
//! [`cadence`] is a popular Statsd client for Rust. The [`SentryMetricSink`] provides a drop-in
//! integration to send metrics captured via `cadence` to Sentry. For direct usage of Sentry
//! metrics, see the [`metrics`](crate::metrics) module.
//!
//! # Usage
//!
//! To use the `cadence` integration, enable the `metrics-cadence1` feature in your `Cargo.toml`.
//! Then, create a [`SentryMetricSink`] and pass it to your `cadence` client:
//!
//! ```
//! use cadence::StatsdClient;
//! use sentry::cadence::SentryMetricSink;
//!
//! let client = StatsdClient::from_sink("sentry.test", SentryMetricSink::new());
//! ```
//!
//! # Side-by-side Usage
//!
//! If you want to send metrics to Sentry and another backend at the same time, you can use
//! [`SentryMetricSink::wrap`] to wrap another [`MetricSink`]:
//!
//! ```
//! use cadence::{StatsdClient, NopMetricSink};
//! use sentry::cadence::SentryMetricSink;
//!
//! let sink = SentryMetricSink::wrap(NopMetricSink);
//! let client = StatsdClient::from_sink("sentry.test", sink);
//! ```

use std::sync::Arc;

use cadence::{MetricSink, NopMetricSink};

use crate::metrics::Metric;
use crate::{Client, Hub};

/// A [`MetricSink`] that sends metrics to Sentry.
///
/// This metric sends all metrics to Sentry. The Sentry client is internally buffered, so submission
/// will be delayed.
///
/// Optionally, this sink can also forward metrics to another [`MetricSink`]. This is useful if you
/// want to send metrics to Sentry and another backend at the same time. Use
/// [`SentryMetricSink::wrap`] to construct such a sink.
#[derive(Debug)]
pub struct SentryMetricSink<S = NopMetricSink> {
    client: Option<Arc<Client>>,
    sink: S,
}

impl<S> SentryMetricSink<S>
where
    S: MetricSink,
{
    /// Creates a new [`SentryMetricSink`], wrapping the given [`MetricSink`].
    pub fn wrap(sink: S) -> Self {
        Self { client: None, sink }
    }

    /// Creates a new [`SentryMetricSink`] sending data to the given [`Client`].
    pub fn with_client(mut self, client: Arc<Client>) -> Self {
        self.client = Some(client);
        self
    }
}

impl SentryMetricSink {
    /// Creates a new [`SentryMetricSink`].
    ///
    /// It is not required that a client is available when this sink is created. The sink sends
    /// metrics to the client of the Sentry hub that is registered when the metrics are emitted.
    pub fn new() -> Self {
        Self {
            client: None,
            sink: NopMetricSink,
        }
    }
}

impl Default for SentryMetricSink {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricSink for SentryMetricSink {
    fn emit(&self, string: &str) -> std::io::Result<usize> {
        if let Ok(metric) = Metric::parse_statsd(string) {
            if let Some(ref client) = self.client {
                client.add_metric(metric);
            } else if let Some(client) = Hub::current().client() {
                client.add_metric(metric);
            }
        }

        // NopMetricSink returns `0`, which is correct as Sentry is buffering the metrics.
        self.sink.emit(string)
    }

    fn flush(&self) -> std::io::Result<()> {
        let flushed = if let Some(ref client) = self.client {
            client.flush(None)
        } else if let Some(client) = Hub::current().client() {
            client.flush(None)
        } else {
            true
        };

        let sink_result = self.sink.flush();

        if !flushed {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "failed to flush metrics to Sentry",
            ))
        } else {
            sink_result
        }
    }
}

#[cfg(test)]
mod tests {
    use cadence::{Counted, Distributed};
    use sentry_types::protocol::latest::EnvelopeItem;

    use crate::test::with_captured_envelopes;

    use super::*;

    #[test]
    fn test_basic_metrics() {
        let envelopes = with_captured_envelopes(|| {
            let client = cadence::StatsdClient::from_sink("sentry.test", SentryMetricSink::new());
            client.count("some.count", 1).unwrap();
            client.count("some.count", 10).unwrap();
            client
                .count_with_tags("count.with.tags", 1)
                .with_tag("foo", "bar")
                .send();
            client.distribution("some.distr", 1).unwrap();
            client.distribution("some.distr", 2).unwrap();
            client.distribution("some.distr", 3).unwrap();
        });
        assert_eq!(envelopes.len(), 1);

        let mut items = envelopes[0].items();
        let Some(EnvelopeItem::Statsd(metrics)) = items.next() else {
            panic!("expected metrics");
        };
        let metrics = std::str::from_utf8(metrics).unwrap();

        println!("{metrics}");

        assert!(metrics
            .contains("sentry.test.count.with.tags@none:1|c|#environment:production,foo:bar|T"));
        assert!(metrics.contains("sentry.test.some.count@none:11|c|#environment:production|T"));
        assert!(metrics.contains("sentry.test.some.distr@none:1:2:3|d|#environment:production|T"));
        assert_eq!(items.next(), None);
    }
}
