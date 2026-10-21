//! Embedded composers preserve literal drafts and show validation feedback in place.

use super::tests::new_test_composer;
use super::*;
use pretty_assertions::assert_eq;

#[test]
fn plain_text_composer_keeps_command_prefixes_literal() {
    let (mut composer, _rx) = new_test_composer();
    composer.config = ChatComposerConfig::plain_text();
    composer.set_disable_paste_burst(/*disabled*/ true);
    composer.set_footer_hint_override(Some(Vec::new()));
    for (prefix, modifiers) in [
        ('!', KeyModifiers::NONE),
        ('/', KeyModifiers::NONE),
        ('?', KeyModifiers::NONE),
        ('?', KeyModifiers::SHIFT),
    ] {
        composer.handle_key_event(KeyEvent::new(KeyCode::Char(prefix), modifiers));
        composer.handle_paste("keep this literal".into());
        assert_eq!(
            composer.handle_key_event(KeyCode::Tab.into()).0,
            InputResult::Submitted {
                text: format!("{prefix}keep this literal"),
                text_elements: Vec::new(),
            }
        );
    }
}

#[test]
fn buffered_typing_survives_vim_changes_and_cancellation() {
    let (mut composer, _rx) = new_test_composer();
    composer.handle_key_event(KeyCode::Char('x').into());
    composer.set_vim_enabled(/*enabled*/ true);
    assert_eq!(composer.current_text(), "x");
    composer.set_vim_enabled(/*enabled*/ false);
    composer.handle_key_event(KeyCode::Char('y').into());
    assert_eq!(composer.clear_for_ctrl_c(), Some("xy".into()));
    assert!(!composer.is_in_paste_burst());
    composer.handle_key_event(KeyCode::Up.into());
    assert_eq!(composer.current_text(), "xy");
}

#[test]
fn paste_enter_retains_a_one_character_first_line() {
    let (mut composer, _rx) = new_test_composer();
    composer.handle_key_event(KeyCode::Char('x').into());
    assert!(composer.handle_paste_enter(Instant::now()));
    assert_eq!(composer.clear_for_ctrl_c(), Some("x\n".into()));
}

#[test]
fn embedded_composer_shows_validation_error_without_losing_draft() {
    let (mut composer, mut rx) = new_test_composer();
    composer.config = ChatComposerConfig::plain_text();
    composer.set_footer_hint_override(Some(vec![("enter".into(), "submit".into())]));
    composer.set_context_window_pending(/*pending*/ true);
    let text = "x".repeat(MAX_USER_INPUT_TEXT_CHARS + 1);
    composer.handle_paste(text.clone());
    assert_eq!(
        composer.handle_key_event(KeyCode::Enter.into()).0,
        InputResult::None
    );
    assert_eq!(composer.current_text_with_pending(), text);
    assert!(matches!(rx.try_recv(), Ok(AppEvent::InsertHistoryCell(_))));
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 5,
    );
    let mut buffer = Buffer::empty(area);
    composer.render(area, &mut buffer);
    let footer = (0..area.width)
        .map(|x| buffer[(x, area.bottom() - 1)].symbol())
        .collect::<String>();
    insta::assert_snapshot!(footer.trim_end(), @"  Message too long; limit 1048576 characters");
    assert!(composer.footer_flash_delay().is_some());
    composer.footer.flash.as_mut().unwrap().expires_at = Instant::now() - Duration::from_secs(1);
    assert_eq!(composer.footer_flash_delay(), None);
}
