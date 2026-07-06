use super::StartupInput;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyEventState;
use crossterm::event::KeyModifiers;
use crossterm::event::MediaKeyCode;
use crossterm::event::ModifierKeyCode;

use super::MAX_STARTUP_INPUT_BYTES;

const PASTE_START: &[u8] = b"\x1b[200~";
const PASTE_END: &[u8] = b"\x1b[201~";
const STARTUP_OSC_RESPONSE_PREFIXES: [&[u8]; 2] = [b"\x1b]10;", b"\x1b]11;"];
const MAX_STARTUP_INPUT_CONTROLS: usize = 32 * 1024;
const MAX_STARTUP_INPUT_ACTIONS: usize = 32 * 1024;

#[derive(Debug, Eq, PartialEq)]
pub(super) struct ExtractedStartupInput {
    pub(super) input: Vec<StartupInput>,
    pub(super) complete: bool,
    pub(super) paste_open: bool,
}

#[derive(Clone, Copy)]
pub(super) enum IncompleteInputPhase {
    QueuedUserInput,
    ProbeResponse,
}

pub(super) fn parse_startup_input(buffer: &[u8]) -> ExtractedStartupInput {
    parse_startup_input_with_osc_settlement(buffer, false)
}

fn parse_startup_input_with_osc_settlement(
    buffer: &[u8],
    settle_incomplete_osc: bool,
) -> ExtractedStartupInput {
    let mut input = Vec::new();
    let mut input_bytes = 0;
    let mut input_controls = 0;
    let mut input_actions = 0;
    let mut input_truncated = false;
    let mut index = 0;
    let mut complete = true;
    let mut in_paste = false;
    while index < buffer.len() {
        if in_paste {
            let remaining = &buffer[index..];
            if remaining.starts_with(PASTE_END) {
                in_paste = false;
                index += PASTE_END.len();
                continue;
            }
            if PASTE_END.starts_with(remaining) {
                complete = false;
                break;
            }
            push_startup_input_byte(
                &mut input,
                &mut input_bytes,
                &mut input_controls,
                &mut input_truncated,
                /*paste*/ true,
                buffer[index],
                MAX_STARTUP_INPUT_BYTES,
            );
            index += 1;
            continue;
        }

        if buffer[index] != b'\x1b' {
            push_startup_input_byte(
                &mut input,
                &mut input_bytes,
                &mut input_controls,
                &mut input_truncated,
                /*paste*/ false,
                buffer[index],
                MAX_STARTUP_INPUT_BYTES,
            );
            index += 1;
            continue;
        }

        let remaining = &buffer[index..];
        if remaining.starts_with(PASTE_START) {
            in_paste = true;
            index += PASTE_START.len();
            continue;
        }
        if PASTE_START.starts_with(remaining) {
            complete = false;
            break;
        }

        // A standalone Escape immediately before a terminal reply is not an Alt+Escape key.
        // Leave the second Escape for the next parser iteration so the complete reply remains
        // framed and can be removed without leaking its payload into the startup draft.
        if remaining.starts_with(b"\x1b\x1b") {
            push_startup_input_action(
                &mut input,
                &mut input_actions,
                &mut input_truncated,
                StartupInput::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            );
            index += 1;
            continue;
        }
        if remaining.starts_with(b"\x1b[\x1b") {
            push_startup_input_action(
                &mut input,
                &mut input_actions,
                &mut input_truncated,
                StartupInput::Key(KeyEvent::new(KeyCode::Char('['), KeyModifiers::ALT)),
            );
            index += 2;
            continue;
        }
        if remaining.starts_with(b"\x1bO\x1b") {
            push_startup_input_action(
                &mut input,
                &mut input_actions,
                &mut input_truncated,
                StartupInput::Key(KeyEvent::new(KeyCode::Char('O'), KeyModifiers::ALT)),
            );
            index += 2;
            continue;
        }

        // Linux consoles encode F1-F5 as `CSI [ A` through `CSI [ E`. The second `[` is in the
        // normal CSI-final range, so recognize this legacy exception before generic CSI parsing.
        if remaining.starts_with(b"\x1b[[") {
            let Some(final_byte) = remaining.get(3) else {
                complete = false;
                break;
            };
            let key = match *final_byte {
                value @ b'A'..=b'E' => Some(KeyEvent::new(
                    KeyCode::F(value - b'A' + 1),
                    KeyModifiers::NONE,
                )),
                _ => None,
            };
            push_startup_input_action(
                &mut input,
                &mut input_actions,
                &mut input_truncated,
                key.map_or(StartupInput::UnknownAction, StartupInput::Key),
            );
            index += 4;
            continue;
        }

        match buffer.get(index + 1) {
            Some(b'[') => {
                let Some(end) = buffer[index + 2..]
                    .iter()
                    .position(|byte| (0x40..=0x7e).contains(byte))
                else {
                    complete = false;
                    break;
                };
                let sequence_end = index + end + 3;
                let sequence = &buffer[index..sequence_end];
                if !is_startup_csi_response(sequence) {
                    push_startup_input_action(
                        &mut input,
                        &mut input_actions,
                        &mut input_truncated,
                        decode_csi_key(sequence)
                            .map_or(StartupInput::UnknownAction, StartupInput::Key),
                    );
                }
                index = sequence_end;
            }
            Some(b']') => {
                let sequence_end = osc_sequence_end(buffer, index);
                if let Some(end) = sequence_end
                    && is_startup_osc_response(&buffer[index..end])
                {
                    index = end;
                    continue;
                }
                if sequence_end.is_none()
                    && !settle_incomplete_osc
                    && could_be_startup_osc_response(remaining)
                {
                    complete = false;
                    break;
                }
                push_startup_input_action(
                    &mut input,
                    &mut input_actions,
                    &mut input_truncated,
                    StartupInput::Key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::ALT)),
                );
                index += 2;
            }
            Some(b'O') => {
                let Some(final_byte) = buffer.get(index + 2) else {
                    complete = false;
                    break;
                };
                if !(0x40..=0x7e).contains(final_byte) {
                    complete = false;
                    break;
                }
                let key = match *final_byte {
                    b'A' => Some(KeyCode::Up),
                    b'B' => Some(KeyCode::Down),
                    b'C' => Some(KeyCode::Right),
                    b'D' => Some(KeyCode::Left),
                    b'F' => Some(KeyCode::End),
                    b'H' => Some(KeyCode::Home),
                    b'P' => Some(KeyCode::F(1)),
                    b'Q' => Some(KeyCode::F(2)),
                    b'R' => Some(KeyCode::F(3)),
                    b'S' => Some(KeyCode::F(4)),
                    _ => None,
                }
                .map(|code| KeyEvent::new(code, KeyModifiers::NONE));
                push_startup_input_action(
                    &mut input,
                    &mut input_actions,
                    &mut input_truncated,
                    key.map_or(StartupInput::UnknownAction, StartupInput::Key),
                );
                index += 3;
            }
            Some(byte) if !byte.is_ascii() => {
                let scalar_width = utf8_scalar_width(*byte);
                if scalar_width == 0 {
                    index += 2;
                    continue;
                }
                if buffer.len() < index + 1 + scalar_width {
                    complete = false;
                    break;
                }
                let key = std::str::from_utf8(&buffer[index + 1..index + 1 + scalar_width])
                    .ok()
                    .and_then(|text| text.chars().next())
                    .map(|ch| KeyEvent::new(KeyCode::Char(ch), KeyModifiers::ALT));
                push_startup_input_action(
                    &mut input,
                    &mut input_actions,
                    &mut input_truncated,
                    key.map_or(StartupInput::UnknownAction, StartupInput::Key),
                );
                index += 1 + scalar_width;
            }
            Some(byte) => {
                let key = key_code_from_codepoint(u32::from(*byte))
                    .map(|code| KeyEvent::new(code, KeyModifiers::ALT));
                push_startup_input_action(
                    &mut input,
                    &mut input_actions,
                    &mut input_truncated,
                    key.map_or(StartupInput::UnknownAction, StartupInput::Key),
                );
                index += 2;
            }
            None => {
                complete = false;
                break;
            }
        }
    }

    let paste_open = in_paste;
    if paste_open {
        complete = false;
    }
    for input in &mut input {
        if let StartupInput::Plain(bytes) | StartupInput::Paste(bytes) = input
            && let Some(incomplete_start) = incomplete_utf8_suffix_start(bytes)
        {
            bytes.truncate(incomplete_start);
            if !input_truncated {
                complete = false;
            }
        }
    }
    ExtractedStartupInput {
        input,
        complete,
        paste_open,
    }
}

