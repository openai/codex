use crate::input::BrowserInputModifiers;
use crate::input::BrowserKeyInput;
use crate::input::BrowserMouseButton;
use crate::input::BrowserMouseInput;
use crate::input::BrowserMouseKind;

/// Tracks the pointer state Carbonyl expects across terminal mouse reports.
#[derive(Default)]
pub(crate) struct HumanTerminalMouseState {
    last_position: Option<(u16, u16)>,
    pressed_button: BrowserMouseButton,
}

/// Encodes a browser key as terminal input when Carbonyl understands that key directly.
///
/// Modified printable shortcuts fall back to CDP so combinations such as Ctrl+C do not become
/// Carbonyl process-control bytes.
pub(crate) fn key_bytes(input: &BrowserKeyInput) -> Option<Vec<u8>> {
    if let Some(text) = input.text.as_deref()
        && !text.is_empty()
    {
        return Some(text.as_bytes().to_vec());
    }
    match input.key.as_str() {
        "Enter" => Some(vec![b'\r']),
        "Tab" if !input.modifiers.shift => Some(vec![b'\t']),
        "Backspace" => Some(vec![0x7f]),
        "Escape" => Some(vec![0x1b]),
        "ArrowUp" => Some(arrow_bytes(/*key*/ b'A', input.modifiers)),
        "ArrowDown" => Some(arrow_bytes(/*key*/ b'B', input.modifiers)),
        "ArrowRight" => Some(arrow_bytes(/*key*/ b'C', input.modifiers)),
        "ArrowLeft" => Some(arrow_bytes(/*key*/ b'D', input.modifiers)),
        "Tab" | "Delete" | "Home" | "End" | "PageUp" | "PageDown" => None,
        _ => None,
    }
}

pub(crate) fn mouse_bytes(
    input: BrowserMouseInput,
    state: &mut HumanTerminalMouseState,
) -> Option<Vec<u8>> {
    state.last_position = Some((input.column, input.row));
    let modifiers = mouse_modifier_bits(input.modifiers);
    let (button_code, suffix) = match input.kind {
        BrowserMouseKind::Down => {
            state.pressed_button = input.button;
            (mouse_button_code(input.button)? + modifiers, b'M')
        }
        BrowserMouseKind::Up => {
            state.pressed_button = BrowserMouseButton::None;
            (mouse_button_code(input.button)? + modifiers, b'm')
        }
        BrowserMouseKind::Move => {
            let button = if state.pressed_button == BrowserMouseButton::None {
                input.button
            } else {
                state.pressed_button
            };
            let button_code = mouse_button_code(button).unwrap_or(/*default*/ 3);
            (button_code + 32 + modifiers, b'M')
        }
        BrowserMouseKind::Wheel { delta_y, .. } if delta_y < 0.0 => (64 + modifiers, b'M'),
        BrowserMouseKind::Wheel { delta_y, .. } if delta_y > 0.0 => (65 + modifiers, b'M'),
        BrowserMouseKind::Wheel { .. } => return None,
    };
    Some(sgr_mouse_bytes(
        button_code,
        input.column,
        input.row,
        suffix,
    ))
}

pub(crate) fn release_mouse_bytes(state: &mut HumanTerminalMouseState) -> Option<Vec<u8>> {
    let button = std::mem::take(&mut state.pressed_button);
    let (column, row) = state.last_position?;
    Some(sgr_mouse_bytes(
        mouse_button_code(button)?,
        column,
        row,
        /*suffix*/ b'm',
    ))
}

fn arrow_bytes(key: u8, modifiers: BrowserInputModifiers) -> Vec<u8> {
    let modifier = keyboard_modifier_code(modifiers);
    if modifier == 1 {
        vec![0x1b, b'[', key]
    } else {
        format!("\x1b[1;{modifier}{key}", key = char::from(key)).into_bytes()
    }
}

fn keyboard_modifier_code(modifiers: BrowserInputModifiers) -> u8 {
    1 + u8::from(modifiers.shift)
        + (u8::from(modifiers.alt) << 1)
        + (u8::from(modifiers.control) << 2)
        + (u8::from(modifiers.meta) << 3)
}

fn mouse_modifier_bits(modifiers: BrowserInputModifiers) -> u8 {
    (u8::from(modifiers.shift) << 2)
        | (u8::from(modifiers.alt || modifiers.meta) << 3)
        | (u8::from(modifiers.control) << 4)
}

fn mouse_button_code(button: BrowserMouseButton) -> Option<u8> {
    match button {
        BrowserMouseButton::None => None,
        BrowserMouseButton::Left => Some(0),
        BrowserMouseButton::Middle => Some(1),
        BrowserMouseButton::Right => Some(2),
    }
}

fn sgr_mouse_bytes(button: u8, column: u16, row: u16, suffix: u8) -> Vec<u8> {
    format!(
        "\x1b[<{button};{};{}{}",
        column.saturating_add(/*rhs*/ 1),
        row.saturating_add(/*rhs*/ 1),
        char::from(suffix)
    )
    .into_bytes()
}

#[cfg(test)]
#[path = "terminal_input_tests.rs"]
mod tests;
