use criterion::{black_box, criterion_group, criterion_main, Criterion};
use opentelemetry::{
    global::{self, BoxedTracer},
    InstrumentationScope, KeyValue,
};
use opentelemetry_sdk::trace as sdktrace;
use std::sync::OnceLock;

#[cfg(not(target_os = "windows"))]
use pprof::criterion::{Output, PProfProfiler};

/*
Adding results in comments for a quick reference.
Apple M4 Pro
    Total Number of Cores:	14 (10 performance and 4 efficiency)

Tracer_With_Name/new_each_time  20 ns
Tracer_With_Name/reuse_existing 383 ps
Tracer_With_Name_And_Scope_Attrs/new_each_time 63 ns
Tracer_With_Name_And_Scope_Attrs/reuse_existing 385 ps
*/

fn get_tracer() -> &'static BoxedTracer {
    static TRACER: OnceLock<BoxedTracer> = OnceLock::new();
    TRACER.get_or_init(|| global::tracer("tracer"))
}

fn get_tracer_with_scope_attrs() -> &'static BoxedTracer {
    static TRACER_WITH_ATTRS: OnceLock<BoxedTracer> = OnceLock::new();
    TRACER_WITH_ATTRS.get_or_init(|| {
        let scope = InstrumentationScope::builder("tracer")
            .with_attributes([KeyValue::new("key", "value")])
            .build();
        global::tracer_with_scope(scope)
    })
}

fn create_provider() -> sdktrace::SdkTracerProvider {
    // Provider is empty, no exporters, no processors etc.
    // as the goal is measurement of tracer creation time.
    sdktrace::SdkTracerProvider::builder().build()
}

fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("Tracer_With_Name");
    group.bench_function("new_each_time", |b| {
        let provider = create_provider();
        global::set_tracer_provider(provider);
        b.iter(|| {
            black_box(global::tracer("tracer"));
        });
    });

    group.bench_function("reuse_existing", |b| {
        let provider = create_provider();
        global::set_tracer_provider(provider);
        b.iter(|| {
            black_box(get_tracer());
        });
    });

    group.finish();

    let mut group = c.benchmark_group("Tracer_With_Name_And_Scope_Attrs");
    group.bench_function("new_each_time", |b| {
        let provider = create_provider();
        global::set_tracer_provider(provider);
        b.iter(|| {
            let scope = InstrumentationScope::builder("tracer")
                .with_attributes([KeyValue::new("key", "value")])
                .build();
            black_box(global::tracer_with_scope(scope));
        });
    });

    group.bench_function("reuse_existing", |b| {
        let provider = create_provider();
        global::set_tracer_provider(provider);
        b.iter(|| {
            black_box(get_tracer_with_scope_attrs());
        });
    });
    group.finish();
}

#[cfg(not(target_os = "windows"))]
criterion_group! {
    name = benches;
    config = Criterion::default().with_profiler(PProfProfiler::new(100, Output::Flamegraph(None)))
                               .warm_up_time(std::time::Duration::from_secs(1))
                               .measurement_time(std::time::Duration::from_secs(2));
    targets = criterion_benchmark
}

#[cfg(target_os = "windows")]
criterion_group! {
    name = benches;
    config = Criterion::default().warm_up_time(std::time::Duration::from_secs(1))
                               .measurement_time(std::time::Duration::from_secs(2));
    targets = criterion_benchmark
}

criterion_main!(benches);
