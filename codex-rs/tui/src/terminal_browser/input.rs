use codex_terminal_browser::BrowserInputModifiers;
use codex_terminal_browser::BrowserKeyInput;
use codex_terminal_browser::BrowserMouseButton;
use codex_terminal_browser::BrowserMouseInput;
use codex_terminal_browser::BrowserMouseKind;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use crossterm::event::MouseButton;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;
use ratatui::layout::Rect;

/// Converts a crossterm key event into Carbonyl input.
///
/// Release events and keys without a Chromium keyboard equivalent are ignored.
pub(crate) fn browser_key_input(event: KeyEvent) -> Option<BrowserKeyInput> {
    if !matches!(event.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
        return None;
    }
    let modifiers = browser_modifiers(event.modifiers);
    let (key, code, text) = match event.code {
        KeyCode::Char(character) => {
            let code = browser_key_code(character);
            let text = (!modifiers.control && !modifiers.alt && !modifiers.meta)
                .then(|| character.to_string());
            (character.to_string(), code, text)
        }
        KeyCode::Enter => ("Enter".to_string(), "Enter".to_string(), None),
        KeyCode::Tab | KeyCode::BackTab => ("Tab".to_string(), "Tab".to_string(), None),
        KeyCode::Backspace => ("Backspace".to_string(), "Backspace".to_string(), None),
        KeyCode::Delete => ("Delete".to_string(), "Delete".to_string(), None),
        KeyCode::Esc => ("Escape".to_string(), "Escape".to_string(), None),
        KeyCode::Left => ("ArrowLeft".to_string(), "ArrowLeft".to_string(), None),
        KeyCode::Right => ("ArrowRight".to_string(), "ArrowRight".to_string(), None),
        KeyCode::Up => ("ArrowUp".to_string(), "ArrowUp".to_string(), None),
        KeyCode::Down => ("ArrowDown".to_string(), "ArrowDown".to_string(), None),
        KeyCode::Home => ("Home".to_string(), "Home".to_string(), None),
        KeyCode::End => ("End".to_string(), "End".to_string(), None),
        KeyCode::PageUp => ("PageUp".to_string(), "PageUp".to_string(), None),
        KeyCode::PageDown => ("PageDown".to_string(), "PageDown".to_string(), None),
        KeyCode::F(number) => {
            let key = format!("F{number}");
            (key.clone(), key, None)
        }
        _ => return None,
    };
    let modifiers = if matches!(event.code, KeyCode::BackTab) {
        BrowserInputModifiers {
            shift: true,
            ..modifiers
        }
    } else {
        modifiers
    };
    Some(BrowserKeyInput {
        key,
        code,
        text,
        modifiers,
    })
}

fn browser_key_code(character: char) -> String {
    if character.is_ascii_alphabetic() {
        return format!("Key{}", character.to_ascii_uppercase());
    }
    if character.is_ascii_digit() {
        return format!("Digit{character}");
    }
    let code = match character {
        ' ' => "Space",
        '!' => "Digit1",
        '@' => "Digit2",
        '#' => "Digit3",
        '$' => "Digit4",
        '%' => "Digit5",
        '^' => "Digit6",
        '&' => "Digit7",
        '*' => "Digit8",
        '(' => "Digit9",
        ')' => "Digit0",
        '-' | '_' => "Minus",
        '=' | '+' => "Equal",
        '[' | '{' => "BracketLeft",
        ']' | '}' => "BracketRight",
        '\\' | '|' => "Backslash",
        ';' | ':' => "Semicolon",
        '\'' | '"' => "Quote",
        ',' | '<' => "Comma",
        '.' | '>' => "Period",
        '/' | '?' => "Slash",
        '`' | '~' => "Backquote",
        _ => return character.to_string(),
    };
    code.to_string()
}

/// Converts a crossterm mouse event into coordinates relative to `viewport`.
///
/// Events outside the browser viewport are ignored so surrounding panel chrome retains ownership
/// of its own interactions.
pub(crate) fn browser_mouse_input(event: MouseEvent, viewport: Rect) -> Option<BrowserMouseInput> {
    if viewport.is_empty() || !viewport.contains((event.column, event.row).into()) {
        return None;
    }
    let (kind, button) = match event.kind {
        MouseEventKind::Moved => (BrowserMouseKind::Move, BrowserMouseButton::None),
        MouseEventKind::Down(button) => (BrowserMouseKind::Down, browser_mouse_button(button)),
        MouseEventKind::Up(button) => (BrowserMouseKind::Up, browser_mouse_button(button)),
        MouseEventKind::Drag(button) => (BrowserMouseKind::Move, browser_mouse_button(button)),
        MouseEventKind::ScrollUp => (
            BrowserMouseKind::Wheel {
                delta_x: 0.0,
                delta_y: -100.0,
            },
            BrowserMouseButton::None,
        ),
        MouseEventKind::ScrollDown => (
            BrowserMouseKind::Wheel {
                delta_x: 0.0,
                delta_y: 100.0,
            },
            BrowserMouseButton::None,
        ),
        MouseEventKind::ScrollLeft => (
            BrowserMouseKind::Wheel {
                delta_x: -100.0,
                delta_y: 0.0,
            },
            BrowserMouseButton::None,
        ),
        MouseEventKind::ScrollRight => (
            BrowserMouseKind::Wheel {
                delta_x: 100.0,
                delta_y: 0.0,
            },
            BrowserMouseButton::None,
        ),
    };
    Some(BrowserMouseInput {
        kind,
        button,
        column: event.column.saturating_sub(viewport.x),
        row: event.row.saturating_sub(viewport.y),
        viewport_cols: viewport.width,
        viewport_rows: viewport.height,
        modifiers: browser_modifiers(event.modifiers),
    })
}

fn browser_mouse_button(button: MouseButton) -> BrowserMouseButton {
    match button {
        MouseButton::Left => BrowserMouseButton::Left,
        MouseButton::Middle => BrowserMouseButton::Middle,
        MouseButton::Right => BrowserMouseButton::Right,
    }
}

fn browser_modifiers(modifiers: KeyModifiers) -> BrowserInputModifiers {
    BrowserInputModifiers {
        alt: modifiers.contains(KeyModifiers::ALT),
        control: modifiers.contains(KeyModifiers::CONTROL),
        meta: modifiers.contains(KeyModifiers::SUPER),
        shift: modifiers.contains(KeyModifiers::SHIFT),
    }
}
