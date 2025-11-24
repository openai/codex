//! run with `$ cargo bench --bench log -- --exact <test_name>` to run specific test for logs
//! So to run test named "full-log-with-attributes/with-context" you would run `$ cargo bench --bench log -- --exact full-log-with-attributes/with-context`
//! To run all tests for logs you would run `$ cargo bench --bench log`
//!
/*
The benchmark results:
criterion = "0.5.1"
OS: Ubuntu 22.04.3 LTS (5.15.146.1-microsoft-standard-WSL2)
Hardware: AMD EPYC 7763 64-Core Processor - 2.44 GHz, 16vCPUs,
RAM: 64.0 GB
| Test                           | Average time|
|--------------------------------|-------------|
| Logger_Creation                |  30 ns      |
| LoggerProvider_Creation        | 909 ns      |
| Logging_Comparable_To_Appender | 87 ns       |
*/

use opentelemetry::time::now;
use std::collections::HashMap;

use criterion::{criterion_group, criterion_main, Criterion};

use opentelemetry::logs::{AnyValue, LogRecord as _, Logger, LoggerProvider, Severity};
use opentelemetry::trace::Tracer;
use opentelemetry::trace::TracerProvider;
use opentelemetry::{InstrumentationScope, Key};
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::logs::{LogProcessor, SdkLogRecord, SdkLogger, SdkLoggerProvider};
use opentelemetry_sdk::trace::{Sampler, SdkTracerProvider};

#[derive(Debug)]
struct NoopProcessor;

impl LogProcessor for NoopProcessor {
    fn emit(&self, _data: &mut SdkLogRecord, _scope: &InstrumentationScope) {}

    fn force_flush(&self) -> OTelSdkResult {
        Ok(())
    }

    fn shutdown(&self) -> OTelSdkResult {
        Ok(())
    }
}

fn log_benchmark_group<F: Fn(&SdkLogger)>(c: &mut Criterion, name: &str, f: F) {
    let mut group = c.benchmark_group(name);

    group.bench_function("no-context", |b| {
        let provider = SdkLoggerProvider::builder()
            .with_log_processor(NoopProcessor {})
            .build();

        let logger = provider.logger("no-context");

        b.iter(|| f(&logger));
    });

    group.bench_function("with-context", |b| {
        let provider = SdkLoggerProvider::builder()
            .with_log_processor(NoopProcessor {})
            .build();

        let logger = provider.logger("with-context");

        // setup tracing as well.
        let tracer_provider = SdkTracerProvider::builder()
            .with_sampler(Sampler::AlwaysOn)
            .build();
        let tracer = tracer_provider.tracer("bench-tracer");

        // Act
        tracer.in_span("bench-span", |_cx| {
            b.iter(|| f(&logger));
        });
    });

    group.finish();
}

fn log_provider_creation(c: &mut Criterion) {
    c.bench_function("LoggerProvider_Creation", |b| {
        b.iter(|| {
            let _provider = SdkLoggerProvider::builder()
                .with_log_processor(NoopProcessor {})
                .build();
        });
    });
}

fn logger_creation(c: &mut Criterion) {
    // Provider is created once, outside of the benchmark
    let provider = SdkLoggerProvider::builder()
        .with_log_processor(NoopProcessor {})
        .build();

    c.bench_function("Logger_Creation", |b| {
        b.iter(|| {
            let _logger = provider.logger("benchmark");
        });
    });
}

