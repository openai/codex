use std::hint::black_box;

use criterion::{Criterion, criterion_group};
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Widget;

fn line_render(criterion: &mut Criterion) {
    for alignment in [Alignment::Left, Alignment::Center, Alignment::Right] {
        let mut group = criterion.benchmark_group(format!("line_render/{alignment}"));
        group.sample_size(1000);

        let line = &Line::from(vec![
            "This".red(),
            " ".green(),
            "is".italic(),
            " ".blue(),
            "SPARTA!!".bold(),
        ])
        .alignment(alignment);

        for width in [0, 3, 4, 6, 7, 10, 42] {
            let area = Rect::new(0, 0, width, 1);

            group.bench_function(width.to_string(), |bencher| {
                let mut buffer = Buffer::empty(area);
                bencher.iter(|| black_box(line).render(area, &mut buffer));
            });
        }
        group.finish();
    }
}

criterion_group!(benches, line_render);