fn osc_sequence_end(buffer: &[u8], start: usize) -> Option<usize> {
    let mut end = start + 2;
    while end < buffer.len() {
        if buffer[end] == b'\x07' {
            return Some(end + 1);
        }
        if buffer[end] == b'\x1b' && buffer.get(end + 1) == Some(&b'\\') {
            return Some(end + 2);
        }
        end += 1;
    }
    None
}

fn could_be_startup_osc_response(sequence: &[u8]) -> bool {
    STARTUP_OSC_RESPONSE_PREFIXES
        .iter()
        .any(|prefix| prefix.starts_with(sequence) || sequence.starts_with(prefix))
}

fn is_startup_osc_response(sequence: &[u8]) -> bool {
    let sequence = sequence
        .strip_suffix(b"\x07")
        .or_else(|| sequence.strip_suffix(b"\x1b\\"));
    let Some(sequence) = sequence else {
        return false;
    };
    STARTUP_OSC_RESPONSE_PREFIXES.iter().any(|prefix| {
        sequence
            .strip_prefix(*prefix)
            .is_some_and(|payload| !payload.contains(&b'\x1b') && !payload.contains(&b'\x07'))
    })
}

fn is_startup_csi_response(sequence: &[u8]) -> bool {
    if sequence.starts_with(b"\x1b[?") && matches!(sequence.last(), Some(b'u' | b'c')) {
        return true;
    }
    if sequence.last() != Some(&b'R') {
        return false;
    }
    let Ok(payload) = std::str::from_utf8(&sequence[2..sequence.len() - 1]) else {
        return false;
    };
    let Some((row, column)) = payload.split_once(';') else {
        return false;
    };
    column == "1" && row.parse::<u16>().is_ok()
}

