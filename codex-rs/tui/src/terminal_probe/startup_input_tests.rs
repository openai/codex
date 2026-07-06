use super::*;
use pretty_assertions::assert_eq;

#[test]
fn extracts_startup_input_around_terminal_responses() {
    assert_eq!(
        parse_startup_input(
            b"draft\x1B[20;1R\x1B]10;rgb:eeee/eeee/eeee\x1B\\\x1B[200~ text\x1B[201~"
        )
        .input,
        vec![
            StartupInput::Plain(b"draft".to_vec()),
            StartupInput::Paste(b" text".to_vec()),
        ]
    );
}

#[test]
fn consumes_framed_startup_osc_responses_with_unrecognized_payloads() {
    assert_eq!(
        parse_startup_input(b"before\x1b]10;rgb:fff/fff/fff\x1b\\\x1b]11;?\x07after"),
        ExtractedStartupInput {
            input: vec![StartupInput::Plain(b"beforeafter".to_vec())],
            complete: true,
            paste_open: false,
        }
    );
}

#[test]
fn extracts_startup_input_around_ss3_key_sequences() {
    assert_eq!(
        parse_startup_input(b"draft\x1BOP text").input,
        vec![
            StartupInput::Plain(b"draft".to_vec()),
            StartupInput::Key(KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE)),
            StartupInput::Plain(b" text".to_vec()),
        ]
    );
}

#[test]
fn consumes_linux_console_function_key_sequences() {
    assert_eq!(
        parse_startup_input(b"before\x1b[[A\x1b[[Eafter").input,
        vec![
            StartupInput::Plain(b"before".to_vec()),
            StartupInput::Key(KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE)),
            StartupInput::Key(KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE)),
            StartupInput::Plain(b"after".to_vec()),
        ]
    );
    assert!(!parse_startup_input(b"before\x1b[[").complete);
}

#[test]
fn consumes_an_entire_meta_modified_utf8_scalar() {
    assert_eq!(
        parse_startup_input("before \u{1b}é after".as_bytes()),
        ExtractedStartupInput {
            input: vec![
                StartupInput::Plain(b"before ".to_vec()),
                StartupInput::Key(KeyEvent::new(KeyCode::Char('é'), KeyModifiers::ALT,)),
                StartupInput::Plain(b" after".to_vec()),
            ],
            complete: true,
            paste_open: false,
        }
    );
    assert!(!parse_startup_input(b"before \x1b\xc3").complete);
}

#[test]
fn decodes_inherited_enhanced_key_encodings() {
    assert_eq!(
        parse_startup_input(b"\x1b[97u\x1b[98;1:3u\x1b[27;5;99~\x1b[97:65;2u").input,
        vec![
            StartupInput::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE,)),
            StartupInput::Key(KeyEvent::new_with_kind(
                KeyCode::Char('b'),
                KeyModifiers::NONE,
                KeyEventKind::Release,
            )),
            StartupInput::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL,)),
            StartupInput::Key(KeyEvent::new(KeyCode::Char('A'), KeyModifiers::NONE,)),
        ]
    );
}

#[test]
fn decodes_shift_tab_as_an_action() {
    assert_eq!(
        parse_startup_input(b"\x1b[9;2uX").input,
        vec![
            StartupInput::Key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT)),
            StartupInput::Plain(b"X".to_vec()),
        ]
    );
}

#[test]
fn decodes_kitty_functional_keys_without_inserting_private_use_text() {
    assert_eq!(
        parse_startup_input(b"\x1b[57352u\x1b[57358u\x1b[57376u\x1b[57399u\x1b[63743u").input,
        vec![
            StartupInput::Key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)),
            StartupInput::Key(KeyEvent::new(KeyCode::CapsLock, KeyModifiers::NONE)),
            StartupInput::Key(KeyEvent::new(KeyCode::F(13), KeyModifiers::NONE)),
            StartupInput::Key(KeyEvent::new_with_kind_and_state(
                KeyCode::Char('0'),
                KeyModifiers::NONE,
                KeyEventKind::Press,
                KeyEventState::KEYPAD,
            )),
            StartupInput::UnknownAction,
        ]
    );
}

#[test]
fn preserves_unknown_escape_sequences_as_actions() {
    assert_eq!(
        parse_startup_input(b"draft\x1b[999x").input,
        vec![
            StartupInput::Plain(b"draft".to_vec()),
            StartupInput::UnknownAction,
        ]
    );
}

#[test]
fn preserves_whitespace_inside_bracketed_paste() {
    assert_eq!(
        parse_startup_input(b"\x1B[200~a\r\n\tb\x1B[201~"),
        ExtractedStartupInput {
            input: vec![StartupInput::Paste(b"a\r\n\tb".to_vec())],
            complete: true,
            paste_open: false,
        }
    );
}

