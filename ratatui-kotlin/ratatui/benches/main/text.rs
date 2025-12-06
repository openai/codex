use criterion::{BatchSize, Bencher, Criterion, criterion_group};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::{Line, Text};
use ratatui::widgets::Widget;

/// Benchmark for rendering a text.
fn text(c: &mut Criterion) {
    let mut group = c.benchmark_group("text");
    for (width, height) in [
        (200, 50),     // 1080p fullscreen area with medium font.
        (1, u16::MAX), // Heavily vertically skewed area.
        (u16::MAX, 1), // Heavily horizontally skewed area.
        (4096, 4096),  // Max sized area for benchmarking (~sqrt(u16::MAX) * 16, ~768 MB buffer).
    ] {
        let buffer_size = Rect::new(0, 0, width, height);

        // Generates sample text content scaled to the given rendering area.
        // The number of repeated text blocks is roughly proportional to the area size.
        //  - Small areas produce a few lines (at least 5 x 1 lines).
        //  - Large areas produce many lines (up to ~5 x 1000 lines).
        let make_text = |height: u16| {
            let repeat = (height as usize / 5).clamp(1, 1000);
            Text::from(
                (0..repeat)
                    .flat_map(|_| {
                        vec![
                            Line::from("The quick brown fox jumps over the lazy dog. Pack my box with five dozen liquor jugs."),
                            Line::from("🦀 Rustaceans unite! 東京・İstanbul・Sydney・San Francisco・Warsaw 🌏 RustConf連携中！").bold(),
                            Line::from("naïve cafés ☕ serve résumé-ready developers 👩‍💻🧑🏾‍💻 testing text rendering engines.").green(),
                            Line::from("ゼロ幅スペース\u{200B}、結合絵文字👨‍👩‍👧‍👦、全角文字ＡＢＣ、半角abcが混在。").blue(),
                            Line::from("Emoji test: 🙂😇🤖👩🏻‍🎨🧑‍🚀 — wrapped in a buffer for layout & clipping check.").italic(),
                        ]
                    })
                    .collect::<Vec<_>>(),
            )
        };

        group.bench_with_input(
            format!("render/{width}x{height}"),
            &make_text(height),
            |b, text| render(b, text, buffer_size),
        );
    }
    group.finish();
}

/// Render the text into a buffer of the given `size`.
fn render(bencher: &mut Bencher, text: &Text, size: Rect) {
    let mut buffer = Buffer::empty(size);
    // We use `iter_batched` to clone the value in the setup function.
    // See https://github.com/ratatui/ratatui/pull/377.
    bencher.iter_batched(
        || text.to_owned(),
        |bench_text| {
            bench_text.render(buffer.area, &mut buffer);
        },
        BatchSize::SmallInput,
    );
}

criterion_group!(benches, text);
