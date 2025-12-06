use criterion::{BatchSize, Criterion, criterion_group};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Gauge, Widget};

/// Benchmark for rendering a gauge.
fn gauge(c: &mut Criterion) {
    let mut group = c.benchmark_group("gauge");

    let (width, height) = (200, 50); // 1080p fullscreen with medium font
    let buffer_size = Rect::new(0, 0, width, height);

    // Render an empty gauge
    group.bench_with_input(
        format!("render_empty/{width}x{height}"),
        &Gauge::default(),
        |b, gauge| {
            let mut buffer = Buffer::empty(buffer_size);
            // We use `iter_batched` to clone the value in the setup function because
            // `Widget::render` consumes the widget.
            b.iter_batched(
                || gauge.to_owned(),
                |bench_gauge| {
                    bench_gauge.render(buffer.area, &mut buffer);
                },
                BatchSize::SmallInput,
            );
        },
    );

    // Render with all features
    group.bench_with_input(
        format!("render_all_feature/{width}x{height}"),
        &Gauge::default()
            .block(Block::bordered().title("Progress"))
            .gauge_style(Style::new().white().on_black().italic())
            .percent(20)
            .label("20%")
            .use_unicode(true),
        |b, gauge| {
            let mut buffer = Buffer::empty(buffer_size);
            // We use `iter_batched` to clone the value in the setup function because
            // `Widget::render` consumes the widget.
            b.iter_batched(
                || gauge.to_owned(),
                |bench_gauge| {
                    bench_gauge.render(buffer.area, &mut buffer);
                },
                BatchSize::SmallInput,
            );
        },
    );

    group.finish();
}

criterion_group!(benches, gauge);
