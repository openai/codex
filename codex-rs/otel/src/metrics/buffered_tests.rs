use super::*;
use crate::MetricsConfig;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::MetricData;
use pretty_assertions::assert_eq;
use std::sync::Barrier;
use std::sync::RwLock;

fn client() -> MetricsClient {
    MetricsClient::new(
        MetricsConfig::in_memory("test", "test", "1", Default::default()).with_runtime_reader(),
    )
    .unwrap()
}

fn observe(buffer: &BufferedMetrics) {
    buffer
        .record(
            "test.operations",
            "test.duration",
            Duration::from_millis(/*millis*/ 7),
            &[("component", "test")],
        )
        .unwrap();
}

fn totals(metrics: &MetricsClient) -> (u64, u64, f64) {
    let snapshot = metrics.snapshot().unwrap();
    let mut totals = (0, 0, 0.0);
    for metric in snapshot
        .scope_metrics()
        .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
    {
        match metric.data() {
            AggregatedMetrics::U64(MetricData::Sum(sum)) => {
                totals.0 += sum
                    .data_points()
                    .map(opentelemetry_sdk::metrics::data::SumDataPoint::value)
                    .sum::<u64>();
            }
            AggregatedMetrics::F64(MetricData::Histogram(histogram)) => {
                for point in histogram.data_points() {
                    totals.1 += point.count();
                    totals.2 += point.sum();
                }
            }
            _ => panic!("unexpected metric"),
        }
    }
    totals
}

#[test]
fn startup_limits_and_validation_preserve_whole_pairs() {
    let buffer = BufferedMetrics::new();
    let oversized = "x".repeat(MAX_METADATA_BYTES + 1);
    assert!(matches!(
        buffer.record("count", "duration", Duration::ZERO, &[("tag", &oversized)]),
        Err(MetricsError::OperationMetadataTooLarge)
    ));
    assert!(matches!(
        buffer.record(
            "count",
            "duration",
            Duration::ZERO,
            &[("tag", "value"); MAX_TAGS + 1]
        ),
        Err(MetricsError::OperationMetadataTooLarge)
    ));
    for name in ["invalid duration", "1duration", &"d".repeat(256)] {
        assert!(buffer.record("count", name, Duration::ZERO, &[]).is_err());
        assert!(
            buffer
                .record(name, "duration", Duration::ZERO, &[])
                .is_err()
        );
    }
    assert!(
        buffer
            .record(
                "count",
                "duration",
                Duration::ZERO,
                &[("tag", "invalid value")]
            )
            .is_err()
    );
    for _ in 0..MAX_PENDING_OPERATIONS + 1 {
        observe(&buffer);
    }
    let metrics = client();
    buffer.enable(&metrics);
    assert_eq!(totals(&metrics), (256, 256, 1792.0));
    buffer.enable(&metrics);
    assert_eq!(totals(&metrics), (0, 0, 0.0));
    for name in ["invalid duration", "1duration", &"d".repeat(256)] {
        assert!(buffer.record("count", name, Duration::ZERO, &[]).is_err());
        assert!(
            buffer
                .record(name, "duration", Duration::ZERO, &[])
                .is_err()
        );
    }
    assert_eq!(totals(&metrics), (0, 0, 0.0));
}

#[test]
fn opt_out_discards_pending_and_live_observations_until_reenabled() {
    let buffer = BufferedMetrics::new();
    observe(&buffer);
    buffer.disable();
    observe(&buffer);
    let first = client();
    buffer.enable(&first);
    assert_eq!(totals(&first), (0, 0, 0.0));
    observe(&buffer);
    assert_eq!(totals(&first), (1, 1, 7.0));
    buffer.disable();
    observe(&buffer);
    assert_eq!(totals(&first), (0, 0, 0.0));
    let second = client();
    buffer.enable(&second);
    observe(&buffer);
    assert_eq!(totals(&second), (1, 1, 7.0));
    assert_eq!(totals(&first), (0, 0, 0.0));
}

#[test]
fn concurrent_installation_and_replacement_record_each_pair_once() {
    let buffer = BufferedMetrics::new();
    let first = client();
    let second = client();
    let barrier = Barrier::new(/*n*/ 5);
    observe(&buffer);
    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| {
                barrier.wait();
                for _ in 0..16 {
                    observe(&buffer);
                }
            });
        }
        barrier.wait();
        buffer.enable(&first);
        buffer.enable(&second);
    });
    let a = totals(&first);
    let b = totals(&second);
    assert_eq!((a.0 + b.0, a.1 + b.1, a.2 + b.2), (65, 65, 455.0));
    assert_eq!((a.0, b.0), (a.1, b.1));
    buffer.enable(&second);
    assert_eq!(totals(&second), (0, 0, 0.0));
}

#[test]
fn redirecting_a_global_handle_does_not_change_the_buffers_installation() {
    let buffer = BufferedMetrics::new();
    let first = client();
    let second = client();
    let active = Arc::new(RwLock::new(Arc::clone(&first.inner)));
    let redirected = MetricsClient {
        inner: Arc::clone(&first.inner),
        active: Some(Arc::clone(&active)),
    };
    buffer.enable(&redirected);
    *active.write().unwrap() = Arc::clone(&second.inner);
    observe(&buffer);
    assert_eq!(totals(&first), (1, 1, 7.0));
    assert_eq!(totals(&second), (0, 0, 0.0));
    buffer.enable(&second);
    observe(&buffer);
    assert_eq!(totals(&first), (0, 0, 0.0));
    assert_eq!(totals(&second), (1, 1, 7.0));
}
