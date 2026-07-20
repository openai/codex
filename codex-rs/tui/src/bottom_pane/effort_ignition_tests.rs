use super::styles::envelope;
use super::styles::paint_style;
use super::styles::spark_frame;
use super::*;
use crate::terminal_palette::rgb_color;
use pretty_assertions::assert_eq;
use ratatui::widgets::Widget;

const WIDTH: u16 = 44;
const HEIGHT: u16 = 3;
const DRAFT: &str = "  > keep my draft exactly as typed";

fn test_buffer(area: Rect) -> Buffer {
    let mut buf = Buffer::empty(area);
    if area.height > 1 {
        for (column, glyph) in DRAFT.chars().take(usize::from(area.width)).enumerate() {
            buf[(area.x + column as u16, area.y + 1)].set_symbol(&glyph.to_string());
        }
    }
    buf
}

fn frame(area: Rect, buf: &Buffer) -> Vec<String> {
    let symbols = (area.y..area.bottom()).map(|row| {
        (area.x..area.right())
            .map(|column| buf[(column, row)].symbol())
            .collect::<String>()
    });
    let tint = (area.y..area.bottom()).map(|row| {
        (area.x..area.right())
            .map(|column| {
                if buf[(column, row)].bg == Color::Reset {
                    '.'
                } else {
                    '#'
                }
            })
            .collect::<String>()
    });
    symbols.chain(tint).collect()
}

fn style_name(style: IgnitionStyle) -> &'static str {
    match style {
        IgnitionStyle::Wave => "wave",
        IgnitionStyle::Aurora => "aurora",
        IgnitionStyle::Pulse => "pulse",
    }
}

fn paint(tier: EffortTier, style: IgnitionStyle, elapsed: Duration, area: Rect, buf: &mut Buffer) {
    let term_bg = (18, 22, 28);
    let mut canvas = Canvas {
        area,
        protected: Rect::new(
            area.x,
            area.y.saturating_add(1),
            area.width,
            area.height.saturating_sub(1).min(1),
        ),
        buf,
        band_rgb: user_message_bg_rgb(term_bg),
        color_level: StdoutColorLevel::TrueColor,
    };
    paint_style(
        tier,
        style,
        elapsed,
        tier.hues(/*on_light_bg*/ false),
        &mut canvas,
    );
}

#[test]
fn effort_tier_maps_only_max_and_ultra() {
    assert_eq!(
        [
            Some(&ReasoningEffort::Max),
            Some(&ReasoningEffort::Ultra),
            Some(&ReasoningEffort::XHigh),
            None,
        ]
        .map(EffortTier::from_effort),
        [Some(EffortTier::Max), Some(EffortTier::Ultra), None, None]
    );
}

#[test]
fn effort_animation_requires_motion_and_a_reliable_palette() {
    for (color_level, supported) in [
        (StdoutColorLevel::TrueColor, true),
        (StdoutColorLevel::Ansi256, true),
        (StdoutColorLevel::Ansi16, false),
        (StdoutColorLevel::Unknown, false),
    ] {
        assert_eq!(
            effort_animation_enabled(/*animations_enabled*/ true, color_level),
            supported
        );
        assert!(!effort_animation_enabled(
            /*animations_enabled*/ false,
            color_level,
        ));
    }
}

#[test]
fn prompt_accent_blends_with_the_terminal_foreground() {
    for (tier, fg, bg, expected) in [
        (
            EffortTier::Ultra,
            (224, 220, 214),
            (18, 22, 28),
            (191, 142, 249),
        ),
        (EffortTier::Max, (30, 32, 36), (250, 248, 244), (155, 88, 5)),
    ] {
        assert_eq!(
            tier.accent_color_for(
                /*charge*/ 1.0,
                Some(fg),
                Some(bg),
                StdoutColorLevel::TrueColor,
            ),
            Some(rgb_color(expected))
        );
    }
}

#[test]
fn prompt_accent_degrades_with_terminal_color_support() {
    let fg = (224, 220, 214);
    let bg = (18, 22, 28);

    assert!(matches!(
        EffortTier::Ultra.accent_color_for(
            /*charge*/ 1.0,
            Some(fg),
            Some(bg),
            StdoutColorLevel::Ansi256,
        ),
        Some(Color::Indexed(_))
    ));
    for (tier, color_level, expected) in [
        (
            EffortTier::Max,
            StdoutColorLevel::Ansi16,
            Some(Color::Yellow),
        ),
        (
            EffortTier::Ultra,
            StdoutColorLevel::Ansi16,
            Some(Color::Magenta),
        ),
        (EffortTier::Ultra, StdoutColorLevel::Unknown, None),
    ] {
        assert_eq!(
            tier.accent_color_for(/*charge*/ 1.0, Some(fg), Some(bg), color_level),
            expected
        );
    }
}

#[test]
fn max_and_ultra_prompts_render_their_accent_and_glyph() {
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 1, /*height*/ 1,
    );
    for (tier, glyph, color) in [
        (EffortTier::Max, "›", Color::Yellow),
        (EffortTier::Ultra, "»", Color::Magenta),
    ] {
        let mut buf = Buffer::empty(area);
        tier.prompt_for(
            /*charge*/ 1.0,
            Some((224, 220, 214)),
            Some((18, 22, 28)),
            StdoutColorLevel::Ansi16,
        )
        .render(area, &mut buf);
        let prompt = &buf[(0, 0)];
        assert_eq!(prompt.symbol(), glyph);
        assert_eq!(prompt.style().fg, Some(color));
        assert!(prompt.style().add_modifier.contains(Modifier::BOLD));
    }
}

