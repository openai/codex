//! Static pager fallback preserves styled buffer slices at arbitrary scroll offsets.

use super::super::CachedRenderable;
use super::render_offset_content;
use crate::render::Insets;
use crate::render::renderable::InsetRenderable;
use crate::render::renderable::Renderable;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Text;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;

#[test]
fn scrolled_static_renderables_match_clipped_buffer_slices() {
    let lines = vec![
        Line::from(vec![
            "prefix ".green(),
            "漢字 ｶﾞ ".bold(),
            "styled words that wrap across rows".blue(),
        ]),
        Line::default(),
        Line::from("last 漢字 ｶﾞ row".red()),
    ];
    for width in [5, 7, 13, 28] {
        let paragraph = || Paragraph::new(Text::from(lines.clone())).wrap(Wrap { trim: false });
        let renderables: Vec<Box<dyn Renderable>> = vec![
            Box::new(paragraph()),
            Box::new(CachedRenderable::new(paragraph())),
            Box::new(InsetRenderable::new(
                Box::new(CachedRenderable::new(paragraph())) as Box<dyn Renderable>,
                Insets::tlbr(
                    /*top*/ 2, /*left*/ 1, /*bottom*/ 1, /*right*/ 1,
                ),
            )),
        ];
        for renderable in renderables {
            let height = renderable.desired_height(width);
            for offset in [0, 1, 2, 3, height.saturating_sub(/*rhs*/ 1), height] {
                for (x, y, visible_height) in [(0, 0, 4), (3, 2, 3)] {
                    let area = Rect::new(x, y, width, visible_height);
                    let canvas = Rect::new(/*x*/ 0, /*y*/ 0, area.right(), area.bottom());
                    let mut expected = Buffer::empty(canvas);
                    let mut actual = Buffer::empty(canvas);
                    let expected_height = visible_height.min(height.saturating_sub(offset));
                    let full_area =
                        Rect::new(/*x*/ 0, /*y*/ 0, width, offset + expected_height);
                    let mut full = Buffer::empty(full_area);
                    renderable.render(full_area, &mut full);
                    for row in 0..expected_height {
                        for column in 0..width {
                            expected[(x + column, y + row)] = full[(column, offset + row)].clone();
                        }
                    }
                    let actual_height =
                        render_offset_content(area, &mut actual, &*renderable, offset);
                    assert_eq!(
                        (actual_height, actual),
                        (expected_height, expected),
                        "width={width}, offset={offset}, area={area:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn fallback_handles_offsets_near_maximum_height() {
    struct MaximumHeightRenderable;

    impl Renderable for MaximumHeightRenderable {
        fn render(&self, area: Rect, buf: &mut Buffer) {
            let last_row = area.bottom().saturating_sub(/*rhs*/ 1);
            buf[(area.x, last_row)].set_symbol("x");
        }

        fn desired_height(&self, _width: u16) -> u16 {
            u16::MAX
        }
    }

    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 1, /*height*/ 2,
    );
    let mut actual = Buffer::empty(area);
    let mut expected = Buffer::empty(area);
    expected[(area.x, area.y)].set_symbol("x");
    let height = render_offset_content(area, &mut actual, &MaximumHeightRenderable, u16::MAX - 1);

    assert_eq!((height, actual), (1, expected));
}
