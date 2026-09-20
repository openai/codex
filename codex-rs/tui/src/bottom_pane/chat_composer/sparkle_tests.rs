//! Covers the original starfield appearance, protected content, and composer eligibility.

use super::*;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::BottomPaneParams;
use crate::bottom_pane::RestrictedInputMode;
use crate::bottom_pane::chat_composer_history::HistoryEntry;
use crate::render::renderable::Renderable;
use crate::slash_command::SlashCommand;
use crate::terminal_palette::with_test_default_colors;
use crate::terminal_probe::DefaultColors;
use crate::tui::FrameRequester;
use pretty_assertions::assert_eq;
use ratatui::layout::Position;
use ratatui::style::Color;

use super::field::DOTS;

fn pane() -> BottomPane {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    BottomPane::new(BottomPaneParams {
        app_event_tx: AppEventSender::new(tx),
        frame_requester: FrameRequester::test_dummy(),
        has_input_focus: true,
        enhanced_keys_supported: false,
        placeholder_text: "Ask Codex to do anything".into(),
        disable_paste_burst: true,
        animations_enabled: true,
        effects: Default::default(),
        skills: None,
    })
}

fn palette<T>(render: impl FnOnce() -> T) -> T {
    with_test_default_colors(
        DefaultColors {
            fg: (230, 216, 255),
            bg: (36, 27, 53),
        },
        render,
    )
}

fn enabled() -> Tui {
    Tui {
        animations: true,
        effects: Default::default(),
        ..Tui::default()
    }
}

fn draw(composer: &ChatComposer, width: u16, now: Instant) -> (Buffer, Rect) {
    let area = Rect::new(
        /*x*/ 0,
        /*y*/ 0,
        width,
        composer.desired_height(width),
    );
    let layout = composer.layout_with_options(area, Default::default());
    let phase = composer.sparkle.phase.replace(Phase::Unarmed);
    let mut buffer = Buffer::empty(area);
    composer.render(area, &mut buffer);
    composer.sparkle.phase.set(phase);
    composer.render_sparkle_at(
        layout.composer,
        layout.textarea,
        composer.cursor_pos(area),
        now,
        &mut buffer,
    );
    (buffer, layout.textarea)
}

