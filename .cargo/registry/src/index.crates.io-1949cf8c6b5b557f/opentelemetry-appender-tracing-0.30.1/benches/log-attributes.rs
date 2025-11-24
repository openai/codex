/*
// Run this benchmark with:
 // cargo bench --bench log-attributes
 // Adding results in comments for a quick reference.
 // Apple M4 Pro
 //     Total Number of Cores:	14 (10 performance and 4 efficiency)

| Test                 | Average time | Increment |
|----------------------|--------------|-----------|
| otel_0_attributes    | 72 ns        | -         |
| otel_1_attributes    | 117 ns       | +45 ns    |
| otel_2_attributes    | 155 ns       | +38 ns    |
| otel_3_attributes    | 196 ns       | +41 ns    |
| otel_4_attributes    | 240 ns       | +44 ns    |
| otel_5_attributes    | 278 ns       | +38 ns    |
| otel_6_attributes    | 346 ns       | +68 ns    | // Array is full. 6th attribute causes vec! to be allocated
| otel_7_attributes    | 390 ns       | +44 ns    |
| otel_8_attributes    | 431 ns       | +41 ns    |
| otel_9_attributes    | 480 ns       | +49 ns    |
| otel_10_attributes   | 519 ns       | +39 ns    |
| otel_11_attributes   | 625 ns       | +106 ns   | // vec! initial capacity is 5. 11th attribute causes vec! to be reallocated
| otel_12_attributes   | 676 ns       | +51 ns    |
*/

use criterion::{criterion_group, criterion_main, Criterion};
use opentelemetry::InstrumentationScope;
use opentelemetry_appender_tracing::layer as tracing_layer;
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::logs::{LogProcessor, SdkLogRecord, SdkLoggerProvider};
use opentelemetry_sdk::Resource;
#[cfg(not(target_os = "windows"))]
use pprof::criterion::{Output, PProfProfiler};
use tracing::error;
use tracing_subscriber::prelude::*;
use tracing_subscriber::Registry;

#[derive(Debug)]
struct NoopProcessor;

impl LogProcessor for NoopProcessor {
    fn emit(&self, _: &mut SdkLogRecord, _: &InstrumentationScope) {}

    fn force_flush(&self) -> OTelSdkResult {
        Ok(())
    }
}

/// Creates a single benchmark for a specific number of attributes
fn create_benchmark(c: &mut Criterion, num_attributes: usize) {
    let provider = SdkLoggerProvider::builder()
        .with_resource(
            Resource::builder_empty()
                .with_service_name("benchmark")
                .build(),
        )
        .with_log_processor(NoopProcessor)
        .build();

    let ot_layer = tracing_layer::OpenTelemetryTracingBridge::new(&provider);
    let subscriber = Registry::default().with(ot_layer);

    tracing::subscriber::with_default(subscriber, || {
        c.bench_function(&format!("otel_{}_attributes", num_attributes), |b| {
            b.iter(|| {
                // Dynamically generate the error! macro call based on the number of attributes
                match num_attributes {
                    0 => {
                        error!(
                            name : "CheckoutFailed",
                            message = "Unable to process checkout."
                        );
                    }
                    1 => {
                        error!(
                            name : "CheckoutFailed",
                            field1 = "field1",
                            message = "Unable to process checkout."
                        );
                    }
                    2 => {
                        error!(
                            name : "CheckoutFailed",
                            field1 = "field1",
                            field2 = "field2",
                            message = "Unable to process checkout."
                        );
                    }
                    3 => {
                        error!(
                            name : "CheckoutFailed",
                            field1 = "field1",
                            field2 = "field2",
                            field3 = "field3",
                            message = "Unable to process checkout."
                        );
                    }
                    4 => {
                        error!(
                            name : "CheckoutFailed",
                            field1 = "field1",
                            field2 = "field2",
                            field3 = "field3",
                            field4 = "field4",
                            message = "Unable to process checkout."
                        );
                    }
                    5 => {
                        error!(
                            name : "CheckoutFailed",
                            field1 = "field1",
                            field2 = "field2",
                            field3 = "field3",
                            field4 = "field4",
                            field5 = "field5",
                            message = "Unable to process checkout."
                        );
                    }
                    6 => {
                        error!(
                            name : "CheckoutFailed",
                            field1 = "field1",
                            field2 = "field2",
                            field3 = "field3",
                            field4 = "field4",
                            field5 = "field5",
                            field6 = "field6",
                            message = "Unable to process checkout."
                        );
                    }
                    7 => {
                        error!(
                            name : "CheckoutFailed",
                            field1 = "field1",
                            field2 = "field2",
                            field3 = "field3",
                            field4 = "field4",
                            field5 = "field5",
                            field6 = "field6",
                            field7 = "field7",
                            message = "Unable to process checkout."
                        );
                    }
                    8 => {
                        error!(
                            name : "CheckoutFailed",
                            field1 = "field1",
                            field2 = "field2",
                            field3 = "field3",
                            field4 = "field4",
                            field5 = "field5",
                            field6 = "field6",
                            field7 = "field7",
                            field8 = "field8",
                            message = "Unable to process checkout."
                        );
                    }
                    9 => {
                        error!(
                            name : "CheckoutFailed",
                            field1 = "field1",
                            field2 = "field2",
                            field3 = "field3",
                            field4 = "field4",
                            field5 = "field5",
                            field6 = "field6",
                            field7 = "field7",
                            field8 = "field8",
                            field9 = "field9",
                            message = "Unable to process checkout."
                        );
                    }
                    10 => {
                        error!(
                            name : "CheckoutFailed",
                            field1 = "field1",
                            field2 = "field2",
                            field3 = "field3",
                            field4 = "field4",
                            field5 = "field5",
                            field6 = "field6",
                            field7 = "field7",
                            field8 = "field8",
                            field9 = "field9",
                            field10 = "field10",
                            message = "Unable to process checkout."
                        );
                    }
                    11 => {
                        error!(
                            name : "CheckoutFailed",
                            field1 = "field1",
                            field2 = "field2",
                            field3 = "field3",
                            field4 = "field4",
                            field5 = "field5",
                            field6 = "field6",
                            field7 = "field7",
                            field8 = "field8",
                            field9 = "field9",
                            field10 = "field10",
                            field11 = "field11",
                            message = "Unable to process checkout."
                        );
                    }
                    12 => {
                        error!(
                            name : "CheckoutFailed",
                            field1 = "field1",
                            field2 = "field2",
                            field3 = "field3",
                            field4 = "field4",
                            field5 = "field5",
                            field6 = "field6",
                            field7 = "field7",
                            field8 = "field8",
                            field9 = "field9",
                            field10 = "field10",
                            field11 = "field11",
                            field12 = "field12",
                            message = "Unable to process checkout."
                        );
                    }
                    _ => {
                        // Fall back to 10 attributes for any higher number
                        error!(
                            name : "CheckoutFailed",
                            field1 = "field1",
                            field2 = "field2",
                            field3 = "field3",
                            field4 = "field4",
                            field5 = "field5",
                            field6 = "field6",
                            field7 = "field7",
                            field8 = "field8",
                            field9 = "field9",
                            field10 = "field10",
                            message = "Unable to process checkout."
                        );
                    }
                }
            });
        });
    });
}

fn criterion_benchmark(c: &mut Criterion) {
    create_benchmark(c, 2);
    // Run benchmarks for 0 to 12 attributes
    // for num_attributes in 0..=12 {
    //     create_benchmark(c, 2);
    // }
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
