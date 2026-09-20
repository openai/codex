//! Responsive navigation and report loading regression coverage.
use super::*;
use pretty_assertions::assert_eq;

#[test]
fn reports_keep_tabs_fixed_and_expose_data_on_small_screens() {
    for (width, height) in [(120, 36), (80, 24), (40, 16)] {
        let mut view = fixture::view(models::AccountKind::Business);
        view.section = Section::Usage;
        let tokens = screen(&mut view, width, height);
        assert!(tokens.contains("tokens"));
        assert!(tokens.contains("Sep 2"));
        let tabs = tokens.lines().take(3).collect::<Vec<_>>();
        view.sections[Section::Usage].history = Load::Error("Couldn't load tokens.".into());
        let failed = screen(&mut view, width, height);
        assert_eq!(tabs, failed.lines().take(3).collect::<Vec<_>>());
        assert!(failed.contains("Couldn't load tokens."));
        if width == 40 {
            insta::assert_snapshot!(format!("{tokens}\n{failed}"));
        }
    }
}

#[test]
fn keyboard_tabs_and_help_preserve_report_reading_position() {
    let mut view = fixture::view(models::AccountKind::Business);
    view.section = Section::Usage;
    view.sections[Section::Usage].detail = Some(6);
    screen(&mut view, /*width*/ 80, /*height*/ 16);
    press(&mut view, KeyCode::PageDown);
    screen(&mut view, /*width*/ 80, /*height*/ 16);
    let offset = view.scroll_offset();
    press(&mut view, KeyCode::BackTab);
    assert_eq!(view.section, Section::Credits);
    screen(&mut view, /*width*/ 80, /*height*/ 16);
    press(&mut view, KeyCode::PageDown);
    screen(&mut view, /*width*/ 80, /*height*/ 16);
    press(&mut view, KeyCode::Tab);
    assert_eq!(
        (
            view.section,
            view.scroll_offset(),
            view.sections[Section::Usage].detail
        ),
        (Section::Usage, offset, Some(6))
    );
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
        (view.show_help, view.is_done, view.scroll_offset()),
        (false, false, offset)
    );
    press(&mut view, KeyCode::Char('?'));
    assert_eq!(view.scroll_offset(), 0);
    press(&mut view, KeyCode::Char('?'));
    assert_eq!(view.scroll_offset(), offset);
    press(&mut view, KeyCode::Char('z'));
    screen(&mut view, /*width*/ 80, /*height*/ 16);
    let dashboard_offset = view.scroll_offset();
    press(&mut view, KeyCode::Char('?'));
    press(&mut view, KeyCode::Esc);
    assert_eq!(
        (view.zoomed, view.scroll_offset()),
        (false, dashboard_offset)
    );
    press(&mut view, KeyCode::Enter);
    screen(&mut view, /*width*/ 80, /*height*/ 16);
    assert_eq!(
        (
            view.zoomed,
            view.scroll_offset(),
            view.sections[Section::Usage].detail
        ),
        (true, offset, Some(6))
    );
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
