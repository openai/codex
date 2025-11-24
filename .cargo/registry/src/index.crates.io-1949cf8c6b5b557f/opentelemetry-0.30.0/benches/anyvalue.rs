use criterion::{black_box, criterion_group, criterion_main, Criterion};
use opentelemetry::{logs::AnyValue, Value};

// Run this benchmark with:
// cargo bench --bench anyvalue
// Results:
// CreateOTelValueString 1-2 ns
// CreateOTelAnyValueString 15 ns
// CreateOTelValueInt 1-2 ns
// CreateOTelAnyValueInt 15 ns

fn criterion_benchmark(c: &mut Criterion) {
    attributes_creation(c);
}

fn attributes_creation(c: &mut Criterion) {
    c.bench_function("CreateOTelValueString", |b| {
        b.iter(|| {
            let _v = black_box(Value::String("value1".into()));
        });
    });

    c.bench_function("CreateOTelAnyValueString", |b| {
        b.iter(|| {
            let _v = black_box(AnyValue::String("value1".into()));
        });
    });

    c.bench_function("CreateOTelValueInt", |b| {
        b.iter(|| {
            let _v = black_box(Value::I64(123));
        });
    });

    c.bench_function("CreateOTelAnyValueInt", |b| {
        b.iter(|| {
            let _v = black_box(AnyValue::Int(123));
        });
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
