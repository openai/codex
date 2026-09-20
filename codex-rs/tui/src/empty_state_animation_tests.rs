//! Logo visibility, theme contrast, placement, and redraw lifecycle tests.

use super::*;
use pretty_assertions::assert_eq;

impl EmptyStateAnimation {
    // App integration tests inspect lifecycle eligibility without advancing the render clock.
    pub(crate) fn is_eligible_for_test(&self) -> bool {
        self.eligible
    }
}

#[test]
fn loop_closes_and_light_themes_preserve_coverage() {
    let mut renderer = Renderer::default();
    let dark = Lighting::terminal(/*fg*/ (210, 221, 235), /*bg*/ (15, 20, 37));
    let first = renderer
        .frame(
            /*columns*/ 60, /*rows*/ 21, /*phase*/ 0.0, &dark,
        )
        .to_vec();
    assert_eq!(
        renderer.frame(
            /*columns*/ 60, /*rows*/ 21, /*phase*/ 1.0, &dark
        ),
        first
    );
    let other_mark = renderer.frame(
        /*columns*/ 60, /*rows*/ 21, /*phase*/ 0.5, &dark,
    );
    assert!(first.iter().any(|cell| cell.dots != 0));
    assert!(other_mark.iter().any(|cell| cell.dots != 0));
    assert_ne!(other_mark, first);
    let light = Lighting::terminal(/*fg*/ (32, 32, 32), /*bg*/ (250, 250, 250));
    let light_frame = renderer.frame(
        /*columns*/ 60, /*rows*/ 21, /*phase*/ 0.0, &light,
    );
    assert_eq!(
        light_frame.iter().map(|cell| cell.dots).collect::<Vec<_>>(),
        first.iter().map(|cell| cell.dots).collect::<Vec<_>>()
    );
    for cell in light_frame.iter().filter(|cell| cell.dots != 0) {
        let [_, r, g, b] = cell.rgb.to_be_bytes();
        assert!(
            r < 200 && g < 200 && b < 200,
            "light-theme ink must contrast with the background"
        );
    }
}

#[test]
fn faded_presentation_settles_without_redraws_then_resumes() {
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 48, /*height*/ 17,
    );
    let start = Instant::now();
    for elapsed in [
        Duration::from_secs(/*secs*/ 2),
        sequence::SPIN_DURATION - FRAME_INTERVAL * 2,
    ] {
        let mut animation = EmptyStateAnimation::default();
        animation.start_fresh();
        animation.spin_elapsed = elapsed;
        let mut buffer = Buffer::empty(area);
        animation.render_in_at(area, &mut buffer, Presentation::Animated, start);
        let moving = buffer.clone();
        buffer.reset();
        assert_eq!(
            animation.render_in_at(
                area,
                &mut buffer,
                Presentation::Faded,
                start + FRAME_INTERVAL,
            ),
            Some(FRAME_INTERVAL)
        );
        assert_eq!(buffer, moving);
        let settled_at = start + FRAME_INTERVAL + sequence::STATIC_FADE;
        buffer.reset();
        assert_eq!(
            animation.render_in_at(area, &mut buffer, Presentation::Faded, settled_at,),
            None
        );
        assert!(buffer.content.iter().any(|cell| cell.symbol() != " "));
        // Missing OSC colors must preserve the terminal's readable default foreground.
        if terminal_palette::default_bg().is_none() {
            assert!(
                buffer
                    .content
                    .iter()
                    .filter(|cell| cell.symbol() != " ")
                    .all(|cell| cell.fg == ratatui::style::Color::Reset)
            );
        }
        let settled = buffer.clone();
        buffer.reset();
        assert_eq!(
            animation.render_in_at(
                area,
                &mut buffer,
                Presentation::Faded,
                settled_at + Duration::from_secs(/*secs*/ 60),
            ),
            None
        );
        assert_eq!(buffer, settled);
        buffer.reset();
        assert_eq!(
            animation.render_in_at(
                area,
                &mut buffer,
                Presentation::Animated,
                settled_at + Duration::from_secs(/*secs*/ 61),
            ),
            Some(FRAME_INTERVAL)
        );
        assert_eq!(buffer, settled);
        assert_eq!(animation.spin_elapsed, elapsed);
        buffer.reset();
        assert_eq!(
            animation.render_in_at(
                area,
                &mut buffer,
                Presentation::Animated,
                settled_at + Duration::from_secs(/*secs*/ 61) + FRAME_INTERVAL,
            ),
            Some(FRAME_INTERVAL)
        );
        assert_ne!(animation.opacity, sequence::STATIC_OPACITY);
    }
}

