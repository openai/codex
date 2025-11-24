/*
    The benchmark results:
    criterion = "0.5.1"
    rustc 1.83.0 (90b35a623 2024-11-26)
    OS: Ubuntu 22.04.4 LTS (5.15.167.4-microsoft-standard-WSL2)
    Hardware: Intel(R) Xeon(R) Platinum 8370C CPU @ 2.80GHz   2.79 GHz
    RAM: 64.0 GB
    | Test                                                  | Average time|
    |-------------------------------------------------------|-------------|
    | Histogram_Record                                      | 186.24 ns   |
    | Histogram_Record_With_Non_Static_Values               | 264.70 ns   |

*/

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use opentelemetry::{
    metrics::{Histogram, MeterProvider as _},
    KeyValue,
};
use opentelemetry_sdk::metrics::{ManualReader, SdkMeterProvider};
#[cfg(not(target_os = "windows"))]
use pprof::criterion::{Output, PProfProfiler};
use rand::{
    rngs::{self},
    Rng, SeedableRng,
};
use std::cell::RefCell;

thread_local! {
    /// Store random number generator for each thread
    static CURRENT_RNG: RefCell<rngs::SmallRng> = RefCell::new(rngs::SmallRng::from_os_rng());
}

static ATTRIBUTE_VALUES: [&str; 10] = [
    "value1", "value2", "value3", "value4", "value5", "value6", "value7", "value8", "value9",
    "value10",
];

// Run this benchmark with:
// cargo bench --bench metrics_histogram
fn create_histogram(name: &'static str) -> Histogram<u64> {
    let meter_provider: SdkMeterProvider = SdkMeterProvider::builder()
        .with_reader(ManualReader::builder().build())
        .build();
    let meter = meter_provider.meter("benchmarks");

    meter.u64_histogram(name).build()
}

fn criterion_benchmark(c: &mut Criterion) {
    histogram_record(c);

    let attribute_values: [String; 10] = (1..=10)
        .map(|i| format!("value{}", i))
        .collect::<Vec<String>>()
        .try_into()
        .expect("Expected a Vec of length 10");

    histogram_record_with_non_static_values(c, attribute_values);
}

fn histogram_record(c: &mut Criterion) {
    let histogram = create_histogram("Histogram_Record");
    c.bench_function("Histogram_Record", |b| {
        b.iter_batched(
            || {
                // 4*4*10*10 = 1600 time series.
                CURRENT_RNG.with(|rng| {
                    let mut rng = rng.borrow_mut();
                    [
                        rng.random_range(0..4),
                        rng.random_range(0..4),
                        rng.random_range(0..10),
                        rng.random_range(0..10),
                    ]
                })
            },
            |rands| {
                let index_first_attribute = rands[0];
                let index_second_attribute = rands[1];
                let index_third_attribute = rands[2];
                let index_fourth_attribute = rands[3];
                histogram.record(
                    1,
                    &[
                        KeyValue::new("attribute1", ATTRIBUTE_VALUES[index_first_attribute]),
                        KeyValue::new("attribute2", ATTRIBUTE_VALUES[index_second_attribute]),
                        KeyValue::new("attribute3", ATTRIBUTE_VALUES[index_third_attribute]),
                        KeyValue::new("attribute4", ATTRIBUTE_VALUES[index_fourth_attribute]),
                    ],
                );
            },
            BatchSize::SmallInput,
        );
    });
}

fn histogram_record_with_non_static_values(c: &mut Criterion, attribute_values: [String; 10]) {
    let histogram = create_histogram("Histogram_Record_With_Non_Static_Values");
    c.bench_function("Histogram_Record_With_Non_Static_Values", |b| {
        b.iter_batched(
            || {
                // 4*4*10*10 = 1600 time series.
                CURRENT_RNG.with(|rng| {
                    let mut rng = rng.borrow_mut();
                    [
                        rng.random_range(0..4),
                        rng.random_range(0..4),
                        rng.random_range(0..10),
                        rng.random_range(0..10),
                    ]
                })
            },
            |rands| {
                let index_first_attribute = rands[0];
                let index_second_attribute = rands[1];
                let index_third_attribute = rands[2];
                let index_fourth_attribute = rands[3];
                histogram.record(
                    1,
                    &[
                        KeyValue::new(
                            "attribute1",
                            attribute_values[index_first_attribute].as_str().to_owned(),
                        ),
                        KeyValue::new(
                            "attribute2",
                            attribute_values[index_second_attribute].as_str().to_owned(),
                        ),
                        KeyValue::new(
                            "attribute3",
                            attribute_values[index_third_attribute].as_str().to_owned(),
                        ),
                        KeyValue::new(
                            "attribute4",
                            attribute_values[index_fourth_attribute].as_str().to_owned(),
                        ),
                    ],
                );
            },
            BatchSize::SmallInput,
        );
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
