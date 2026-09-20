//! Fixed navigation and keyboard help around existing report bodies.
use super::*;
use pretty_assertions::assert_eq;

#[test]
fn reports_keep_tabs_fixed_when_a_request_fails() {
    for (width, height) in [(120, 36), (80, 24), (40, 16)] {
        let mut view = fixture::view(models::AccountKind::Business);
        view.section = Section::Usage;
        let loaded = screen(&mut view, width, height);
        view.sections[Section::Usage].history = Load::Error("Couldn't load tokens.".into());
        let failed = screen(&mut view, width, height);
        assert_eq!(
            loaded.lines().take(3).collect::<Vec<_>>(),
            failed.lines().take(3).collect::<Vec<_>>()
        );
        assert!(failed.contains("Couldn't load tokens."));
    }
}

#[test]
fn keyboard_help_preserves_report_selection_and_reading_position() {
    let mut view = fixture::view(models::AccountKind::Business);
    view.section = Section::Usage;
    view.sections[Section::Usage].detail = Some(6);
    screen(&mut view, /*width*/ 80, /*height*/ 16);
    press(&mut view, KeyCode::PageDown);
    screen(&mut view, /*width*/ 80, /*height*/ 16);
    let offset = view.scroll_offset();
    view.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT));
    let help = screen(&mut view, /*width*/ 40, /*height*/ 16);
    assert!(help.contains("Usage shortcuts"));
    assert!(help.lines().nth_back(/*n*/ 1).unwrap().contains("scroll"));
    assert!(!help.lines().last().unwrap().contains("tab report"));
    insta::assert_snapshot!("narrow_help", help);
    press(&mut view, KeyCode::PageDown);
    screen(&mut view, /*width*/ 40, /*height*/ 16);
    assert!(view.scroll_offset() > 0);
    press(&mut view, KeyCode::Esc);
    assert_eq!(
        (
            view.show_help,
            view.is_done,
            view.section,
            view.sections[Section::Usage].detail,
            view.scroll_offset()
        ),
        (false, false, Section::Usage, Some(6), offset)
    );
    press(&mut view, KeyCode::Char('?'));
    assert_eq!(view.scroll_offset(), 0);
    press(&mut view, KeyCode::Char('?'));
    assert_eq!(view.scroll_offset(), offset);
}

#[test]
fn dashboard_navigation_scrolls_to_selected_card() {
    let mut view = fixture::view(models::AccountKind::Consumer);
    press(&mut view, KeyCode::Char('z'));
    screen(&mut view, /*width*/ 80, /*height*/ 24);
    press(&mut view, KeyCode::Char('6'));
    let output = screen(&mut view, /*width*/ 80, /*height*/ 24);
    assert!(view.scroll_offset() > 0);
    assert!(
        output
            .lines()
            .any(|line| line.contains('▸') && line.contains("Chats"))
    );
}