#[test]
fn preserves_response_shaped_bytes_inside_bracketed_paste() {
    let pasted = b"\x1B[20;1R\x1B]10;rgb:eeee/eeee/eeee\x1B\\\x1B[?7u\x1B[?64;1;2c";
    let mut buffer = b"\x1B[200~".to_vec();
    buffer.extend_from_slice(pasted);
    buffer.extend_from_slice(b"\x1B[201~");

    assert_eq!(
        parse_startup_input(&buffer),
        ExtractedStartupInput {
            input: vec![StartupInput::Paste(pasted.to_vec())],
            complete: true,
            paste_open: false,
        }
    );
}

#[test]
fn alt_right_bracket_settles_as_user_input() {
    let alt_right_bracket = StartupInput::Key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::ALT));
    assert_eq!(
        parse_startup_input(b"draft\x1b]more"),
        ExtractedStartupInput {
            input: vec![
                StartupInput::Plain(b"draft".to_vec()),
                alt_right_bracket.clone(),
                StartupInput::Plain(b"more".to_vec()),
            ],
            complete: true,
            paste_open: false,
        }
    );
    assert_eq!(
        settle_incomplete_input(b"draft\x1b]", IncompleteInputPhase::QueuedUserInput,),
        Some(vec![
            StartupInput::Plain(b"draft".to_vec()),
            alt_right_bracket.clone(),
        ])
    );
    assert_eq!(
        settle_incomplete_input(
            b"draft\x1b]10;ordinary",
            IncompleteInputPhase::QueuedUserInput,
        ),
        Some(vec![
            StartupInput::Plain(b"draft".to_vec()),
            alt_right_bracket,
            StartupInput::Plain(b"10;ordinary".to_vec()),
        ])
    );
}

#[test]
fn alt_right_bracket_text_does_not_absorb_a_later_osc_response() {
    assert_eq!(
        parse_startup_input(b"before\x1b]10;ordinary \x1b]10;rgb:eeee/eeee/eeee\x1b\\after"),
        ExtractedStartupInput {
            input: vec![
                StartupInput::Plain(b"before".to_vec()),
                StartupInput::Key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::ALT,)),
                StartupInput::Plain(b"10;ordinary after".to_vec()),
            ],
            complete: true,
            paste_open: false,
        }
    );
}

#[test]
fn standalone_escape_does_not_consume_a_following_probe_response() {
    assert_eq!(
        parse_startup_input(b"before\x1b\x1b[20;1R\x1b]10;rgb:eeee/eeee/eeee\x1b\\after"),
        ExtractedStartupInput {
            input: vec![
                StartupInput::Plain(b"before".to_vec()),
                StartupInput::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
                StartupInput::Plain(b"after".to_vec()),
            ],
            complete: true,
            paste_open: false,
        }
    );
}

#[test]
fn incomplete_alt_prefixes_do_not_consume_following_probe_responses() {
    for (prefix, key) in [
        (
            b"\x1b[".as_slice(),
            KeyEvent::new(KeyCode::Char('['), KeyModifiers::ALT),
        ),
        (
            b"\x1bO".as_slice(),
            KeyEvent::new(KeyCode::Char('O'), KeyModifiers::ALT),
        ),
    ] {
        let mut buffer = b"before".to_vec();
        buffer.extend_from_slice(prefix);
        buffer.extend_from_slice(b"\x1b[20;1Rafter");
        assert_eq!(
            parse_startup_input(&buffer),
            ExtractedStartupInput {
                input: vec![
                    StartupInput::Plain(b"before".to_vec()),
                    StartupInput::Key(key),
                    StartupInput::Plain(b"after".to_vec()),
                ],
                complete: true,
                paste_open: false,
            }
        );

        let mut incomplete = b"before".to_vec();
        incomplete.extend_from_slice(prefix);
        assert_eq!(
            settle_incomplete_input(&incomplete, IncompleteInputPhase::QueuedUserInput),
            Some(vec![
                StartupInput::Plain(b"before".to_vec()),
                StartupInput::Key(key),
            ])
        );
    }
}

#[test]
fn partial_expected_osc_response_is_not_settled_as_user_text() {
    assert_eq!(
        settle_incomplete_input(
            b"draft\x1b]10;rgb:eeee/eeee/eeee",
            IncompleteInputPhase::ProbeResponse,
        ),
        None
    );
    assert!(
        settle_incomplete_input(
            b"draft\x1b]10;ordinary",
            IncompleteInputPhase::QueuedUserInput,
        )
        .is_some()
    );
}

