//! Logo visibility, theme contrast, placement, and redraw lifecycle tests.

use super::*;
use pretty_assertions::assert_eq;

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

pub(crate) fn fixture() -> (EmptyStateAnimation, Size, Rect, Buffer) {
    let size = Size::new(/*width*/ 120, /*height*/ 44);
    let mut animation = EmptyStateAnimation::default();
    animation.start_fresh();
    animation.cell_aspect = Some((size, 0.5));
    let area = Rect::new(/*x*/ 0, /*y*/ 0, size.width, size.height);
    let bottom = Rect::new(/*x*/ 0, /*y*/ 39, size.width, /*height*/ 5);
    (animation, size, bottom, Buffer::empty(area))
}

#[test]
fn upper_right_stage_shrinks_and_hides_without_touching_occupied_cells() {
    let (_, size, bottom, mut buffer) = fixture();
    assert_eq!(
        stage(size, bottom, &buffer, /*aspect*/ 0.5),
        Some(Rect::new(
            /*x*/ 56, /*y*/ 2, /*width*/ 60, /*height*/ 21
        ))
    );
    buffer[(56, 2)].set_symbol("header");
    let smaller = stage(size, bottom, &buffer, /*aspect*/ 0.5).unwrap();
    assert!(smaller.width < 60);
    buffer[(115, 2)].set_symbol("notice");
    assert_eq!(stage(size, bottom, &buffer, /*aspect*/ 0.5), None);
}

#[test]
fn faded_presentation_settles_without_redraws_then_resumes() {
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 48, /*height*/ 17,
    );
    let start = Instant::now();
    for elapsed in [
        Duration::from_secs(/*secs*/ 2),
        sequence::SPIN_DURATION + sequence::COMPLETION_FADE - FRAME_INTERVAL,
    ] {
        let mut animation = EmptyStateAnimation::default();
        animation.start_fresh();
        animation.spin_elapsed = elapsed;
        let mut buffer = Buffer::empty(area);
        animation.render_in_at(
            area,
            &mut buffer,
            Presentation::Animated,
            AnimationEnd::Hide,
            start,
        );
        let moving = buffer.clone();
        buffer.reset();
        assert_eq!(
            animation.render_in_at(
                area,
                &mut buffer,
                Presentation::Faded,
                AnimationEnd::Hide,
                start + FRAME_INTERVAL,
            ),
            Some(FRAME_INTERVAL)
        );
        assert_eq!(buffer, moving);
        let settled_at = start + FRAME_INTERVAL + sequence::STATIC_FADE;
        buffer.reset();
        assert_eq!(
            animation.render_in_at(
                area,
                &mut buffer,
                Presentation::Faded,
                AnimationEnd::Hide,
                settled_at,
            ),
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
                AnimationEnd::Hide,
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
                AnimationEnd::Hide,
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
                AnimationEnd::Hide,
                settled_at + Duration::from_secs(/*secs*/ 61) + FRAME_INTERVAL,
            ),
            Some(FRAME_INTERVAL)
        );
        assert_ne!(animation.opacity, sequence::STATIC_OPACITY);
    }
}

