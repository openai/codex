//! Snapshot coverage for draft text and live voice controls in the composer.

use super::tests::new_test_composer;
use crate::render::renderable::Renderable;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

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