fn text(buffer: &Buffer) -> String {
    buffer
        .content
        .chunks(usize::from(buffer.area.width))
        .map(|row| {
            row.iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn dots(buffer: &Buffer) -> Vec<(u16, u16, String)> {
    buffer
        .content
        .iter()
        .enumerate()
        .filter(|&(_, cell)| DOTS.contains(&cell.symbol()))
        .map(|(index, cell)| {
            let (x, y) = buffer.pos_of(index);
            (x, y, cell.symbol().to_string())
        })
        .collect()
}

fn key(pane: &mut BottomPane, key: KeyCode) -> InputResult {
    pane.handle_key_event(KeyEvent::new(key, KeyModifiers::NONE))
}

fn type_text(pane: &mut BottomPane, text: &str) {
    for ch in text.chars() {
        key(pane, KeyCode::Char(ch));
    }
}

#[test]
fn sparkle_matches_the_original_starfield_and_protects_the_placeholder_and_cursor() {
    palette(|| {
        let now = Instant::now();
        let mut snapshots = Vec::new();
        for width in [40, 80] {
            let mut pane = pane();
            let (plain, _) = draw(&pane.composer, width, now);
            let cursor = pane.composer.cursor_pos(plain.area);
            pane.mark_fresh_task_for_sparkle("gpt-6-astra", &enabled());
            let (active, textarea) = draw(&pane.composer, width, now);
            assert_eq!(pane.composer.cursor_pos(active.area), cursor);
            let (later, _) = draw(&pane.composer, width, now + Duration::from_secs(/*secs*/ 2));
            let protected = Rect::new(
                textarea.x,
                textarea.y,
                pane.composer.placeholder_text.width() as u16,
                /*height*/ 1,
            );
            let placeholder = |buffer: &Buffer| {
                (protected.x..protected.right())
                    .map(|x| buffer[(x, protected.y)].symbol())
                    .collect::<String>()
            };
            assert_eq!(placeholder(&active), placeholder(&plain));
            assert_eq!(placeholder(&later), placeholder(&plain));
            assert!(
                dots(&active)
                    .iter()
                    .chain(dots(&later).iter())
                    .all(|(x, y, _)| !protected.contains(Position::new(*x, *y))
                        && cursor != Some((*x, *y)))
            );
            if width == 80 {
                assert!(dots(&active).len() > 6);
                assert!(
                    dots(&active)
                        .iter()
                        .chain(dots(&later).iter())
                        .any(|(x, y, _)| *y == textarea.y && *x >= protected.right())
                );
            }
            let (typed, _) = {
                key(&mut pane, KeyCode::Char('x'));
                draw(&pane.composer, width, now)
            };
            assert_eq!(
                pane.composer.cursor_pos(active.area),
                cursor.map(|(x, y)| (x + 1, y))
            );
            assert!(!dots(&active).is_empty());
            assert!(dots(&typed).is_empty());
            snapshots.push(format!(
                "{width} columns, cursor {cursor:?}\n{}\nfirst input:\n{}",
                text(&active),
                text(&typed)
            ));
        }
        insta::assert_snapshot!(snapshots.join("\n\n"));
    });
}

#[test]
fn sparkle_fades_without_adding_dots_and_finishes_by_fifteen_seconds() {
    palette(|| {
        let now = Instant::now();
        let mut pane = pane();
        pane.mark_fresh_task_for_sparkle("astra", &enabled());
        draw(&pane.composer, /*width*/ 80, now);
        let (last, _) = draw(
            &pane.composer,
            /*width*/ 80,
            now + IDLE_TIMEOUT - IDLE_FADE,
        );
        let (fading, _) = draw(
            &pane.composer,
            /*width*/ 80,
            now + Duration::from_millis(/*millis*/ 14_500),
        );
        let (finished, _) = draw(&pane.composer, /*width*/ 80, now + IDLE_TIMEOUT);
        assert!(!dots(&fading).is_empty());
        assert!(dots(&fading).iter().all(|dot| dots(&last).contains(dot)));
        let contrast = |cell: &ratatui::buffer::Cell| match (cell.fg, cell.bg) {
            (Color::Rgb(fr, fg, fb), Color::Rgb(br, bg, bb)) => {
                u32::from(fr.abs_diff(br)) + u32::from(fg.abs_diff(bg)) + u32::from(fb.abs_diff(bb))
            }
            colors => panic!("expected true color: {colors:?}"),
        };
        for (x, y, _) in dots(&fading) {
            assert!(contrast(&fading[(x, y)]) < contrast(&last[(x, y)]));
        }
        assert!(dots(&finished).is_empty());
        pane.select_sparkle_model("astra", &enabled());
        assert!(
            dots(
                &draw(
                    &pane.composer,
                    /*width*/ 80,
                    now + IDLE_TIMEOUT + IDLE_FADE
                )
                .0
            )
            .is_empty()
        );
        insta::assert_snapshot!(format!(
            "before fade:\n{}\nfading:\n{}\nfinished:\n{}",
            text(&last),
            text(&fading),
            text(&finished)
        ));
    });
}

#[test]
fn the_first_buffered_character_and_empty_escape_clear_visible_stars() {
    palette(|| {
        let now = Instant::now();
        for input in [
            KeyCode::Char('x'),
            KeyCode::Char('?'),
            KeyCode::Esc,
            KeyCode::Left,
        ] {
            let mut pane = pane();
            pane.composer.set_disable_paste_burst(/*disabled*/ false);
            pane.mark_fresh_task_for_sparkle("astra", &enabled());
            assert!(!dots(&draw(&pane.composer, /*width*/ 80, now).0).is_empty());
            key(&mut pane, input);
            assert!(dots(&draw(&pane.composer, /*width*/ 80, now).0).is_empty());
            assert!(pane.composer.current_text().is_empty());
            assert_eq!(
                pane.composer.sparkle.draft.get(),
                if input == KeyCode::Char('x') {
                    SparkleDraft::Dismissed
                } else {
                    SparkleDraft::Untouched
                }
            );
        }
    });
}

#[test]
fn shortcut_help_preserves_an_unused_sparkle_but_literal_question_marks_do_not() {
    palette(|| {
        let now = Instant::now();
        let mut snapshots = Vec::new();
        for (scenario, input) in [
            (
                "unmodified",
                KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE),
            ),
            (
                "shifted",
                KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT),
            ),
            (
                "remapped",
                KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
            ),
            (
                "close",
                KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE),
            ),
        ] {
            let mut pane = pane();
            if scenario == "remapped" {
                pane.composer.toggle_shortcuts_keys =
                    vec![crate::key_hint::plain(KeyCode::Char('q'))];
            }
            pane.mark_fresh_task_for_sparkle("gpt-5.5", &enabled());
            pane.handle_key_event(input);
            if scenario == "close" {
                pane.handle_key_event(input);
            }
            assert_eq!(
                (
                    pane.composer.footer.mode,
                    pane.composer.current_text(),
                    pane.composer.sparkle.draft.get()
                ),
                (
                    if scenario == "close" {
                        super::super::FooterMode::ComposerEmpty
                    } else {
                        super::super::FooterMode::ShortcutOverlay
                    },
                    String::new(),
                    SparkleDraft::Untouched
                ),
                "{scenario}"
            );
            let help = draw(&pane.composer, /*width*/ 80, now).0;
            assert!(dots(&help).is_empty());
            type_text(&mut pane, "/model");
            assert!(matches!(
                key(&mut pane, KeyCode::Enter),
                InputResult::Command(SlashCommand::Model)
            ));
            pane.select_sparkle_model("gpt-6-astra", &enabled());
            let selected = draw(&pane.composer, /*width*/ 80, now).0;
            assert!(!dots(&selected).is_empty(), "{scenario}");
            if scenario == "unmodified" {
                snapshots.push(format!(
                    "shortcut help:\n{}\n\nafter /model selects Astra:\n{}",
                    text(&help),
                    text(&selected)
                ));
            }
        }

        for scenario in ["paste", "remapped literal", "quit reminder"] {
            let mut pane = pane();
            pane.mark_fresh_task_for_sparkle("gpt-5.5", &enabled());
            if scenario == "paste" {
                pane.handle_paste("?".into());
            } else {
                if scenario == "remapped literal" {
                    pane.composer.toggle_shortcuts_keys =
                        vec![crate::key_hint::plain(KeyCode::Char('q'))];
                } else {
                    pane.composer.show_quit_shortcut_hint(
                        crate::key_hint::ctrl(KeyCode::Char('c')),
                        /*has_focus*/ true,
                    );
                }
                key(&mut pane, KeyCode::Char('?'));
            }
            assert_eq!(
                (
                    pane.composer.current_text(),
                    pane.composer.sparkle.draft.get()
                ),
                ("?".into(), SparkleDraft::Dismissed),
                "{scenario}"
            );
            let literal = draw(&pane.composer, /*width*/ 80, now).0;
            key(&mut pane, KeyCode::Backspace);
            type_text(&mut pane, "/model");
            assert!(matches!(
                key(&mut pane, KeyCode::Enter),
                InputResult::Command(SlashCommand::Model)
            ));
            pane.select_sparkle_model("gpt-6-astra", &enabled());
            let selected = draw(&pane.composer, /*width*/ 80, now).0;
            assert!(dots(&selected).is_empty(), "{scenario}");
            if scenario == "paste" {
                snapshots.push(format!(
                    "literal pasted question mark:\n{}\n\nafter clearing and selecting Astra:\n{}",
                    text(&literal),
                    text(&selected)
                ));
            }
        }
        insta::assert_snapshot!(snapshots.join("\n\n"));
    });
}

