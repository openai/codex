use super::*;
use pretty_assertions::assert_eq;

use crate::terminal_palette::with_test_default_colors;
use crate::terminal_probe::DefaultColors;

#[test]
fn rtl_headers_remain_stable_during_the_sweep() {
    with_test_default_colors(
        DefaultColors {
            fg: (240, 240, 240),
            bg: (16, 16, 16),
        },
        || {
            let mut headers = Vec::new();
            for text in [
                "שלום עולם",
                "Checking שָׁלוֹם (test.rs) 123",
                "Checking lines ١٢-٣٤",
                "مرحبا",
            ] {
                let initial = summary_shimmer(text, Duration::ZERO, MotionMode::Animated);
                let sweep = summary_shimmer(
                    text,
                    Duration::from_millis(/*millis*/ 1100),
                    MotionMode::Animated,
                );
                assert_eq!(initial, sweep);
                assert_eq!(
                    initial,
                    crate::motion::shimmer_text(text, MotionMode::Animated)
                );
                headers.push(initial);
            }
            insta::assert_debug_snapshot!(headers);
        },
    );
}

#[test]
fn short_and_long_labels_sweep_smoothly_in_both_themes() {
    let mut frames = Vec::new();
    for (theme, colors) in [
        (
            "dark",
            DefaultColors {
                fg: (240, 240, 240),
                bg: (16, 16, 16),
            },
        ),
        (
            "light",
            DefaultColors {
                fg: (16, 16, 16),
                bg: (240, 240, 240),
            },
        ),
    ] {
        with_test_default_colors(colors, || {
            for text in ["Working", "Preparing bootstrap diagnostics", "界e\u{301}界"] {
                // Initial delay, the first sweep, the gap, and the next sweep.
                for ms in [0, 850, 1100, 1350, 3600, 5100] {
                    let spans =
                        summary_shimmer(text, Duration::from_millis(ms), MotionMode::Animated);
                    let levels = spans
                        .iter()
                        .map(|span| match span.style.fg {
                            Some(ratatui::style::Color::Rgb(level, _, _)) => level,
                            color => panic!("expected interpolated RGB, got {color:?}"),
                        })
                        .collect::<Vec<_>>();
                    frames.push(format!("{theme} {text} {ms}ms: {levels:?}"));
                }
            }
        });
    }
    insta::assert_snapshot!(frames.join("\n"));
}

#[test]
fn working_has_overlapping_highlights_without_frame_to_frame_flashes() {
    with_test_default_colors(
        DefaultColors {
            fg: (240, 240, 240),
            bg: (16, 16, 16),
        },
        || {
            let mut previous = Vec::new();
            // Sample two sweeps and the intervening gap at the status row's 32 ms cadence.
            for ms in (0..=5600).step_by(/*step*/ 32) {
                let spans =
                    summary_shimmer("Working", Duration::from_millis(ms), MotionMode::Animated);
                let brightness = spans
                    .iter()
                    .map(|span| match span.style.fg {
                        Some(ratatui::style::Color::Rgb(r, _, _)) => r,
                        color => panic!("expected interpolated RGB, got {color:?}"),
                    })
                    .collect::<Vec<_>>();
                if brightness.iter().any(|value| *value > 220) {
                    assert!(brightness.iter().filter(|value| **value > 160).count() >= 2);
                }
                for (current, previous) in brightness.iter().zip(&previous) {
                    // At this speed the cosine band's maximum change over 32 ms is
                    // 112 * sin(PI * (13 * 0.032) / 6), rounded up.
                    assert!(u8::abs_diff(*current, *previous) <= 25);
                }
                previous = brightness;
            }
            let midpoint = summary_shimmer(
                "Working",
                Duration::from_millis(/*millis*/ 1100),
                MotionMode::Animated,
            );
            let expected = "Working"
                .chars()
                .zip([128, 156, 212, 240, 212, 156, 128])
                .map(|(ch, level)| {
                    Span::styled(
                        ch.to_string(),
                        Style::default().fg(rgb_color((level, level, level))),
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(midpoint, expected);
        },
    );
}

#[test]
fn sweep_preserves_combining_characters_and_emoji_clusters() {
    let text = "e\u{301}👨‍👩‍👧‍👦界";
    let spans = with_test_default_colors(
        DefaultColors {
            fg: (240, 240, 240),
            bg: (16, 16, 16),
        },
        || summary_shimmer(text, Duration::ZERO, MotionMode::Animated),
    );
    assert_eq!(
        spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<Vec<_>>(),
        vec!["e\u{301}", "👨‍👩‍👧‍👦", "界"]
    );
    assert_eq!(
        summary_shimmer(text, Duration::from_secs(/*secs*/ 1), MotionMode::Reduced),
        vec![Span::from(text.to_owned())]
    );
}