fn decode_csi_key(sequence: &[u8]) -> Option<KeyEvent> {
    let final_byte = *sequence.last()?;
    let payload = std::str::from_utf8(&sequence[2..sequence.len() - 1]).ok()?;
    match final_byte {
        b'u' => decode_csi_u_key(payload),
        b'~' => decode_csi_tilde_key(payload),
        b'Z' if payload.is_empty() => Some(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)),
        b'A' | b'B' | b'C' | b'D' | b'F' | b'H' | b'P' | b'Q' | b'R' | b'S' => {
            let code = match final_byte {
                b'A' => KeyCode::Up,
                b'B' => KeyCode::Down,
                b'C' => KeyCode::Right,
                b'D' => KeyCode::Left,
                b'F' => KeyCode::End,
                b'H' => KeyCode::Home,
                b'P' => KeyCode::F(1),
                b'Q' => KeyCode::F(2),
                b'R' => KeyCode::F(3),
                b'S' => KeyCode::F(4),
                _ => return None,
            };
            let modifier_field = payload.rsplit_once(';').map(|(_, field)| field);
            let (modifiers, kind) = decode_modifiers_and_kind(modifier_field);
            Some(KeyEvent::new_with_kind(code, modifiers, kind))
        }
        _ => None,
    }
}

fn decode_csi_u_key(payload: &str) -> Option<KeyEvent> {
    let mut fields = payload.split(';');
    let mut codepoints = fields.next()?.split(':');
    let codepoint = codepoints.next()?.parse::<u32>().ok()?;
    let modifier_field = fields.next();
    let (mut modifiers, kind) = decode_modifiers_and_kind(modifier_field);
    let (mut code, key_state) = match kitty_functional_key(codepoint) {
        Some(key) => key,
        // Kitty reserves the Unicode private-use area for functional keys. An unrecognized value
        // is still an action, never printable prompt text.
        None if (0xe000..=0xf8ff).contains(&codepoint) => return None,
        None => (key_code_from_codepoint(codepoint)?, KeyEventState::empty()),
    };
    add_modifier_for_modifier_key(code, &mut modifiers);
    if modifiers.contains(KeyModifiers::SHIFT)
        && let Some(shifted) = codepoints
            .next()
            .and_then(|codepoint| codepoint.parse::<u32>().ok())
            .and_then(char::from_u32)
    {
        code = KeyCode::Char(shifted);
        modifiers.remove(KeyModifiers::SHIFT);
    }
    if code == KeyCode::Tab && modifiers.contains(KeyModifiers::SHIFT) {
        code = KeyCode::BackTab;
    }
    Some(KeyEvent::new_with_kind_and_state(
        code,
        modifiers,
        kind,
        key_state | decode_key_event_state(modifier_field),
    ))
}