#[test]
fn shortcut_help_hides_visible_sparkles_without_restarting_their_deadline() {
    palette(|| {
        let started = Instant::now();
        let during = started + Duration::from_secs(/*secs*/ 4);
        let expired = started + IDLE_TIMEOUT;
        let mut snapshots = Vec::new();
        for (scenario, input) in [
            (
                "unmodified",
                KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE),
            ),
            (
                "shifted",
                KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT),
            ),
            (
                "remapped",
                KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
            ),
        ] {
            for close_at in [during, expired] {
                let mut pane = pane();
                if scenario == "remapped" {
                    pane.composer.toggle_shortcuts_keys =
                        vec![crate::key_hint::plain(KeyCode::Char('q'))];
                }
                pane.mark_fresh_task_for_sparkle("gpt-6-astra", &enabled());
                let active = draw(&pane.composer, /*width*/ 80, started).0;
                let expected = draw(&pane.composer, /*width*/ 80, during).0;
                assert!(!dots(&active).is_empty());
                assert!(!dots(&expected).is_empty());
                pane.handle_key_event(input);
                assert!(
                    matches!(pane.composer.sparkle.phase.get(), Phase::Visible(at) if at == started)
                );
                assert_eq!(pane.composer.current_text(), "");
                let help = draw(&pane.composer, /*width*/ 80, during).0;
                assert!(dots(&help).is_empty(), "{scenario}");
                if close_at == expired {
                    let still_open = draw(&pane.composer, /*width*/ 80, expired).0;
                    assert!(dots(&still_open).is_empty());
                    assert!(pane.composer.sparkle.phase.get() == Phase::Finished);
                }
                pane.handle_key_event(input);
                let closed = draw(&pane.composer, /*width*/ 80, close_at).0;
                assert_eq!(
                    dots(&closed),
                    if close_at == expired {
                        Vec::new()
                    } else {
                        dots(&expected)
                    },
                    "{scenario}"
                );
                if scenario == "unmodified" {
                    snapshots.push(format!(
                        "expires during help: {}\ninitial:\n{}\nshortcut help:\n{}\nafter closing:\n{}",
                        close_at == expired,
                        text(&active),
                        text(&help),
                        text(&closed)
                    ));
                }
            }
        }

        let mut waiting = pane();
        waiting.mark_fresh_task_for_sparkle("gpt-6-astra", &enabled());
        key(&mut waiting, KeyCode::Char('?'));
        let help = draw(&waiting.composer, /*width*/ 80, expired).0;
        assert!(dots(&help).is_empty());
        assert!(waiting.composer.sparkle.phase.get() == Phase::Waiting);
        key(&mut waiting, KeyCode::Char('?'));
        assert!(!dots(&draw(&waiting.composer, /*width*/ 80, expired).0).is_empty());

        let mut literal = pane();
        literal.mark_fresh_task_for_sparkle("gpt-6-astra", &enabled());
        assert!(!dots(&draw(&literal.composer, /*width*/ 80, started).0).is_empty());
        literal.handle_paste("?".into());
        assert_eq!(literal.composer.current_text(), "?");
        key(&mut literal, KeyCode::Backspace);
        assert!(dots(&draw(&literal.composer, /*width*/ 80, during).0).is_empty());
        insta::assert_snapshot!(snapshots.join("\n\n"));
    });
}

