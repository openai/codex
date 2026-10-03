//! Exercises dialog layout independently of any production confirmation caller.

use super::*;
use pretty_assertions::assert_eq;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

struct TestPane {
    presentation: ViewPresentation,
}

impl BottomPaneView for TestPane {
    fn presentation(&self) -> ViewPresentation {
        self.presentation
    }
}

impl Renderable for TestPane {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                if self.presentation == ViewPresentation::Inline {
                    buf[(x, y)].set_symbol(".");
                } else if y == area.top() {
                    buf[(x, y)].set_symbol("D");
                }
            }
        }
    }

    fn desired_height(&self, _width: u16) -> u16 {
        3
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        Some((area.x, area.y))
    }

    fn cursor_style(&self, _area: Rect) -> SetCursorStyle {
        SetCursorStyle::SteadyBar
    }
}

#[test]
fn centered_dialog_retains_backdrop_and_routes_cursor() {
    let backdrop = TestPane {
        presentation: ViewPresentation::Inline,
    };
    let dialog = TestPane {
        presentation: super::super::SelectionViewParams::confirmation().presentation,
    };
    let overlay = DialogOverlay {
        backdrop: RenderableItem::Borrowed(&backdrop),
        dialog: CenteredView(&dialog),
    };
    let mut snapshots = Vec::new();
    for (width, height, expected_cursor) in [(80, 9, (4, 3)), (24, 7, (2, 2)), (12, 2, (2, 0))] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| overlay.render(frame.area(), frame.buffer_mut()))
            .unwrap();
        let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
        assert_eq!(
            (overlay.cursor_pos(area), overlay.cursor_style(area)),
            (Some(expected_cursor), SetCursorStyle::SteadyBar)
        );
        let views: Vec<Box<dyn BottomPaneView>> = vec![
            Box::new(TestPane {
                presentation: ViewPresentation::Inline,
            }),
            Box::new(TestPane {
                presentation: ViewPresentation::Centered,
            }),
        ];
        let stack = ViewStack(&views);
        let mut stacked = Buffer::empty(area);
        stack.render(area, &mut stacked);
        assert_eq!(&stacked, terminal.backend().buffer());
        assert_eq!(stack.cursor_pos(area), overlay.cursor_pos(area));
        snapshots.push(format!("{width}x{height}\n{}", terminal.backend()));
    }
    insta::assert_snapshot!(snapshots.join("\n"));
}
