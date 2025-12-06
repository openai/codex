use std::hint::black_box;
use std::rc::Rc;

use criterion::{Criterion, criterion_group};
use ratatui::layout::Constraint::{Fill, Length, Max, Min, Percentage, Ratio};
use ratatui::layout::{Layout, Rect};

const SPLIT_BY: u16 = 10;

fn layout_split(criterion: &mut Criterion) {
    for size in [16, 64, 256] {
        let mut group = criterion.benchmark_group(format!("constraints {size}x{size}"));
        let area = Rect::new(0, 0, size, size);
        group.bench_function("Fill", |bencher| {
            bencher.iter(|| layout_fill(black_box(area)));
        });
        group.bench_function("Length", |bencher| {
            bencher.iter(|| layout_length(black_box(area)));
        });
        group.bench_function("Max", |bencher| {
            bencher.iter(|| layout_max(black_box(area)));
        });
        group.bench_function("Min", |bencher| {
            bencher.iter(|| layout_min(black_box(area)));
        });
        group.bench_function("Percentage", |bencher| {
            bencher.iter(|| layout_percentage(black_box(area)));
        });
        group.bench_function("Ratio", |bencher| {
            bencher.iter(|| layout_ratio(black_box(area)));
        });
        group.finish();
    }
}

fn layout_fill(area: Rect) -> Rc<[Rect]> {
    Layout::vertical([Fill(1); SPLIT_BY as usize]).split(area)
}

fn layout_length(area: Rect) -> Rc<[Rect]> {
    Layout::vertical([Length(area.width / SPLIT_BY); SPLIT_BY as usize]).split(area)
}

fn layout_max(area: Rect) -> Rc<[Rect]> {
    Layout::vertical([Max(area.width / SPLIT_BY); SPLIT_BY as usize]).split(area)
}

fn layout_min(area: Rect) -> Rc<[Rect]> {
    Layout::vertical([Min(area.width / SPLIT_BY); SPLIT_BY as usize]).split(area)
}

fn layout_percentage(area: Rect) -> Rc<[Rect]> {
    Layout::vertical([Percentage(100 / SPLIT_BY); SPLIT_BY as usize]).split(area)
}

fn layout_ratio(area: Rect) -> Rc<[Rect]> {
    Layout::vertical([Ratio(1, SPLIT_BY.into()); SPLIT_BY as usize]).split(area)
}

criterion_group!(benches, layout_split);