#[test]
fn disconnected_sparkle_edits_are_tracked_between_renders() {
    palette(|| {
        let now = Instant::now();
        let mut snapshots = Vec::new();
        for model in ["gpt-6-astra", "gpt-5.5"] {
            for (scenario, input, draft) in [
                ("ordinary", "x", SparkleDraft::Dismissed),
                ("offline question mark", "?", SparkleDraft::Dismissed),
                ("recognized command", "/status", SparkleDraft::Command),
                ("unknown command", "/xyz", SparkleDraft::Dismissed),
            ] {
                let mut pane = pane();
                pane.mark_fresh_task_for_sparkle(model, &enabled());
                let initial = draw(&pane.composer, /*width*/ 80, now).0;
                assert_eq!(dots(&initial).is_empty(), model == "gpt-5.5");
                for key in [KeyCode::Null, KeyCode::Enter, KeyCode::Tab] {
                    pane.handle_restricted_key(
                        KeyEvent::new(key, KeyModifiers::NONE),
                        RestrictedInputMode::Disconnected,
                    );
                }
                pane.handle_restricted_key(
                    KeyEvent::new_with_kind(
                        KeyCode::Char('x'),
                        KeyModifiers::NONE,
                        crossterm::event::KeyEventKind::Release,
                    ),
                    RestrictedInputMode::Disconnected,
                );
                assert_eq!(pane.composer.sparkle.draft.get(), SparkleDraft::Untouched);
                assert_eq!(
                    dots(&draw(&pane.composer, /*width*/ 80, now).0),
                    dots(&initial)
                );
                for ch in input.chars() {
                    pane.handle_restricted_key(
                        KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE),
                        RestrictedInputMode::Disconnected,
                    );
                }
                assert_eq!(pane.composer.current_text(), input, "{scenario}");
                assert_eq!(pane.composer.sparkle.draft.get(), draft, "{scenario}");
                for _ in input.chars() {
                    pane.handle_restricted_key(
                        KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
                        RestrictedInputMode::Disconnected,
                    );
                }
                assert_eq!(pane.composer.current_text(), "");
                pane.select_sparkle_model("gpt-6-astra", &enabled());
                let after = draw(&pane.composer, /*width*/ 80, now).0;
                assert_eq!(
                    dots(&after).is_empty(),
                    draft == SparkleDraft::Dismissed,
                    "{scenario}"
                );
                if scenario == "ordinary" {
                    snapshots.push(format!(
                        "{model}\nbefore disconnect:\n{}\nafter typing and erasing without rendering:\n{}",
                        text(&initial),
                        text(&after)
                    ));
                }
            }
        }
        insta::assert_snapshot!(snapshots.join("\n\n"));
    });
}

#[test]
fn a_fresh_astra_response_waits_for_the_pending_command_to_leave() {
    palette(|| {
        let now = Instant::now();
        for completion in ["submit", "erase", "draft"] {
            let mut pane = pane();
            type_text(&mut pane, "/status");
            pane.mark_fresh_task_for_sparkle("astra", &enabled());
            let pending = draw(&pane.composer, /*width*/ 80, now).0;
            assert!(dots(&pending).is_empty());

            match completion {
                "submit" => assert!(matches!(
                    key(&mut pane, KeyCode::Enter),
                    InputResult::Command(SlashCommand::Status)
                )),
                "erase" => {
                    for _ in "/status".chars() {
                        key(&mut pane, KeyCode::Backspace);
                    }
                }
                "draft" => {
                    type_text(&mut pane, "-draft");
                    pane.composer
                        .set_text_content(String::new(), Vec::new(), Vec::new());
                }
                _ => unreachable!(),
            }
            let completed = draw(&pane.composer, /*width*/ 80, now).0;
            assert_eq!(
                dots(&completed).is_empty(),
                completion == "draft",
                "{completion}"
            );
            if completion == "submit" {
                insta::assert_snapshot!(format!(
                    "command during startup:\n{}\n\nafter command:\n{}",
                    text(&pending),
                    text(&completed)
                ));
            }
        }
    });
}

#[test]
fn an_attachment_ends_the_sparkle_even_after_the_composer_is_cleared() {
    palette(|| {
        let now = Instant::now();
        let mut pane = pane();
        pane.mark_fresh_task_for_sparkle("astra", &enabled());
        assert!(!dots(&draw(&pane.composer, /*width*/ 80, now).0).is_empty());
        pane.attach_image(crate::test_support::test_path_buf("/tmp/image.png"));
        assert!(!pane.composer.is_empty());
        assert!(dots(&draw(&pane.composer, /*width*/ 80, now).0).is_empty());
        pane.on_ctrl_c();
        assert!(pane.composer.is_empty());
        pane.select_sparkle_model("astra", &enabled());
        assert!(dots(&draw(&pane.composer, /*width*/ 80, now).0).is_empty());
    });
}

