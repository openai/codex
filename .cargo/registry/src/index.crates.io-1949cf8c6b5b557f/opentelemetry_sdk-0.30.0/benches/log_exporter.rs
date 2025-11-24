/*
    The benchmark results:
    criterion = "0.5.1"
    OS: Ubuntu 22.04.3 LTS (5.15.146.1-microsoft-standard-WSL2)
    Hardware: AMD EPYC 7763 64-Core Processor - 2.44 GHz, 16vCPUs,
    RAM: 64.0 GB
    | Test                           | Average time|
    |--------------------------------|-------------|
    | LogExporterWithFuture          | 111 ns      |
    | LogExporterWithoutFuture       | 92 ns      |
*/

use opentelemetry::time::now;
use opentelemetry_sdk::error::OTelSdkResult;
use std::sync::Mutex;

use criterion::{criterion_group, criterion_main, Criterion};

use opentelemetry::logs::{LogRecord as _, Logger, LoggerProvider, Severity};
use opentelemetry::InstrumentationScope;
use opentelemetry_sdk::logs::LogBatch;
use opentelemetry_sdk::logs::LogProcessor;
use opentelemetry_sdk::logs::SdkLogRecord;
use opentelemetry_sdk::logs::SdkLoggerProvider;
#[cfg(not(target_os = "windows"))]
use pprof::criterion::{Output, PProfProfiler};
use std::fmt::Debug;

// Run this benchmark with:
// cargo bench --bench log_exporter
pub trait LogExporterWithFuture: Send + Sync + Debug {
    fn export(&mut self, batch: LogBatch<'_>) -> impl std::future::Future<Output = ()> + Send;
}

pub trait LogExporterWithoutFuture: Send + Sync + Debug {
    fn export(&mut self, batch: LogBatch<'_>);
}

#[derive(Debug)]
struct NoOpExporterWithFuture {}

impl LogExporterWithFuture for NoOpExporterWithFuture {
    async fn export(&mut self, _batch: LogBatch<'_>) {}
}

#[derive(Debug)]
struct NoOpExporterWithoutFuture {}
impl LogExporterWithoutFuture for NoOpExporterWithoutFuture {
    fn export(&mut self, _batch: LogBatch<'_>) {}
}

#[derive(Debug)]
struct ExportingProcessorWithFuture {
    exporter: Mutex<NoOpExporterWithFuture>,
}

impl ExportingProcessorWithFuture {
    fn new(exporter: NoOpExporterWithFuture) -> Self {
        ExportingProcessorWithFuture {
            exporter: Mutex::new(exporter),
        }
    }
}

impl LogProcessor for ExportingProcessorWithFuture {
    fn emit(&self, record: &mut SdkLogRecord, scope: &InstrumentationScope) {
        let mut exporter = self.exporter.lock().expect("lock error");
        let logs = [(record as &SdkLogRecord, scope)];
        futures_executor::block_on(exporter.export(LogBatch::new(&logs)));
    }

    fn force_flush(&self) -> OTelSdkResult {
        Ok(())
    }

    fn shutdown(&self) -> OTelSdkResult {
        Ok(())
    }
}

#[derive(Debug)]
struct ExportingProcessorWithoutFuture {
    exporter: Mutex<NoOpExporterWithoutFuture>,
}

impl ExportingProcessorWithoutFuture {
    fn new(exporter: NoOpExporterWithoutFuture) -> Self {
        ExportingProcessorWithoutFuture {
            exporter: Mutex::new(exporter),
        }
    }
}

impl LogProcessor for ExportingProcessorWithoutFuture {
    fn emit(&self, record: &mut SdkLogRecord, scope: &InstrumentationScope) {
        let logs = [(record as &SdkLogRecord, scope)];
        self.exporter
            .lock()
            .expect("lock error")
            .export(LogBatch::new(&logs));
    }

    fn force_flush(&self) -> OTelSdkResult {
        Ok(())
    }

    fn shutdown(&self) -> OTelSdkResult {
        Ok(())
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    exporter_with_future(c);
    exporter_without_future(c);
}

fn exporter_with_future(c: &mut Criterion) {
    let provider = SdkLoggerProvider::builder()
        .with_log_processor(ExportingProcessorWithFuture::new(NoOpExporterWithFuture {}))
        .build();
    let logger = provider.logger("benchmark");

    c.bench_function("LogExporterWithFuture", |b| {
        b.iter(|| {
            let mut log_record = logger.create_log_record();
            let now = now();
            log_record.set_observed_timestamp(now);
            log_record.set_target("my-target".to_string());
            log_record.set_event_name("CheckoutFailed");
            log_record.set_severity_number(Severity::Warn);
            log_record.set_severity_text("WARN");
            log_record.add_attribute("book_id", "12345");
            log_record.add_attribute("book_title", "Rust Programming Adventures");
            log_record.add_attribute("message", "Unable to process checkout.");

            logger.emit(log_record);
        });
    });
}

fn exporter_without_future(c: &mut Criterion) {
    let provider = SdkLoggerProvider::builder()
        .with_log_processor(ExportingProcessorWithoutFuture::new(
            NoOpExporterWithoutFuture {},
        ))
        .build();
    let logger = provider.logger("benchmark");

    c.bench_function("LogExporterWithoutFuture", |b| {
        b.iter(|| {
            let mut log_record = logger.create_log_record();
            let now = now();
            log_record.set_observed_timestamp(now);
            log_record.set_target("my-target".to_string());
            log_record.set_event_name("CheckoutFailed");
            log_record.set_severity_number(Severity::Warn);
            log_record.set_severity_text("WARN");
            log_record.add_attribute("book_id", "12345");
            log_record.add_attribute("book_title", "Rust Programming Adventures");
            log_record.add_attribute("message", "Unable to process checkout.");

            logger.emit(log_record);
        });
    });
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
