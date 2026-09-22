use super::*;
use crate::chatwidget::tests::make_chatwidget_manual_with_sender;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

#[derive(Debug)]
struct CountingHistoryCell {
    desired_height_calls: Arc<AtomicUsize>,
    display_lines_calls: Arc<AtomicUsize>,
    desired_height: u16,
    line_count: usize,
    stable_height: bool,
}

impl HistoryCell for CountingHistoryCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        let frame = self.display_lines_calls.fetch_add(1, Ordering::Relaxed) + 1;
        (0..self.line_count)
            .map(|row| Line::from(format!("frame {frame} row {row}")))
            .collect()
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        Vec::new()
    }

    fn desired_height(&self, _width: u16) -> u16 {
        self.desired_height_calls.fetch_add(1, Ordering::Relaxed);
        self.desired_height
    }

    fn has_stable_transcript_height(&self) -> bool {
        self.stable_height
    }
}

async fn widget_with_counting_cell(
    desired_height: u16,
    line_count: usize,
    stable_height: bool,
) -> (ChatWidget, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let (mut widget, _sender, _events, _operations) = make_chatwidget_manual_with_sender().await;
    let desired_height_calls = Arc::new(AtomicUsize::new(0));
    let display_lines_calls = Arc::new(AtomicUsize::new(0));
    widget.transcript.active_cell = Some(Box::new(CountingHistoryCell {
        desired_height_calls: Arc::clone(&desired_height_calls),
        display_lines_calls: Arc::clone(&display_lines_calls),
        desired_height,
        line_count,
        stable_height,
    }));
    (widget, desired_height_calls, display_lines_calls)
}

fn render_frame(widget: &ChatWidget, width: u16) -> Buffer {
    let renderable = widget.as_renderable();
    let height = renderable.desired_height(width);
    let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
    let mut buffer = Buffer::empty(area);
    renderable.render(area, &mut buffer);
    buffer
}

fn contains_text(buffer: &Buffer, text: &str) -> bool {
    buffer
        .content
        .chunks(usize::from(buffer.area.width))
        .any(|row| {
            row.iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>()
                .contains(text)
        })
}

#[tokio::test]
async fn owned_bottom_pane_preserves_draft_cursor_and_read_only_notice() {
    let (mut widget, _sender, _events, _operations) = make_chatwidget_manual_with_sender().await;
    widget.handle_paste("draft stays here".to_string());
    let area = Rect::new(
        /*x*/ 0, /*y*/ 8, /*width*/ 60, /*height*/ 8,
    );
    let render_bottom = |widget: &ChatWidget| {
        let bottom = widget.bottom_pane_renderable(
            /*footer*/ None,
            crate::bottom_pane::CommandPopupPlacement::Overlay,
            /*composer_gap*/ None,
        );
        let mut buffer = Buffer::empty(area);
        bottom.render(area, &mut buffer);
        (buffer, bottom.cursor_pos(area), bottom.cursor_style(area))
    };
    let before = render_bottom(&widget);
    assert!(contains_text(&before.0, "draft stays here"));
    assert!(before.1.is_some());

    widget.transcript.active_cell = Some(Box::new(history_cell::PlainHistoryCell::new(vec![
        "live output belongs in the transcript".into(),
    ])));
    assert_eq!(render_bottom(&widget), before);

    widget.show_external_writer_thread();
    let (notice, cursor, _) = render_bottom(&widget);
    assert!(contains_text(
        &notice,
        "This conversation is open in another app"
    ));
    assert_eq!(cursor, None);
}

#[derive(Debug)]
struct PresentationCell(&'static str);

impl HistoryCell for PresentationCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        vec![format!("compact {}", self.0).into()]
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        vec![format!("raw {}", self.0).into()]
    }

    fn transcript_lines(&self, _width: u16) -> Vec<Line<'static>> {
        vec![format!("detailed {}", self.0).into()]
    }
}