#[test]
fn history_search_acceptance_and_cancellation_update_sparkles_before_rendering() {
    palette(|| {
        let now = Instant::now();
        let mut snapshots = Vec::new();
        for (entry, query) in [("git status", "git"), ("/status", "status")] {
            for completion in ["accept", "escape", "control-c"] {
                let mut pane = pane();
                pane.mark_fresh_task_for_sparkle("gpt-5.5", &enabled());
                pane.composer
                    .history
                    .record_local_submission(HistoryEntry::new(entry.into()));
                pane.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
                type_text(&mut pane, query);
                assert!(pane.composer.history_search_active());
                assert_eq!(pane.composer.current_text(), entry);
                assert_eq!(pane.composer.sparkle.draft.get(), SparkleDraft::Untouched);

                match completion {
                    "accept" => {
                        key(&mut pane, KeyCode::Enter);
                        assert_eq!(pane.composer.sparkle.draft.get(), SparkleDraft::Dismissed);
                        pane.on_ctrl_c();
                    }
                    "escape" => {
                        key(&mut pane, KeyCode::Esc);
                    }
                    "control-c" => {
                        pane.on_ctrl_c();
                    }
                    _ => unreachable!(),
                }
                assert!(!pane.composer.history_search_active());
                assert!(pane.composer.is_empty());
                pane.select_sparkle_model("astra", &enabled());
                let buffer = draw(&pane.composer, /*width*/ 80, now).0;
                assert_eq!(dots(&buffer).is_empty(), completion == "accept");
                if entry == "git status" && completion != "control-c" {
                    snapshots.push(format!("{completion}:\n{}", text(&buffer)));
                }
            }
        }
        insta::assert_snapshot!(snapshots.join("\n\n"));
    });
}

#[test]
fn canceled_history_search_restores_visible_stars_only_before_the_original_deadline() {
    palette(|| {
        let started = Instant::now();
        let during = started + Duration::from_secs(/*secs*/ 4);
        let expired = started + IDLE_TIMEOUT;
        let mut snapshots = Vec::new();
        for (cancel, expires_while_open) in [
            ("escape", false),
            ("control-c", false),
            ("escape", true),
            ("accept", false),
        ] {
            let mut pane = pane();
            pane.mark_fresh_task_for_sparkle("gpt-6-astra", &enabled());
            pane.composer
                .history
                .record_local_submission(HistoryEntry::new("stored draft".into()));
            let active = draw(&pane.composer, /*width*/ 80, started).0;
            let expected = draw(&pane.composer, /*width*/ 80, during).0;
            assert!(!dots(&active).is_empty());
            pane.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
            type_text(&mut pane, "st");
            pane.handle_paste("ored".into());
            assert_eq!(pane.composer.current_text(), "stored draft");
            assert!(pane.composer.sparkle.phase.get() == Phase::Visible(started));
            let searching = draw(&pane.composer, /*width*/ 80, during).0;
            assert!(dots(&searching).is_empty());
            if expires_while_open {
                assert!(dots(&draw(&pane.composer, /*width*/ 80, expired).0).is_empty());
            }

            if cancel == "escape" {
                key(&mut pane, KeyCode::Esc);
            } else if cancel == "accept" {
                key(&mut pane, KeyCode::Enter);
            } else {
                pane.on_ctrl_c();
            }
            assert_eq!(pane.composer.is_empty(), cancel != "accept");
            let at = if expires_while_open { expired } else { during };
            let restored = draw(&pane.composer, /*width*/ 80, at).0;
            assert_eq!(
                dots(&restored),
                if expires_while_open || cancel == "accept" {
                    Vec::new()
                } else {
                    dots(&expected)
                },
                "{cancel}, expires while open: {expires_while_open}",
            );
            if cancel == "escape" {
                snapshots.push(format!(
                    "expires while open: {expires_while_open}\ninitial:\n{}\nsearching:\n{}\nafter escape:\n{}",
                    text(&active),
                    text(&searching),
                    text(&restored),
                ));
            }
        }
        insta::assert_snapshot!(snapshots.join("\n\n"));
    });
}

