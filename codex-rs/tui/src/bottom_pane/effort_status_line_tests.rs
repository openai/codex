use super::*;

use crate::terminal_palette::indexed_color;
use pretty_assertions::assert_eq;
use ratatui::style::Stylize;

fn text(line: &Line<'static>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

#[test]
fn clock_waits_for_the_first_visible_frame() {
    let transition =
        EffortStatusLineTransition::new(EffortTier::Ultra, Line::from("gpt-5.4 high · main"));
    assert!(!transition.is_finished());
    assert_eq!(transition.started_at.get(), None);

    let current = Line::from("gpt-5.4 ultra · main");
    let line = transition
        .render_line(Some(&current), /*width*/ 40)
        .expect("first frame should render the outgoing line");
    assert_eq!(text(&line), "gpt-5.4 high · main");
    assert!(transition.started_at.get().is_some());
}

#[test]
fn outgoing_status_line_moves_right_and_clips_at_the_edge() {
    let previous = Line::from("gpt-5.4 high · main");
    let current = Line::from("gpt-5.4 ultra · main");

    let first = transition_line_at(
        EffortTier::Ultra,
        Some(&previous),
        Some(&current),
        Duration::ZERO,
        /*width*/ 28,
    )
    .expect("outgoing line should be visible");
    let middle = transition_line_at(
        EffortTier::Ultra,
        Some(&previous),
        Some(&current),
        Duration::from_millis(300),
        /*width*/ 28,
    )
    .expect("outgoing line should be visible");

    assert_eq!(text(&first), "gpt-5.4 high · main");
    let first_offset = text(&first)
        .find("gpt-5.4")
        .expect("first frame should contain the outgoing model");
    let middle_offset = text(&middle)
        .find("gpt-5.4")
        .expect("middle frame should contain the outgoing model");
    assert!(middle_offset > first_offset);
    assert!(middle.width() <= 28);
}

#[test]
fn label_and_refreshed_line_appear_in_order() {
    let previous = Line::from("gpt-5.4 high · main");
    let current = Line::from("gpt-5.4 max · main");

    let label = transition_line_at(
        EffortTier::Max,
        Some(&previous),
        Some(&current),
        Duration::from_millis(1500),
        /*width*/ 32,
    )
    .expect("label should be visible");
    let refreshed = transition_line_at(
        EffortTier::Max,
        Some(&previous),
        Some(&current),
        Duration::from_millis(2200),
        /*width*/ 32,
    )
    .expect("refreshed status line should be visible");

    assert_eq!(text(&label), "             M A X");
    assert_eq!(text(&refreshed), "gpt-5.4 max · main");
}

#[test]
fn unicode_and_span_boundaries_are_safe_on_narrow_rows() {
    let previous = Line::from(vec!["模型 ".cyan(), "👩‍💻 main".underlined()]);
    let current = Line::from(vec!["模型 ".cyan(), "ultra · main".underlined()]);

    for width in 0..=12 {
        for elapsed in [
            Duration::ZERO,
            Duration::from_millis(300),
            Duration::from_millis(700),
            Duration::from_millis(1450),
            Duration::from_millis(2250),
            Duration::from_millis(2500),
        ] {
            let line = transition_line_at(
                EffortTier::Ultra,
                Some(&previous),
                Some(&current),
                elapsed,
                width,
            );
            assert!(line.is_none_or(|line| line.width() <= usize::from(width)));
        }
    }
}

#[test]
fn palette_status_line_colors_survive_the_fade() {
    for color in [
        Color::Cyan,
        Color::Green,
        Color::Magenta,
        indexed_color(/*index*/ 123),
    ] {
        let mut style = Style::default().fg(color);
        apply_fade(
            &mut style,
            EffortTier::Ultra,
            /*opacity*/ 0.4,
            /*tint*/ 1.0,
        );
        assert_eq!(style.fg, Some(color));
        assert!(style.add_modifier.contains(Modifier::DIM));
    }
}

#[test]
fn ultra_letters_start_wide_at_the_edges_and_converge_to_the_center() {
    let scattered = tier_label_line(
        EffortTier::Ultra,
        /*width*/ 40,
        /*assemble*/ 0.0,
        /*opacity*/ 1.0,
    );
    let settled = tier_label_line(
        EffortTier::Ultra,
        /*width*/ 40,
        /*assemble*/ 1.0,
        /*opacity*/ 1.0,
    );
    let positions = text(&scattered)
        .char_indices()
        .filter_map(|(index, letter)| letter.is_ascii_uppercase().then_some(index))
        .collect::<Vec<_>>();
    let gaps = positions
        .windows(2)
        .map(|pair| pair[1].saturating_sub(pair[0]))
        .collect::<Vec<_>>();

    assert_eq!(positions.len(), 5);
    assert!(gaps[0] > gaps[1]);
    assert!(gaps[3] > gaps[2]);
    assert_eq!(text(&settled), "               U L T R A");
}

#[test]
fn transition_frames_snapshot() {
    let previous = Line::from(vec!["gpt-5.4 high".cyan(), " · main".dim()]);
    let max = Line::from(vec!["gpt-5.4 max".cyan(), " · main".dim()]);
    let ultra = Line::from(vec!["gpt-5.4 ultra".cyan(), " · main".dim()]);
    let mut frames = Vec::new();

    for (tier, current) in [(EffortTier::Max, &max), (EffortTier::Ultra, &ultra)] {
        for millis in [
            0, 300, 520, 680, 820, 1050, 1320, 1550, 1800, 2020, 2250, 2500,
        ] {
            let line = transition_line_at(
                tier,
                Some(&previous),
                Some(current),
                Duration::from_millis(millis),
                /*width*/ 32,
            )
            .expect("frame should render");
            let label = tier.label();
            frames.push(format!("{label:5} {millis:4}ms │{:<32}│", text(&line)));
        }
    }

    insta::assert_snapshot!("effort_status_line_transition_frames", frames.join("\n"));
}
