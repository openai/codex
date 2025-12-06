use criterion::{BatchSize, Bencher, BenchmarkId, Criterion, criterion_group};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::{List, ListItem, ListState, StatefulWidget, Widget};

/// Benchmark for rendering a list.
/// It only benchmarks the render with a different amount of items.
fn list(c: &mut Criterion) {
    let mut group = c.benchmark_group("list");

    for line_count in [64, 2048, 16384] {
        let lines: Vec<ListItem> = (0..line_count)
            .map(|_| ListItem::new(fakeit::words::sentence(10)))
            .collect();

        // Render default list
        group.bench_with_input(
            BenchmarkId::new("render", line_count),
            &List::new(lines.clone()),
            render,
        );

        // Render with an offset to the middle of the list and a selected item
        group.bench_with_input(
            BenchmarkId::new("render_scroll_half", line_count),
            &List::new(lines.clone()).highlight_symbol(">>"),
            |b, list| {
                render_stateful(
                    b,
                    list,
                    ListState::default()
                        .with_offset(line_count / 2)
                        .with_selected(Some(line_count / 2)),
                );
            },
        );
    }

    group.finish();
}

/// Render the list into a common size buffer.
fn render(bencher: &mut Bencher, list: &List) {
    let mut buffer = Buffer::empty(Rect::new(0, 0, 200, 50));
    // We use `iter_batched` to clone the value in the setup function.
    // See https://github.com/ratatui/ratatui/pull/377.
    bencher.iter_batched(
        || list.to_owned(),
        |bench_list| {
            Widget::render(bench_list, buffer.area, &mut buffer);
        },
        BatchSize::LargeInput,
    );
}

/// render the list into a common size buffer with a state
fn render_stateful(bencher: &mut Bencher, list: &List, mut state: ListState) {
    let mut buffer = Buffer::empty(Rect::new(0, 0, 200, 50));
    // We use `iter_batched` to clone the value in the setup function.
    // See https://github.com/ratatui/ratatui/pull/377.
    bencher.iter_batched(
        || list.to_owned(),
        |bench_list| {
            StatefulWidget::render(bench_list, buffer.area, &mut buffer, &mut state);
        },
        BatchSize::LargeInput,
    );
}

criterion_group!(benches, list);