#[test]
fn effort_ignition_clock_waits_for_its_first_visible_frame() {
    let ignition = EffortIgnition::new(EffortTier::Ultra, IgnitionStyle::Wave);
    let area = Rect::default();
    let mut buf = Buffer::empty(area);
    assert!(!ignition.render(area, area, &mut buf));

    assert_eq!(ignition.started_at.get(), None);
    assert!(!ignition.is_finished());
    assert_eq!(ignition.charge_alpha(), 0.0);
}

#[cfg(unix)]
#[test]
fn effort_ignition_finishes_when_palette_is_unavailable() {
    let ignition = EffortIgnition::new(EffortTier::Ultra, IgnitionStyle::Wave);
    let area = Rect {
        width: 1,
        height: 1,
        ..Rect::default()
    };
    let mut buf = Buffer::empty(area);
    assert!(!ignition.render(area, area, &mut buf));

    assert_eq!(ignition.started_at.get(), None);
    assert!(ignition.is_finished());
}

#[test]
fn effort_ignition_random_styles_do_not_repeat_immediately() {
    let mut previous = None;
    for _ in 0..100 {
        let style = IgnitionStyle::random(previous);
        assert_ne!(Some(style), previous);
        previous = Some(style);
    }
}

#[test]
fn envelope_is_zero_outside_and_full_in_the_middle() {
    assert_eq!(
        [0.0, 1.0, 0.5].map(|elapsed| {
            envelope(
                elapsed, /*total*/ 1.0, /*fade_in*/ 0.2, /*fade_out*/ 0.2,
            )
        }),
        [0.0, 0.0, 1.0]
    );
}

#[test]
fn wave_only_paints_while_a_sweep_is_crossing_the_composer() {
    let area = Rect::new(/*x*/ 0, /*y*/ 0, WIDTH, HEIGHT);
    for (millis, expected) in [(0, false), (475, true), (3000, false)] {
        let mut buf = test_buffer(area);
        paint(
            EffortTier::Max,
            IgnitionStyle::Wave,
            Duration::from_millis(millis),
            area,
            &mut buf,
        );
        let painted = frame(area, &buf)[HEIGHT as usize..]
            .iter()
            .any(|row| row.contains('#'));
        assert_eq!(painted, expected);
    }
}

#[test]
fn spark_only_fires_after_landing() {
    let start = Duration::from_millis(900);
    assert_eq!(
        [850, 950, 1050, 1150, 1250]
            .map(|millis| spark_frame(Duration::from_millis(millis), start)),
        [None, Some("·"), Some("✦"), Some("✧"), None]
    );
}

#[test]
fn effort_ignition_styles_preserve_draft_and_paint_expected_content() {
    let area = Rect::new(/*x*/ 0, /*y*/ 0, WIDTH, HEIGHT);
    for style in IgnitionStyle::ALL {
        let mut painted = false;
        for millis in [180, 420, 720, 1100, 1580] {
            let mut buf = test_buffer(area);
            paint(
                EffortTier::Ultra,
                style,
                Duration::from_millis(millis),
                area,
                &mut buf,
            );
            let rendered = frame(area, &buf);
            assert!(rendered[1].starts_with(DRAFT));
            painted |= rendered[HEIGHT as usize..]
                .iter()
                .any(|row| row.contains('#'))
                || rendered[..HEIGHT as usize]
                    .iter()
                    .any(|row| row.contains(['✦', '✧', '·']));
        }
        assert!(
            painted,
            "{} never painted visible content",
            style_name(style)
        );
    }
}

#[test]
fn effort_ignition_styles_are_safe_on_tiny_and_offset_areas() {
    for style in IgnitionStyle::ALL {
        for width in 0..=8 {
            for height in 0..=3 {
                let area = Rect::new(/*x*/ 2, /*y*/ 4, width, height);
                let mut buf = Buffer::empty(Rect::new(
                    /*x*/ 0,
                    /*y*/ 0,
                    width.saturating_add(4),
                    height.saturating_add(8),
                ));
                for millis in [0, 180, 720, 1450, 2350] {
                    paint(
                        EffortTier::Max,
                        style,
                        Duration::from_millis(millis),
                        area,
                        &mut buf,
                    );
                }
            }
        }
    }
}

#[test]
fn effort_ignition_animation_gallery_snapshot() {
    let area = Rect::new(/*x*/ 0, /*y*/ 0, WIDTH, HEIGHT);
    let mut frames = Vec::new();
    for style in IgnitionStyle::ALL {
        for tier in [EffortTier::Max, EffortTier::Ultra] {
            let tier_name = match tier {
                EffortTier::Max => "MAX",
                EffortTier::Ultra => "ULTRA",
            };
            for millis in [0, 180, 420, 720, 1100, 1580, 2050] {
                let mut buf = test_buffer(area);
                paint(tier, style, Duration::from_millis(millis), area, &mut buf);
                let name = style_name(style);
                let rendered = frame(area, &buf).join("│");
                frames.push(format!("{name:8} {tier_name:5} {millis:4}ms │{rendered}│"));
            }
        }
    }
    insta::assert_snapshot!("effort_ignition_animation_gallery", frames.join("\n"));
}
