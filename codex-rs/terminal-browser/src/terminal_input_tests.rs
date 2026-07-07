use pretty_assertions::assert_eq;

use super::*;

#[test]
fn plain_text_and_navigation_keys_use_carbonyl_terminal_input() {
    let text = BrowserKeyInput {
        key: "?".to_string(),
        code: "Slash".to_string(),
        text: Some("?".to_string()),
        modifiers: BrowserInputModifiers {
            shift: true,
            ..Default::default()
        },
    };
    let enter = BrowserKeyInput {
        key: "Enter".to_string(),
        code: "Enter".to_string(),
        text: None,
        modifiers: BrowserInputModifiers::default(),
    };
    let command_left = BrowserKeyInput {
        key: "ArrowLeft".to_string(),
        code: "ArrowLeft".to_string(),
        text: None,
        modifiers: BrowserInputModifiers {
            meta: true,
            ..Default::default()
        },
    };

    assert_eq!(key_bytes(&text), Some(b"?".to_vec()));
    assert_eq!(key_bytes(&enter), Some(b"\r".to_vec()));
    assert_eq!(key_bytes(&command_left), Some(b"\x1b[1;9D".to_vec()));
}

#[test]
fn modified_printable_shortcuts_fall_back_to_cdp() {
    let control_c = BrowserKeyInput {
        key: "c".to_string(),
        code: "KeyC".to_string(),
        text: None,
        modifiers: BrowserInputModifiers {
            control: true,
            ..Default::default()
        },
    };

    assert_eq!(key_bytes(&control_c), None);
}

#[test]
fn unicode_text_falls_back_to_cdp() {
    let unicode = BrowserKeyInput {
        key: "é".to_string(),
        code: "é".to_string(),
        text: Some("é".to_string()),
        modifiers: BrowserInputModifiers::default(),
    };

    assert_eq!(key_bytes(&unicode), None);
}

#[test]
fn mouse_reports_preserve_carbonyl_coordinates_and_button_state() {
    let mut state = HumanTerminalMouseState::default();
    let press = BrowserMouseInput {
        kind: BrowserMouseKind::Down,
        button: BrowserMouseButton::Left,
        column: 1,
        row: 0,
        viewport_cols: 40,
        viewport_rows: 20,
        modifiers: BrowserInputModifiers::default(),
    };
    let drag = BrowserMouseInput {
        kind: BrowserMouseKind::Move,
        column: 2,
        row: 1,
        ..press
    };

    assert_eq!(
        mouse_bytes(press, &mut state),
        Some(b"\x1b[<0;2;1M".to_vec())
    );
    assert_eq!(
        mouse_bytes(drag, &mut state),
        Some(b"\x1b[<32;3;2M".to_vec())
    );
    assert_eq!(
        release_mouse_bytes(&mut state),
        Some(b"\x1b[<0;3;2m".to_vec())
    );
    assert_eq!(release_mouse_bytes(&mut state), None);
}

#[test]
fn wheel_reports_use_carbonyl_scroll_buttons() {
    let mut state = HumanTerminalMouseState::default();
    let scroll_up = BrowserMouseInput {
        kind: BrowserMouseKind::Wheel {
            delta_x: 0.0,
            delta_y: -100.0,
        },
        button: BrowserMouseButton::None,
        column: 4,
        row: 3,
        viewport_cols: 40,
        viewport_rows: 20,
        modifiers: BrowserInputModifiers::default(),
    };

    assert_eq!(
        mouse_bytes(scroll_up, &mut state),
        Some(b"\x1b[<64;5;4M".to_vec())
    );
}
