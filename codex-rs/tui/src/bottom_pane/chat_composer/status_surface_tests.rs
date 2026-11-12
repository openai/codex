//! Persistent status survives transient footer modes and suggestion overlays.

use super::*;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::osc8_hyperlink;
use crate::terminal_hyperlinks::strip_osc8;
use pretty_assertions::assert_eq;
use tokio::sync::mpsc::unbounded_channel;

fn composer() -> ChatComposer {
    let (tx, _rx) = unbounded_channel();
    let mut composer = ChatComposer::new(
        /*has_input_focus*/ true,
        AppEventSender::new(tx),
        /*enhanced_keys_supported*/ false,
        "Ask Codex".to_string(),
        /*disable_paste_burst*/ true,
    );
    composer.set_status_line_enabled(/*enabled*/ true);
    composer.set_status_line(Some(
        Line::from("MODEL · ~/project · Context 20% used").into(),
    ));
    composer
}

#[test]
fn extension_status_line_clips_links_to_footer_layout() {
    let mut composer = composer();
    let tree: crate::ui_tree::UiTree = r#"{
        "type": "row", "props": {"gap": 1}, "children": [
            "plain",
            { "type": "row", "props": {"gap": 1}, "children": [
                "·",
                { "type": "link", "props": { "href": "https://example.com/pr/42" }, "children": [
                    "PR #42"
                ] }
            ] }
        ]
    }"#
    .parse()
    .unwrap();
    composer.set_status_line(Some(StatusLine::from(tree)));

    for separate_status_line in [false, true] {
        for width in [12, 40] {
            let options = composer.resolve_render_options(ComposerRenderOptions {
                separate_status_line,
                ..ComposerRenderOptions::default()
            });
            let area = Rect::new(
                /*x*/ 0,
                /*y*/ 0,
                width,
                composer.desired_height_with_options(width, options),
            );
            let mut buffer = Buffer::empty(area);
            composer.render_with_options(area, &mut buffer, /*mask_char*/ None, options);
            let linked = buffer
                .content
                .iter()
                .filter(|cell| cell.symbol().contains("https://example.com/pr/42"))
                .collect::<Vec<_>>();
            let linked_text: String = linked
                .iter()
                .map(|cell| strip_osc8(cell.symbol()))
                .collect();
            assert_eq!(
                linked_text,
                if width == 12 { "PR#" } else { "PR #42" },
                "{buffer:?}"
            );
            assert!(linked.iter().all(|cell| !cell.symbol().contains('…')));
            assert!(buffer.content.iter().any(|cell| cell.symbol() == "p"));
        }
    }

    composer.set_active_agent_label(Some("bot".to_string()));
    let (output, _) = render(&composer, /*width*/ 40, /*footer*/ None);
    assert!(output.contains("plain"), "{output}");
    assert!(!output.contains("bot"), "{output}");

    composer.set_status_line(/*status_line*/ None);
    let agent_rows = [false, true].map(|unfocused| {
        let options = ComposerRenderOptions {
            separate_status_line: true,
            unfocused,
            ..Default::default()
        };
        render_status(&composer, Rect::new(0, 0, 40, 8), options).1
    });
    assert_eq!(agent_rows, [vec!["bot".to_string()], vec![]]);
}

fn render_status(
    composer: &ChatComposer,
    area: Rect,
    options: ComposerRenderOptions<'_>,
) -> (Buffer, Vec<String>) {
    let layout = composer.layout_with_options(area, options);
    let status = layout.status;
    let mut buf = Buffer::empty(area);
    composer.render_with_options(area, &mut buf, /*mask_char*/ None, options);
    let rows = (status.y..status.bottom())
        .map(|y| {
            (status.x..status.right())
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
                .trim()
                .to_owned()
        })
        .collect();
    (buf, rows)
}

