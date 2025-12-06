use criterion::{BatchSize, Bencher, Criterion, criterion_group};
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Block, Padding, Widget};

/// Benchmark for rendering a block.
fn block(c: &mut Criterion) {
    let mut group = c.benchmark_group("block");

    for (width, height) in [
        (100, 50),  // vertically split screen
        (200, 50),  // 1080p fullscreen with medium font
        (256, 256), // Max sized area
    ] {
        let buffer_size = Rect::new(0, 0, width, height);

        // Render an empty block
        group.bench_with_input(
            format!("render_empty/{width}x{height}"),
            &Block::new(),
            |b, block| render(b, block, buffer_size),
        );

        // Render with all features
        group.bench_with_input(
            format!("render_all_feature/{width}x{height}"),
            &Block::bordered()
                .padding(Padding::new(5, 5, 2, 2))
                .title("test title")
                .title_bottom(Line::from("bottom left title").alignment(Alignment::Right)),
            |b, block| render(b, block, buffer_size),
        );
    }

    group.finish();
}

/// Render the block into a buffer of the given `size`.
fn render(bencher: &mut Bencher, block: &Block, size: Rect) {
    let mut buffer = Buffer::empty(size);
    // We use `iter_batched` to clone the value in the setup function.
    // See https://github.com/ratatui/ratatui/pull/377.
    bencher.iter_batched(
        || block.to_owned(),
        |bench_block| {
            bench_block.render(buffer.area, &mut buffer);
        },
        BatchSize::SmallInput,
    );
}

criterion_group!(benches, block);
