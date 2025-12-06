use criterion::{BatchSize, Bencher, BenchmarkId, Criterion, criterion_group};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::widgets::{Row, StatefulWidget, Table, TableState, Widget};

/// Benchmark for rendering a table.
/// It only benchmarks the render with a different number of rows, and columns.
fn table(c: &mut Criterion) {
    let mut group = c.benchmark_group("table");

    for row_count in [64, 2048, 16384] {
        for col_count in [2, 4, 8] {
            let bench_sizes = format!("{row_count}x{col_count}");
            let rows: Vec<Row> = (0..row_count)
                .map(|_| Row::new((0..col_count).map(|_| fakeit::words::quote())))
                .collect();

            // Render default table
            group.bench_with_input(
                BenchmarkId::new("render", &bench_sizes),
                &Table::new(rows.clone(), [] as [Constraint; 0]),
                render,
            );

            // Render with an offset to the middle of the table and a selected row
            group.bench_with_input(
                BenchmarkId::new("render_scroll_half", &bench_sizes),
                &Table::new(rows, [] as [Constraint; 0]).highlight_symbol(">>"),
                |b, table| {
                    render_stateful(
                        b,
                        table,
                        TableState::default()
                            .with_offset(row_count / 2)
                            .with_selected(Some(row_count / 2)),
                    );
                },
            );
        }
    }

    group.finish();
}

fn render(bencher: &mut Bencher, table: &Table) {
    let mut buffer = Buffer::empty(Rect::new(0, 0, 200, 50));
    bencher.iter_batched(
        || table.to_owned(),
        |bench_table| {
            Widget::render(bench_table, buffer.area, &mut buffer);
        },
        BatchSize::LargeInput,
    );
}

fn render_stateful(bencher: &mut Bencher, table: &Table, mut state: TableState) {
    let mut buffer = Buffer::empty(Rect::new(0, 0, 200, 50));
    bencher.iter_batched(
        || table.to_owned(),
        |bench_table| {
            StatefulWidget::render(bench_table, buffer.area, &mut buffer, &mut state);
        },
        BatchSize::LargeInput,
    );
}

criterion_group!(benches, table);
