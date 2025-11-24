/*
    The benchmark results:
    criterion = "0.5.1"
    Hardware: Apple M4 Pro
    Total Number of Cores:   14 (10 performance and 4 efficiency)
    | Test                                        | Average time|
    |---------------------------------------------|-------------|
    | exporter_disabled_concurrent_processor      |  2.5 ns     |
    | exporter_disabled_simple_processor          |  5.3 ns     |
*/

// cargo bench --bench log_enabled --features="spec_unstable_logs_enabled,experimental_logs_concurrent_log_processor"

use criterion::{criterion_group, criterion_main, Criterion};
use opentelemetry::logs::{Logger, LoggerProvider};
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::logs::concurrent_log_processor::SimpleConcurrentLogProcessor;
use opentelemetry_sdk::logs::{
    LogBatch, LogExporter, LogProcessor, SdkLoggerProvider, SimpleLogProcessor,
};
use opentelemetry_sdk::Resource;
#[cfg(not(target_os = "windows"))]
use pprof::criterion::{Output, PProfProfiler};

#[derive(Debug)]
struct NoopExporter;
impl LogExporter for NoopExporter {
    async fn export(&self, _: LogBatch<'_>) -> OTelSdkResult {
        Ok(())
    }

    #[inline]
    fn event_enabled(
        &self,
        _level: opentelemetry::logs::Severity,
        _target: &str,
        _name: Option<&str>,
    ) -> bool {
        false
    }

    fn set_resource(&mut self, _: &Resource) {}
}

fn benchmark_exporter_enabled_false<T>(c: &mut Criterion, name: &str, processor: T)
where
    T: LogProcessor + Send + Sync + 'static,
{
    let provider = SdkLoggerProvider::builder()
        .with_log_processor(processor)
        .build();
    let logger = provider.logger("test_logger");

    c.bench_function(name, |b| {
        b.iter(|| {
            criterion::black_box(logger.event_enabled(
                opentelemetry::logs::Severity::Debug,
                "target",
                Some("name"),
            ));
        });
    });
}

fn criterion_benchmark(c: &mut Criterion) {
    let processor = SimpleConcurrentLogProcessor::new(NoopExporter);
    benchmark_exporter_enabled_false(c, "exporter_disabled_concurrent_processor", processor);
    let simple = SimpleLogProcessor::new(NoopExporter);
    benchmark_exporter_enabled_false(c, "exporter_disabled_simple_processor", simple);
}

#[cfg(not(target_os = "windows"))]
criterion_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(std::time::Duration::from_secs(1))
        .measurement_time(std::time::Duration::from_secs(2))
        .with_profiler(PProfProfiler::new(100, Output::Flamegraph(None)));
    targets = criterion_benchmark
}
#[cfg(target_os = "windows")]
criterion_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(std::time::Duration::from_secs(1))
        .measurement_time(std::time::Duration::from_secs(2));
    targets = criterion_benchmark
}
criterion_main!(benches);
