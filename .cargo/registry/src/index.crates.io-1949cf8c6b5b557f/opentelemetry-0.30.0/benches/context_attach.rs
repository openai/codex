use criterion::{
    black_box, criterion_group, criterion_main, measurement::WallTime, BenchmarkGroup, BenchmarkId,
    Criterion,
};
use opentelemetry::{
    trace::{SpanContext, TraceContextExt},
    Context,
};

// Run this benchmark with:
// cargo bench --bench context_attach

fn criterion_benchmark(c: &mut Criterion) {
    let span_context = Context::new().with_remote_span_context(SpanContext::empty_context());
    let contexts = vec![
        ("empty_cx", Context::new()),
        ("single_value_cx", Context::new().with_value(Value(4711))),
        ("span_cx", span_context),
    ];
    for (name, cx) in contexts {
        single_cx_scope(&mut group(c), name, &cx);
        nested_cx_scope(&mut group(c), name, &cx);
        overlapping_cx_scope(&mut group(c), name, &cx);
    }
}

fn single_cx_scope(
    group: &mut BenchmarkGroup<'_, WallTime>,
    context_type: &str,
    context: &Context,
) {
    group.bench_function(BenchmarkId::new("single_cx", context_type), |b| {
        b.iter_batched(
            || context.clone(),
            |cx| {
                single_cx(cx);
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

#[inline(never)]
fn single_cx(cx: Context) {
    let _cx_guard = black_box(cx.attach());
    let _ = black_box(dummy_work());
}

fn nested_cx_scope(group: &mut BenchmarkGroup<'_, WallTime>, cx_type: &str, context: &Context) {
    group.bench_function(BenchmarkId::new("nested_cx", cx_type), |b| {
        b.iter_batched(
            || (context.clone(), context.clone()),
            |(cx1, cx2)| {
                nested_cx(cx1, cx2);
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

#[inline(never)]
fn nested_cx(cx1: Context, cx2: Context) {
    let _outer_cx_guard = black_box(cx1.attach());
    let _inner_cx_guard = black_box(cx2.attach());
    let _ = black_box(dummy_work());
}

fn overlapping_cx_scope(
    group: &mut BenchmarkGroup<'_, WallTime>,
    cx_type: &str,
    context: &Context,
) {
    // This is to ensure that the context is restored after the benchmark,
    // see https://github.com/open-telemetry/opentelemetry-rust/issues/1887
    let _restore_cx_guard = Context::current().attach();
    group.bench_function(BenchmarkId::new("out_of_order_cx_drop", cx_type), |b| {
        b.iter_batched(
            || (context.clone(), context.clone()),
            |(cx1, cx2)| {
                out_of_order_cx_drop(cx1, cx2);
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

#[inline(never)]
fn out_of_order_cx_drop(cx1: Context, cx2: Context) {
    let outer_cx_guard = cx1.attach();
    let inner_cx_guard = cx2.attach();
    let _ = black_box(dummy_work());
    drop(outer_cx_guard);
    drop(inner_cx_guard);
}

#[inline(never)]
fn dummy_work() -> i32 {
    black_box(1 + 1)
}

fn group(c: &mut Criterion) -> BenchmarkGroup<WallTime> {
    c.benchmark_group("context_attach")
}

#[derive(Debug, PartialEq)]
struct Value(i32);

criterion_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(std::time::Duration::from_secs(1))
        .measurement_time(std::time::Duration::from_secs(2));
    targets = criterion_benchmark
}
criterion_main!(benches);