fn kitty_functional_key(codepoint: u32) -> Option<(KeyCode, KeyEventState)> {
    let code = match codepoint {
        57348 => KeyCode::Insert,
        57349 => KeyCode::Delete,
        57350 => KeyCode::Left,
        57351 => KeyCode::Right,
        57352 => KeyCode::Up,
        57353 => KeyCode::Down,
        57354 => KeyCode::PageUp,
        57355 => KeyCode::PageDown,
        57356 => KeyCode::Home,
        57357 => KeyCode::End,
        57358 => KeyCode::CapsLock,
        57359 => KeyCode::ScrollLock,
        57360 => KeyCode::NumLock,
        57361 => KeyCode::PrintScreen,
        57362 => KeyCode::Pause,
        57363 => KeyCode::Menu,
        value @ 57364..=57398 => KeyCode::F((value - 57363) as u8),
        57399 => KeyCode::Char('0'),
        57400 => KeyCode::Char('1'),
        57401 => KeyCode::Char('2'),
        57402 => KeyCode::Char('3'),
        57403 => KeyCode::Char('4'),
        57404 => KeyCode::Char('5'),
        57405 => KeyCode::Char('6'),
        57406 => KeyCode::Char('7'),
        57407 => KeyCode::Char('8'),
        57408 => KeyCode::Char('9'),
        57409 => KeyCode::Char('.'),
        57410 => KeyCode::Char('/'),
        57411 => KeyCode::Char('*'),
        57412 => KeyCode::Char('-'),
        57413 => KeyCode::Char('+'),
        57414 => KeyCode::Enter,
        57415 => KeyCode::Char('='),
        57416 => KeyCode::Char(','),
        57417 => KeyCode::Left,
        57418 => KeyCode::Right,
        57419 => KeyCode::Up,
        57420 => KeyCode::Down,
        57421 => KeyCode::PageUp,
        57422 => KeyCode::PageDown,
        57423 => KeyCode::Home,
        57424 => KeyCode::End,
        57425 => KeyCode::Insert,
        57426 => KeyCode::Delete,
        57427 => KeyCode::KeypadBegin,
        57428 => KeyCode::Media(MediaKeyCode::Play),
        57429 => KeyCode::Media(MediaKeyCode::Pause),
        57430 => KeyCode::Media(MediaKeyCode::PlayPause),
        57431 => KeyCode::Media(MediaKeyCode::Reverse),
        57432 => KeyCode::Media(MediaKeyCode::Stop),
        57433 => KeyCode::Media(MediaKeyCode::FastForward),
        57434 => KeyCode::Media(MediaKeyCode::Rewind),
        57435 => KeyCode::Media(MediaKeyCode::TrackNext),
        57436 => KeyCode::Media(MediaKeyCode::TrackPrevious),
        57437 => KeyCode::Media(MediaKeyCode::Record),
        57438 => KeyCode::Media(MediaKeyCode::LowerVolume),
        57439 => KeyCode::Media(MediaKeyCode::RaiseVolume),
        57440 => KeyCode::Media(MediaKeyCode::MuteVolume),
        57441 => KeyCode::Modifier(ModifierKeyCode::LeftShift),
        57442 => KeyCode::Modifier(ModifierKeyCode::LeftControl),
        57443 => KeyCode::Modifier(ModifierKeyCode::LeftAlt),
        57444 => KeyCode::Modifier(ModifierKeyCode::LeftSuper),
        57445 => KeyCode::Modifier(ModifierKeyCode::LeftHyper),
        57446 => KeyCode::Modifier(ModifierKeyCode::LeftMeta),
        57447 => KeyCode::Modifier(ModifierKeyCode::RightShift),
        57448 => KeyCode::Modifier(ModifierKeyCode::RightControl),
        57449 => KeyCode::Modifier(ModifierKeyCode::RightAlt),
        57450 => KeyCode::Modifier(ModifierKeyCode::RightSuper),
        57451 => KeyCode::Modifier(ModifierKeyCode::RightHyper),
        57452 => KeyCode::Modifier(ModifierKeyCode::RightMeta),
        57453 => KeyCode::Modifier(ModifierKeyCode::IsoLevel3Shift),
        57454 => KeyCode::Modifier(ModifierKeyCode::IsoLevel5Shift),
        _ => return None,
    };
    let state = if (57399..=57427).contains(&codepoint) {
        KeyEventState::KEYPAD
    } else {
        KeyEventState::empty()
    };
    Some((code, state))
}

