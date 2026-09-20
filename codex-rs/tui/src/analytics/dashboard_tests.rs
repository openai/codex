//! Overview cards retain alignment and geometry while selection and report states change.
use super::*;
use crate::style::accent_style;
use pretty_assertions::assert_eq;
use ratatui::style::Modifier;

use crate::analytics::sections::Section;
#[test]
fn dashboard_focus_uses_marker_and_default_foreground_title() {
    let mut view = fixture::view(models::AccountKind::Consumer);
    view.zoomed = false;
    for section in [Section::Usage, Section::Activity] {
        view.section = section;
        let (lines, _) = view.dashboard_lines(/*width*/ 140, /*height*/ 56);
        let title = lines
            .iter()
            .find(|line| line.spans.iter().any(|span| span.content == "▸"))
            .unwrap();
        let marker = title.spans.iter().find(|span| span.content == "▸").unwrap();
        assert_eq!(marker.style, accent_style());
        let focused_title = title
            .spans
            .iter()
            .find(|span| span.content.contains(view.tab_label(section)))
            .unwrap();
        assert_eq!(focused_title.style.fg, None);
        assert!(focused_title.style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(title.to_string().matches('▸').count(), 1);
    }
}

#[test]
fn dashboard_cards_align_and_keep_stable_summary_heights() {
    let mut view = fixture::view(models::AccountKind::Consumer);
    press(&mut view, KeyCode::Char('z'));
    let wide = screen(&mut view, /*width*/ 144, /*height*/ 64);
    for (left, right) in [
        ("1 Overview", "2 Usage"),
        ("3 Messages", "4 Plugins"),
        ("5 Skills", "6 Chats"),
    ] {
        assert!(
            wide.lines()
                .any(|line| line.contains(left) && line.contains(right))
        );
    }
    assert!(!wide.contains("Coverage from"));
    let before = view.dashboard_lines(/*width*/ 140, /*height*/ 56);
    press(&mut view, KeyCode::Left);
    view.sections[Section::Plugins].history = Load::Error("Access denied for this report.".into());
    let after = view.dashboard_lines(/*width*/ 140, /*height*/ 56);
    assert_eq!((before.0.len(), before.1), (after.0.len(), after.1));
    let edges = |lines: &[ratatui::text::Line<'_>]| {
        lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.to_string().starts_with(['╭', '╰']))
            .map(|(index, _)| index)
            .collect::<Vec<_>>()
    };
    assert_eq!(edges(&before.0), edges(&after.0));
    for width in [1, 4, 40, 100, 140] {
        assert!(
            view.dashboard_lines(width, /*height*/ 56)
                .0
                .iter()
                .all(|line| line.width() <= width)
        );
    }
    let narrow = screen(&mut view, /*width*/ 72, /*height*/ 48);
    assert!(narrow.contains("1 Overview"));
    let context = (
        view.section,
        view.sections.0.each_ref().map(|state| state.cursor),
        view.sections.0.each_ref().map(|state| state.group),
    );
    press(&mut view, KeyCode::Enter);
    assert!(view.zoomed);
    assert_eq!(
        context,
        (
            view.section,
            view.sections.0.each_ref().map(|state| state.cursor),
            view.sections.0.each_ref().map(|state| state.group)
        )
    );
    insta::assert_snapshot!(format!("{wide}\n{narrow}"));
}

#[test]
fn overview_closes_on_escape_with_retained_day_details() {
    let mut view = fixture::view(models::AccountKind::Consumer);
    press(&mut view, KeyCode::Enter);
    assert_eq!(view.sections[Section::Usage].detail, Some(6));
    press(&mut view, KeyCode::Char('z'));
    assert!(!view.zoomed);
    press(&mut view, KeyCode::Esc);
    assert!(view.is_done);
}
