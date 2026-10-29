use super::*;

use crate::terminal_hyperlinks::osc8_hyperlink;
use crate::terminal_palette::indexed_color;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
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
        EffortStatusLineTransition::new(EffortTier::Ultra, "gpt-5.4 high · main".into());
    assert!(!transition.is_finished());
    assert_eq!(transition.started_at.get(), None);

    let current = HyperlinkLine::from("gpt-5.4 ultra · main");
    let line = transition
        .render_line(Some(current), /*width*/ 40)
        .expect("first frame should render the outgoing line");
    assert_eq!(text(&line.line), "gpt-5.4 high · main");
    assert!(transition.started_at.get().is_some());
}

#[test]
fn outgoing_status_line_moves_right_and_clips_at_the_edge() {
    let previous = HyperlinkLine::from("gpt-5.4 high · main");
    let current = HyperlinkLine::from("gpt-5.4 ultra · main");

    let first = transition_line_at(
        EffortTier::Ultra,
        &previous,
        Some(current.clone()),
        Duration::ZERO,
        /*width*/ 28,
    )
    .expect("outgoing line should be visible");
    let middle = transition_line_at(
        EffortTier::Ultra,
        &previous,
        Some(current),
        Duration::from_millis(300),
        /*width*/ 28,
    )
    .expect("outgoing line should be visible");

    assert_eq!(text(&first.line), "gpt-5.4 high · main");
    let first_offset = text(&first.line)
        .find("gpt-5.4")
        .expect("first frame should contain the outgoing model");
    let middle_offset = text(&middle.line)
        .find("gpt-5.4")
        .expect("middle frame should contain the outgoing model");
    assert!(middle_offset > first_offset);
    assert!(middle.width() <= 28);
}

#[test]
fn label_and_refreshed_line_appear_in_order() {
    let previous = HyperlinkLine::from("gpt-5.4 high · main");
    let current = HyperlinkLine::from("gpt-5.4 max · main");

    let label = transition_line_at(
        EffortTier::Max,
        &previous,
        Some(current.clone()),
        Duration::from_millis(1500),
        /*width*/ 32,
    )
    .expect("label should be visible");
    let refreshed = transition_line_at(
        EffortTier::Max,
        &previous,
        Some(current),
        Duration::from_millis(2200),
        /*width*/ 32,
    )
    .expect("refreshed status line should be visible");

    assert_eq!(text(&label.line), "             M A X");
    assert_eq!(text(&refreshed.line), "gpt-5.4 max · main");
}

#[test]
fn unicode_and_span_boundaries_are_safe_on_narrow_rows() {
    let previous = HyperlinkLine::from(Line::from(vec!["模型 ".cyan(), "👩‍💻 main".underlined()]));
    let current = HyperlinkLine::from(Line::from(vec![
        "模型 ".cyan(),
        "ultra · main".underlined(),
    ]));

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
                &previous,
                Some(current.clone()),
                elapsed,
                width,
            );
            assert!(line.is_none_or(|line| line.width() <= usize::from(width)));
        }
    }
}

#[test]
fn links_follow_the_visible_text_through_the_effort_transition() {
    let old_url = "https://example.com/pr/42";
    let new_url = "https://example.com/pr/43";
    let mut previous = HyperlinkLine::from("a ");
    previous.push_span("PR42界".into(), Some(old_url));
    let mut current = HyperlinkLine::from("b ");
    current.push_span("PR43".into(), Some(new_url));

    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 8, /*height*/ 1,
    );
    for (elapsed_ms, visible, linked_columns, url) in [
        // One cell of slide offset. The wide trailing glyph cannot fit the last cell.
        (310, " a PR42 ", 3..7, old_url),
        (1500, " M A X  ", 0..0, old_url),
        (2200, "b PR43  ", 2..6, new_url),
    ] {
        let frame = transition_line_at(
            EffortTier::Max,
            &previous,
            Some(current.clone()),
            Duration::from_millis(elapsed_ms),
            area.width,
        )
        .expect("visible frame");
        let mut buf = Buffer::empty(area);
        frame.render_fitted(area, &mut buf);
        for (x, ch) in (0..area.width).zip(visible.chars()) {
            let symbol = ch.to_string();
            let expected = if linked_columns.contains(&x) {
                osc8_hyperlink(url, &symbol)
            } else {
                symbol
            };
            assert_eq!(buf[(x, 0)].symbol(), expected, "{elapsed_ms}ms, column {x}");
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
    let previous = HyperlinkLine::from(Line::from(vec!["gpt-5.4 high".cyan(), " · main".dim()]));
    let max = HyperlinkLine::from(Line::from(vec!["gpt-5.4 max".cyan(), " · main".dim()]));
    let ultra = HyperlinkLine::from(Line::from(vec!["gpt-5.4 ultra".cyan(), " · main".dim()]));
    let mut frames = Vec::new();

    for (tier, current) in [(EffortTier::Max, &max), (EffortTier::Ultra, &ultra)] {
        for millis in [
            0, 300, 520, 680, 820, 1050, 1320, 1550, 1800, 2020, 2250, 2500,
        ] {
            let line = transition_line_at(
                tier,
                &previous,
                Some(current.clone()),
                Duration::from_millis(millis),
                /*width*/ 32,
            )
            .expect("frame should render");
            let label = tier.label();
            frames.push(format!("{label:5} {millis:4}ms │{:<32}│", text(&line.line)));
        }
    }

    insta::assert_snapshot!("effort_status_line_transition_frames", frames.join("\n"));
}