fn add_modifier_for_modifier_key(code: KeyCode, modifiers: &mut KeyModifiers) {
    let KeyCode::Modifier(modifier) = code else {
        return;
    };
    match modifier {
        ModifierKeyCode::LeftShift | ModifierKeyCode::RightShift => {
            modifiers.insert(KeyModifiers::SHIFT);
        }
        ModifierKeyCode::LeftControl | ModifierKeyCode::RightControl => {
            modifiers.insert(KeyModifiers::CONTROL);
        }
        ModifierKeyCode::LeftAlt | ModifierKeyCode::RightAlt => {
            modifiers.insert(KeyModifiers::ALT);
        }
        ModifierKeyCode::LeftSuper | ModifierKeyCode::RightSuper => {
            modifiers.insert(KeyModifiers::SUPER);
        }
        ModifierKeyCode::LeftHyper | ModifierKeyCode::RightHyper => {
            modifiers.insert(KeyModifiers::HYPER);
        }
        ModifierKeyCode::LeftMeta | ModifierKeyCode::RightMeta => {
            modifiers.insert(KeyModifiers::META);
        }
        ModifierKeyCode::IsoLevel3Shift | ModifierKeyCode::IsoLevel5Shift => {}
    }
}

fn decode_csi_tilde_key(payload: &str) -> Option<KeyEvent> {
    let fields = payload.split(';').collect::<Vec<_>>();
    if fields.as_slice() == ["200"] || fields.as_slice() == ["201"] {
        return None;
    }
    if let ["27", modifier, codepoint] = fields.as_slice() {
        let (modifiers, kind) = decode_modifiers_and_kind(Some(modifier));
        let code = key_code_from_codepoint(codepoint.parse::<u32>().ok()?)?;
        return Some(KeyEvent::new_with_kind(code, modifiers, kind));
    }

    let code = match fields.first()?.parse::<u8>().ok()? {
        1 | 7 => KeyCode::Home,
        2 => KeyCode::Insert,
        3 => KeyCode::Delete,
        4 | 8 => KeyCode::End,
        5 => KeyCode::PageUp,
        6 => KeyCode::PageDown,
        value @ 11..=15 => KeyCode::F(value - 10),
        value @ 17..=21 => KeyCode::F(value - 11),
        value @ 23..=26 => KeyCode::F(value - 12),
        value @ 28..=29 => KeyCode::F(value - 15),
        value @ 31..=34 => KeyCode::F(value - 17),
        _ => return None,
    };
    let (modifiers, kind) = decode_modifiers_and_kind(fields.get(1).copied());
    Some(KeyEvent::new_with_kind(code, modifiers, kind))
}

fn decode_modifiers_and_kind(field: Option<&str>) -> (KeyModifiers, KeyEventKind) {
    let mut parts = field.unwrap_or("1").split(':');
    let mask = parts
        .next()
        .and_then(|mask| mask.parse::<u8>().ok())
        .unwrap_or(1)
        .saturating_sub(1);
    let mut modifiers = KeyModifiers::NONE;
    modifiers.set(KeyModifiers::SHIFT, mask & 1 != 0);
    modifiers.set(KeyModifiers::ALT, mask & 2 != 0);
    modifiers.set(KeyModifiers::CONTROL, mask & 4 != 0);
    modifiers.set(KeyModifiers::SUPER, mask & 8 != 0);
    modifiers.set(KeyModifiers::HYPER, mask & 16 != 0);
    modifiers.set(KeyModifiers::META, mask & 32 != 0);
    let kind = match parts.next().and_then(|kind| kind.parse::<u8>().ok()) {
        Some(2) => KeyEventKind::Repeat,
        Some(3) => KeyEventKind::Release,
        _ => KeyEventKind::Press,
    };
    (modifiers, kind)
}

fn decode_key_event_state(field: Option<&str>) -> KeyEventState {
    let mask = field
        .unwrap_or("1")
        .split(':')
        .next()
        .and_then(|mask| mask.parse::<u16>().ok())
        .unwrap_or(1)
        .saturating_sub(1);
    let mut state = KeyEventState::empty();
    state.set(KeyEventState::CAPS_LOCK, mask & 64 != 0);
    state.set(KeyEventState::NUM_LOCK, mask & 128 != 0);
    state
}

