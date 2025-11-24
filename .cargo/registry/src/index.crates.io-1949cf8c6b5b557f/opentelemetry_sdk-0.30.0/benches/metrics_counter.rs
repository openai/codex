/*
    The benchmark results:
    criterion = "0.5.1"
    rustc 1.83.0 (90b35a623 2024-11-26)
    OS: Ubuntu 22.04.4 LTS (5.15.167.4-microsoft-standard-WSL2)
    Hardware: Intel(R) Xeon(R) Platinum 8370C CPU @ 2.80GHz   2.79 GHz
    RAM: 64.0 GB
    | Test                                                  | Average time|
    |-------------------------------------------------------|-------------|
    | Counter_Add_Sorted                                    | 160 ns      |
    | Counter_Add_Unsorted                                  | 164 ns      |
    | Counter_Add_Sorted_With_Non_Static_Values             | 238 ns      |
    | Counter_Overflow                                      | 562 ns      |
    | ThreadLocal_Random_Generator_5                        |  37 ns      |
*/

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use opentelemetry::{
    metrics::{Counter, MeterProvider as _},
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
// cargo bench --bench metrics_counter
fn create_counter(name: &'static str) -> Counter<u64> {
    let meter_provider: SdkMeterProvider = SdkMeterProvider::builder()
        .with_reader(ManualReader::builder().build())
        .build();
    let meter = meter_provider.meter("benchmarks");

    println!("Counter_Created");
    meter.u64_counter(name).build()
}

fn criterion_benchmark(c: &mut Criterion) {
    counter_add_sorted(c);
    counter_add_unsorted(c);

    let attribute_values: [String; 10] = (1..=10)
        .map(|i| format!("value{}", i))
        .collect::<Vec<String>>()
        .try_into()
        .expect("Expected a Vec of length 10");

    counter_add_sorted_with_non_static_values(c, attribute_values);

    counter_overflow(c);
    random_generator(c);
}

fn counter_add_sorted(c: &mut Criterion) {
    let counter = create_counter("Counter_Add_Sorted");
    c.bench_function("Counter_Add_Sorted", |b| {
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
                counter.add(
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

fn counter_add_unsorted(c: &mut Criterion) {
    let counter = create_counter("Counter_Add_Unsorted");
    c.bench_function("Counter_Add_Unsorted", |b| {
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
                counter.add(
                    1,
                    &[
                        KeyValue::new("attribute2", ATTRIBUTE_VALUES[index_second_attribute]),
                        KeyValue::new("attribute3", ATTRIBUTE_VALUES[index_third_attribute]),
                        KeyValue::new("attribute1", ATTRIBUTE_VALUES[index_first_attribute]),
                        KeyValue::new("attribute4", ATTRIBUTE_VALUES[index_fourth_attribute]),
                    ],
                );
            },
            BatchSize::SmallInput,
        );
    });
}

fn counter_add_sorted_with_non_static_values(c: &mut Criterion, attribute_values: [String; 10]) {
    let counter = create_counter("Counter_Add_Sorted_With_Non_Static_Values");
    c.bench_function("Counter_Add_Sorted_With_Non_Static_Values", |b| {
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
                counter.add(
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

fn counter_overflow(c: &mut Criterion) {
    let counter = create_counter("Counter_Overflow");
    // Cause overflow.
    for v in 0..2001 {
        counter.add(100, &[KeyValue::new("A", v.to_string())]);
    }

    c.bench_function("Counter_Overflow", |b| {
        b.iter(|| {
            // 4*4*10*10 = 1600 time series.
            let rands = CURRENT_RNG.with(|rng| {
                let mut rng = rng.borrow_mut();
                [
                    rng.random_range(0..4),
                    rng.random_range(0..4),
                    rng.random_range(0..10),
                    rng.random_range(0..10),
                ]
            });
            let index_first_attribute = rands[0];
            let index_second_attribute = rands[1];
            let index_third_attribute = rands[2];
            let index_fourth_attribute = rands[3];
            counter.add(
                1,
                &[
                    KeyValue::new("attribute2", ATTRIBUTE_VALUES[index_second_attribute]),
                    KeyValue::new("attribute3", ATTRIBUTE_VALUES[index_third_attribute]),
                    KeyValue::new("attribute1", ATTRIBUTE_VALUES[index_first_attribute]),
                    KeyValue::new("attribute4", ATTRIBUTE_VALUES[index_fourth_attribute]),
                ],
            );
        });
    });
}

fn random_generator(c: &mut Criterion) {
    c.bench_function("ThreadLocal_Random_Generator_5", |b| {
        b.iter(|| {
            let __i1 = CURRENT_RNG.with(|rng| {
                let mut rng = rng.borrow_mut();
                [
                    rng.random_range(0..4),
                    rng.random_range(0..4),
                    rng.random_range(0..10),
                    rng.random_range(0..10),
                    rng.random_range(0..10),
                ]
            });
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
