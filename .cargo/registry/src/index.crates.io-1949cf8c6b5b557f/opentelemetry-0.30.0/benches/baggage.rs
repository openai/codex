use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use opentelemetry::baggage::Baggage;
use rand::distr::{Alphanumeric, SampleString};
use rand::Rng;

const MAX_KEY_VALUE_PAIRS: usize = 64;

// Run this benchmark with:
// cargo bench --bench baggage
// Adding results in comments for a quick reference.
// Apple M4 Pro
//     Total Number of Cores:	14 (10 performance and 4 efficiency)
// Results:
// set_baggage_static_key_value 14 ns
// set_baggage_static_key 26 ns
// set_baggage_dynamic 54 ns
// set_baggage_dynamic_with_metadata 75 ns

fn criterion_benchmark(c: &mut Criterion) {
    set_baggage_static_key_value(c);
    set_baggage_static_key(c);
    set_baggage_dynamic(c);
    set_baggage_dynamic_with_metadata(c);
}

fn set_baggage_static_key_value(c: &mut Criterion) {
    let mut baggage = Baggage::new();

    c.bench_function("set_baggage_static_key_value", move |b| {
        b.iter(|| {
            baggage.insert("key", "value");
        })
    });
}

fn set_baggage_static_key(c: &mut Criterion) {
    let mut baggage = Baggage::new();

    c.bench_function("set_baggage_static_key", move |b| {
        b.iter(|| {
            baggage.insert("key", "value".to_string());
        })
    });
}

fn set_baggage_dynamic(c: &mut Criterion) {
    let mut baggage = Baggage::new();

    let mut rng = rand::rng();
    let key_value = (0..MAX_KEY_VALUE_PAIRS)
        .map(|_| {
            (
                Alphanumeric.sample_string(&mut rng, 4),
                Alphanumeric.sample_string(&mut rng, 4),
            )
        })
        .collect::<Vec<(String, String)>>();

    c.bench_function("set_baggage_dynamic", move |b| {
        b.iter_batched(
            || rng.random_range(0..MAX_KEY_VALUE_PAIRS),
            |idx| {
                let (key, value) = key_value[idx].clone();
                baggage.insert(key, value);
            },
            BatchSize::SmallInput,
        )
    });
}

fn set_baggage_dynamic_with_metadata(c: &mut Criterion) {
    let mut baggage = Baggage::new();

    let mut rng = rand::rng();
    let key_value_metadata = (0..MAX_KEY_VALUE_PAIRS)
        .map(|_| {
            (
                Alphanumeric.sample_string(&mut rng, 4),
                Alphanumeric.sample_string(&mut rng, 4),
                Alphanumeric.sample_string(&mut rng, 4),
            )
        })
        .collect::<Vec<(String, String, String)>>();

    c.bench_function("set_baggage_dynamic_with_metadata", move |b| {
        b.iter_batched(
            || rng.random_range(0..MAX_KEY_VALUE_PAIRS),
            |idx| {
                let (key, value, metadata) = key_value_metadata[idx].clone();
                baggage.insert_with_metadata(key, value, metadata);
            },
            BatchSize::SmallInput,
        )
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