#[tokio::test]
async fn owned_live_history_keeps_all_sources_in_each_presentation() {
    let (mut widget, _sender, _events, _operations) = make_chatwidget_manual_with_sender().await;
    widget.transcript.active_cell = Some(Box::new(PresentationCell("active")));
    widget.realtime_conversation.live_transcript_cell = Some(Box::new(PresentationCell("voice")));
    widget.pending_rate_limit_reset_hint = Some(history_cell::PlainHistoryCell::new(vec![
        "rate limit hint".into(),
    ]));
    let text = |lines: Option<Vec<HyperlinkLine>>| {
        lines
            .unwrap_or_default()
            .into_iter()
            .map(|line| line.line.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    };
    let compact = text(widget.active_cell_display_hyperlink_lines(/*width*/ 80));
    widget.set_raw_output_mode(/*enabled*/ true);
    let raw = text(widget.active_cell_display_hyperlink_lines(/*width*/ 80));
    let detailed = text(widget.active_cell_transcript_hyperlink_lines(/*width*/ 80));

    insta::assert_snapshot!(format!("{compact}\n---\n{raw}\n---\n{detailed}"), @"
    compact active

    compact voice

    rate limit hint
    ---
    raw active

    raw voice

    rate limit hint
    ---
    detailed active

    detailed voice

    rate limit hint
    ");

    widget.local_settings.tui.animations = false;
    widget.realtime_conversation = Default::default();
    crate::chatwidget::realtime::tests::activate_voice_for_thread(&mut widget, ThreadId::new());
    widget.realtime_conversation.pending_history_cells.extend([
        Box::new(PresentationCell("deferred first")) as Box<dyn HistoryCell>,
        Box::new(PresentationCell("deferred second")) as Box<dyn HistoryCell>,
    ]);
    widget.on_realtime_transcript_delta("assistant".into(), "assistant ".into());
    widget.on_realtime_transcript_delta("user".into(), "user caption".into());
    widget.on_realtime_transcript_done("user".into(), "user caption".into());
    widget.on_realtime_transcript_delta("assistant".into(), "caption".into());
    let expected = [
        "active",
        "deferred first",
        "deferred second",
        "user caption",
        "assistant caption",
        "rate limit hint",
    ];

    for latest_speaker in ["assistant", "user"] {
        widget.on_realtime_transcript_delta(latest_speaker.into(), String::new());
        widget.set_raw_output_mode(/*enabled*/ false);
        let compact = text(widget.active_cell_display_hyperlink_lines(/*width*/ 80));
        let frame = render_frame(&widget, /*width*/ 80);
        let inline = frame
            .content
            .chunks(usize::from(frame.area.width))
            .map(|row| {
                row.iter()
                    .map(ratatui::buffer::Cell::symbol)
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        widget.set_raw_output_mode(/*enabled*/ true);
        let raw = text(widget.active_cell_display_hyperlink_lines(/*width*/ 80));
        let detailed = text(widget.active_cell_transcript_hyperlink_lines(/*width*/ 80));
        for (presentation, rendered) in [
            ("compact", compact),
            ("inline", inline),
            ("raw", raw),
            ("detailed", detailed),
        ] {
            let actual = rendered
                .lines()
                .filter_map(|line| {
                    expected
                        .iter()
                        .find(|source| line.contains(**source))
                        .copied()
                })
                .collect::<Vec<_>>();
            assert_eq!(
                actual, expected,
                "{presentation}; latest speaker {latest_speaker}"
            );
        }
    }
}

#[tokio::test]
async fn external_writer_view_shows_notice_instead_of_composer() {
    let (mut widget, _sender, _events, _operations) = make_chatwidget_manual_with_sender().await;
    widget.show_external_writer_thread();

    let frame = crate::terminal_palette::with_test_default_colors(
        crate::terminal_probe::DefaultColors {
            fg: (230, 230, 230),
            bg: (20, 20, 20),
        },
        || render_frame(&widget, /*width*/ 60),
    );
    let rows: Vec<String> = frame
        .content
        .chunks(usize::from(frame.area.width))
        .map(|row| row.iter().map(ratatui::buffer::Cell::symbol).collect())
        .collect();
    insta::assert_snapshot!(
        rows.iter()
            .map(|row| row.trim_end())
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert_ne!(frame[(0, 1)].bg, ratatui::style::Color::Reset);
    assert_eq!(frame[(0, 1)].bg, frame[(59, 4)].bg);
    assert_eq!(
        (frame[(3, 5)].fg, frame[(3, 5)].modifier),
        (
            crate::terminal_palette::rgb_color((230, 230, 230)),
            ratatui::style::Modifier::BOLD
        ),
    );
    assert_eq!(frame[(5, 5)].modifier, ratatui::style::Modifier::empty());
    assert!(!widget.bottom_pane.composer_input_enabled());
}

#[tokio::test]
async fn external_writer_fork_progress_restores_the_previous_view() {
    let (mut widget, _sender, _events, _operations) = make_chatwidget_manual_with_sender().await;
    for locked in [false, true] {
        widget.external_writer_view = locked;
        let previous = render_frame(&widget, /*width*/ 60);
        widget.fork_in_progress = true;
        let frame = render_frame(&widget, /*width*/ 60);
        let rows = frame
            .content
            .chunks(usize::from(frame.area.width))
            .map(|row| {
                row.iter()
                    .map(ratatui::buffer::Cell::symbol)
                    .collect::<String>()
            })
            .collect::<Vec<_>>();
        insta::assert_snapshot!(
            "fork_pending",
            rows.iter()
                .map(|row| row.trim_end())
                .collect::<Vec<_>>()
                .join("\n")
        );
        widget.fork_in_progress = false;
        assert_eq!(render_frame(&widget, /*width*/ 60), previous);
    }
}

#[tokio::test]
async fn external_writer_notice_uses_current_transcript_shortcut() {
    let (mut widget, _sender, _events, _operations) = make_chatwidget_manual_with_sender().await;
    let mut keymap = crate::keymap::RuntimeKeymap::defaults();
    keymap.app.open_transcript = vec![crate::key_hint::ctrl(KeyCode::Char('k'))];
    widget.bottom_pane.set_keymap_bindings(&keymap);
    widget.show_external_writer_thread();

    assert!(contains_text(
        &render_frame(&widget, /*width*/ 80),
        "ctrl+k transcript"
    ));
}

#[test]
fn active_transcript_preserves_clipped_markdown_hyperlinks() {
    let cell = history_cell::AgentMarkdownCell::new(
        "Earlier content\n\n[OSC8 label](https://example.com/)".to_string(),
        std::path::Path::new("/tmp"),
    );
    let renderable = TranscriptAreaRenderable {
        child: &cell,
        top: 1,
        right: 2,
        persistent_layout: None,
    };
    let area = Rect::new(
        /*x*/ 2, /*y*/ 1, /*width*/ 40, /*height*/ 3,
    );
    let mut buffer = Buffer::empty(area);
    renderable.render(area, &mut buffer);

    let linked_text = buffer
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .filter(|symbol| symbol.starts_with("\x1b]8;;https://example.com/\x07"))
        .map(crate::terminal_hyperlinks::strip_osc8)
        .collect::<String>();
    assert_eq!(linked_text, "OSC8 labelhttps://example.com/");

    let visible_rows = buffer
        .content
        .chunks(usize::from(area.width))
        .map(|row| {
            row.iter()
                .map(|cell| crate::terminal_hyperlinks::strip_osc8(cell.symbol()))
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>();
    insta::assert_debug_snapshot!(visible_rows, @r#"
    [
        "",
        "",
        "  OSC8 label (https://example.com/)",
    ]
    "#);

    let size = ratatui::layout::Size::new(/*width*/ 44, /*height*/ 5);
    let mut terminal =
        crate::custom_terminal::Terminal::with_screen_size_and_cursor_position_for_test(
            ratatui::backend::CrosstermBackend::new(Vec::new()),
            size,
            area.as_position(),
        );
    terminal.set_viewport_area(Rect::new(
        /*x*/ 0,
        /*y*/ 0,
        size.width,
        size.height,
    ));
    terminal
        .draw_with_size(size, |frame| renderable.render(area, frame.buffer_mut()))
        .expect("render terminal frame");
    let output = String::from_utf8(terminal.backend().writer().clone()).expect("UTF-8 output");
    assert!(output.contains("\x1b]8;;https://example.com/\x07OSC8 label\x1b]8;;\x07"));
    assert!(output.contains("\x1b]8;;https://example.com/\x07https://example.com/\x1b]8;;\x07"));
}

#[tokio::test]
async fn initial_session_header_starts_at_the_top_of_the_viewport() {
    let (mut widget, _sender, _events, _operations) = make_chatwidget_manual_with_sender().await;
    widget.transcript.active_cell =
        Some(ChatWidget::placeholder_session_header_cell(&widget.config));

    let frame = render_frame(&widget, /*width*/ 48);
    let header = frame
        .content
        .chunks(usize::from(frame.area.width))
        .take(/*n*/ 6)
        .map(|row| {
            row.iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
        .replace(crate::version::CODEX_CLI_VERSION, "<VERSION>");

    let cwd = widget.config.cwd.as_path().display().to_string();
    let normalized_cwd = format!("{:<width$}", "/tmp/project", width = cwd.len());

    insta::assert_snapshot!(header.replace(&cwd, &normalized_cwd), @r"
    ╭───────────────────────────────────────╮
    │ >_ OpenAI Codex (v<VERSION>)              │
    │                                       │
    │ model:     loading   /model to change │
    │ directory: /tmp/project               │
    ╰───────────────────────────────────────╯
    ");
}

#[tokio::test]
async fn active_cell_layout_reuses_heights_without_freezing_animation() {
    let (widget, desired_height_calls, display_lines_calls) = widget_with_counting_cell(
        /*desired_height*/ 2, /*line_count*/ 2, /*stable_height*/ true,
    )
    .await;

    let first = render_frame(&widget, /*width*/ 80);
    let second = render_frame(&widget, /*width*/ 80);

    assert_eq!(desired_height_calls.load(Ordering::Relaxed), 1);
    assert_eq!(display_lines_calls.load(Ordering::Relaxed), 2);
    assert!(contains_text(&first, "frame 1 row 0"));
    assert!(contains_text(&second, "frame 2 row 0"));
    let cached = widget
        .transcript
        .active_cell_layout
        .get()
        .expect("active-cell layout should be cached");
    assert_eq!(cached.desired_height, Some(2));
    assert_eq!(cached.rendered_height, Some(2));
}

#[tokio::test]
async fn active_cell_layout_preserves_custom_height_and_bottom_scroll() {
    let (widget, desired_height_calls, _display_lines_calls) = widget_with_counting_cell(
        /*desired_height*/ 1, /*line_count*/ 3, /*stable_height*/ true,
    )
    .await;

    let first = render_frame(&widget, /*width*/ 80);
    let second = render_frame(&widget, /*width*/ 80);

    let mut transcript = second.clone();
    transcript.resize(Rect {
        height: 2,
        ..second.area
    });
    insta::assert_snapshot!(
        "cached_active_cell_bottom_scroll",
        format!("{transcript:?}")
    );
    assert!(contains_text(&first, "frame 1 row 2"));
    assert!(!contains_text(&first, "frame 1 row 0"));
    assert!(contains_text(&second, "frame 2 row 2"));
    assert!(!contains_text(&second, "frame 2 row 0"));
    assert_eq!(desired_height_calls.load(Ordering::Relaxed), 1);
    let cached = widget
        .transcript
        .active_cell_layout
        .get()
        .expect("active-cell layout should be cached");
    assert_eq!(cached.desired_height, Some(1));
    assert_eq!(cached.rendered_height, Some(3));
}

#[tokio::test]
async fn active_cell_layout_invalidates_width_revision_mode_theme_and_identity() {
    let (mut widget, desired_height_calls, _display_lines_calls) = widget_with_counting_cell(
        /*desired_height*/ 1, /*line_count*/ 1, /*stable_height*/ true,
    )
    .await;

    render_frame(&widget, /*width*/ 80);
    render_frame(&widget, /*width*/ 80);
    assert_eq!(desired_height_calls.load(Ordering::Relaxed), 1);

    render_frame(&widget, /*width*/ 81);
    assert_eq!(desired_height_calls.load(Ordering::Relaxed), 2);

    widget.transcript.bump_active_cell_revision();
    render_frame(&widget, /*width*/ 81);
    assert_eq!(desired_height_calls.load(Ordering::Relaxed), 3);

    widget.raw_output_mode = true;
    render_frame(&widget, /*width*/ 81);
    assert_eq!(desired_height_calls.load(Ordering::Relaxed), 4);

    let mut cached = widget
        .transcript
        .active_cell_layout
        .get()
        .expect("active-cell layout should be cached");
    cached.key.syntax_theme_revision = cached.key.syntax_theme_revision.wrapping_sub(1);
    widget.transcript.active_cell_layout.set(Some(cached));
    render_frame(&widget, /*width*/ 81);
    assert_eq!(desired_height_calls.load(Ordering::Relaxed), 5);

    let replacement_height_calls = Arc::new(AtomicUsize::new(0));
    widget.transcript.active_cell = Some(Box::new(CountingHistoryCell {
        desired_height_calls: Arc::clone(&replacement_height_calls),
        display_lines_calls: Arc::new(AtomicUsize::new(0)),
        desired_height: 1,
        line_count: 1,
        stable_height: true,
    }));
    render_frame(&widget, /*width*/ 81);
    assert_eq!(replacement_height_calls.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn externally_mutable_active_cells_bypass_persistent_layout_cache() {
    let (widget, desired_height_calls, display_lines_calls) = widget_with_counting_cell(
        /*desired_height*/ 2, /*line_count*/ 2, /*stable_height*/ false,
    )
    .await;

    render_frame(&widget, /*width*/ 80);
    render_frame(&widget, /*width*/ 80);

    assert_eq!(desired_height_calls.load(Ordering::Relaxed), 2);
    assert_eq!(display_lines_calls.load(Ordering::Relaxed), 2);
    assert_eq!(widget.transcript.active_cell_layout.get(), None);
}

#[tokio::test]
async fn removing_active_cell_invalidates_layout_before_reusing_its_identity() {
    let (mut widget, desired_height_calls, _display_lines_calls) = widget_with_counting_cell(
        /*desired_height*/ 2, /*line_count*/ 2, /*stable_height*/ true,
    )
    .await;

    render_frame(&widget, /*width*/ 80);
    let revision = widget.transcript.active_cell_revision;
    let active_cell = widget
        .transcript
        .take_active_cell()
        .expect("active cell should exist");

    assert_eq!(widget.transcript.active_cell_layout.get(), None);
    widget.transcript.active_cell = Some(active_cell);
    assert_eq!(widget.transcript.active_cell_revision, revision);

    render_frame(&widget, /*width*/ 80);
    assert_eq!(desired_height_calls.load(Ordering::Relaxed), 2);
}

#[tokio::test]
async fn external_writer_notice_offers_command_center_on_shared_servers() {
    let endpoint = crate::resolve_remote_addr("ws://127.0.0.1:4500").unwrap();
    for target in [
        crate::AppServerTarget::LocalDaemon {
            allow_embedded_fallback: true,
            endpoint: endpoint.clone(),
        },
        crate::AppServerTarget::Remote { endpoint },
    ] {
        let (mut widget, _sender, _events, _operations) =
            make_chatwidget_manual_with_sender().await;
        widget.remote_connection = crate::status::remote_connection::remote_connection_status_value(
            &target, /*server_version*/ None,
        );
        widget.show_external_writer_thread();
        for width in [60, 100] {
            let rendered = crate::chatwidget::tests::render_bottom_popup(&widget, width);
            insta::assert_snapshot!(format!("external_writer_command_center_{width}"), rendered);
        }
    }
}
