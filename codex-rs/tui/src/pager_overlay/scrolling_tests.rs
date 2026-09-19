use super::super::CachedRenderable;
use super::CellRenderable;
use super::HyperlinkLinesRenderable;
use super::render_offset_content;
use crate::history_cell::HistoryCell;
use crate::history_cell::UserHistoryCell;
use crate::render::Insets;
use crate::render::renderable::InsetRenderable;
use crate::render::renderable::Renderable;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::visible_lines;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Text;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;
use std::sync::Arc;

#[derive(Debug)]
struct HyperlinkTestCell {
    lines: Vec<HyperlinkLine>,
}

impl HistoryCell for HyperlinkTestCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        visible_lines(self.lines.clone())
    }

    fn transcript_hyperlink_lines(&self, _width: u16) -> Vec<HyperlinkLine> {
        self.lines.clone()
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        visible_lines(self.lines.clone())
    }

    fn has_stable_transcript_height(&self) -> bool {
        false
    }
}

/// Forces the unchanged full-height fallback while preserving the wrapped renderable's output.
struct LegacyOnlyRenderable {
    inner: Box<dyn Renderable>,
}

impl Renderable for LegacyOnlyRenderable {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.inner.render(area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.inner.desired_height(width)
    }
}

fn scrolled_hyperlink_lines() -> Vec<HyperlinkLine> {
    let mut linked = HyperlinkLine::new(Line::from(vec![
        "prefix ".green(),
        "漢字 ".bold(),
        "ｶﾞ ".italic(),
    ]));
    linked.push_span(
        "clickable 漢字 ｶﾞ".cyan().underlined(),
        Some("https://example.com/a/wrapped-private-destination"),
    );
    linked.push_span(
        " suffix words that continue wrapping".blue(),
        /*destination*/ None,
    );

    vec![
        HyperlinkLine::new(Line::from("first styled row".magenta())),
        linked,
        HyperlinkLine::new(Line::default()),
        HyperlinkLine::new(Line::from("last 漢字 ｶﾞ row".red())),
    ]
}

fn scrolled_test_renderables(lines: &[HyperlinkLine]) -> Vec<(&'static str, Box<dyn Renderable>)> {
    let cell: Arc<dyn HistoryCell> = Arc::new(HyperlinkTestCell {
        lines: lines.to_vec(),
    });
    let user: Arc<dyn HistoryCell> = Arc::new(UserHistoryCell {
        spoken: false,
        message: "highlighted 漢字 user message that wraps\nsecond user line".to_string(),
        text_elements: Vec::new(),
        local_image_paths: Vec::new(),
        remote_image_urls: Vec::new(),
    });

    vec![
        (
            "uncached history cell",
            Box::new(CellRenderable {
                cache: Default::default(),
                cell: cell.clone(),
                highlighted: false,
            }),
        ),
        (
            "highlighted cached user history cell",
            Box::new(CachedRenderable::new(CellRenderable {
                cache: Default::default(),
                cell: user,
                highlighted: true,
            })),
        ),
        (
            "inset cached history cell",
            Box::new(InsetRenderable::new(
                Box::new(CachedRenderable::new(CellRenderable {
                    cache: Default::default(),
                    cell,
                    highlighted: false,
                })) as Box<dyn Renderable>,
                Insets::tlbr(
                    /*top*/ 2, /*left*/ 1, /*bottom*/ 1, /*right*/ 1,
                ),
            )),
        ),
        (
            "live tail",
            Box::new(HyperlinkLinesRenderable {
                lines: lines.to_vec(),
            }),
        ),
        (
            "inset cached live tail",
            Box::new(InsetRenderable::new(
                Box::new(CachedRenderable::new(HyperlinkLinesRenderable {
                    lines: lines.to_vec(),
                })) as Box<dyn Renderable>,
                Insets::tlbr(
                    /*top*/ 1, /*left*/ 1, /*bottom*/ 1, /*right*/ 1,
                ),
            )),
        ),
        (
            "unsupported paragraph fallback",
            Box::new(
                Paragraph::new(Text::from(visible_lines(lines.to_vec())))
                    .wrap(Wrap { trim: false }),
            ),
        ),
    ]
}

#[test]
fn scrolled_transcript_renderables_match_full_height_fallback() {
    let lines = scrolled_hyperlink_lines();

    for width in [5, 7, 13, 28] {
        for ((name, renderable), (_, legacy_inner)) in scrolled_test_renderables(&lines)
            .into_iter()
            .zip(scrolled_test_renderables(&lines))
        {
            let legacy = LegacyOnlyRenderable {
                inner: legacy_inner,
            };
            let height = renderable.desired_height(width);
            for offset in [
                0,
                1,
                2,
                3,
                height.saturating_sub(/*rhs*/ 2),
                height.saturating_sub(/*rhs*/ 1),
                height,
            ] {
                for (x, y, visible_height) in [(0, 0, 4), (3, 2, 3)] {
                    let area = Rect::new(x, y, width, visible_height);
                    let full_area = Rect::new(
                        /*x*/ 0,
                        /*y*/ 0,
                        area.right().saturating_add(/*rhs*/ 2),
                        area.bottom().saturating_add(/*rhs*/ 2),
                    );
                    let mut expected = Buffer::empty(full_area);
                    let mut actual = Buffer::empty(full_area);

                    let expected_height =
                        render_offset_content(area, &mut expected, &legacy, offset);
                    let actual_height =
                        render_offset_content(area, &mut actual, &*renderable, offset);

                    assert_eq!(
                        (actual_height, actual),
                        (expected_height, expected),
                        "renderable={name}, width={width}, offset={offset}, area={area:?}",
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
