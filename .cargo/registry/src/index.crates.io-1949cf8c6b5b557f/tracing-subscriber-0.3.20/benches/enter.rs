use criterion::{criterion_group, criterion_main, Criterion};
use tracing_subscriber::prelude::*;

fn enter(c: &mut Criterion) {
    let mut group = c.benchmark_group("enter");
    let _subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .finish()
        .set_default();
    group.bench_function("enabled", |b| {
        let span = tracing::info_span!("foo");
        b.iter_with_large_drop(|| span.enter())
    });
    group.bench_function("disabled", |b| {
        let span = tracing::debug_span!("foo");
        b.iter_with_large_drop(|| span.enter())
    });
}

fn enter_exit(c: &mut Criterion) {
    let mut group = c.benchmark_group("enter_exit");
    let _subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .finish()
        .set_default();
    group.bench_function("enabled", |b| {
        let span = tracing::info_span!("foo");
        b.iter(|| span.enter())
    });
    group.bench_function("disabled", |b| {
        let span = tracing::debug_span!("foo");
        b.iter(|| span.enter())
    });
}

fn enter_many(c: &mut Criterion) {
    let mut group = c.benchmark_group("enter_many");
    let _subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .finish()
        .set_default();
    group.bench_function("enabled", |b| {
        let span1 = tracing::info_span!("span1");
        let _e1 = span1.enter();
        let span2 = tracing::info_span!("span2");
        let _e2 = span2.enter();
        let span3 = tracing::info_span!("span3");
        let _e3 = span3.enter();
        let span = tracing::info_span!("foo");
        b.iter_with_large_drop(|| span.enter())
    });
    group.bench_function("disabled", |b| {
        let span1 = tracing::info_span!("span1");
        let _e1 = span1.enter();
        let span2 = tracing::info_span!("span2");
        let _e2 = span2.enter();
        let span3 = tracing::info_span!("span3");
        let _e3 = span3.enter();
        let span = tracing::debug_span!("foo");
        b.iter_with_large_drop(|| span.enter())
    });
}
criterion_group!(benches, enter, enter_exit, enter_many);
criterion_main!(benches);