#[test]
fn skipped_frames_preserve_spin_until_visible_time_resumes() {
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 48, /*height*/ 17,
    );
    let start = Instant::now();
    // The same pause is used before editor handoff and after a job-control resume.
    // Calling it after an arbitrarily long interruption must still stop at the last frame.
    let mut animation = EmptyStateAnimation::default();
    animation.start_fresh();
    let mut buffer = Buffer::empty(area);
    animation.render_in_at(area, &mut buffer, Presentation::Animated, start);
    let visible = Duration::from_millis(/*millis*/ 150);
    buffer.reset();
    animation.render_in_at(area, &mut buffer, Presentation::Animated, start + visible);
    let before = buffer.clone();
    animation.pause_clock();
    let resumed = start + Duration::from_secs(/*secs*/ 3600);
    buffer.reset();
    assert_eq!(
        animation.render_in_at(area, &mut buffer, Presentation::Animated, resumed),
        Some(FRAME_INTERVAL)
    );
    assert_eq!(buffer, before);
    assert_eq!(animation.spin_elapsed, visible);
    buffer.reset();
    animation.render_in_at(
        area,
        &mut buffer,
        Presentation::Animated,
        resumed + FRAME_INTERVAL,
    );
    assert_eq!(animation.spin_elapsed, visible + FRAME_INTERVAL);
}

#[test]
fn hidden_and_clipped_frames_preserve_spin_budget_without_restarting() {
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 48, /*height*/ 17,
    );
    let start = Instant::now();
    for hidden_area in [area, Rect::default()] {
        let mut animation = EmptyStateAnimation::default();
        animation.start_fresh();
        animation.spin_elapsed = sequence::SPIN_DURATION - FRAME_INTERVAL;
        let mut buffer = Buffer::empty(area);
        animation.render_in_at(area, &mut buffer, Presentation::Animated, start);
        let before = buffer.clone();
        buffer.reset();
        let hidden = if hidden_area.is_empty() {
            Presentation::Animated
        } else {
            Presentation::Hidden
        };
        assert_eq!(
            animation.render_in_at(
                hidden_area,
                &mut buffer,
                hidden,
                start + Duration::from_secs(/*secs*/ 20)
            ),
            None
        );
        assert_eq!(buffer, Buffer::empty(area));
        let resumed = start + Duration::from_secs(/*secs*/ 40);
        animation.render_in_at(area, &mut buffer, Presentation::Animated, resumed);
        assert_eq!(buffer, before);
        buffer.reset();
        assert_eq!(
            animation.render_in_at(
                area,
                &mut buffer,
                Presentation::Animated,
                resumed + FRAME_INTERVAL
            ),
            None
        );
        assert!(buffer.content.iter().any(|cell| cell.symbol() != " "));
        assert_eq!(animation.opacity, 1.0);
        animation.pause_clock();
    }
}

#[test]
fn onboarding_keeps_full_color_after_budget_and_pause() {
    crate::terminal_palette::with_test_default_colors(
        crate::terminal_probe::DefaultColors {
            fg: (210, 221, 235),
            bg: (15, 20, 37),
        },
        || {
            let area = Rect::new(
                /*x*/ 0, /*y*/ 0, /*width*/ 48, /*height*/ 17,
            );
            let start = Instant::now();
            let mut animation = EmptyStateAnimation::default();
            animation.start_fresh();
            let mut buffer = Buffer::empty(area);
            animation.render_in_at(area, &mut buffer, Presentation::Animated, start);
            buffer.reset();
            assert_eq!(
                animation.render_in_at(
                    area,
                    &mut buffer,
                    Presentation::Animated,
                    start + sequence::SPIN_DURATION
                ),
                None
            );
            assert_eq!(animation.opacity, 1.0);
            assert!(buffer.content.iter().any(|cell| cell.symbol() != " "));
            insta::assert_snapshot!(
                "onboarding_settled_logo",
                buffer
                    .content
                    .chunks(usize::from(area.width))
                    .map(|row| row
                        .iter()
                        .map(ratatui::buffer::Cell::symbol)
                        .collect::<String>()
                        .trim_end()
                        .to_owned())
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            let settled = buffer.clone();
            animation.pause_clock();
            buffer.reset();
            assert_eq!(
                animation.render_in_at(
                    area,
                    &mut buffer,
                    Presentation::Faded,
                    start + Duration::from_secs(/*secs*/ 3600)
                ),
                None
            );
            assert_eq!(buffer, settled);
        },
    );
}
