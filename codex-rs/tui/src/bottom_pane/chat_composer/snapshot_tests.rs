//! Snapshot coverage for draft text, shortcuts, and live voice controls in the composer.

use super::tests::new_test_composer;
use super::tests::snapshot_composer_state_with_width;
use crate::bottom_pane::footer::FooterMode;
use crate::key_hint;
use crate::key_hint::ShortcutHint;
use crate::keymap::RuntimeKeymap;
use crate::render::renderable::Renderable;
use crossterm::event::KeyCode;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn shortcut_footer_displays_configured_chords() {
    use codex_config::types::KeybindingSpec;
    use codex_config::types::KeybindingsSpec;
    use codex_config::types::TuiKeymap;

    let mut config = TuiKeymap::default();
    config.global.open_external_editor = Some(KeybindingsSpec::One(KeybindingSpec(
        "ctrl-g ctrl-g".to_string(),
    )));
    config.editor.insert_newline = Some(KeybindingsSpec::One(KeybindingSpec(
        "ctrl-x enter".to_string(),
    )));
    let keymap = RuntimeKeymap::from_config(&config).expect("valid composer chords");
    let (mut composer, _rx) = new_test_composer();
    composer.set_keymap_bindings(&keymap);
    composer.footer.mode = FooterMode::ShortcutOverlay;

    let hints = composer.footer_props().key_hints;
    assert_eq!(
        hints.external_editor,
        Some(ShortcutHint::Chord {
            prefix: key_hint::ctrl(KeyCode::Char('g')),
            completion: key_hint::ctrl(KeyCode::Char('g')),
        })
    );
    assert_eq!(
        hints.insert_newline,
        Some(ShortcutHint::Chord {
            prefix: key_hint::ctrl(KeyCode::Char('x')),
            completion: key_hint::plain(KeyCode::Enter),
        })
    );

    snapshot_composer_state_with_width(
        "footer_mode_configured_key_chords",
        /*width*/ 100,
        /*enhanced_keys_supported*/ false,
        |composer| {
            composer.set_keymap_bindings(&keymap);
            composer.footer.mode = FooterMode::ShortcutOverlay;
        },
    );
}

#[test]
fn draft_and_voice_composer_snapshots() {
    use crate::bottom_pane::voice_strip::VoiceStripPhase;
    use crate::bottom_pane::voice_strip::VoiceStripState;
    use crate::tui::FrameRequester;

    crate::terminal_palette::with_test_default_colors(
        crate::terminal_probe::DefaultColors {
            fg: (230, 216, 255),
            bg: (36, 27, 53),
        },
        || {
            for voice_active in [false, true] {
                let (mut composer, _rx) = new_test_composer();
                if voice_active {
                    composer.set_voice_strip(
                        Some(VoiceStripState {
                            mute_hint: None,
                            phase: VoiceStripPhase::Active,
                            microphone_live: true,
                            microphone_muted: false,
                            microphone_history: vec![0, 37, 73, 110, 146, 255],
                            speaker_history: vec![255, 146, 110, 73, 37, 0],
                            activity: "listening",
                            animations: false,
                            progress: true,
                        }),
                        FrameRequester::test_dummy(),
                    );
                } else {
                    composer.set_text_content(
                        "Explore the night sky".to_string(),
                        Vec::new(),
                        Vec::new(),
                    );
                }
                let width = 60;
                let area = Rect::new(
                    /*x*/ 0,
                    /*y*/ 0,
                    width,
                    composer.desired_height(width),
                );
                let mut buffer = Buffer::empty(area);
                composer.render(area, &mut buffer);
                let rows = buffer
                    .content
                    .chunks(usize::from(width))
                    .map(|row| {
                        row.iter()
                            .map(ratatui::buffer::Cell::symbol)
                            .collect::<String>()
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                let name = if voice_active {
                    "voice_composer"
                } else {
                    "draft_composer"
                };
                insta::assert_snapshot!(name, rows);
            }
        },
    );
}
