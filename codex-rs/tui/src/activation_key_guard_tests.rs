use super::*;
use crate::key_hint;
use crossterm::event::KeyCode;

#[test]
fn suppresses_activation_until_release_and_bounds_legacy_presses() {
    let start = Instant::now();
    let key = key_hint::plain(KeyCode::Enter);
    let mut guard = ActivationKeyGuard::new_at(key, start, /*reported_repeats_only*/ true);

    assert!(guard.consume_at(key, KeyEventKind::Repeat, start));
    assert!(guard.consume_at(key, KeyEventKind::Release, start));
    assert!(!guard.consume_at(key, KeyEventKind::Press, start));

    let mut guard = ActivationKeyGuard::new_at(key, start, /*reported_repeats_only*/ false);
    for elapsed in [900, 1_300] {
        assert!(guard.consume_at(
            key,
            KeyEventKind::Press,
            start + Duration::from_millis(/*millis*/ elapsed),
        ));
    }
    assert!(!guard.consume_at(
        key,
        KeyEventKind::Press,
        start + Duration::from_millis(/*millis*/ 2_301),
    ));
    assert!(!guard.is_active());

    let mut guard = ActivationKeyGuard::new_at(key, start, /*reported_repeats_only*/ true);
    let modal_visible = start + Duration::from_secs(/*secs*/ 5);
    guard.extend_for_protected_view(modal_visible);
    assert!(guard.consume_at(key, KeyEventKind::Press, modal_visible));
    assert!(guard.consume_at(key, KeyEventKind::Release, modal_visible));
    assert!(!guard.consume_at(key, KeyEventKind::Press, modal_visible));

    let mut guard = ActivationKeyGuard::new_at(
        key_hint::plain(KeyCode::BackTab),
        start,
        /*reported_repeats_only*/ true,
    );
    let tab = key_hint::plain(KeyCode::Tab);
    assert!(guard.consume_at(tab, KeyEventKind::Repeat, start));
    assert!(guard.consume_at(tab, KeyEventKind::Release, start));
    assert!(!guard.is_active());
}
