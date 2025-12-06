use std::hint::black_box;

use criterion::{BenchmarkId, Criterion};
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::Rect;
use ratatui::text::Line;

criterion::criterion_group!(benches, empty, filled, with_lines);

const fn rect(size: u16) -> Rect {
    Rect::new(0, 0, size, size)
}

fn empty(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer/empty");
    for size in [16, 64, 255] {
        let area = rect(size);
        group.bench_with_input(BenchmarkId::from_parameter(size), &area, |b, &area| {
            b.iter(|| {
                let _buffer = Buffer::empty(black_box(area));
            });
        });
    }
    group.finish();
}

/// This likely should have the same performance as `empty`, but it's here for completeness
/// and to catch any potential performance regressions.
fn filled(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer/filled");
    for size in [16, 64, 255] {
        let area = rect(size);
        let cell = Cell::new("AAAA"); // simulate a multi-byte character
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &(area, cell),
            |b, (area, cell)| {
                b.iter(|| {
                    let _buffer = Buffer::filled(black_box(*area), cell.clone());
                });
            },
        );
    }
    group.finish();
}

fn with_lines(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer/with_lines");
    for size in [16, 64, 255] {
        let word_count = 50;
        let lines = fakeit::words::sentence(word_count);
        let lines = lines.lines().map(Line::from);
        group.bench_with_input(BenchmarkId::from_parameter(size), &lines, |b, lines| {
            b.iter(|| {
                let _buffer = Buffer::with_lines(black_box(lines.clone()));
            });
        });
    }
    group.finish();
}