#[test]
fn three_rotations_stop_then_fade_without_restarting() {
    let (mut animation, size, bottom, mut buffer) = fixture();
    animation.spin_elapsed = sequence::SPIN_DURATION;
    assert_eq!(
        animation.render(size, bottom, &mut buffer, Presentation::Animated,),
        Some(FRAME_INTERVAL)
    );
    assert_eq!(animation.opacity, 1.0);
    let stopped = buffer.clone();
    animation.last_frame = None;
    animation.spin_elapsed += sequence::COMPLETION_FADE / 2;
    buffer.reset();
    assert_eq!(
        animation.render(size, bottom, &mut buffer, Presentation::Animated,),
        Some(FRAME_INTERVAL)
    );
    assert_eq!(animation.opacity, 0.5);
    // Fading changes color only: rotation has stopped on the third completed turn.
    assert_eq!(
        buffer
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<Vec<_>>(),
        stopped
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<Vec<_>>()
    );
    animation.spin_elapsed = sequence::SPIN_DURATION + sequence::COMPLETION_FADE;
    for presentation in [Presentation::Animated, Presentation::Faded] {
        buffer.reset();
        assert_eq!(
            animation.render(size, bottom, &mut buffer, presentation,),
            None
        );
        assert_eq!(buffer, Buffer::empty(buffer.area));
    }
    animation.start_fresh();
    assert_eq!(
        animation.render(size, bottom, &mut buffer, Presentation::Animated,),
        Some(FRAME_INTERVAL)
    );
    assert_eq!(animation.spin_elapsed, Duration::ZERO);
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
    animation.render_in_at(
        area,
        &mut buffer,
        Presentation::Animated,
        AnimationEnd::Hide,
        start,
    );
    let visible = Duration::from_millis(/*millis*/ 150);
    buffer.reset();
    animation.render_in_at(
        area,
        &mut buffer,
        Presentation::Animated,
        AnimationEnd::Hide,
        start + visible,
    );
    let before = buffer.clone();
    animation.pause_clock();
    let resumed = start + Duration::from_secs(/*secs*/ 3600);
    buffer.reset();
    assert_eq!(
        animation.render_in_at(
            area,
            &mut buffer,
            Presentation::Animated,
            AnimationEnd::Hide,
            resumed
        ),
        Some(FRAME_INTERVAL)
    );
    assert_eq!(buffer, before);
    assert_eq!(animation.spin_elapsed, visible);
    buffer.reset();
    animation.render_in_at(
        area,
        &mut buffer,
        Presentation::Animated,
        AnimationEnd::Hide,
        resumed + FRAME_INTERVAL,
    );
    assert_eq!(animation.spin_elapsed, visible + FRAME_INTERVAL);
}

#[test]
fn hidden_and_clipped_frames_preserve_completion_fade_without_restarting() {
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 48, /*height*/ 17,
    );
    let start = Instant::now();
    for hidden_area in [area, Rect::default()] {
        let mut animation = EmptyStateAnimation::default();
        animation.start_fresh();
        animation.spin_elapsed = sequence::SPIN_DURATION + sequence::COMPLETION_FADE / 2;
        let mut buffer = Buffer::empty(area);
        animation.render_in_at(
            area,
            &mut buffer,
            Presentation::Animated,
            AnimationEnd::Hide,
            start,
        );
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
                AnimationEnd::Hide,
                start + Duration::from_secs(/*secs*/ 20)
            ),
            None
        );
        assert_eq!(buffer, Buffer::empty(area));
        let resumed = start + Duration::from_secs(/*secs*/ 40);
        animation.render_in_at(
            area,
            &mut buffer,
            Presentation::Animated,
            AnimationEnd::Hide,
            resumed,
        );
        assert_eq!(buffer, before);
        buffer.reset();
        assert_eq!(
            animation.render_in_at(
                area,
                &mut buffer,
                Presentation::Animated,
                AnimationEnd::Hide,
                resumed + sequence::COMPLETION_FADE / 2
            ),
            None
        );
        assert_eq!(buffer, Buffer::empty(area));
        animation.pause_clock();
        assert!(!animation.eligible);
    }
}

#[test]
fn onboarding_keeps_a_settled_mark_after_budget_and_pause() {
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 48, /*height*/ 17,
    );
    let start = Instant::now();
    let mut animation = EmptyStateAnimation::default();
    animation.start_fresh();
    animation.spin_elapsed = sequence::SPIN_DURATION + sequence::COMPLETION_FADE;
    let mut buffer = Buffer::empty(area);
    assert_eq!(
        animation.render_in_at(
            area,
            &mut buffer,
            Presentation::Animated,
            AnimationEnd::Faded,
            start
        ),
        None
    );
    assert!(buffer.content.iter().any(|cell| cell.symbol() != " "));
    let settled = buffer.clone();
    animation.pause_clock();
    buffer.reset();
    assert_eq!(
        animation.render_in_at(
            area,
            &mut buffer,
            Presentation::Faded,
            AnimationEnd::Faded,
            start + Duration::from_secs(/*secs*/ 3600)
        ),
        None
    );
    assert_eq!(buffer, settled);
}