#[test]
fn history_search_preserves_a_held_sparkle_command_without_reclassifying_it() {
    palette(|| {
        let now = Instant::now();
        let during = now + Duration::from_secs(/*secs*/ 4);
        let mut snapshots = Vec::new();
        for model in ["gpt-6-astra", "gpt-5.5"] {
            for cancel in ["escape", "control-c"] {
                let mut pane = pane();
                pane.composer.set_disable_paste_burst(/*disabled*/ false);
                pane.mark_fresh_task_for_sparkle(model, &enabled());
                pane.composer
                    .history
                    .record_local_submission(HistoryEntry::new("history prompt".into()));
                let initial = draw(&pane.composer, /*width*/ 80, now).0;
                let expected = draw(&pane.composer, /*width*/ 80, during).0;
                assert_eq!(dots(&initial).is_empty(), model == "gpt-5.5");
                key(&mut pane, KeyCode::Char('/'));
                assert!(pane.composer.is_in_paste_burst());
                assert_eq!(pane.composer.current_text(), "");
                assert_eq!(pane.composer.sparkle.draft.get(), SparkleDraft::Command);
                pane.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
                assert!(pane.composer.history_search_active());
                assert_eq!(pane.composer.current_text(), "/");
                assert_eq!(pane.composer.sparkle.draft.get(), SparkleDraft::Command);
                pane.handle_paste("history".into());
                assert_eq!(pane.composer.current_text(), "history prompt");
                assert_eq!(pane.composer.sparkle.draft.get(), SparkleDraft::Command);
                let searching = draw(&pane.composer, /*width*/ 80, during).0;
                assert!(dots(&searching).is_empty());

                if cancel == "escape" {
                    key(&mut pane, KeyCode::Esc);
                } else {
                    pane.on_ctrl_c();
                }
                assert_eq!(pane.composer.current_text(), "/");
                key(&mut pane, KeyCode::Backspace);
                assert_eq!(pane.composer.sparkle.draft.get(), SparkleDraft::Untouched);
                let erased = draw(&pane.composer, /*width*/ 80, during).0;
                assert_eq!(dots(&erased), dots(&expected));
                type_text(&mut pane, "/model");
                pane.composer
                    .handle_paste_burst_flush(now + Duration::from_secs(/*secs*/ 1));
                assert!(matches!(
                    key(&mut pane, KeyCode::Enter),
                    InputResult::Command(SlashCommand::Model)
                ));
                pane.select_sparkle_model("gpt-6-astra", &enabled());
                let selected = draw(&pane.composer, /*width*/ 80, during).0;
                assert!(!dots(&selected).is_empty());
                if cancel == "escape" {
                    snapshots.push(format!("{model}\nsearching with an explicit pasted query:\n{}\nafter canceling and erasing the slash:\n{}\nafter /model selects Astra:\n{}", text(&searching), text(&erased), text(&selected)));
                }
            }
        }
        insta::assert_snapshot!(snapshots.join("\n\n"));
    });
}

#[test]
fn cancelling_history_search_preserves_a_typed_command_despite_attachment_previews() {
    palette(|| {
        let now = Instant::now();
        let mut pane = pane();
        pane.mark_fresh_task_for_sparkle("gpt-5.5", &enabled());
        type_text(&mut pane, "/status");
        let mut entry = HistoryEntry::new("history attachment".into());
        entry.local_image_paths = vec![crate::test_support::test_path_buf("/tmp/image.png")];
        entry.remote_image_urls = vec!["https://example.com/image.png".into()];
        pane.composer.history.record_local_submission(entry);
        pane.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
        type_text(&mut pane, "history");
        assert!(!pane.composer.local_image_paths().is_empty());
        assert!(!pane.composer.remote_image_urls().is_empty());
        assert_eq!(pane.composer.sparkle.draft.get(), SparkleDraft::Command);
        key(&mut pane, KeyCode::Esc);
        assert_eq!(pane.composer.current_text(), "/status");
        assert!(pane.composer.local_image_paths().is_empty());
        assert!(pane.composer.remote_image_urls().is_empty());
        assert!(matches!(
            key(&mut pane, KeyCode::Enter),
            InputResult::Command(SlashCommand::Status)
        ));
        pane.select_sparkle_model("astra", &enabled());
        assert!(!dots(&draw(&pane.composer, /*width*/ 80, now).0).is_empty());
    });
}

#[test]
fn replacing_a_draft_outside_the_history_preview_dismisses_sparkles_during_search() {
    let mut pane = pane();
    pane.mark_fresh_task_for_sparkle("gpt-5.5", &enabled());
    pane.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
    assert!(pane.composer.history_search_active());
    pane.composer
        .set_text_content("external draft".into(), Vec::new(), Vec::new());
    assert_eq!(pane.composer.sparkle.draft.get(), SparkleDraft::Dismissed);
    key(&mut pane, KeyCode::Esc);
    assert!(pane.composer.is_empty());
    assert_eq!(pane.composer.sparkle.draft.get(), SparkleDraft::Dismissed);
}