fn frame(buffer: &Buffer) -> String {
    buffer
        .content
        .chunks(usize::from(buffer.area.width))
        .map(|row| {
            let row = row
                .iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>();
            format!("|{}|", strip_osc8(&row))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn extension_uses_the_full_width_after_resize() {
    let mut composer = composer();
    composer.footer.show_warnings_key = None;
    let tree: crate::ui_tree::UiTree =
        r#"{"type":"row","props":{"wrap":"wrap","gap":1},"children":[
        "Ready", {"type":"row","props":{"gap":1},"children":["PR",
        {"type":"link","props":{"href":"https://example.com/pr/12345"},"children":["12345"]}]}]}"#
            .parse()
            .unwrap();
    composer.set_status_line(Some(StatusLine::from(tree)));
    composer.set_collaboration_modes_enabled(/*enabled*/ true);
    let linked_number: String = "12345"
        .chars()
        .map(|digit| osc8_hyperlink("https://example.com/pr/12345", &digit.to_string()))
        .collect();
    for fullscreen in [false, true] {
        for (width, mode, expected_rows) in [
            (16, None, 1),
            (15, None, if fullscreen { 2 } else { 1 }),
            (16, None, 1),
            (27, Some(CollaborationModeIndicator::Plan), 1),
            (19, Some(CollaborationModeIndicator::Plan), 1),
        ] {
            composer.set_collaboration_mode_indicator(mode);
            let options = ComposerRenderOptions {
                separate_status_line: fullscreen,
                warning_count: 1,
                ..Default::default()
            };
            let height = composer.desired_height_with_options(width, options);
            let (buffer, rows) = render_status(&composer, Rect::new(0, 0, width, height), options);
            assert_eq!(rows.len(), expected_rows);
            for row in &rows {
                assert_eq!(*row, strip_osc8(row).replace("12345", &linked_number));
            }
            let warning = composer.footer.warning_notice_area.get().unwrap();
            assert_eq!(buffer[(warning.x, warning.y)].symbol(), "⚠");
            assert_eq!((warning.right(), warning.y), (width - 1, height - 1));
        }
    }
}

#[test]
fn provider_status_and_hints_use_separate_rows() {
    let mut composer = composer();
    let tree: crate::ui_tree::UiTree = r#"{"type":"row","props":{"wrap":"wrap","gap":2},"children":[{"type":"bold","children":["codex-internal"]},"feat/statusline","Build 1042","128 tests passed","Deploy ready"]}"#.parse().unwrap();
    composer.set_status_line(Some(tree.into()));
    composer.set_collaboration_modes_enabled(/*enabled*/ true);
    composer.set_collaboration_mode_indicator(Some(CollaborationModeIndicator::Plan));
    let options = ComposerRenderOptions {
        warning_count: 2,
        ..Default::default()
    };
    let frames = [22, 4].map(|height| {
        let area = Rect::new(0, 0, 84, height);
        let (buffer, _) = render_status(&composer, area, options);
        let rect = composer.footer.warning_notice_area.get().unwrap();
        assert_eq!(buffer[(rect.x, rect.y)].symbol(), "⚠");
        let layout = composer.layout_with_options(area, options);
        (layout.status.y..layout.footer.bottom())
            .map(|y| {
                (0..area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n")
    });
    insta::assert_snapshot!(frames.join("\n---\n"));
}

#[test]
fn status_rows_respect_host_budget_and_yield_to_the_draft() {
    let mut composer = composer();
    let tree: crate::ui_tree::UiTree = r#""one\ntwo\nthree\nfour\nfive""#.parse().unwrap();
    composer.set_status_line(Some(StatusLine::from(tree)));
    composer.set_text_content("a\nb\nc".into(), Vec::new(), Vec::new());
    composer.move_cursor_to_end();
    for separate_status_line in [false, true] {
        for (height, fullscreen) in [
            (7, "one\ntwo…"),
            (9, "one\ntwo…"),
            (10, "one\ntwo\nthre…"),
            (13, "one\ntwo\nthre…"),
            (14, "one\ntwo\nthree\nfour…"),
        ] {
            let options = ComposerRenderOptions {
                separate_status_line,
                max_height: Some(height),
                ..Default::default()
            };
            let expected = if separate_status_line {
                fullscreen
            } else {
                "one\ntwo…"
            };
            let area = Rect::new(/*x*/ 0, /*y*/ 0, /*width*/ 7, height);
            let (buffer, rows) = render_status(&composer, area, options);
            assert_eq!(rows.join("\n"), expected);
            let input = composer.layout_with_options(area, options).textarea;
            assert!(input.height >= 3);
            let draft = [0, 1, 2].map(|dy| buffer[(input.x, input.y + dy)].symbol());
            assert_eq!(draft, ["a", "b", "c"]);
            assert_eq!(
                composer.cursor_pos_with_options(area, options),
                Some((input.x + 1, input.y + 2))
            );
            if height == 7 && separate_status_line {
                insta::assert_snapshot!(
                    "status_rows_respect_host_budget_and_yield_to_the_draft",
                    frame(&buffer)
                );
            }
        }
    }
    composer.set_collaboration_modes_enabled(/*enabled*/ true);
    composer.set_collaboration_mode_indicator(Some(CollaborationModeIndicator::Plan));
    assert_eq!(
        render_status(&composer, Rect::new(0, 0, 48, 12), Default::default()).1,
        ["one…"]
    );
    composer.set_collaboration_mode_indicator(/*indicator*/ None);
    let options = ComposerRenderOptions {
        separate_status_line: true,
        max_height: Some(14),
        ..Default::default()
    };
    let area = Rect::new(0, 0, 7, 14);
    assert_eq!(
        render_status(&composer, area, options).1,
        ["one", "two", "three", "four…"]
    );
    let tree: crate::ui_tree::UiTree = r#""x\ny""#.parse().unwrap();
    composer.set_status_line(Some(StatusLine::from(tree)));
    assert_eq!(render_status(&composer, area, options).1, ["x", "y"]);
    assert_eq!(
        composer.footer_hint_height(/*width*/ 7, ComposerRenderOptions::default()),
        0
    );
    composer.show_footer_flash("Copied".into(), Duration::from_secs(/*secs*/ 3));
    assert_eq!(
        composer.footer_hint_height(/*width*/ 7, ComposerRenderOptions::default()),
        1
    );
}

#[test]
fn sidebar_focus_keeps_dim_extension_status_rows() {
    let mut composer = composer();
    composer.insert_str("draft");
    let tree: crate::ui_tree::UiTree = r#""one\ntwo\nthree\nfour""#.parse().unwrap();
    composer.set_status_line(Some(tree.into()));
    let frames = [false, true, false].map(|unfocused| {
        let options = ComposerRenderOptions {
            separate_status_line: true,
            warning_count: usize::from(unfocused),
            unfocused,
            ..Default::default()
        };
        let height = composer.desired_height_with_options(/*width*/ 40, options);
        let (buffer, rows) = render_status(&composer, Rect::new(0, 0, 40, height), options);
        if unfocused {
            assert_eq!(composer.footer.warning_notice_area.get(), None);
            assert!(!buffer.content.iter().any(|cell| cell.symbol() == "⚠"));
            let status = composer.layout_with_options(buffer.area, options).status;
            for y in status.y..status.bottom() {
                assert!(
                    buffer[(status.x, y)]
                        .modifier
                        .contains(ratatui::style::Modifier::DIM)
                );
            }
        }
        rows.join("\n")
    });
    insta::assert_snapshot!(frames.join("\n---\n"), @"
    one
    two
    three
    four
    ---
    one
    two
    three…
    ---
    one
    two
    three
    four
    ");
}

#[test]
fn thumbnails_fit_after_basic_footer_rows_and_before_extra_status() {
    let mut composer = composer();
    composer.attach_image(PathBuf::from("preview.png"));
    composer.insert_str("draft");
    let tree: crate::ui_tree::UiTree = r#""one\ntwo\nthree\nfour""#.parse().unwrap();
    composer.set_status_line(Some(tree.into()));
    composer.set_collaboration_mode_indicator(Some(CollaborationModeIndicator::Plan));
    let previews = crate::transcript_image::composer::ComposerImages::for_terminal(
        crate::pets::ImageProtocol::Kitty,
        /*previous*/ None,
    )
    .unwrap();
    let mut frames = Vec::new();
    for (fullscreen, cap, expected) in [
        (true, 4, (0, 0, 1)),
        (true, 5, (0, 1, 1)),
        (true, 6, (0, 1, 1)),
        (true, 8, (0, 1, 1)),
        (true, 9, (4, 1, 1)),
        (true, 10, (4, 2, 1)),
        (false, 8, (0, 1, 1)),
        (false, 9, (4, 1, 1)),
    ] {
        let options = ComposerRenderOptions {
            separate_status_line: fullscreen,
            image_previews: Some(&previews),
            max_height: Some(cap),
            ..Default::default()
        };
        let height = composer.desired_height_with_options(/*width*/ 32, options);
        assert_eq!(height, 3 + expected.0 + expected.1 + expected.2);
        let area = Rect::new(0, 0, 32, height);
        let layout = composer.layout_with_options(area, options);
        assert_eq!(
            (
                layout.thumbnails.height,
                layout.status.height,
                layout.footer.height
            ),
            expected
        );
        if fullscreen && (cap == 8 || cap == 9) {
            let (buffer, _) = render_status(&composer, area, options);
            frames.push(format!("height {cap}\n{}", frame(&buffer)));
        }
    }
    insta::assert_snapshot!(frames.join("\n\n"));

    let options = ComposerRenderOptions {
        image_previews: Some(&previews),
        ..Default::default()
    };
    composer.popups.active = ActivePopup::File(FileSearchPopup::new());
    let with_popup = composer.layout_with_options(Rect::new(0, 0, 32, 9), options);
    assert!(with_popup.popup.height > 0);
    assert_eq!(with_popup.thumbnails.height, 0);
    composer.popups.active = ActivePopup::None;
    composer.set_status_line(/*status_line*/ None);
    let hints_only = composer.layout_with_options(Rect::new(0, 0, 32, 8), options);
    assert_eq!(
        (
            hints_only.thumbnails.height,
            hints_only.status.height,
            hints_only.footer.height
        ),
        (4, 0, 1)
    );

    composer.insert_str("\nsecond line");
    let search = TranscriptFooter {
        text: "Find: abc".into(),
        cursor_column: Some(9),
        is_interactive: true,
    };
    for separate_status_line in [false, true] {
        let options = ComposerRenderOptions {
            separate_status_line,
            image_previews: Some(&previews),
            footer: Some(&search),
            ..Default::default()
        };
        let area = Rect::new(0, 0, 32, 4);
        let layout = composer.layout_with_options(area, options);
        assert_eq!(
            (
                layout.textarea.height,
                layout.thumbnails.height,
                layout.status.height,
                layout.footer.height
            ),
            (1, 0, 0, 1)
        );
        assert_eq!(
            composer.cursor_pos_with_options(area, options),
            Some((11, 3))
        );
    }
}

#[test]
fn extension_status_paints_with_the_current_theme_at_the_same_width() {
    use crate::render::highlight::current_syntax_theme;
    use crate::render::highlight::set_syntax_theme;
    use ratatui::style::Color;
    use syntect::highlighting::Color as ThemeColor;

    let mut composer = composer();
    let content: crate::ui_tree::UiTree =
        r#"{"type":"text","props":{"color":"primary"},"children":["λ"]}"#
            .parse()
            .unwrap();
    composer.set_status_line(Some(StatusLine::from(content)));
    let previous = current_syntax_theme();
    let colors = [1, 2].map(|index| {
        let mut theme = previous.clone();
        theme.settings.foreground = Some(ThemeColor {
            r: index,
            g: 0,
            b: 0,
            a: 0,
        });
        set_syntax_theme(theme);
        let options = ComposerRenderOptions::default();
        let width = 24;
        let area = Rect::new(
            0,
            0,
            width,
            composer.desired_height_with_options(width, options),
        );
        let mut buffer = Buffer::empty(area);
        composer.render_with_options(area, &mut buffer, /*mask_char*/ None, options);
        buffer
            .content
            .iter()
            .find(|cell| cell.symbol() == "λ")
            .unwrap()
            .fg
    });
    set_syntax_theme(previous);
    assert_eq!(colors, [Color::Red, Color::Green]);
}

#[test]
fn extension_status_keeps_its_rows_when_effort_changes() {
    let mut composer = composer();
    let tree: crate::ui_tree::UiTree = r#""one\ntwo\nthree""#.parse().unwrap();
    composer.set_status_line(Some(StatusLine::from(tree)));
    composer.set_active_reasoning_effort_baseline(/*effort*/ None);
    composer.set_active_reasoning_effort(
        Some(&ReasoningEffort::Max),
        /*animations_enabled*/ true,
    );
    for separate_status_line in [false, true] {
        let options = ComposerRenderOptions {
            separate_status_line,
            ..Default::default()
        };
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 7, /*height*/ 12,
        );
        assert_eq!(
            render_status(&composer, area, options).1,
            if separate_status_line {
                vec!["one", "two", "three"]
            } else {
                vec!["one", "two…"]
            }
        );
    }
}

#[test]
fn native_status_wraps_links_within_the_host_budget() {
    let mut composer = composer();
    composer.set_collaboration_modes_enabled(/*enabled*/ true);
    let url = "https://example.com/pr/42";
    let mut status = HyperlinkLine::from("لا ");
    status.push_span(
        "PR #42 build green deploy ready all unit tests passed release queued".underlined(),
        Some(url),
    );
    composer.set_status_line(Some(status.into()));
    let mut frames = Vec::new();
    composer.footer.toggle_shortcuts_key = None;
    for (fullscreen, mode, status_height) in [
        (true, None, 4),
        (true, Some(CollaborationModeIndicator::Plan), 3),
        (false, None, 2),
        (false, Some(CollaborationModeIndicator::Plan), 1),
    ] {
        composer.set_collaboration_mode_indicator(mode);
        let (_, rows) = render_status(
            &composer,
            Rect::new(0, 0, 18, 20),
            ComposerRenderOptions {
                separate_status_line: fullscreen,
                ..Default::default()
            },
        );
        assert_eq!(rows.len(), status_height);
        for row in &rows {
            let text = strip_osc8(row);
            let (prefix, rest) = text
                .strip_prefix("لا ")
                .map_or(("", text.as_str()), |rest| ("لا ", rest));
            let (link, end) = rest
                .strip_suffix('…')
                .map_or((rest, ""), |link| (link, "…"));
            let visible_link: String = link
                .chars()
                .map(|ch| osc8_hyperlink(url, &ch.to_string()))
                .collect();
            assert_eq!(*row, format!("{prefix}{visible_link}{end}"));
        }
        frames.push(
            rows.iter()
                .map(|row| strip_osc8(row))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    insta::assert_snapshot!(frames.join("\n---\n"));
}

#[test]
fn native_wrap_tracks_agent_label_and_content_changes_at_the_same_width() {
    let mut composer = composer();
    composer.set_status_line(Some(Line::from("build ready").into()));
    let area = Rect::new(0, 0, 16, 14);
    composer.footer.toggle_shortcuts_key = None;
    for agent in ["frontend", "backend"] {
        composer.set_active_agent_label(Some(agent.into()));
        assert_eq!(
            render_status(&composer, area, Default::default()).1,
            ["build ready ·", agent]
        );
    }
    composer.set_status_line(Some(Line::from("tests green").into()));
    assert_eq!(
        render_status(&composer, area, Default::default()).1,
        ["tests green ·", "backend"]
    );
    let wide = Rect { width: 36, ..area };
    assert_eq!(
        render_status(&composer, wide, Default::default()).1,
        ["tests green · backend"]
    );
    assert_eq!(
        render_status(&composer, area, Default::default()).1,
        ["tests green ·", "backend"]
    );
}

#[tokio::test(start_paused = true)]
async fn native_effort_transition_restores_wrapping_after_its_final_frame() {
    let mut composer = composer();
    composer.set_status_line(Some(
        Line::from("build ready tests passed deploy queued").into(),
    ));
    composer.set_active_reasoning_effort_baseline(/*effort*/ None);
    composer.set_active_reasoning_effort(
        Some(&ReasoningEffort::Max),
        /*animations_enabled*/ true,
    );
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 14, /*height*/ 20,
    );
    for separate_status_line in [false, true] {
        let options = ComposerRenderOptions {
            separate_status_line,
            ..Default::default()
        };
        assert_eq!(render_status(&composer, area, options).1.len(), 1);
    }

    let options = ComposerRenderOptions {
        separate_status_line: true,
        ..Default::default()
    };
    let status = composer.layout_with_options(area, options).status;
    assert_eq!(status.height, 1);
    let (requester, mut frames) = FrameRequester::test_channel();
    composer.set_frame_requester(requester);
    tokio::time::advance(Duration::from_secs(/*secs*/ 10)).await;
    composer.render_status_surface(status, &mut Buffer::empty(area), options);
    frames.try_recv().expect("final paint requests a redraw");
    let wrapped = composer.layout_with_options(area, options).status;
    let mut buf = Buffer::empty(area);
    composer.render_status_surface(wrapped, &mut buf, options);
    assert!(wrapped.height > 1);
    assert!(frame(&buf).contains("tests"));
    assert!(frames.try_recv().is_err());
}

fn render(
    composer: &ChatComposer,
    width: u16,
    footer: Option<&TranscriptFooter>,
) -> (String, Option<(u16, u16)>) {
    render_with_height(composer, width, footer, /*height*/ None)
}

fn render_with_height(
    composer: &ChatComposer,
    width: u16,
    footer: Option<&TranscriptFooter>,
    height: Option<u16>,
) -> (String, Option<(u16, u16)>) {
    let options = composer.resolve_render_options(ComposerRenderOptions {
        separate_status_line: true,
        command_popup_placement: CommandPopupPlacement::Overlay,
        footer,
        ..ComposerRenderOptions::default()
    });
    let height = height.unwrap_or_else(|| composer.desired_height_with_options(width, options));
    let area = Rect::new(/*x*/ 0, /*y*/ 8, width, height);
    let mut buf = Buffer::empty(Rect::new(/*x*/ 0, /*y*/ 0, width, area.bottom()));
    composer.render_with_options(area, &mut buf, /*mask_char*/ None, options);
    let text = buf
        .content
        .chunks(usize::from(width))
        .map(|row| {
            row.iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");
    (text, composer.cursor_pos_with_options(area, options))
}

#[test]
fn passive_activity_keeps_shortcuts_only_when_the_complete_hint_fits() {
    let mut composer = composer();
    let mut line = Line::from(key_hint::key_label_spans("ctrl+o t/f4"));
    line.push_span(" inspect activity".dim());
    let mut footer = TranscriptFooter {
        text: line.into(),
        cursor_column: None,
        is_interactive: false,
    };
    for (width, expected) in [
        (80, "ctrl+o t/f4 inspect activity · ? shortcuts"),
        (40, "ctrl+o t/f4 inspect activity"),
    ] {
        let (text, _) = render(&composer, width, Some(&footer));
        assert_eq!(text.lines().last().map(str::trim), Some(expected));
    }
    footer.is_interactive = true;
    let (text, _) = render(&composer, /*width*/ 80, Some(&footer));
    assert_eq!(
        text.lines().last().map(str::trim),
        Some("ctrl+o t/f4 inspect activity")
    );
    footer.is_interactive = false;
    composer.footer.toggle_shortcuts_key = None;
    let (text, _) = render(&composer, /*width*/ 80, Some(&footer));
    assert_eq!(
        text.lines().last().map(str::trim),
        Some("ctrl+o t/f4 inspect activity")
    );
}

#[test]
fn fullscreen_plan_indicator_keeps_the_cycle_hint_when_it_fits() {
    let mut composer = composer();
    composer.set_collaboration_modes_enabled(/*enabled*/ true);
    composer.set_collaboration_mode_indicator(Some(CollaborationModeIndicator::Plan));
    let (wide, _) = render(&composer, /*width*/ 100, /*footer*/ None);
    assert!(
        wide.contains("Plan mode (⇧tab to cycle)"),
        "idle fullscreen must explain how to leave Plan mode: {wide}"
    );
    insta::assert_snapshot!("fullscreen_plan_cycle_hint", wide);

    let mut status = HyperlinkLine::from("project · ");
    status.push_span("PR #42".underlined(), Some("https://example.com/pull/42"));
    composer.set_status_line(Some(status.into()));
    composer.set_active_agent_label(Some("reviewer".to_string()));
    let (narrow, _) = render(&composer, /*width*/ 37, /*footer*/ None);
    assert!(
        strip_osc8(&narrow).contains("project · PR #42 · reviewer"),
        "{narrow}"
    );
    assert!(
        narrow.lines().last().unwrap().contains("Plan mode"),
        "{narrow}"
    );
    composer.set_active_agent_label(/*active_agent_label*/ None);

    composer.set_task_running(/*running*/ true);
    let (running, _) = render(&composer, /*width*/ 100, /*footer*/ None);
    assert!(running.contains("Plan mode"), "{running}");
    assert!(!running.contains("⇧tab"), "{running}");
    composer.set_task_running(/*running*/ false);

    let footer = TranscriptFooter {
        text: "Search: verify".into(),
        cursor_column: Some(8),
        is_interactive: true,
    };
    let (search, _) = render(&composer, /*width*/ 100, Some(&footer));
    assert!(search.contains("Search: verify"), "{search}");
    assert!(search.contains("Plan mode"), "{search}");
    assert!(!search.contains("⇧tab"), "{search}");

    // Configured items such as git-branch can have no value outside a repository.
    composer.set_status_line(/*status_line*/ None);
    for separate_status_line in [false, true] {
        let options = ComposerRenderOptions {
            separate_status_line,
            ..Default::default()
        };
        let width = 20;
        let area = Rect::new(
            0,
            0,
            width,
            composer.desired_height_with_options(width, options),
        );
        let (buf, _) = render_status(&composer, area, options);
        let hint: String = (0..width)
            .map(|x| buf[(x, area.bottom() - 1)].symbol())
            .collect();
        assert_eq!(
            hint.trim(),
            "Plan mode",
            "separate_status_line={separate_status_line}"
        );
    }
    let (empty_wide, _) = render(&composer, /*width*/ 100, /*footer*/ None);
    assert!(
        empty_wide.contains("Plan mode (⇧tab to cycle)"),
        "{empty_wide}"
    );

    composer.set_input_enabled(/*enabled*/ false, /*placeholder*/ None);
    let (read_only, _) = render(&composer, /*width*/ 100, /*footer*/ None);
    assert!(
        read_only.lines().last().unwrap().contains("Plan mode"),
        "{read_only}"
    );
    composer.set_input_enabled(/*enabled*/ true, /*placeholder*/ None);
    composer.set_parent_owned_thread();
    let (parent_owned, _) = render(&composer, /*width*/ 100, /*footer*/ None);
    assert!(
        parent_owned.lines().last().unwrap().contains("Plan mode"),
        "{parent_owned}"
    );
}

#[test]
fn footer_layout_in_fullscreen_and_scrollback() {
    let mut composer = composer();
    composer.command_center_enabled = true;
    composer.set_vim_enabled(/*enabled*/ true);
    composer.set_collaboration_modes_enabled(/*enabled*/ true);
    composer.set_collaboration_mode_indicator(Some(CollaborationModeIndicator::Plan));
    composer.footer.ide_context_active = true;
    composer.set_status_line(Some("Build ready".into()));
    let mut frames = Vec::new();
    for (fullscreen, width, rows, drafting) in [
        (true, 120, 2, false),
        (true, 60, 2, false),
        (true, 31, 3, false),
        (true, 31, 3, true),
        (true, 31, 2, true),
        (false, 31, 4, true),
        (false, 31, 1, true),
    ] {
        if drafting && !composer.is_task_running {
            composer.insert_str("draft");
            composer.set_task_running(/*running*/ true);
        }
        let options = ComposerRenderOptions {
            separate_status_line: fullscreen,
            warning_count: 2,
            ..Default::default()
        };
        let area = Rect::new(/*x*/ 0, /*y*/ 0, width, 3 + rows);
        let layout = composer.layout_with_options(area, options);
        let mut buf = Buffer::empty(area);
        composer.render_with_options(area, &mut buf, /*mask_char*/ None, options);
        let lines = (layout.status.y..area.bottom())
            .map(|y| {
                (0..width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect::<Vec<_>>();
        if let Some(hit) = composer.footer.warning_notice_area.get() {
            assert_eq!(buf[(hit.x, hit.y)].symbol(), "⚠");
            assert_eq!(hit.y, layout.footer.bottom() - 1);
        }
        frames.push(format!(
            "{} {width}×{rows} {}\n{}",
            if fullscreen {
                "fullscreen"
            } else {
                "scrollback"
            },
            if drafting { "draft" } else { "empty" },
            lines.join("\n")
        ));
    }
    insta::assert_snapshot!(frames.join("\n---\n"));
}

#[test]
fn active_side_action_keeps_plan_expanded_on_the_other_row() {
    let mut composer = composer();
    composer.set_collaboration_mode_indicator(Some(CollaborationModeIndicator::Plan));
    composer.set_side_conversation_active(/*active*/ true);
    let label = "Side from main thread · ctrl+/ to switch · ctrl+c to close";
    composer.set_side_conversation_context_label(Some(label.to_string()));
    let options = ComposerRenderOptions {
        separate_status_line: true,
        warning_count: 2,
        ..Default::default()
    };
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 63, /*height*/ 6,
    );
    let (buf, _) = render_status(&composer, area, options);
    let text = frame(&buf);
    let mut rows = text.lines().rev().map(|row| row.trim_matches('|').trim());
    assert_eq!(rows.next(), Some(label));
    assert!(rows.next().unwrap().contains("Plan mode (⇧tab to cycle)"));
}

#[test]
fn oversized_goal_yields_to_an_eligible_whole_hint_with_status_disabled() {
    let mut composer = composer();
    composer.set_status_line_enabled(/*enabled*/ false);
    composer.set_context_window(Some(100), /*used_tokens*/ None);
    let (text, _) = render(&composer, /*width*/ 18, /*footer*/ None);
    assert_eq!(text.lines().last().map(str::trim), Some("? for shortcuts"));
    let (text, _) = render(&composer, /*width*/ 20, /*footer*/ None);
    assert_eq!(
        text.lines().last().map(str::trim),
        Some("100% context left")
    );

    composer.set_context_window_hidden(/*hidden*/ true);
    composer.set_goal_status_indicator(Some(GoalStatusIndicator::Paused));
    let (text, _) = render(&composer, /*width*/ 18, /*footer*/ None);
    assert_eq!(text.lines().last().map(str::trim), Some("? for shortcuts"));
    let options = ComposerRenderOptions {
        separate_status_line: true,
        ..Default::default()
    };
    assert_eq!(composer.footer_hint_height(/*width*/ 18, options), 1);
    composer.set_input_enabled(/*enabled*/ false, /*placeholder*/ None);
    assert_eq!(composer.footer_hint_height(/*width*/ 18, options), 0);
    let (text, _) = render(&composer, /*width*/ 80, /*footer*/ None);
    assert_eq!(
        text.lines().last().map(str::trim),
        Some("Goal paused (/goal resume)")
    );
}

#[test]
fn shortcut_help_stays_above_the_prompt_and_bottom_status() {
    let mut composer = composer();
    composer.handle_key_event(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
    for (width, height, snapshot) in [
        (130, None, Some("shortcut_help_above_wide")),
        (80, None, None),
        (40, None, Some("shortcut_help_above_narrow")),
        (80, Some(12), None),
    ] {
        let (text, cursor) = render_with_height(&composer, width, /*footer*/ None, height);
        let rows: Vec<_> = text.lines().collect();
        let help_row = rows
            .iter()
            .position(|line| line.contains("Keyboard shortcuts"))
            .unwrap();
        let prompt_row = rows
            .iter()
            .position(|line| line.contains("Ask Codex"))
            .unwrap();
        assert!(help_row < prompt_row);
        assert!(
            rows[..prompt_row]
                .iter()
                .any(|line| line.trim() == "/keymap customize")
        );
        assert_eq!(text.matches("? / esc close").count(), 1);
        assert_eq!(
            (
                rows.last().map(|line| line.trim()),
                rows[rows.len() - 2].trim(),
                cursor,
            ),
            (
                Some("? / esc close"),
                "MODEL · ~/project · Context 20% used",
                Some((2, prompt_row as u16))
            ),
        );
        assert_eq!(prompt_row + 3, rows.len() - 1);
        if let Some(name) = snapshot {
            insta::assert_snapshot!(name, text);
        }
    }
}

#[test]
fn shortcut_help_keeps_bottom_anchored_composer_status_and_cursor_stable() {
    let (width, available_height) = (40, 12);
    let mut composer = composer();
    let mut geometry = Vec::new();
    for event in [
        None,
        Some(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE)),
        Some(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
    ] {
        if let Some(event) = event {
            composer.handle_key_event(event);
        }
        let options = composer.resolve_render_options(ComposerRenderOptions {
            separate_status_line: true,
            ..ComposerRenderOptions::default()
        });
        let height = composer
            .desired_height_with_options(width, options)
            .min(available_height);
        let area = Rect::new(/*x*/ 3, 80 - height, width, height);
        let ComposerLayout {
            composer: surface,
            footer,
            status,
            ..
        } = composer.layout_with_options(area, options);
        geometry.push((
            surface,
            status,
            footer,
            composer.cursor_pos_with_options(area, options),
        ));
    }
    assert_eq!(geometry, vec![geometry[0]; 3]);
}

#[test]
fn shortcut_help_keeps_input_and_close_hint_visible_when_height_is_tiny() {
    for status_enabled in [true, false] {
        let mut composer = composer();
        composer.set_status_line_enabled(status_enabled);
        composer.handle_key_event(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
        let options = composer.resolve_render_options(ComposerRenderOptions {
            separate_status_line: true,
            ..ComposerRenderOptions::default()
        });
        for height in 0..=7 {
            let area = Rect::new(/*x*/ 3, /*y*/ 5, /*width*/ 40, height);
            let mut buf = Buffer::empty(area);
            composer.render_with_options(area, &mut buf, /*mask_char*/ None, options);
            if let Some((x, y)) = composer.cursor_pos_with_options(area, options) {
                assert!(x >= area.x && x < area.right() && y >= area.y && y < area.bottom());
            }
            if height >= 4 {
                let ComposerLayout {
                    composer: surface,
                    footer,
                    ..
                } = composer.layout_with_options(area, options);
                assert_eq!(
                    (surface.height, footer.height, footer.bottom()),
                    (3, 1, area.bottom())
                );
                let last_row = (area.x..area.right())
                    .map(|x| buf[(x, area.bottom() - 1)].symbol())
                    .collect::<String>();
                assert_eq!(last_row.trim(), "? / esc close");
            }
        }
    }
}

#[test]
fn shortcut_help_close_row_uses_runtime_binding_and_narrow_fallback() {
    let chord = Some(key_hint::ShortcutHint::Chord {
        prefix: key_hint::ctrl(KeyCode::Char('x')),
        completion: key_hint::KeyBinding::new(
            KeyCode::F(12),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT | KeyModifiers::ALT,
        ),
    });
    for (width, binding, expected) in [
        (80, chord, "⌃x ⌃⇧⌥f12 / esc close"),
        (22, chord, "esc close"),
        (80, None, "esc close"),
    ] {
        let mut composer = composer();
        composer.handle_key_event(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
        composer.footer.toggle_shortcuts_key = binding;
        let (text, _) = render_with_height(
            &composer,
            width,
            /*footer*/ None,
            /*height*/ Some(8),
        );
        assert_eq!(text.lines().last().map(str::trim), Some(expected));
        assert_eq!(text.matches("close").count(), 1);
        assert_eq!(text.matches("/keymap customize").count(), 1);
    }
}

#[test]
fn status_survives_hints_feedback_and_queue() {
    let mut composer = composer();
    composer.set_collaboration_mode_indicator(Some(CollaborationModeIndicator::Plan));
    let mut states = Vec::new();
    states.push(render(&composer, /*width*/ 64, /*footer*/ None).0);
    composer.footer.mode = FooterMode::ShortcutOverlay;
    composer.show_footer_flash("Copied selection".into(), Duration::from_secs(/*secs*/ 3));
    states.push(render(&composer, /*width*/ 64, /*footer*/ None).0);
    composer.footer.flash = None;
    composer.footer.mode = FooterMode::ComposerEmpty;
    composer.set_footer_hint_override(Some(vec![("Ctrl+X".into(), "waiting for chord".into())]));
    states.push(render(&composer, /*width*/ 64, /*footer*/ None).0);
    composer.set_footer_hint_override(/*items*/ None);
    composer.set_text_content("Continue the task".into(), Vec::new(), Vec::new());
    composer.set_task_running(/*running*/ true);
    states.push(render(&composer, /*width*/ 64, /*footer*/ None).0);
    for popup in [
        ActivePopup::File(FileSearchPopup::new()),
        ActivePopup::Skill(SkillPopup::new(Vec::new())),
    ] {
        composer.popups.active = popup;
        let (text, _) = render(&composer, /*width*/ 64, /*footer*/ None);
        assert!(text.contains("MODEL · ~/project · Context 20% used"));
        assert_eq!(text.matches("Plan mode").count(), 1);
    }
    for state in &states {
        assert_eq!(state.matches("Plan mode").count(), 1);
        assert!(state.contains("MODEL · ~/project · Context 20% used"));
    }
    insta::assert_snapshot!("persistent_status_and_messages", states.join("\n---\n"));
}

#[test]
fn transcript_search_keeps_status_and_query_cursor() {
    let composer = composer();
    let footer = TranscriptFooter {
        text: vec![
            Line::from("Find: needle"),
            Line::from("Enter next · Esc close"),
        ]
        .into(),
        cursor_column: Some(8),
        is_interactive: true,
    };
    let options = ComposerRenderOptions {
        separate_status_line: true,
        footer: Some(&footer),
        ..Default::default()
    };
    for (height, status_rows, hint_rows) in [(4, 0, 1), (5, 0, 2), (6, 1, 2)] {
        let area = Rect::new(0, 0, 48, height);
        let layout = composer.layout_with_options(area, options);
        assert_eq!(
            (layout.status.height, layout.footer.height),
            (status_rows, hint_rows)
        );
        let mut buf = Buffer::empty(area);
        composer.render_with_options(area, &mut buf, /*mask_char*/ None, options);
        assert_eq!(buf[(2, layout.footer.y)].symbol(), "F");
        assert_eq!(
            composer.cursor_pos_with_options(area, options),
            Some((10, layout.footer.y))
        );
    }
}

#[test]
fn input_history_search_uses_the_final_row() {
    let mut composer = composer();
    composer.begin_history_search();
    let (text, cursor) = render(&composer, /*width*/ 48, /*footer*/ None);
    assert_eq!(
        cursor.map(|(_, y)| y),
        Some(text.lines().count() as u16 - 1)
    );
}

#[test]
fn slash_suggestions_take_space_before_status_expands() {
    let mut composer = composer();
    let tree: crate::ui_tree::UiTree = r#""one\ntwo\nthree\nfour""#.parse().unwrap();
    composer.set_status_line(Some(tree.into()));
    for ch in ['/', 'm'] {
        composer.handle_key_event(KeyEvent::from(KeyCode::Char(ch)));
    }
    let options = ComposerRenderOptions {
        separate_status_line: true,
        command_popup_placement: CommandPopupPlacement::AboveComposer,
        ..Default::default()
    };
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 7,
    );
    let (buf, rows) = render_status(&composer, area, options);
    assert_eq!(rows, ["one…"]);
    insta::assert_snapshot!(frame(&buf));
}

#[test]
fn slash_popup_preserves_footer_and_selection_style() {
    let (tx, _rx) = unbounded_channel::<AppEvent>();
    let mut composer = ChatComposer::new(
        /*has_input_focus*/ true,
        AppEventSender::new(tx),
        /*enhanced_keys_supported*/ false,
        "Ask Codex to do anything".to_string(),
        /*disable_paste_burst*/ true,
    );
    composer.set_status_line_enabled(/*enabled*/ true);
    composer.set_status_line(Some("model · high · fast".into()));
    for ch in ['/', 'm'] {
        composer.handle_key_event(KeyEvent::from(KeyCode::Char(ch)));
    }
    composer.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));

    for (width, clipped) in [(80, false), (32, false), (80, true)] {
        let height = if clipped {
            6
        } else {
            composer.desired_height(width)
        };
        let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
        let mut buf = Buffer::empty(area);
        composer.render(area, &mut buf);
        let ComposerLayout {
            composer: surface,
            textarea,
            popup,
            status,
            ..
        } = composer.layout_with_options(area, ComposerRenderOptions::default());
        assert_eq!(popup.bottom(), surface.y);
        assert_eq!(surface.bottom(), status.y);
        assert_eq!(composer.cursor_pos(area), Some((4, textarea.y)));
        let status_text: String = (0..width).map(|x| buf[(x, status.y)].symbol()).collect();
        assert!(status_text.contains("model · high · fast"));
        if !clipped && width == 80 {
            insta::assert_snapshot!("slash_popup_footer_wide", format!("{buf:?}"));
        }
    }

    let footer = TranscriptFooter {
        text: "transcript status".into(),
        cursor_column: None,
        is_interactive: false,
    };
    let options = composer.resolve_render_options(ComposerRenderOptions {
        footer: Some(&footer),
        ..Default::default()
    });
    assert!(options.footer.is_some());
    let width = 80;
    let area = Rect::new(
        /*x*/ 0,
        /*y*/ 0,
        width,
        composer.desired_height_with_options(width, options),
    );
    let mut buf = Buffer::empty(area);
    composer.render_with_options(area, &mut buf, /*mask_char*/ None, options);
    let footer_text: String = (0..width)
        .map(|x| buf[(x, area.bottom() - 1)].symbol())
        .collect();
    assert!(footer_text.contains("transcript status"));
}

#[test]
fn activity_shortcut_help_follows_runtime_binding() {
    let mut snapshots = Vec::new();
    for configured in [
        serde_json::json!("f4"),
        serde_json::json!("ctrl-x t"),
        serde_json::json!([]),
    ] {
        let config = serde_json::from_value(serde_json::json!({
            "global": {"focus_activity": configured}
        }))
        .unwrap();
        let keymap = crate::keymap::RuntimeKeymap::from_config(&config).unwrap();
        let mut composer = composer();
        composer.set_keymap_bindings(&keymap);
        composer.handle_key_event(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
        let (text, _) = render(&composer, /*width*/ 130, /*footer*/ None);
        snapshots.push(format!("{configured}\n{text}"));
    }
    insta::assert_snapshot!(snapshots.join("\n---\n"));
}
