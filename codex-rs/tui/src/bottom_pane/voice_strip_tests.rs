//! Snapshot and layout regressions for compact voice controls across widths and states.
//! Supplied microphone and speaker histories remain independent when rendered.

use super::*;
use pretty_assertions::assert_eq;
use ratatui::style::Color;

fn active_strip() -> VoiceStrip {
    VoiceStrip::new(
        VoiceStripState {
            mute_hint: crate::keymap::RuntimeKeymap::defaults()
                .chat
                .voice_mute_hint(),
            phase: VoiceStripPhase::Active,
            microphone_live: true,
            microphone_muted: false,
            microphone_history: vec![
                0, 1, 37, 73, 110, 146, 183, 219, 183, 146, 110, 73, 37, 1, 0,
            ],
            speaker_history: vec![0, 37, 73, 110, 146, 219],
            activity: "listening",
            animations: true,
            progress: true,
        },
        FrameRequester::test_dummy(),
    )
}

fn rows(strip: &VoiceStrip, width: u16) -> (String, Buffer) {
    let area = Rect::new(/*x*/ 0, /*y*/ 0, width, /*height*/ 2);
    let mut buffer = Buffer::empty(area);
    strip.render(area, &mut buffer);
    let text = buffer
        .content
        .chunks(usize::from(width))
        .map(|cells| {
            cells
                .iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>()
        })
        .map(|line| line.trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n");
    (text, buffer)
}

#[test]
fn active_dashboard_preserves_live_colors_controls_and_width() {
    let mut strip = active_strip();
    let (actual, buffer) = rows(&strip, /*width*/ 47);
    assert_eq!(buffer[(45, 0)].symbol(), "p");
    assert_eq!(buffer[(46, 0)].symbol(), " ");
    assert!(actual.starts_with(" voice "));
    assert!(actual.lines().nth(1).unwrap().starts_with("   mic "));
    insta::assert_snapshot!(actual, @r"
    voice ● listening ctrl+x mute     /voice stop
      mic ▆▅▄▃▂▁  codex ▁▃▄▅▆█
    ");
    assert_eq!(buffer[(7, 0)].fg, Color::Red);
    assert_eq!(buffer[(7, 1)].fg, Color::Cyan);
    assert_eq!(buffer[(22, 1)].fg, Color::Magenta);
    assert!(actual.contains("/voice stop"));
    let (compact, buffer) = rows(&strip, /*width*/ 37);
    assert!(compact.contains("ctrl+x mute") && compact.contains("/voice stop"));
    assert_eq!(buffer[(36, 0)].symbol(), " ");
    let (narrow, buffer) = rows(&strip, /*width*/ 22);
    assert!(narrow.contains("/voice stop"));
    assert_eq!(buffer[(21, 0)].symbol(), " ");
    assert!(narrow.contains("mic") && narrow.contains("codex"));
    let started_at = strip.started_at;
    let state = VoiceStripState {
        microphone_history: vec![255],
        ..strip.state.clone()
    };
    strip.update(state);
    assert_eq!(strip.started_at, started_at);
    let (full, _) = rows(&strip, /*width*/ 45);
    assert!(full.lines().nth(1).unwrap().contains('█'));
}

#[test]
fn connecting_and_muted_indicators_follow_actual_capture() {
    let mut strip = active_strip();
    strip.state.phase = VoiceStripPhase::Connecting;
    strip.state.activity = "connecting";
    strip.state.microphone_live = false;
    strip.state.animations = false;
    let (connecting, buffer) = rows(&strip, /*width*/ 45);
    assert!(connecting.contains("voice ◌ connecting"));
    assert!(!connecting.contains("ctrl+x mute"));
    assert_ne!(buffer[(7, 0)].fg, Color::Red);
    strip.state.microphone_live = true;
    let (_, buffer) = rows(&strip, /*width*/ 45);
    assert_eq!(buffer[(7, 0)].symbol(), "◌");
    assert_eq!(buffer[(7, 0)].fg, Color::Red);
    strip.state.phase = VoiceStripPhase::Active;
    strip.state.activity = "listening";
    strip.state.microphone_history = vec![255];
    let (reduced, _) = rows(&strip, /*width*/ 45);
    assert!(reduced.lines().nth(1).unwrap().contains('█'));
    let stop_column = reduced.lines().next().unwrap().find("/voice stop");
    strip.state.microphone_muted = true;
    strip.state.activity = "muted";
    let (muted, buffer) = rows(&strip, /*width*/ 45);
    assert!(muted.contains("voice ◌ muted"));
    assert!(
        muted
            .lines()
            .nth(1)
            .unwrap()
            .starts_with("   mic ▁▁▁▁▁▁  codex ")
    );
    assert!(muted.lines().nth(1).unwrap().ends_with("▁▃▄▅▆█"));
    assert_eq!(
        muted.lines().next().unwrap().find("/voice stop"),
        stop_column
    );
    assert_eq!(buffer[(7, 1)].fg, Color::DarkGray);
    assert_eq!(buffer[(22, 1)].fg, Color::Magenta);
    assert_ne!(buffer[(7, 0)].fg, Color::Red);
}

#[test]
fn waveform_preserves_real_sample_order_width_and_channel_colors() {
    let mut strip = active_strip();
    let (history, _) = rows(&strip, /*width*/ 45);
    assert!(history.ends_with("mic ▆▅▄▃▂▁  codex ▁▃▄▅▆█"));
    let (clipped, _) = rows(&strip, /*width*/ 22);
    assert!(clipped.ends_with("mic ▃▂▁  codex ▅▆█"));
    strip.state.microphone_history = vec![0, 0];
    let (silent, buffer) = rows(&strip, /*width*/ 45);
    assert!(silent.contains("mic ▁▁▁▁▁▁  codex ▁▃▄▅▆█"));
    assert_eq!(buffer[(7, 1)].fg, Color::DarkGray);
    assert_eq!(buffer[(22, 1)].fg, Color::Magenta);
}

#[test]
fn voice_controls_follow_configured_mute_binding_and_unbinding() {
    let mut strip = active_strip();
    let mut layouts = Vec::new();
    for (binding, expected) in [
        ("'f8'", "f8 mute"),
        ("'ctrl-x m'", "ctrl+x m mute"),
        ("[]", "/voice mute"),
    ] {
        let config = toml::from_str(&format!("[chat]\ntoggle_voice_mute = {binding}")).unwrap();
        strip.state.mute_hint = crate::keymap::RuntimeKeymap::from_config(&config)
            .unwrap()
            .chat
            .voice_mute_hint();
        let (text, _) = rows(&strip, /*width*/ 80);
        assert!(text.contains(expected));
        assert!(text.contains("/voice stop"));
        for muted in [false, true] {
            strip.state.microphone_muted = muted;
            strip.state.activity = if muted { "muted" } else { "listening" };
            for width in [80, 22] {
                let (text, _) = rows(&strip, width);
                layouts.push(format!("{binding}, muted={muted}, width={width}:\n{text}"));
            }
        }
        strip.state.microphone_muted = false;
        strip.state.activity = "listening";
    }
    insta::assert_snapshot!(layouts.join("\n\n"));
}
