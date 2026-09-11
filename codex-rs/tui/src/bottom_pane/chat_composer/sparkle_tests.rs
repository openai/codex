use super::super::tests::new_test_composer;
use super::*;
use crate::render::renderable::Renderable;
use crate::terminal_palette::with_test_default_colors;
use crate::terminal_probe::DefaultColors;
use codex_protocol::openai_models::ReasoningEffort;
use pretty_assertions::assert_eq;
use ratatui::style::Style;

const SPARKLE_PALETTE: DefaultColors = DefaultColors {
    fg: (230, 216, 255),
    bg: (36, 27, 53),
};

fn new_test_sparkle() -> Sparkle {
    Sparkle {
        started: Instant::now(),
        phase: Cell::new(Phase::Waiting),
        terminal_focused: true,
    }
}

fn new_test_pane() -> BottomPane {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    BottomPane::new(crate::bottom_pane::BottomPaneParams {
        app_event_tx: crate::app_event_sender::AppEventSender::new(tx),
        frame_requester: crate::tui::FrameRequester::test_dummy(),
        has_input_focus: true,
        enhanced_keys_supported: false,
        placeholder_text: "Ask Codex to do anything".into(),
        disable_paste_burst: true,
        animations_enabled: true,
        skills: None,
    })
}

fn buffer_text(buffer: &Buffer) -> String {
    buffer
        .content
        .chunks(usize::from(buffer.area.width))
        .map(|row| {
            row.iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn rendered_sparkle(
    composer: &ChatComposer,
    area: Rect,
    now: Instant,
    baseline: &Buffer,
) -> Buffer {
    let [surface, _, textarea, _] = composer.layout_areas(area);
    let mut buffer = baseline.clone();
    composer.render_sparkle_at(
        surface,
        textarea,
        composer.cursor_pos(area),
        now,
        &mut buffer,
    );
    buffer
}

#[test]
fn sparkle_keeps_the_existing_composer_layout() {
    with_test_default_colors(
        DefaultColors {
            fg: (230, 216, 255),
            bg: (36, 27, 53),
        },
        || {
            let (mut composer, _rx) = new_test_composer();
            composer.set_text_content("Explore the night sky".to_string(), Vec::new(), Vec::new());
            let area = Rect::new(
                /*x*/ 0,
                /*y*/ 0,
                /*width*/ 60,
                composer.desired_height(/*width*/ 60),
            );
            let [surface, _, _, _] = composer.layout_areas(area);
            let mut buffer = Buffer::empty(area);
            composer.render(area, &mut buffer);
            render_stars(
                surface,
                composer.cursor_pos(area),
                /*protected_area*/ None,
                Duration::ZERO,
                /*foreground*/ (230, 216, 255),
                /*visibility*/ 1.0,
                &mut buffer,
            );
            // Snapshot the composer layout without freezing the star distribution.
            let rows = (area.y..area.bottom())
                .map(|y| {
                    (area.x..area.right())
                        .map(|x| {
                            let symbol = buffer[(x, y)].symbol();
                            if DOTS.contains(&symbol) { " " } else { symbol }
                        })
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n");
            insta::assert_snapshot!("astra_current_composer", rows);
        },
    );
}

#[test]
fn sparkle_covers_the_untouched_composer_and_fades_after_interaction() {
    with_test_default_colors(SPARKLE_PALETTE, || {
        let width = 40;
        let (mut composer, _rx) = new_test_composer();
        let area = Rect::new(
            /*x*/ 0,
            /*y*/ 0,
            width,
            composer.desired_height(width),
        );
        let [_, _, textarea, _] = composer.layout_areas(area);
        let mut baseline = Buffer::empty(area);
        composer.render(area, &mut baseline);
        let sparkle = new_test_sparkle();
        let started = sparkle.started;
        composer.astra_sparkle = Some(sparkle);

        let idle = started + Duration::from_secs(63);
        let active = rendered_sparkle(&composer, area, idle, &baseline);
        let stars = active
            .content
            .iter()
            .enumerate()
            .filter_map(|(index, cell)| DOTS.contains(&cell.symbol()).then_some(index))
            .collect::<Vec<_>>();
        let star_rows = stars
            .iter()
            .map(|index| active.pos_of(*index).1)
            .collect::<Vec<_>>();
        assert!(star_rows.iter().any(|y| *y < textarea.y));
        assert!(
            star_rows
                .iter()
                .any(|y| (textarea.y..textarea.bottom()).contains(y))
        );
        assert!(star_rows.iter().any(|y| *y >= textarea.bottom()));
        let cursor = composer.cursor_pos(area).unwrap();
        assert_eq!(active[cursor], baseline[cursor]);

        composer.astra_sparkle.as_ref().unwrap().interact(idle);
        let fading = rendered_sparkle(&composer, area, idle + INTERACTION_FADE / 2, &baseline);
        assert!(fading.content.iter().enumerate().all(|(index, cell)| {
            !DOTS.contains(&cell.symbol()) || active.content[index].symbol() == cell.symbol()
        }));
        assert!(stars.iter().any(|index| {
            let cell = &fading.content[*index];
            DOTS.contains(&cell.symbol()) && cell.fg != active.content[*index].fg
        }));

        let finished = rendered_sparkle(&composer, area, idle + INTERACTION_FADE, &baseline);
        assert_eq!(finished, baseline);
        insta::assert_snapshot!(
            format!("astra_sparkle_interaction_{width}"),
            format!(
                "idle\n{}\n\nfading\n{}\n\nfinished\n{}",
                buffer_text(&active),
                buffer_text(&fading),
                buffer_text(&finished),
            )
        );
    });
}

#[test]
fn sparkle_auto_fades_fifteen_seconds_after_its_first_visible_frame() {
    with_test_default_colors(SPARKLE_PALETTE, || {
        let (mut composer, _rx) = new_test_composer();
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 8,
        );
        let mut baseline = Buffer::empty(area);
        composer.render(area, &mut baseline);
        composer.astra_sparkle = Some(new_test_sparkle());
        let first_frame = composer.astra_sparkle.as_ref().unwrap().started
            + Duration::from_secs(/*secs*/ 63);
        let first = rendered_sparkle(&composer, area, first_frame, &baseline);
        assert_ne!(first, baseline);

        let deadline = first_frame + IDLE_TIMEOUT;
        let just_before = deadline - Duration::from_millis(/*millis*/ 50);
        let active = rendered_sparkle(&composer, area, just_before, &baseline);
        assert_ne!(active, baseline);
        assert_eq!(
            composer
                .astra_sparkle
                .as_ref()
                .unwrap()
                .frame(just_before)
                .unwrap()
                .next,
            Duration::from_millis(/*millis*/ 50)
        );

        let sparkle = composer.astra_sparkle.as_ref().unwrap();
        assert_eq!(
            [
                sparkle.frame(deadline).unwrap().visibility,
                sparkle.frame(deadline + IDLE_FADE / 4).unwrap().visibility,
                sparkle.frame(deadline + IDLE_FADE / 2).unwrap().visibility,
            ],
            [1.0, 0.75, 0.5]
        );
        let fading = rendered_sparkle(&composer, area, deadline + IDLE_FADE / 2, &baseline);
        assert!(fading.content.iter().enumerate().all(|(index, cell)| {
            !DOTS.contains(&cell.symbol()) || active.content[index].symbol() == cell.symbol()
        }));
        let finished = rendered_sparkle(&composer, area, deadline + IDLE_FADE, &baseline);
        assert_eq!(finished, baseline);
        assert!(matches!(
            composer.astra_sparkle.as_ref().unwrap().phase.get(),
            Phase::Finished
        ));
        insta::assert_snapshot!(
            "astra_idle_timeout",
            format!(
                "before timeout\n{}\n\nfading\n{}\n\nfinished\n{}",
                buffer_text(&active),
                buffer_text(&fading),
                buffer_text(&finished),
            )
        );
    });
}

#[test]
fn sparkle_preserves_placeholder_spaces_during_idle_animation() {
    with_test_default_colors(SPARKLE_PALETTE, || {
        let (mut composer, _rx) = new_test_composer();
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 8,
        );
        let [_, _, textarea, _] = composer.layout_areas(area);
        let mut baseline = Buffer::empty(area);
        composer.render(area, &mut baseline);
        composer.astra_sparkle = Some(new_test_sparkle());
        let start = composer.astra_sparkle.as_ref().unwrap().started
            + Duration::from_secs(/*secs*/ 63);
        let end = textarea.x + composer.placeholder_text.width() as u16;
        for second in 0..IDLE_TIMEOUT.as_secs() {
            let active = rendered_sparkle(
                &composer,
                area,
                start + Duration::from_secs(second),
                &baseline,
            );
            for x in textarea.x..end {
                assert_eq!(active[(x, textarea.y)], baseline[(x, textarea.y)]);
            }
            assert_ne!(active, baseline);
            if second == 2 {
                insta::assert_snapshot!("astra_placeholder_spaces", buffer_text(&active));
            }
        }
    });
}

#[test]
fn idle_deadline_elapses_while_terminal_focus_is_elsewhere() {
    with_test_default_colors(SPARKLE_PALETTE, || {
        let (mut composer, _rx) = new_test_composer();
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 8,
        );
        let mut baseline = Buffer::empty(area);
        composer.render(area, &mut baseline);
        composer.astra_sparkle = Some(new_test_sparkle());
        let first_frame = composer.astra_sparkle.as_ref().unwrap().started;
        let active = rendered_sparkle(&composer, area, first_frame, &baseline);
        assert_ne!(active, baseline);
        composer.astra_sparkle.as_mut().unwrap().terminal_focused = false;
        let after_deadline = first_frame + IDLE_TIMEOUT + IDLE_FADE;
        let hidden = rendered_sparkle(&composer, area, after_deadline, &baseline);
        assert_eq!(hidden, baseline);
        assert!(matches!(
            composer.astra_sparkle.as_ref().unwrap().phase.get(),
            Phase::Visible { .. }
        ));
        composer.astra_sparkle.as_mut().unwrap().terminal_focused = true;
        assert_eq!(
            rendered_sparkle(&composer, area, after_deadline, &baseline),
            baseline
        );
        assert!(matches!(
            composer.astra_sparkle.as_ref().unwrap().phase.get(),
            Phase::Finished
        ));
    });
}

#[test]
fn input_during_the_idle_fade_finishes_quickly_without_brightening() {
    let sparkle = new_test_sparkle();
    let first_frame = sparkle.started;
    sparkle.frame(first_frame).unwrap();
    sparkle
        .frame(first_frame + IDLE_TIMEOUT - Duration::from_millis(/*millis*/ 50))
        .unwrap();
    let interaction = first_frame + IDLE_TIMEOUT + IDLE_FADE / 2;
    let before = sparkle.frame(interaction).unwrap().visibility;
    sparkle.interact(interaction);
    assert_eq!(sparkle.frame(interaction).unwrap().visibility, before);
    assert!(sparkle.frame(interaction + INTERACTION_FADE).is_none());
}

#[test]
fn typing_and_paste_start_one_fade_without_decorating_the_draft() {
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;

    with_test_default_colors(SPARKLE_PALETTE, || {
        for paste in [false, true] {
            let (mut composer, _rx) = new_test_composer();
            let area = Rect::new(
                /*x*/ 0, /*y*/ 0, /*width*/ 60, /*height*/ 8,
            );
            composer.astra_sparkle = Some(new_test_sparkle());
            let mut initial = Buffer::empty(area);
            composer.render(area, &mut initial);

            if paste {
                composer.handle_paste("first  line\n中文  words".to_owned());
            } else {
                composer.handle_key_event(KeyEvent::from(KeyCode::Char('a')));
                composer.handle_key_event(KeyEvent::from(KeyCode::Char(' ')));
            }
            let sparkle = composer.astra_sparkle.take().unwrap();
            let Phase::Fading { started, .. } = sparkle.phase.get() else {
                panic!("first input should start fading");
            };
            let [surface, _, textarea, _] = composer.layout_areas(area);
            let mut baseline = Buffer::empty(area);
            composer.render(area, &mut baseline);
            composer.astra_sparkle = Some(sparkle);

            let fading =
                rendered_sparkle(&composer, area, started + INTERACTION_FADE / 2, &baseline);
            for y in textarea.y..textarea.bottom() {
                for x in surface.x..surface.right() {
                    assert_eq!(fading[(x, y)], baseline[(x, y)]);
                }
            }
            assert_eq!(
                rendered_sparkle(&composer, area, started + INTERACTION_FADE, &baseline),
                baseline
            );
            composer.set_text_content(String::new(), Vec::new(), Vec::new());
            assert!(matches!(
                composer.astra_sparkle.as_ref().unwrap().phase.get(),
                Phase::Finished
            ));
        }
    });
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn sparkle_stops_scheduling_after_the_fade() {
    let (mut composer, _rx) = new_test_composer();
    let (draw_tx, mut draw_rx) = tokio::sync::broadcast::channel(/*capacity*/ 4);
    composer.set_frame_requester(crate::tui::FrameRequester::new(draw_tx));
    composer.astra_sparkle = Some(new_test_sparkle());
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 60, /*height*/ 3,
    );
    let render = |composer: &ChatComposer| {
        with_test_default_colors(SPARKLE_PALETTE, || {
            composer.render(area, &mut Buffer::empty(area))
        });
    };
    render(&composer);
    assert!(
        tokio::time::timeout(FRAME_TICK * 2, draw_rx.recv())
            .await
            .is_ok()
    );
    composer
        .astra_sparkle
        .as_ref()
        .expect("sparkle")
        .interact(Instant::now() - INTERACTION_FADE);
    render(&composer);
    assert!(
        tokio::time::timeout(FRAME_TICK * 2, draw_rx.recv())
            .await
            .is_err()
    );
}

#[test]
fn sparkle_preserves_content_cursor_and_background() {
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 3,
    );
    let mut before = Buffer::empty(area);
    before.set_style(area, Style::default().bg(rgb_color((36, 27, 53))));
    before[(0, 0)].set_symbol("界");
    before[(2, 0)].set_symbol("!");
    before[(4, 0)].set_symbol("✦");
    before[(5, 0)].set_style(Style::default().reversed());
    before[(6, 0)].set_diff_option(CellDiffOption::Skip);
    let protected = [(0, 0), (1, 0), (2, 0), (3, 0), (4, 0), (5, 0), (6, 0)];
    let mut seen = std::collections::HashMap::new();
    for tick in 0..80 {
        let mut after = before.clone();
        render_stars(
            area,
            Some((3, 0)),
            /*protected_area*/ None,
            FRAME_TICK * tick,
            /*foreground*/ (230, 216, 255),
            /*visibility*/ 1.0,
            &mut after,
        );
        assert_eq!(
            protected.map(|p| after[p].clone()),
            protected.map(|p| before[p].clone())
        );
        for (index, cell) in after.content.iter().enumerate() {
            assert_eq!(cell.bg, before.content[index].bg);
            if DOTS.contains(&cell.symbol())
                && let Some(previous) = seen.insert(index, cell.symbol().to_string())
            {
                assert_eq!(cell.symbol(), previous);
            }
        }
    }
    assert!(!seen.is_empty());
}

#[test]
fn stars_fade_using_the_custom_terminal_foreground() {
    for colors in [
        DefaultColors {
            fg: (230, 216, 255),
            bg: (36, 27, 53),
        },
        DefaultColors {
            fg: (101, 123, 131),
            bg: (253, 246, 227),
        },
    ] {
        with_test_default_colors(colors, || {
            let area = Rect::new(
                /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 3,
            );
            let mut shades: std::collections::HashMap<usize, std::collections::HashSet<Color>> =
                std::collections::HashMap::new();
            for tick in 0..40 {
                let mut buffer = Buffer::empty(area);
                buffer.set_style(area, Style::default().bg(rgb_color(colors.bg)));
                render_stars(
                    area,
                    /*cursor*/ None,
                    /*protected_area*/ None,
                    FRAME_TICK * tick,
                    colors.fg,
                    /*visibility*/ 1.0,
                    &mut buffer,
                );
                for (index, cell) in buffer.content.iter().enumerate() {
                    if DOTS.contains(&cell.symbol()) {
                        let Color::Rgb(r, g, b) = cell.fg else {
                            panic!("expected RGB fade")
                        };
                        shades.entry(index).or_default().insert(cell.fg);
                        for (actual, fg, bg) in [
                            (r, colors.fg.0, colors.bg.0),
                            (g, colors.fg.1, colors.bg.1),
                            (b, colors.fg.2, colors.bg.2),
                        ] {
                            assert!((fg.min(bg)..=fg.max(bg)).contains(&actual));
                        }
                        assert_eq!(cell.bg, rgb_color(colors.bg));
                    }
                }
            }
            assert!(shades.values().any(|colors| colors.len() > 3));
        });
    }
}

#[test]
fn offline_composer_keys_fade_stars() {
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyEventKind;
    use crossterm::event::KeyModifiers;

    with_test_default_colors(SPARKLE_PALETTE, || {
        for code in [KeyCode::Enter, KeyCode::Tab, KeyCode::Left] {
            let mut pane = new_test_pane();
            let area = Rect::new(
                /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 6,
            );
            pane.composer.astra_sparkle = Some(new_test_sparkle());
            pane.composer.render(area, &mut Buffer::empty(area));
            pane.handle_disconnected_key(KeyEvent::new(KeyCode::Null, KeyModifiers::NONE));
            assert!(matches!(
                pane.composer.astra_sparkle.as_ref().unwrap().phase.get(),
                Phase::Visible { .. }
            ));
            pane.handle_disconnected_key(KeyEvent::new_with_kind(
                code,
                KeyModifiers::NONE,
                KeyEventKind::Release,
            ));
            assert!(matches!(
                pane.composer.astra_sparkle.as_ref().unwrap().phase.get(),
                Phase::Visible { .. }
            ));

            pane.handle_disconnected_key(KeyEvent::new(code, KeyModifiers::NONE));
            assert!(matches!(
                pane.composer.astra_sparkle.as_ref().unwrap().phase.get(),
                Phase::Fading { .. }
            ));
            assert_eq!(pane.composer.current_text(), "");
        }
    });
}

#[test]
fn model_changes_and_disable_setting_control_sparkle() {
    let mut pane = new_test_pane();
    with_test_default_colors(
        DefaultColors {
            fg: (230, 216, 255),
            bg: (36, 27, 53),
        },
        || {
            let mut settings = Tui {
                whimsy: true,
                animations: true,
                ..Tui::default()
            };
            pane.set_astra_sparkle("gpt-6-astra", &settings);
            let started = pane
                .composer
                .astra_sparkle
                .as_ref()
                .map(|sparkle| sparkle.started);
            pane.set_sparkle_terminal_focus(/*focused*/ false);
            pane.set_sparkle_terminal_focus(/*focused*/ true);
            for model in ["astra", "ASTRA-preview", "openai/astra-2026-09-01"] {
                pane.set_astra_sparkle(model, &settings);
                assert_eq!(
                    pane.composer
                        .astra_sparkle
                        .as_ref()
                        .and_then(Sparkle::enabled_foreground),
                    Some((230, 216, 255)),
                );
                assert_eq!(
                    pane.composer
                        .astra_sparkle
                        .as_ref()
                        .map(|sparkle| sparkle.started),
                    started
                );
            }
            pane.composer.interact_with_astra_sparkle();
            pane.set_astra_sparkle("gpt-6-astra", &settings);
            assert_eq!(
                pane.composer
                    .astra_sparkle
                    .as_ref()
                    .and_then(Sparkle::enabled_foreground),
                None,
            );
            for model in [
                "gpt-5.6-sol",
                "astral",
                "castrated",
                "astra2",
                "astra_preview",
            ] {
                pane.set_astra_sparkle(model, &settings);
                assert_eq!(
                    pane.composer
                        .astra_sparkle
                        .as_ref()
                        .and_then(Sparkle::enabled_foreground),
                    None,
                    "{model}",
                );
            }
            for (whimsy, animations) in [(false, true), (true, false), (false, false), (true, true)]
            {
                settings.whimsy = whimsy;
                settings.animations = animations;
                pane.set_astra_sparkle("astra", &settings);
                assert_eq!(
                    pane.composer
                        .astra_sparkle
                        .as_ref()
                        .and_then(Sparkle::enabled_foreground),
                    (whimsy && animations).then_some((230, 216, 255)),
                );
            }
        },
    );
}

#[test]
fn sparkle_yields_to_drafts_effort_bursts_and_popups() {
    with_test_default_colors(
        DefaultColors {
            fg: (230, 216, 255),
            bg: (36, 27, 53),
        },
        || {
            let (mut composer, _rx) = new_test_composer();
            composer.astra_sparkle = Some(new_test_sparkle());
            composer.set_text_content("hello 界".to_string(), Vec::new(), Vec::new());
            composer.set_active_reasoning_effort_baseline(Some(&ReasoningEffort::High));
            let area = Rect::new(
                /*x*/ 0,
                /*y*/ 0,
                /*width*/ 80,
                composer.desired_height(/*width*/ 80),
            );
            for effort in [ReasoningEffort::Max, ReasoningEffort::Ultra] {
                composer
                    .set_active_reasoning_effort(Some(&effort), /*animations_enabled*/ true);
                let started = composer.astra_sparkle.take();
                let mut before = Buffer::empty(area);
                composer.render(area, &mut before);
                composer.astra_sparkle = started;
                let mut after = Buffer::empty(area);
                composer.render(area, &mut after);
                let text = after
                    .content
                    .iter()
                    .map(ratatui::buffer::Cell::symbol)
                    .collect::<String>();
                assert!(text.contains("hello 界"));
                assert!(
                    after
                        .content
                        .iter()
                        .all(|cell| !DOTS.contains(&cell.symbol()))
                );
                assert_eq!(
                    composer.cursor_pos(area).map(|p| after[p].clone()),
                    composer.cursor_pos(area).map(|p| before[p].clone())
                );
                assert!(composer.effort_ignition.is_some());
            }
            composer.set_text_content("/mod".to_string(), Vec::new(), Vec::new());
            composer.draft.textarea.set_cursor(/*pos*/ 4);
            composer.sync_popups();
            assert!(!matches!(composer.popups.active, ActivePopup::None));
            let mut buffer = Buffer::empty(area);
            let [surface, _, textarea, _] = composer.layout_areas(area);
            composer.render_sparkle(surface, textarea, /*cursor*/ None, &mut buffer);
            assert_eq!(buffer, Buffer::empty(area));
        },
    );
}

#[test]
fn sparkle_waits_for_terminal_colors() {
    let (mut composer, _rx) = new_test_composer();
    composer.astra_sparkle = Some(new_test_sparkle());
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 3,
    );
    let [surface, _, textarea, _] = composer.layout_areas(area);
    let mut buffer = Buffer::empty(area);
    composer.render_sparkle(surface, textarea, /*cursor*/ None, &mut buffer);
    assert_eq!(buffer, Buffer::empty(area));
    with_test_default_colors(
        DefaultColors {
            fg: (230, 216, 255),
            bg: (36, 27, 53),
        },
        || {
            buffer.set_style(area, Style::default().bg(rgb_color((36, 27, 53))));
            composer.render_sparkle(surface, textarea, /*cursor*/ None, &mut buffer);
            assert!(
                buffer
                    .content
                    .iter()
                    .any(|cell| DOTS.contains(&cell.symbol()))
            );
        },
    );
}

