use std::hint::black_box;

use criterion::{BatchSize, Bencher, BenchmarkId, Criterion, criterion_group};
use ratatui::layout::Rect;

fn rect_iters_benchmark(c: &mut Criterion) {
    let rect_sizes = vec![[16, 16], [64, 64], [255, 255]];
    let mut group = c.benchmark_group("rect");
    for rect in rect_sizes.into_iter().map(|[width, height]| Rect {
        width,
        height,
        ..Default::default()
    }) {
        group.bench_with_input(
            BenchmarkId::new("rect_rows_iter", rect.height),
            &rect,
            |b, rect| rect_rows_iter(b, *rect),
        );
        group.bench_with_input(
            BenchmarkId::new("rect_rows_collect", rect.height),
            &rect,
            |b, rect| rect_rows_collect(b, *rect),
        );
        group.bench_with_input(
            BenchmarkId::new("rect_columns_iter", rect.width),
            &rect,
            |b, rect| rect_columns_iter(b, *rect),
        );
        group.bench_with_input(
            BenchmarkId::new("rect_columns_collect", rect.width),
            &rect,
            |b, rect| rect_columns_collect(b, *rect),
        );
        group.bench_with_input(
            BenchmarkId::new(
                "rect_positions_iter",
                format!("{}x{}", rect.width, rect.height),
            ),
            &rect,
            |b, rect| rect_positions_iter(b, *rect),
        );
        group.bench_with_input(
            BenchmarkId::new(
                "rect_positions_collect",
                format!("{}x{}", rect.width, rect.height),
            ),
            &rect,
            |b, rect| rect_positions_collect(b, *rect),
        );
    }
    group.finish();
}

fn rect_rows_iter(c: &mut Bencher, rect: Rect) {
    c.iter_batched(
        || black_box(rect),
        |rect| {
            for row in black_box(rect.rows()) {
                black_box(row);
            }
        },
        BatchSize::LargeInput,
    );
}

fn rect_rows_collect(c: &mut Bencher, rect: Rect) {
    c.iter_batched(
        || black_box(rect),
        |rect| black_box(rect.rows()).collect::<Vec<_>>(),
        BatchSize::LargeInput,
    );
}

fn rect_columns_iter(c: &mut Bencher, rect: Rect) {
    c.iter_batched(
        || black_box(rect),
        |rect| {
            for col in black_box(rect.columns()) {
                black_box(col);
            }
        },
        BatchSize::LargeInput,
    );
}

fn rect_columns_collect(c: &mut Bencher, rect: Rect) {
    c.iter_batched(
        || black_box(rect),
        |rect| black_box(rect.columns()).collect::<Vec<_>>(),
        BatchSize::LargeInput,
    );
}

fn rect_positions_iter(c: &mut Bencher, rect: Rect) {
    c.iter_batched(
        || black_box(rect),
        |rect| {
            for pos in black_box(rect.positions()) {
                black_box(pos);
            }
        },
        BatchSize::LargeInput,
    );
}

fn rect_positions_collect(b: &mut Bencher, rect: Rect) {
    b.iter_batched(
        || black_box(rect),
        |rect| black_box(rect.positions()).collect::<Vec<_>>(),
        BatchSize::LargeInput,
    );
}

criterion_group!(benches, rect_iters_benchmark);