#[test]
fn arbitrary_commands_preserve_a_never_used_fresh_opportunity() {
    palette(|| {
        let now = Instant::now();
        for completion in [false, true] {
            let mut pane = pane();
            pane.mark_fresh_task_for_sparkle("gpt-5.5", &enabled());
            assert!(dots(&draw(&pane.composer, /*width*/ 80, now).0).is_empty());
            for command in ["/status", "/diff", "/pwd"] {
                type_text(&mut pane, command);
                assert!(matches!(
                    key(&mut pane, KeyCode::Enter),
                    InputResult::Command(_)
                ));
                assert_eq!(pane.composer.sparkle.draft.get(), SparkleDraft::Untouched);
            }
            type_text(&mut pane, if completion { "/mo" } else { "/model" });
            if completion {
                key(&mut pane, KeyCode::Tab);
            }
            assert!(matches!(
                key(&mut pane, KeyCode::Enter),
                InputResult::Command(SlashCommand::Model)
            ));
            assert_eq!(pane.composer.sparkle.draft.get(), SparkleDraft::Untouched);
            pane.select_sparkle_model("gpt-6-astra", &enabled());
            assert!(!dots(&draw(&pane.composer, /*width*/ 80, now).0).is_empty());
        }
        for input in ["hello", "/model extra", "/other", "漢", "/tmp/file"] {
            let mut pane = pane();
            pane.mark_fresh_task_for_sparkle("gpt-5.5", &enabled());
            type_text(&mut pane, input);
            pane.composer
                .set_text_content(String::new(), Vec::new(), Vec::new());
            pane.select_sparkle_model("gpt-6-astra", &enabled());
            assert_eq!(
                pane.composer.sparkle.draft.get(),
                SparkleDraft::Dismissed,
                "{input}"
            );
            assert!(
                dots(&draw(&pane.composer, /*width*/ 80, now).0).is_empty(),
                "{input}"
            );
        }
    });
}

#[test]
fn canceled_commands_remain_eligible_but_inserted_content_and_ordinary_paste_do_not() {
    palette(|| {
        let now = Instant::now();
        let mut provisional = pane();
        type_text(&mut provisional, "x");
        key(&mut provisional, KeyCode::Backspace);
        let snapshot = provisional.composer.draft_snapshot();
        assert!(snapshot.text.is_empty());
        let mut initialized = pane();
        initialized.mark_fresh_task_for_sparkle("astra", &enabled());
        initialized.inherit_startup_sparkle(snapshot.sparkle_draft);
        assert!(dots(&draw(&initialized.composer, /*width*/ 80, now).0).is_empty());

        let mut canceled = pane();
        canceled.mark_fresh_task_for_sparkle("gpt-5.5", &enabled());
        type_text(&mut canceled, "/sta");
        for _ in 0..4 {
            key(&mut canceled, KeyCode::Backspace);
        }
        assert_eq!(
            canceled.composer.sparkle.draft.get(),
            SparkleDraft::Untouched
        );
        canceled.select_sparkle_model("astra", &enabled());
        assert!(!dots(&draw(&canceled.composer, /*width*/ 80, now).0).is_empty());

        for input in ["paste draft", "inserted"] {
            let mut pane = pane();
            pane.mark_fresh_task_for_sparkle("gpt-5.5", &enabled());
            if input == "inserted" {
                type_text(&mut pane, "/mention");
                assert!(matches!(
                    key(&mut pane, KeyCode::Enter),
                    InputResult::Command(SlashCommand::Mention)
                ));
                pane.composer.insert_str("@");
            } else {
                pane.handle_paste(input.to_string());
            }
            pane.composer
                .set_text_content(String::new(), Vec::new(), Vec::new());
            pane.select_sparkle_model("astra", &enabled());
            assert_eq!(pane.composer.sparkle.draft.get(), SparkleDraft::Dismissed);
            assert!(dots(&draw(&pane.composer, /*width*/ 80, now).0).is_empty());
        }
    });
}

#[test]
fn review_regression_canceling_a_held_slash_restores_sparkle_command_tracking() {
    palette(|| {
        let now = Instant::now();
        let mut snapshots = Vec::new();
        for model in ["gpt-6-astra", "gpt-5.5"] {
            let mut pane = pane();
            pane.composer.set_disable_paste_burst(/*disabled*/ false);
            pane.mark_fresh_task_for_sparkle(model, &enabled());
            let initial = draw(&pane.composer, /*width*/ 80, now).0;
            key(&mut pane, KeyCode::Char('/'));
            assert!(pane.composer.is_in_paste_burst());
            assert_eq!(pane.composer.current_text(), "");
            assert_eq!(pane.composer.sparkle.draft.get(), SparkleDraft::Command);
            key(&mut pane, KeyCode::Backspace);
            assert!(!pane.composer.is_in_paste_burst());
            assert_eq!(pane.composer.current_text(), "");
            assert_eq!(pane.composer.sparkle.draft.get(), SparkleDraft::Untouched);
            let canceled = draw(&pane.composer, /*width*/ 80, now).0;
            assert_eq!(dots(&canceled), dots(&initial));

            type_text(&mut pane, "/model");
            pane.composer
                .handle_paste_burst_flush(now + Duration::from_secs(/*secs*/ 1));
            assert!(matches!(
                key(&mut pane, KeyCode::Enter),
                InputResult::Command(SlashCommand::Model)
            ));
            pane.select_sparkle_model("gpt-6-astra", &enabled());
            let selected = draw(&pane.composer, /*width*/ 80, now).0;
            assert!(!dots(&selected).is_empty());
            snapshots.push(format!(
                "{model}\nafter immediately canceling the held slash:\n{}\nafter /model:\n{}",
                text(&canceled),
                text(&selected)
            ));
        }
        insta::assert_snapshot!(snapshots.join("\n\n"));
    });
}