fn key_code_from_codepoint(codepoint: u32) -> Option<KeyCode> {
    Some(match codepoint {
        8 | 127 => KeyCode::Backspace,
        9 => KeyCode::Tab,
        10 | 13 => KeyCode::Enter,
        27 => KeyCode::Esc,
        codepoint => KeyCode::Char(char::from_u32(codepoint)?),
    })
}

pub(super) fn settle_incomplete_input(
    buffer: &[u8],
    phase: IncompleteInputPhase,
) -> Option<Vec<StartupInput>> {
    if incomplete_utf8_suffix_start(buffer).is_some() {
        let extracted = parse_startup_input(buffer);
        if !extracted.paste_open {
            let mut input = extracted.input;
            input.push(StartupInput::UnknownAction);
            return Some(input);
        }
    }
    let settle_incomplete_osc = matches!(phase, IncompleteInputPhase::QueuedUserInput);
    let settled = parse_startup_input_with_osc_settlement(buffer, settle_incomplete_osc);
    if settled.complete && !settled.paste_open {
        return Some(settled.input);
    }
    for (suffix, key) in [
        (
            b"\x1b[".as_slice(),
            KeyEvent::new(KeyCode::Char('['), KeyModifiers::ALT),
        ),
        (
            b"\x1bO".as_slice(),
            KeyEvent::new(KeyCode::Char('O'), KeyModifiers::ALT),
        ),
    ] {
        if let Some(before_sequence) = buffer.strip_suffix(suffix) {
            let extracted =
                parse_startup_input_with_osc_settlement(before_sequence, settle_incomplete_osc);
            if extracted.complete && !extracted.paste_open {
                let mut input = extracted.input;
                input.push(StartupInput::Key(key));
                return Some(input);
            }
        }
    }
    let (b'\x1b', before_escape) = buffer.split_last()? else {
        return None;
    };
    let extracted = parse_startup_input_with_osc_settlement(before_escape, settle_incomplete_osc);
    if !extracted.complete || extracted.paste_open {
        return None;
    }
    let mut input = extracted.input;
    input.push(StartupInput::Key(KeyEvent::new(
        KeyCode::Esc,
        KeyModifiers::NONE,
    )));
    Some(input)
}

fn utf8_scalar_width(first: u8) -> usize {
    match first {
        0xc2..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf4 => 4,
        _ => 0,
    }
}

fn push_startup_input_byte(
    input: &mut Vec<StartupInput>,
    input_bytes: &mut usize,
    input_controls: &mut usize,
    input_truncated: &mut bool,
    paste: bool,
    byte: u8,
    max_input_bytes: usize,
) {
    if *input_bytes >= max_input_bytes {
        if paste || !byte.is_ascii_control() || *input_controls >= MAX_STARTUP_INPUT_CONTROLS {
            *input_truncated = true;
            return;
        }
        *input_controls += 1;
    }
    match (input.last_mut(), paste) {
        (Some(StartupInput::Plain(bytes)), false) | (Some(StartupInput::Paste(bytes)), true) => {
            bytes.push(byte)
        }
        (_, false) => input.push(StartupInput::Plain(vec![byte])),
        (_, true) => input.push(StartupInput::Paste(vec![byte])),
    }
    if *input_bytes < max_input_bytes {
        *input_bytes += 1;
    }
}

fn push_startup_input_action(
    input: &mut Vec<StartupInput>,
    input_actions: &mut usize,
    input_truncated: &mut bool,
    action: StartupInput,
) {
    if *input_actions >= MAX_STARTUP_INPUT_ACTIONS {
        if *input_actions == MAX_STARTUP_INPUT_ACTIONS {
            input.push(StartupInput::UnknownAction);
            *input_actions += 1;
        }
        *input_truncated = true;
        return;
    }
    input.push(action);
    *input_actions += 1;
}

fn incomplete_utf8_suffix_start(mut input: &[u8]) -> Option<usize> {
    let mut offset = 0;
    loop {
        match std::str::from_utf8(input) {
            Ok(_) => return None,
            Err(err) => match err.error_len() {
                Some(error_len) => {
                    let consumed = err.valid_up_to() + error_len;
                    offset += consumed;
                    input = &input[consumed..];
                }
                None => return Some(offset + err.valid_up_to()),
            },
        }
    }
}

#[cfg(test)]
#[path = "startup_input_tests.rs"]
mod tests;