#[test]
fn startup_input_stays_owned_until_trailing_sequences_are_complete() {
    let cases: &[(&[u8], Vec<StartupInput>, bool, bool)] = &[
        (
            b"draft\x1B[",
            vec![StartupInput::Plain(b"draft".to_vec())],
            false,
            false,
        ),
        (
            b"draft\x1B[A",
            vec![
                StartupInput::Plain(b"draft".to_vec()),
                StartupInput::Key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)),
            ],
            true,
            false,
        ),
        (
            b"draft\x1BO",
            vec![StartupInput::Plain(b"draft".to_vec())],
            false,
            false,
        ),
        (
            b"draft\x1BOP",
            vec![
                StartupInput::Plain(b"draft".to_vec()),
                StartupInput::Key(KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE)),
            ],
            true,
            false,
        ),
        (
            b"draft\x1B[200",
            vec![StartupInput::Plain(b"draft".to_vec())],
            false,
            false,
        ),
        (
            b"draft\x1B[200~",
            vec![StartupInput::Plain(b"draft".to_vec())],
            false,
            true,
        ),
        (
            b"draft\x1B[200~paste\x1B[201",
            vec![
                StartupInput::Plain(b"draft".to_vec()),
                StartupInput::Paste(b"paste".to_vec()),
            ],
            false,
            true,
        ),
        (
            b"draft\x1B[200~paste\x1B[201~",
            vec![
                StartupInput::Plain(b"draft".to_vec()),
                StartupInput::Paste(b"paste".to_vec()),
            ],
            true,
            false,
        ),
        (
            b"draft \xc3",
            vec![StartupInput::Plain(b"draft ".to_vec())],
            false,
            false,
        ),
        (
            b"draft \xc3\xa9",
            vec![StartupInput::Plain("draft é".as_bytes().to_vec())],
            true,
            false,
        ),
    ];
    for (buffer, input, complete, paste_open) in cases {
        assert_eq!(
            parse_startup_input(buffer),
            ExtractedStartupInput {
                input: input.clone(),
                complete: *complete,
                paste_open: *paste_open,
            }
        );
    }
}

#[test]
fn startup_input_cap_does_not_split_utf8() {
    let mut buffer = vec![b'x'; MAX_STARTUP_INPUT_BYTES - 1];
    buffer.extend_from_slice("é".as_bytes());

    assert_eq!(
        parse_startup_input(&buffer),
        ExtractedStartupInput {
            input: vec![StartupInput::Plain(vec![b'x'; MAX_STARTUP_INPUT_BYTES - 1])],
            complete: true,
            paste_open: false,
        }
    );
}

#[test]
fn accepts_maximum_sized_utf8_user_input() {
    let input = "🦀".repeat(MAX_STARTUP_INPUT_BYTES / 4);

    assert_eq!(
        parse_startup_input(input.as_bytes()),
        ExtractedStartupInput {
            input: vec![StartupInput::Plain(input.into_bytes())],
            complete: true,
            paste_open: false,
        }
    );
}

#[test]
fn preserves_controls_and_actions_after_maximum_sized_input() {
    let text = "🦀".repeat(MAX_STARTUP_INPUT_BYTES / 4);
    let mut buffer = text.as_bytes().to_vec();
    buffer.extend_from_slice(b"\x7f\r\t\x1b[13u");
    let mut plain = text.into_bytes();
    plain.extend_from_slice(b"\x7f\r\t");

    assert_eq!(
        parse_startup_input(&buffer),
        ExtractedStartupInput {
            input: vec![
                StartupInput::Plain(plain),
                StartupInput::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            ],
            complete: true,
            paste_open: false,
        }
    );
}

#[test]
fn a_lone_escape_settles_as_an_escape_key() {
    assert_eq!(
        settle_incomplete_input(b"draft\x1b", IncompleteInputPhase::QueuedUserInput),
        Some(vec![
            StartupInput::Plain(b"draft".to_vec()),
            StartupInput::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
        ])
    );
    assert_eq!(
        settle_incomplete_input(
            b"draft\x1b]partial\x1b",
            IncompleteInputPhase::QueuedUserInput,
        ),
        Some(vec![
            StartupInput::Plain(b"draft".to_vec()),
            StartupInput::Key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::ALT,)),
            StartupInput::Plain(b"partial".to_vec()),
            StartupInput::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
        ])
    );
}

#[test]
fn incomplete_utf8_settles_with_a_quarantine_action() {
    assert_eq!(
        settle_incomplete_input(b"draft \xc3", IncompleteInputPhase::QueuedUserInput),
        Some(vec![
            StartupInput::Plain(b"draft ".to_vec()),
            StartupInput::UnknownAction,
        ])
    );
    assert_eq!(
        settle_incomplete_input(b"draft\x1b\xc3", IncompleteInputPhase::QueuedUserInput,),
        Some(vec![
            StartupInput::Plain(b"draft".to_vec()),
            StartupInput::UnknownAction,
        ])
    );
}

#[test]
fn incomplete_csi_payload_cannot_cross_the_probe_handoff() {
    let buffer = b"before\x1b[1;";
    assert_eq!(
        settle_incomplete_input(buffer, IncompleteInputPhase::QueuedUserInput),
        None,
        "settled {buffer:?}"
    );
}