#[test]
fn buffered_commands_and_read_only_inline_arguments_preserve_eligibility() {
    palette(|| {
        let now = Instant::now();
        let mut buffered = pane();
        buffered
            .composer
            .set_disable_paste_burst(/*disabled*/ false);
        buffered.mark_fresh_task_for_sparkle("gpt-5.5", &enabled());
        type_text(&mut buffered, "/status");
        buffered
            .composer
            .handle_paste_burst_flush(now + Duration::from_secs(/*secs*/ 1));
        assert!(matches!(
            key(&mut buffered, KeyCode::Enter),
            InputResult::Command(SlashCommand::Status)
        ));
        assert_eq!(
            buffered.composer.sparkle.draft.get(),
            SparkleDraft::Untouched
        );

        let mut inline = pane();
        inline.mark_fresh_task_for_sparkle("gpt-5.5", &enabled());
        type_text(&mut inline, "/rename task name");
        assert_eq!(inline.composer.sparkle.draft.get(), SparkleDraft::Command);
        assert!(matches!(
            key(&mut inline, KeyCode::Enter),
            InputResult::CommandWithArgs(SlashCommand::Rename, _, _)
        ));
        inline
            .composer
            .set_text_content(String::new(), Vec::new(), Vec::new());
        inline.select_sparkle_model("astra", &enabled());
        assert!(!dots(&draw(&inline.composer, /*width*/ 80, now).0).is_empty());
    });
}

#[test]
fn commands_hide_a_started_sparkle_without_restarting_its_deadline() {
    palette(|| {
        let now = Instant::now();
        let mut pane = pane();
        pane.mark_fresh_task_for_sparkle("astra", &enabled());
        assert!(!dots(&draw(&pane.composer, /*width*/ 80, now).0).is_empty());
        type_text(&mut pane, "/status");
        assert!(
            dots(
                &draw(
                    &pane.composer,
                    /*width*/ 80,
                    now + Duration::from_secs(/*secs*/ 5)
                )
                .0
            )
            .is_empty()
        );
        assert!(matches!(
            key(&mut pane, KeyCode::Enter),
            InputResult::Command(SlashCommand::Status)
        ));
        assert!(
            !dots(
                &draw(
                    &pane.composer,
                    /*width*/ 80,
                    now + Duration::from_secs(/*secs*/ 6)
                )
                .0
            )
            .is_empty()
        );
        type_text(&mut pane, "/diff");
        assert!(dots(&draw(&pane.composer, /*width*/ 80, now + IDLE_TIMEOUT).0).is_empty());
        assert!(matches!(
            key(&mut pane, KeyCode::Enter),
            InputResult::Command(SlashCommand::Diff)
        ));
        assert!(
            dots(
                &draw(
                    &pane.composer,
                    /*width*/ 80,
                    now + IDLE_TIMEOUT + IDLE_FADE
                )
                .0
            )
            .is_empty()
        );
    });
}

#[test]
fn focus_startup_work_and_configuration_do_not_extend_a_started_deadline() {
    palette(|| {
        let now = Instant::now();
        let mut pane = pane();
        pane.mark_fresh_task_for_sparkle("astra", &enabled());
        pane.composer.set_task_running(/*running*/ true);
        assert!(dots(&draw(&pane.composer, /*width*/ 80, now).0).is_empty());
        pane.composer.set_task_running(/*running*/ false);
        assert!(!dots(&draw(&pane.composer, /*width*/ 80, now + IDLE_TIMEOUT).0).is_empty());
        pane.set_sparkle_terminal_focus(/*focused*/ false);
        assert!(
            dots(
                &draw(
                    &pane.composer,
                    /*width*/ 80,
                    now + IDLE_TIMEOUT + Duration::from_secs(/*secs*/ 1)
                )
                .0
            )
            .is_empty()
        );
        pane.set_sparkle_terminal_focus(/*focused*/ true);
        assert!(dots(&draw(&pane.composer, /*width*/ 80, now + IDLE_TIMEOUT * 2).0).is_empty());

        for settings in [
            Tui {
                effects: codex_config::types::TuiEffects {
                    starfield: false,
                    ..Default::default()
                },
                ..enabled()
            },
            Tui {
                animations: false,
                ..enabled()
            },
        ] {
            let mut pane = super::tests::pane();
            pane.mark_fresh_task_for_sparkle("astra", &settings);
            assert!(dots(&draw(&pane.composer, /*width*/ 80, now).0).is_empty());
        }
        let mut narrow = super::tests::pane();
        narrow.mark_fresh_task_for_sparkle("astra", &enabled());
        let (buffer, textarea) = draw(&narrow.composer, /*width*/ 25, now);
        assert!(
            dots(&buffer)
                .iter()
                .all(|(x, y, _)| !textarea.contains(Position::new(*x, *y)))
        );
    });
}