fn logging_comparable_to_appender(c: &mut Criterion) {
    let provider = SdkLoggerProvider::builder()
        .with_log_processor(NoopProcessor {})
        .build();
    let logger = provider.logger("benchmark");

    // This mimics the logic from opentelemetry-tracing-appender closely, but
    // without the overhead of the tracing layer itself.
    c.bench_function("Logging_Comparable_To_Appender", |b| {
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

fn criterion_benchmark(c: &mut Criterion) {
    logger_creation(c);
    log_provider_creation(c);
    logging_comparable_to_appender(c);
    log_benchmark_group(c, "simple-log", |logger| {
        let mut log_record = logger.create_log_record();
        log_record.set_body("simple log".into());
        logger.emit(log_record);
    });

    log_benchmark_group(c, "simple-log-with-int", |logger| {
        let mut log_record = logger.create_log_record();
        log_record.set_body("simple log".into());
        log_record.add_attribute("testint", 2);
        logger.emit(log_record);
    });

    log_benchmark_group(c, "simple-log-with-double", |logger| {
        let mut log_record = logger.create_log_record();
        log_record.set_body("simple log".into());
        log_record.add_attribute("testdouble", 2.2);
        logger.emit(log_record);
    });

    log_benchmark_group(c, "simple-log-with-string", |logger| {
        let mut log_record = logger.create_log_record();
        log_record.set_body("simple log".into());
        log_record.add_attribute("teststring", "test");
        logger.emit(log_record);
    });

    log_benchmark_group(c, "simple-log-with-bool", |logger| {
        let mut log_record = logger.create_log_record();
        log_record.set_body("simple log".into());
        log_record.add_attribute("testbool", AnyValue::Boolean(true));
        logger.emit(log_record);
    });

    let bytes = AnyValue::Bytes(Box::new(vec![25u8, 30u8, 40u8]));
    log_benchmark_group(c, "simple-log-with-bytes", |logger| {
        let mut log_record = logger.create_log_record();
        log_record.set_body("simple log".into());
        log_record.add_attribute("testbytes", bytes.clone());
        logger.emit(log_record);
    });

    let bytes = AnyValue::Bytes(Box::new(vec![
        25u8, 30u8, 40u8, 30u8, 40u8, 30u8, 40u8, 30u8, 40u8, 30u8, 40u8, 30u8, 40u8, 30u8, 40u8,
        30u8, 40u8, 30u8, 40u8, 30u8, 40u8, 30u8, 40u8, 30u8, 40u8, 30u8, 40u8, 30u8, 40u8, 30u8,
        40u8, 30u8, 40u8, 30u8, 40u8, 30u8, 40u8, 30u8, 40u8, 30u8, 40u8, 30u8, 40u8, 30u8, 40u8,
        30u8, 40u8, 30u8, 40u8, 30u8, 40u8, 30u8, 40u8, 30u8, 40u8, 30u8, 40u8, 30u8, 40u8, 30u8,
        40u8, 30u8, 40u8, 30u8, 40u8, 30u8, 40u8, 30u8, 40u8, 30u8, 40u8, 30u8, 40u8, 30u8, 40u8,
        30u8, 40u8, 30u8, 40u8, 30u8, 40u8, 30u8, 40u8, 30u8, 40u8,
    ]));
    log_benchmark_group(c, "simple-log-with-a-lot-of-bytes", |logger| {
        let mut log_record = logger.create_log_record();
        log_record.set_body("simple log".into());
        log_record.add_attribute("testbytes", bytes.clone());
        logger.emit(log_record);
    });

    let vec_any_values = AnyValue::ListAny(Box::new(vec![
        AnyValue::Int(25),
        "test".into(),
        true.into(),
    ]));
    log_benchmark_group(c, "simple-log-with-vec-any-value", |logger| {
        let mut log_record = logger.create_log_record();
        log_record.set_body("simple log".into());
        log_record.add_attribute("testvec", vec_any_values.clone());
        logger.emit(log_record);
    });

    let vec_any_values = AnyValue::ListAny(Box::new(vec![
        AnyValue::Int(25),
        "test".into(),
        true.into(),
    ]));
    let vec_any_values = AnyValue::ListAny(Box::new(vec![
        AnyValue::Int(25),
        "test".into(),
        true.into(),
        vec_any_values,
    ]));
    log_benchmark_group(c, "simple-log-with-inner-vec-any-value", |logger| {
        let mut log_record = logger.create_log_record();
        log_record.set_body("simple log".into());
        log_record.add_attribute("testvec", vec_any_values.clone());
        logger.emit(log_record);
    });

    let map_any_values = AnyValue::Map(Box::new(HashMap::from([
        ("testint".into(), 2.into()),
        ("testdouble".into(), 2.2.into()),
        ("teststring".into(), "test".into()),
    ])));
    log_benchmark_group(c, "simple-log-with-map-any-value", |logger| {
        let mut log_record = logger.create_log_record();
        log_record.set_body("simple log".into());
        log_record.add_attribute("testmap", map_any_values.clone());
        logger.emit(log_record);
    });

    let map_any_values = AnyValue::Map(Box::new(HashMap::from([
        ("testint".into(), 2.into()),
        ("testdouble".into(), 2.2.into()),
        ("teststring".into(), "test".into()),
    ])));
    let map_any_values = AnyValue::Map(Box::new(HashMap::from([
        ("testint".into(), 2.into()),
        ("testdouble".into(), 2.2.into()),
        ("teststring".into(), "test".into()),
        ("testmap".into(), map_any_values),
    ])));
    log_benchmark_group(c, "simple-log-with-inner-map-any-value", |logger| {
        let mut log_record = logger.create_log_record();
        log_record.set_body("simple log".into());
        log_record.add_attribute("testmap", map_any_values.clone());
        logger.emit(log_record);
    });

    log_benchmark_group(c, "long-log", |logger| {
        let mut log_record = logger.create_log_record();
        log_record.set_body("Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Gravida in fermentum et sollicitudin ac orci phasellus. Ullamcorper dignissim cras tincidunt lobortis feugiat vivamus at augue. Magna etiam tempor orci eu. Sed tempus urna et pharetra pharetra massa.".into());
        logger.emit(log_record);
    });

    let now = now();
    log_benchmark_group(c, "full-log", |logger| {
        let mut log_record = logger.create_log_record();
        log_record.set_body("full log".into());
        log_record.set_timestamp(now);
        log_record.set_observed_timestamp(now);
        log_record.set_severity_number(Severity::Warn);
        log_record.set_severity_text(Severity::Warn.name());
        logger.emit(log_record);
    });

    log_benchmark_group(c, "full-log-with-4-attributes", |logger| {
        let mut log_record = logger.create_log_record();
        log_record.set_body("full log".into());
        log_record.set_timestamp(now);
        log_record.set_observed_timestamp(now);
        log_record.set_severity_number(Severity::Warn);
        log_record.set_severity_text(Severity::Warn.name());
        log_record.add_attribute("name", "my-event-name");
        log_record.add_attribute("event.id", 20);
        log_record.add_attribute("user.name", "otel");
        log_record.add_attribute("user.email", "otel@opentelemetry.io");
        logger.emit(log_record);
    });

    log_benchmark_group(c, "full-log-with-9-attributes", |logger| {
        let mut log_record = logger.create_log_record();
        log_record.set_body("full log".into());
        log_record.set_timestamp(now);
        log_record.set_observed_timestamp(now);
        log_record.set_severity_number(Severity::Warn);
        log_record.set_severity_text(Severity::Warn.name());
        log_record.add_attribute("name", "my-event-name");
        log_record.add_attribute("event.id", 20);
        log_record.add_attribute("user.name", "otel");
        log_record.add_attribute("user.email", "otel@opentelemetry.io");
        log_record.add_attribute("code.filename", "log.rs");
        log_record.add_attribute("code.filepath", "opentelemetry_sdk/benches/log.rs");
        log_record.add_attribute("code.lineno", 96);
        log_record.add_attribute("code.namespace", "opentelemetry_sdk::benches::log");
        log_record.add_attribute("log.target", "opentelemetry_sdk::benches::log");
        logger.emit(log_record);
    });

    let attributes: Vec<(Key, AnyValue)> = vec![
        ("name".into(), "my-event-name".into()),
        ("event-id".into(), 20.into()),
        ("user.name".into(), "otel".into()),
        ("user.email".into(), "otel@opentelemetry.io".into()),
        ("code.filename".into(), "log.rs".into()),
        (
            "code.filepath".into(),
            "opentelemetry_sdk/benches/log.rs".into(),
        ),
        ("code.lineno".into(), 96.into()),
        (
            "code.namespace".into(),
            "opentelemetry_sdk::benches::log".into(),
        ),
        (
            "log.target".into(),
            "opentelemetry_sdk::benches::log".into(),
        ),
    ];
    log_benchmark_group(c, "full-log-with-attributes", |logger| {
        let mut log_record = logger.create_log_record();
        log_record.set_body("full log".into());
        log_record.set_timestamp(now);
        log_record.set_observed_timestamp(now);
        log_record.set_severity_number(Severity::Warn);
        log_record.set_severity_text(Severity::Warn.name());
        log_record.add_attributes(attributes.clone());
        logger.emit(log_record);
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(std::time::Duration::from_secs(1))
        .measurement_time(std::time::Duration::from_secs(2));
    targets = criterion_benchmark
}
criterion_main!(benches);
