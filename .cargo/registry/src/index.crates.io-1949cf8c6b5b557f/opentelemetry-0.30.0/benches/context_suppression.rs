use criterion::{black_box, criterion_group, criterion_main, Criterion};
use opentelemetry::Context;

// Run this benchmark with:
// cargo bench --bench context_suppression

// The benchmark results:
// criterion = "0.5.1"
// Hardware: Apple M4 Pro
// Total Number of Cores:   14 (10 performance and 4 efficiency)
// | Benchmark                             | Time   |
// |---------------------------------------|--------|
// | enter_telemetry_suppressed_scope      | 8.3 ns |
// | normal_attach                         | 9.1 ns |
// | is_current_telemetry_suppressed_false | 750 ps |
// | is_current_telemetry_suppressed_true  | 750 ps |

fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("telemetry_suppression");

    // Benchmark the cost of entering a suppressed scope
    group.bench_function("enter_telemetry_suppressed_scope", |b| {
        b.iter(|| {
            let _guard = black_box(Context::enter_telemetry_suppressed_scope());
        });
    });

    // For comparison - normal context attach
    group.bench_function("normal_attach", |b| {
        b.iter(|| {
            let _guard = black_box(Context::current().attach());
        });
    });

    // Benchmark checking if current is suppressed (when not suppressed)
    group.bench_function("is_current_telemetry_suppressed_false", |b| {
        // Make sure we're in a non-suppressed context
        let _restore_ctx = Context::current().attach();
        b.iter(|| {
            let is_suppressed = black_box(Context::is_current_telemetry_suppressed());
            black_box(is_suppressed);
        });
    });

    // Benchmark checking if current is suppressed (when suppressed)
    group.bench_function("is_current_telemetry_suppressed_true", |b| {
        // Enter suppressed context for the duration of the benchmark
        let _suppressed_guard = Context::enter_telemetry_suppressed_scope();
        b.iter(|| {
            let is_suppressed = black_box(Context::is_current_telemetry_suppressed());
            black_box(is_suppressed);
        });
    });

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(std::time::Duration::from_secs(1))
        .measurement_time(std::time::Duration::from_secs(2));
    targets = criterion_benchmark
}
criterion_main!(benches);
