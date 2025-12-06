use std::hint::black_box;

use criterion::{BatchSize, Bencher, BenchmarkId, Criterion, criterion_group};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::{Paragraph, Widget, Wrap};

/// because the scroll offset is a u16, the maximum number of lines that can be scrolled is 65535.
/// This is a limitation of the current implementation and may be fixed by changing the type of the
/// scroll offset to a u32.
const NO_WRAP_WIDTH: u16 = 200;
const WRAP_WIDTH: u16 = 100;
const PARAGRAPH_DEFAULT_HEIGHT: u16 = 50;

/// Benchmark for rendering a paragraph with a given number of lines. The design of this benchmark
/// allows comparison of the performance of rendering a paragraph with different numbers of lines.
/// as well as comparing with the various settings on the scroll and wrap features.
fn paragraph(c: &mut Criterion) {
    let mut group = c.benchmark_group("paragraph");
    for line_count in [64, 2048, u16::MAX] {
        let lines = random_lines(line_count);
        let lines = lines.as_str();
        let y_scroll = line_count - PARAGRAPH_DEFAULT_HEIGHT;

        // benchmark that measures the overhead of creating a paragraph separately from rendering
        group.bench_with_input(BenchmarkId::new("new", line_count), lines, |b, lines| {
            b.iter(|| Paragraph::new(black_box(lines)));
        });

        // render the paragraph with no scroll
        group.bench_with_input(
            BenchmarkId::new("render", line_count),
            &Paragraph::new(lines),
            |bencher, paragraph| render(bencher, paragraph, NO_WRAP_WIDTH),
        );

        // scroll the paragraph by half the number of lines and render
        group.bench_with_input(
            BenchmarkId::new("render_scroll_half", line_count),
            &Paragraph::new(lines).scroll((y_scroll / 2, 0)),
            |bencher, paragraph| render(bencher, paragraph, NO_WRAP_WIDTH),
        );

        // scroll the paragraph by the full number of lines and render
        group.bench_with_input(
            BenchmarkId::new("render_scroll_full", line_count),
            &Paragraph::new(lines).scroll((y_scroll, 0)),
            |bencher, paragraph| render(bencher, paragraph, NO_WRAP_WIDTH),
        );

        // render the paragraph wrapped to 100 characters
        group.bench_with_input(
            BenchmarkId::new("render_wrap", line_count),
            &Paragraph::new(lines).wrap(Wrap { trim: false }),
            |bencher, paragraph| render(bencher, paragraph, WRAP_WIDTH),
        );

        // scroll the paragraph by the full number of lines and render wrapped to 100 characters
        group.bench_with_input(
            BenchmarkId::new("render_wrap_scroll_full", line_count),
            &Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .scroll((y_scroll, 0)),
            |bencher, paragraph| render(bencher, paragraph, WRAP_WIDTH),
        );
    }
    group.finish();
}

/// Render the paragraph into a buffer with the given width.
fn render(bencher: &mut Bencher, paragraph: &Paragraph, width: u16) {
    let mut buffer = Buffer::empty(Rect::new(0, 0, width, PARAGRAPH_DEFAULT_HEIGHT));
    // We use `iter_batched` to clone the value in the setup function.
    // See https://github.com/ratatui/ratatui/pull/377.
    bencher.iter_batched(
        || paragraph.to_owned(),
        |bench_paragraph| {
            bench_paragraph.render(buffer.area, &mut buffer);
        },
        BatchSize::LargeInput,
    );
}

/// Create a string with the given number of lines filled with nonsense words
///
/// English language has about 5.1 average characters per word so including the space between words
/// this should emit around 200 characters per paragraph on average.
fn random_lines(count: u16) -> String {
    let count = i64::from(count);
    let sentence_count = 3;
    let word_count = 11;
    fakeit::words::paragraph(count, sentence_count, word_count, "\n".into())
}

criterion_group!(benches, paragraph);
