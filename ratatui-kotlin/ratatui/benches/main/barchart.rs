use criterion::{Bencher, BenchmarkId, Criterion, criterion_group};
use rand::Rng;
use ratatui::buffer::Buffer;
use ratatui::layout::{Direction, Rect};
use ratatui::widgets::{Bar, BarChart, BarGroup, Widget};

/// Benchmark for rendering a barchart.
fn barchart(c: &mut Criterion) {
    let mut group = c.benchmark_group("barchart");
    let mut rng = rand::rng();

    for data_count in [64, 256, 2048] {
        let data: Vec<Bar> = (0..data_count)
            .map(|i| {
                Bar::default()
                    .label(format!("B{i}"))
                    .value(rng.random_range(0..data_count))
            })
            .collect();

        let bargroup = BarGroup::default().bars(&data);

        // Render a basic barchart
        group.bench_with_input(
            BenchmarkId::new("render", data_count),
            &BarChart::default().data(bargroup.clone()),
            render,
        );

        // Render an horizontal barchart
        group.bench_with_input(
            BenchmarkId::new("render_horizontal", data_count),
            &BarChart::default()
                .direction(Direction::Horizontal)
                .data(bargroup.clone()),
            render,
        );

        // Render a barchart with multiple groups
        group.bench_with_input(
            BenchmarkId::new("render_grouped", data_count),
            &BarChart::default()
                // We call `data` multiple time to add multiple groups.
                // This is not a duplicated call.
                .data(bargroup.clone())
                .data(bargroup.clone())
                .data(bargroup.clone()),
            render,
        );
    }

    group.finish();
}

/// Render the widget in a classical size buffer.
fn render(bencher: &mut Bencher, barchart: &BarChart) {
    let mut buffer = Buffer::empty(Rect::new(0, 0, 200, 50));
    // We use `iter_batched` to clone the value in the setup function.
    // See https://github.com/ratatui/ratatui/pull/377.
    bencher.iter_batched(
        || barchart.clone(),
        |bench_barchart| {
            bench_barchart.render(buffer.area, &mut buffer);
        },
        criterion::BatchSize::LargeInput,
    );
}

criterion_group!(benches, barchart);
