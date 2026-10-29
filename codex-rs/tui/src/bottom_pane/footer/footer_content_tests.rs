#![cfg(test)]

use super::FooterContent;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::osc8_hyperlink;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn clipped_links_keep_their_cells_after_a_halfwidth_katakana_grapheme() {
    let project_url = "https://example.com/project";
    let pr_url = "https://example.com/pr/42";
    let mut line = HyperlinkLine::default();
    line.push_span("ｶﾞ".into(), Some(project_url));
    line.push_span(" · ".into(), /*destination*/ None);
    line.push_span("PR #42".into(), Some(pr_url));

    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 10, /*height*/ 1,
    );
    let mut buf = Buffer::empty(area);
    FooterContent::HyperlinkLine(line).render(area, &mut buf);
    assert_eq!(buf[(0, 0)].symbol(), osc8_hyperlink(project_url, "ｶﾞ"));
    for (x, ch) in (2..5).zip(" · ".chars()) {
        assert_eq!(buf[(x, 0)].symbol(), ch.to_string());
    }
    for (x, ch) in (5..9).zip("PR #".chars()) {
        assert_eq!(
            buf[(x, 0)].symbol(),
            osc8_hyperlink(pr_url, &ch.to_string())
        );
    }
    assert_eq!(buf[(9, 0)].symbol(), "…");
}

#[test]
fn arabic_prefix_keeps_link_on_the_painted_cells_at_exact_fit_and_overflow() {
    let url = "https://example.com/pr/42";
    let mut line = HyperlinkLine::from("لا · ");
    line.push_span("PR #42".into(), Some(url));
    assert_eq!(line.width(), 11);

    for (width, visible_link) in [(11, "PR #42"), (10, "PR #")] {
        let area = Rect::new(/*x*/ 0, /*y*/ 0, width, /*height*/ 1);
        let mut buf = Buffer::empty(area);
        FooterContent::HyperlinkLine(line.clone()).render(area, &mut buf);
        assert_eq!(buf[(4, 0)].symbol(), " ");
        for (x, ch) in (5..width).zip(visible_link.chars()) {
            assert_eq!(buf[(x, 0)].symbol(), osc8_hyperlink(url, &ch.to_string()));
        }
        if width < 11 {
            assert_eq!(buf[(width - 1, 0)].symbol(), "…");
        }
    }
}
