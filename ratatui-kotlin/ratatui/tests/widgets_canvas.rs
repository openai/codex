use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::canvas::Canvas;

#[test]
fn widgets_canvas_draw_labels() {
    let backend = TestBackend::new(5, 5);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let label = String::from("test");
            let canvas = Canvas::default()
                .background_color(Color::Yellow)
                .x_bounds([0.0, 5.0])
                .y_bounds([0.0, 5.0])
                .paint(|ctx| {
                    ctx.print(
                        0.0,
                        0.0,
                        Span::styled(label.clone(), Style::default().fg(Color::Blue)),
                    );
                });
            f.render_widget(canvas, f.area());
        })
        .unwrap();

    let mut expected = Buffer::with_lines(["", "", "", "", "test "]);
    for row in 0..5 {
        for col in 0..5 {
            expected[(col, row)].set_bg(Color::Yellow);
        }
    }
    for col in 0..4 {
        expected[(col, 4)].set_fg(Color::Blue);
    }
    terminal.backend().assert_buffer(&expected);
}
