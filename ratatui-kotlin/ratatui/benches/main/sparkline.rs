use criterion::{Bencher, BenchmarkId, Criterion, criterion_group};
use rand::Rng;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::{Sparkline, Widget};

/// Benchmark for rendering a sparkline.
fn sparkline(c: &mut Criterion) {
    let mut group = c.benchmark_group("sparkline");
    let mut rng = rand::rng();

    for data_count in [64, 256, 2048] {
        let data: Vec<u64> = (0..data_count)
            .map(|_| rng.random_range(0..data_count))
            .collect();

        // Render a basic sparkline
        group.bench_with_input(
            BenchmarkId::new("render", data_count),
            &Sparkline::default().data(&data),
            render,
        );
    }

    group.finish();
}

/// Render the block into a buffer of the given `size`.
fn render(bencher: &mut Bencher, sparkline: &Sparkline) {
    let mut buffer = Buffer::empty(Rect::new(0, 0, 200, 50));
    // We use `iter_batched` to clone the value in the setup function.
    // See https://github.com/ratatui/ratatui/pull/377.
    bencher.iter_batched(
        || sparkline.clone(),
        |bench_sparkline| {
            bench_sparkline.render(buffer.area, &mut buffer);
        },
        criterion::BatchSize::LargeInput,
    );
}

criterion_group!(benches, sparkline);