#[test]
fn sparkle_preserves_voice_indicator_styles() {
    use crate::bottom_pane::voice_strip::VoiceStripPhase;
    use crate::bottom_pane::voice_strip::VoiceStripState;
    use crate::tui::FrameRequester;

    with_test_default_colors(
        DefaultColors {
            fg: (230, 216, 255),
            bg: (36, 27, 53),
        },
        || {
            let (mut composer, _rx) = new_test_composer();
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
                }),
                FrameRequester::test_dummy(),
            );
            let area = Rect::new(
                /*x*/ 0,
                /*y*/ 0,
                /*width*/ 60,
                composer.desired_height(/*width*/ 60),
            );
            let mut baseline = Buffer::empty(area);
            composer.render(area, &mut baseline);
            let mut saw_stars = false;
            for tick in 0..80 {
                composer.astra_sparkle = Some(Sparkle {
                    started: Instant::now() - FRAME_TICK * tick,
                    ..new_test_sparkle()
                });
                let mut actual = Buffer::empty(area);
                composer.render(area, &mut actual);
                saw_stars |= actual
                    .content
                    .iter()
                    .any(|cell| DOTS.contains(&cell.symbol()));
                // Compare whole cells: an unchanged glyph can still inherit a star's color.
                let content = baseline
                    .content
                    .iter()
                    .enumerate()
                    .filter(|(_, cell)| cell.symbol() != " ")
                    .map(|(index, _)| actual.content[index].clone())
                    .collect::<Vec<_>>();
                let expected = baseline
                    .content
                    .iter()
                    .filter(|cell| cell.symbol() != " ")
                    .cloned()
                    .collect::<Vec<_>>();
                assert_eq!(content, expected, "sparkle frame {tick}");
            }
            assert!(!saw_stars);
            insta::assert_snapshot!(
                "astra_voice_composer",
                baseline
                    .content
                    .chunks(60)
                    .map(|row| row
                        .iter()
                        .map(ratatui::buffer::Cell::symbol)
                        .collect::<String>())
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        },
    );
}
