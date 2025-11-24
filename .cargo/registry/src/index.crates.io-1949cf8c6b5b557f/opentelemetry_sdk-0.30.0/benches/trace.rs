use criterion::{criterion_group, criterion_main, Criterion};
use opentelemetry::{
    trace::{Span, Tracer, TracerProvider},
    KeyValue,
};
use opentelemetry_sdk::{
    error::OTelSdkResult,
    trace::{self as sdktrace, SpanData, SpanExporter},
};
#[cfg(not(target_os = "windows"))]
use pprof::criterion::{Output, PProfProfiler};

fn criterion_benchmark(c: &mut Criterion) {
    trace_benchmark_group(c, "start-end-span", |tracer| tracer.start("foo").end());

    trace_benchmark_group(c, "start-end-span-4-attrs", |tracer| {
        let mut span = tracer.start("foo");
        span.set_attribute(KeyValue::new("key1", false));
        span.set_attribute(KeyValue::new("key2", "hello"));
        span.set_attribute(KeyValue::new("key4", 123.456));
        span.end();
    });

    trace_benchmark_group(c, "start-end-span-8-attrs", |tracer| {
        let mut span = tracer.start("foo");
        span.set_attribute(KeyValue::new("key1", false));
        span.set_attribute(KeyValue::new("key2", "hello"));
        span.set_attribute(KeyValue::new("key4", 123.456));
        span.set_attribute(KeyValue::new("key11", false));
        span.set_attribute(KeyValue::new("key12", "hello"));
        span.set_attribute(KeyValue::new("key14", 123.456));
        span.end();
    });

    trace_benchmark_group(c, "start-end-span-all-attr-types", |tracer| {
        let mut span = tracer.start("foo");
        span.set_attribute(KeyValue::new("key1", false));
        span.set_attribute(KeyValue::new("key2", "hello"));
        span.set_attribute(KeyValue::new("key3", 123));
        span.set_attribute(KeyValue::new("key5", 123.456));
        span.end();
    });

    trace_benchmark_group(c, "start-end-span-all-attr-types-2x", |tracer| {
        let mut span = tracer.start("foo");
        span.set_attribute(KeyValue::new("key1", false));
        span.set_attribute(KeyValue::new("key2", "hello"));
        span.set_attribute(KeyValue::new("key3", 123));
        span.set_attribute(KeyValue::new("key5", 123.456));
        span.set_attribute(KeyValue::new("key11", false));
        span.set_attribute(KeyValue::new("key12", "hello"));
        span.set_attribute(KeyValue::new("key13", 123));
        span.set_attribute(KeyValue::new("key15", 123.456));
        span.end();
    });
}

#[derive(Debug)]
struct VoidExporter;

impl SpanExporter for VoidExporter {
    async fn export(&self, _spans: Vec<SpanData>) -> OTelSdkResult {
        Ok(())
    }
}

fn trace_benchmark_group<F: Fn(&sdktrace::SdkTracer)>(c: &mut Criterion, name: &str, f: F) {
    let mut group = c.benchmark_group(name);

    group.bench_function("always-sample", |b| {
        let provider = sdktrace::SdkTracerProvider::builder()
            .with_sampler(sdktrace::Sampler::AlwaysOn)
            .with_simple_exporter(VoidExporter)
            .build();
        let always_sample = provider.tracer("always-sample");

        b.iter(|| f(&always_sample));
    });

    group.bench_function("never-sample", |b| {
        let provider = sdktrace::SdkTracerProvider::builder()
            .with_sampler(sdktrace::Sampler::AlwaysOff)
            .with_simple_exporter(VoidExporter)
            .build();
        let never_sample = provider.tracer("never-sample");
        b.iter(|| f(&never_sample));
    });

    group.finish();
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
