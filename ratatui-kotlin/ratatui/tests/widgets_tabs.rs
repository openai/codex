use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Tabs;
use ratatui::{Terminal, symbols};

#[test]
fn widgets_tabs_should_not_panic_on_narrow_areas() {
    let backend = TestBackend::new(1, 1);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let tabs = Tabs::new(["Tab1", "Tab2"]);
            f.render_widget(
                tabs,
                Rect {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                },
            );
        })
        .unwrap();
    terminal.backend().assert_buffer_lines([" "]);
}

#[test]
fn widgets_tabs_should_truncate_the_last_item() {
    let backend = TestBackend::new(10, 1);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let tabs = Tabs::new(["Tab1", "Tab2"]);
            f.render_widget(
                tabs,
                Rect {
                    x: 0,
                    y: 0,
                    width: 9,
                    height: 1,
                },
            );
        })
        .unwrap();
    let mut expected = Buffer::with_lines([format!(" Tab1 {} T ", symbols::line::VERTICAL)]);
    expected.set_style(Rect::new(1, 0, 4, 1), Style::new().reversed());
    terminal.backend().assert_buffer(&expected);
}
