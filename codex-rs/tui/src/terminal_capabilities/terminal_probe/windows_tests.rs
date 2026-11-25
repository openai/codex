use super::BufferedConsoleInput;
use super::DefaultColors;
use super::decode_console_default_colors;
use pretty_assertions::assert_eq;
use windows_sys::Win32::System::Console::COMMON_LVB_REVERSE_VIDEO;
use windows_sys::Win32::System::Console::FOCUS_EVENT;
use windows_sys::Win32::System::Console::FOCUS_EVENT_RECORD;
use windows_sys::Win32::System::Console::INPUT_RECORD;
use windows_sys::Win32::System::Console::INPUT_RECORD_0;
use windows_sys::Win32::System::Console::KEY_EVENT;
use windows_sys::Win32::System::Console::KEY_EVENT_RECORD;
use windows_sys::Win32::System::Console::KEY_EVENT_RECORD_0;

fn color_table() -> [u32; 16] {
    [
        0x00000000, 0x00000080, 0x00008000, 0x00008080, 0x00800000, 0x00800080, 0x00808000,
        0x00c0c0c0, 0x00808080, 0x000000ff, 0x0000ff00, 0x0000ffff, 0x00ff0000, 0x00ff00ff,
        0x00ffff00, 0x00ffffff,
    ]
}

fn key_record(character: u16, pressed: bool, control_key_state: u32) -> INPUT_RECORD {
    INPUT_RECORD {
        EventType: KEY_EVENT as u16,
        Event: INPUT_RECORD_0 {
            KeyEvent: KEY_EVENT_RECORD {
                bKeyDown: i32::from(pressed),
                wRepeatCount: 1,
                wVirtualKeyCode: character,
                wVirtualScanCode: 0,
                uChar: KEY_EVENT_RECORD_0 {
                    UnicodeChar: character,
                },
                dwControlKeyState: control_key_state,
            },
        },
    }
}

fn push_ascii(input: &mut BufferedConsoleInput, bytes: &[u8]) {
    for byte in bytes {
        input.push(key_record(
            u16::from(*byte),
            /*pressed*/ true,
            /*control_key_state*/ 0,
        ));
    }
}

#[test]
fn preserves_typeahead_records_and_modifier_state_around_color_responses() {
    let mut input = BufferedConsoleInput::default();
    let before = key_record(
        u16::from(b'a'),
        /*pressed*/ true,
        /*control_key_state*/ 8,
    );
    let after = key_record(
        u16::from(b'b'),
        /*pressed*/ true,
        /*control_key_state*/ 16,
    );
    input.push(before);
    push_ascii(&mut input, b"\x1b]10;rgb:5555/5757/5353\x1b\\");
    input.push(after);
    push_ascii(&mut input, b"\x1b]11;rgb:ffff/ffff/ffff\x07");

    let preserved = input.preserved_records();
    assert_eq!(preserved.len(), 2);
    // SAFETY: Both preserved records are constructed as KEY_EVENT records above.
    let states: Vec<_> = preserved
        .iter()
        .map(|record| unsafe { record.Event.KeyEvent.dwControlKeyState })
        .collect();
    assert_eq!(states, vec![8, 16]);
}

#[test]
fn preserves_focus_unicode_and_key_release_console_records() {
    let mut input = BufferedConsoleInput::default();
    let focus = INPUT_RECORD {
        EventType: FOCUS_EVENT as u16,
        Event: INPUT_RECORD_0 {
            FocusEvent: FOCUS_EVENT_RECORD { bSetFocus: 1 },
        },
    };
    let unicode = key_record(
        /*character*/ 0x00e9, /*pressed*/ true, /*control_key_state*/ 0,
    );
    let release = key_record(
        u16::from(b'x'),
        /*pressed*/ false,
        /*control_key_state*/ 2,
    );
    input.push(focus);
    push_ascii(&mut input, b"\x1b]10;rgb:5555/5757/5353\x1b\\");
    input.push(unicode);
    input.push(release);
    push_ascii(&mut input, b"\x1b]11;rgb:ffff/ffff/ffff\x07");

    let preserved = input.preserved_records();
    assert_eq!(preserved.len(), 3);
    assert_eq!(preserved[0].EventType, FOCUS_EVENT as u16);
    // SAFETY: The final records were constructed as KEY_EVENT records above.
    let unicode_character = unsafe { preserved[1].Event.KeyEvent.uChar.UnicodeChar };
    let release_pressed = unsafe { preserved[2].Event.KeyEvent.bKeyDown };
    assert_eq!(unicode_character, 0x00e9);
    assert_eq!(release_pressed, 0);
}

#[test]
fn preserves_fifo_order_when_later_console_records_are_buffered() {
    let mut input = BufferedConsoleInput::default();
    input.push(key_record(
        u16::from(b'a'),
        /*pressed*/ true,
        /*control_key_state*/ 8,
    ));
    push_ascii(&mut input, b"\x1b]10;rgb:5/5/5\x07");
    push_ascii(&mut input, b"\x1b]11;rgb:f/f/f\x07");

    // replay() drains records that arrived after its last probe read before writing anything.
    input.push(key_record(
        u16::from(b'b'),
        /*pressed*/ true,
        /*control_key_state*/ 16,
    ));

    let preserved = input.preserved_records();
    let characters: Vec<_> = preserved
        .iter()
        // SAFETY: Both surviving records are KEY_EVENT values constructed above.
        .map(|record| unsafe { record.Event.KeyEvent.uChar.UnicodeChar })
        .collect();
    assert_eq!(characters, vec![u16::from(b'a'), u16::from(b'b')]);
}

#[test]
fn preserves_osc_looking_bracketed_paste_records() {
    let mut input = BufferedConsoleInput::default();
    push_ascii(
        &mut input,
        b"\x1b[200~\x1b]10;rgb:f/e/d\x07\x1b]11;rgb:0/0/0\x07\x1b[201~",
    );

    assert_eq!(input.preserved_records().len(), input.records.len());
}

#[test]
fn preserves_every_record_when_no_valid_color_response_arrives() {
    let mut input = BufferedConsoleInput::default();
    push_ascii(&mut input, b"typed\x1b]10;incomplete");

    assert_eq!(input.preserved_records().len(), input.records.len());
}

#[test]
fn decodes_console_color_attribute_indices() {
    assert_eq!(
        decode_console_default_colors(/*attributes*/ 0x21, &color_table()),
        DefaultColors {
            fg: (128, 0, 0),
            bg: (0, 128, 0),
        }
    );
}

#[test]
fn decodes_console_color_intensity_indices() {
    assert_eq!(
        decode_console_default_colors(/*attributes*/ 0xe9, &color_table()),
        DefaultColors {
            fg: (255, 0, 0),
            bg: (0, 255, 255),
        }
    );
}

#[test]
fn decodes_console_color_ref_byte_order() {
    let mut colors = color_table();
    colors[3] = 0x00112233;
    colors[4] = 0x00aabbcc;

    assert_eq!(
        decode_console_default_colors(/*attributes*/ 0x43, &colors),
        DefaultColors {
            fg: (0x33, 0x22, 0x11),
            bg: (0xcc, 0xbb, 0xaa),
        }
    );
}

#[test]
fn ignores_reverse_video_when_decoding_default_colors() {
    assert_eq!(
        decode_console_default_colors(
            /*attributes*/ COMMON_LVB_REVERSE_VIDEO | 0x21,
            &color_table(),
        ),
        DefaultColors {
            fg: (128, 0, 0),
            bg: (0, 128, 0),
        }
    );
}
