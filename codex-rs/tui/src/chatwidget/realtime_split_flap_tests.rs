//! Regressions for bounded state used by realtime voice presentation.
//! Retained history must preserve ordering without growing beyond its limits.

use super::*;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

#[derive(Debug)]
struct PlainTranscriptCell(String);

impl HistoryCell for PlainTranscriptCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        vec![Line::from(vec![Span::raw("› "), Span::raw(self.0.clone())])]
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        vec![Line::from(self.0.clone())]
    }
}

fn plain_transcript(text: &str) -> Box<dyn HistoryCell> {
    Box::new(PlainTranscriptCell(text.to_string()))
}

fn cell(text: &str, motion_mode: MotionMode) -> SplitFlapTranscriptCell {
    SplitFlapTranscriptCell::new(
        plain_transcript,
        "user",
        text,
        /*previous*/ None,
        /*discarded_prefix_bytes*/ 0,
        motion_mode,
        FrameRequester::test_dummy(),
    )
}

fn frame(cell: &SplitFlapTranscriptCell, milliseconds: u64) -> String {
    cell.animate_lines(
        cell.inner.display_hyperlink_lines(/*width*/ 16),
        /*width*/ 16,
        Duration::from_millis(milliseconds),
    )
    .into_iter()
    .map(|line| line.line.to_string().trim_end().to_string())
    .collect::<Vec<_>>()
    .join("\n")
}

#[test]
fn split_flap_frames_assemble_a_transcript_from_dark_tiles() {
    let board = cell("GATE 73", MotionMode::Animated);
    let frames = [0, 45, 90, 180, 285, 675]
        .into_iter()
        .map(|milliseconds| format!("{milliseconds:>3}ms {}", frame(&board, milliseconds)))
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!(frames, @r"
      0ms ›
     45ms › T
     90ms › ATGG AG
    180ms › GGTT ET GAT
    285ms › GATE 73 TEG
    675ms › GATE 73
    ");

    let punctuated = cell("HELLO, WORLD?!", MotionMode::Animated);
    assert!(!frame(&punctuated, /*milliseconds*/ 90).contains(','));
    assert!(!frame(&punctuated, /*milliseconds*/ 180).contains(','));
    assert_eq!(frame(&punctuated, /*milliseconds*/ 285), "› HELLO, WORLD?!");
    assert_ne!(
        frame(&board, /*milliseconds*/ 285),
        frame(&board, /*milliseconds*/ 330)
    );

    let lowercase = cell("please keep going", MotionMode::Animated);
    assert!(lowercase.flap_sample.iter().all(u8::is_ascii_lowercase));
    for elapsed in [90, 285] {
        assert!(
            frame(&lowercase, elapsed)
                .chars()
                .all(|ch| !ch.is_ascii_uppercase())
        );
    }
}

#[test]
fn split_flap_paints_the_entire_transcript_row_black() {
    let cell = cell("GATE", MotionMode::Animated);
    let mut source_lines = cell.inner.display_hyperlink_lines(/*width*/ 12);
    source_lines[0].source = Some(crate::terminal_hyperlinks::LogicalLineSource::from_line(
        &source_lines[0].line,
    ));
    let lines = cell.animate_lines(source_lines, /*width*/ 12, Duration::ZERO);
    assert!(lines[0].source.is_none());
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 12, /*height*/ 1,
    );
    let mut buffer = Buffer::empty(area);
    Paragraph::new(visible_lines(lines)).render(area, &mut buffer);

    assert!(buffer.content.iter().all(|tile| tile.bg == Color::Black));
    assert_eq!(buffer[(0, 0)].fg, Color::Cyan);
}

#[test]
fn settled_tiles_briefly_glow_in_the_speakers_color() {
    for (role, motion_mode, expected) in [
        ("user", MotionMode::Animated, Some(Color::Cyan)),
        ("assistant", MotionMode::Animated, Some(Color::Magenta)),
        ("user", MotionMode::Reduced, None),
    ] {
        let cell = SplitFlapTranscriptCell::new(
            plain_transcript,
            role,
            "A",
            /*previous*/ None,
            /*discarded_prefix_bytes*/ 0,
            motion_mode,
            FrameRequester::test_dummy(),
        );
        let tile_color = |elapsed| {
            cell.animate_lines(
                cell.inner.display_hyperlink_lines(/*width*/ 8),
                /*width*/ 8,
                elapsed,
            )[0]
            .line
            .spans
            .iter()
            .find(|span| span.content == "A")
            .and_then(|span| span.style.fg)
        };

        assert_eq!(tile_color(TILE_SETTLE_DURATION), expected);
        assert_eq!(
            tile_color(TILE_SETTLE_DURATION + TILE_AFTERGLOW_DURATION),
            (motion_mode == MotionMode::Animated).then_some(Color::Gray)
        );
    }
}

#[test]
fn reduced_motion_keeps_the_original_transcript_and_requests_no_frames() {
    let (frame_requester, mut requests) = FrameRequester::test_channel();
    let cell = SplitFlapTranscriptCell::new(
        plain_transcript,
        "user",
        "GATE 73",
        /*previous*/ None,
        /*discarded_prefix_bytes*/ 0,
        MotionMode::Reduced,
        frame_requester,
    );

    assert_eq!(cell.display_lines(/*width*/ 16)[0].to_string(), "› GATE 73");
    assert_eq!(cell.transcript_animation_tick(), None);
    assert!(requests.try_recv().is_err());
}

#[test]
fn appended_transcript_keeps_the_previous_words_settled() {
    let mut first = cell("GATE", MotionMode::Animated);
    for arrival in &mut first.tile_arrivals {
        *arrival -= ANIMATION_DURATION;
    }
    let appended = SplitFlapTranscriptCell::new(
        plain_transcript,
        "user",
        "GATE 73",
        Some(&first),
        /*discarded_prefix_bytes*/ 0,
        MotionMode::Animated,
        FrameRequester::test_dummy(),
    );

    assert!(frame(&appended, /*milliseconds*/ 0).starts_with("› GATE "));
    assert_eq!(appended.phase_started_at, first.phase_started_at);
    assert_eq!(appended.tile_arrivals[..4], first.tile_arrivals);
}

#[test]
fn sliding_a_full_window_animates_only_new_tiles_even_when_text_repeats() {
    let text = "A".repeat(/*n*/ 1024);
    for motion_mode in [MotionMode::Animated, MotionMode::Reduced] {
        let mut first = cell(&text, motion_mode);
        for arrival in &mut first.tile_arrivals {
            *arrival -= ANIMATION_DURATION;
        }
        let (frame_requester, mut requests) = FrameRequester::test_channel();
        let shifted = SplitFlapTranscriptCell::new(
            plain_transcript,
            "user",
            &text,
            Some(&first),
            /*discarded_prefix_bytes*/ 2,
            motion_mode,
            frame_requester,
        );

        assert_eq!(shifted.tile_arrivals[..1022], first.tile_arrivals[2..]);
        assert_eq!(
            shifted.tile_arrivals[1022..],
            [
                shifted.started_at - FRAME_INTERVAL,
                shifted.started_at - FRAME_INTERVAL + Duration::from_millis(/*millis*/ 8),
            ]
        );
        assert_eq!(shifted.raw_lines(), first.raw_lines());
        assert_eq!(
            requests.try_recv().is_ok(),
            motion_mode == MotionMode::Animated
        );
        if motion_mode == MotionMode::Reduced {
            assert_eq!(
                shifted.display_lines(/*width*/ 16),
                first.inner.display_lines(/*width*/ 16)
            );
        }
    }
}

#[test]
fn retained_words_stay_visible_while_the_new_window_tail_flips() {
    let mut first = cell("OLD GATE", MotionMode::Animated);
    for arrival in &mut first.tile_arrivals {
        *arrival -= ANIMATION_DURATION;
    }
    let mut shifted = SplitFlapTranscriptCell::new(
        plain_transcript,
        "user",
        "GATE 73",
        Some(&first),
        /*discarded_prefix_bytes*/ 4,
        MotionMode::Animated,
        FrameRequester::test_dummy(),
    );
    assert_eq!(shifted.phase_started_at, first.phase_started_at);
    // Fix the shared phase for deterministic frames without changing tile arrivals.
    shifted.phase_started_at = shifted.started_at;
    let frames = [0, 180, 285, 675]
        .into_iter()
        .map(|milliseconds| format!("{milliseconds:>3}ms {}", frame(&shifted, milliseconds)))
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!(frames, @r"
      0ms › GATE E  GAT
    180ms › GATE 73 GAT
    285ms › GATE 73 TEG
    675ms › GATE 73
    ");
}

#[test]
fn sliding_uses_rendered_glyphs_instead_of_raw_byte_offsets() {
    let markdown = |text: &str| -> Box<dyn HistoryCell> {
        Box::new(crate::history_cell::AgentMarkdownCell::new_spoken(
            text.to_string(),
            std::path::Path::new("."),
        ))
    };
    for (render, text, discarded_bytes, discarded_tiles) in [
        (
            plain_transcript as fn(&str) -> Box<dyn HistoryCell>,
            "éGA 東京B",
            4,
            2,
        ),
        (markdown, "DROP [GATE](https://example.test) **73**", 5, 4),
    ] {
        let first = SplitFlapTranscriptCell::new(
            render,
            "user",
            text,
            /*previous*/ None,
            /*discarded_prefix_bytes*/ 0,
            MotionMode::Animated,
            FrameRequester::test_dummy(),
        );
        let target = format!("{} 9", &text[discarded_bytes..]);
        let shifted = SplitFlapTranscriptCell::new(
            render,
            "user",
            &target,
            Some(&first),
            discarded_bytes,
            MotionMode::Animated,
            FrameRequester::test_dummy(),
        );
        let retained_tiles = first.tile_arrivals.len() - discarded_tiles;

        assert_eq!(
            shifted.tile_arrivals[..retained_tiles],
            first.tile_arrivals[discarded_tiles..]
        );
        assert_eq!(shifted.raw_lines(), render(&target).raw_lines());
    }
}

#[test]
fn replaced_or_changed_speaker_windows_start_fresh() {
    let first = cell("éGATE", MotionMode::Animated);
    for (role, target, discarded_bytes) in [
        ("user", "éGATE", 6),
        ("user", "éGATE", 12),
        ("assistant", "GATE 73", 2),
        ("user", "OTHER", 2),
        ("user", "GATE", 1),
    ] {
        let replaced = SplitFlapTranscriptCell::new(
            plain_transcript,
            role,
            target,
            Some(&first),
            discarded_bytes,
            MotionMode::Animated,
            FrameRequester::test_dummy(),
        );

        assert_eq!(replaced.phase_started_at, replaced.started_at);
        assert!(
            replaced
                .tile_arrivals
                .iter()
                .all(|arrival| *arrival >= replaced.started_at)
        );
    }
}

#[test]
fn unicode_graphemes_and_terminal_width_are_preserved() {
    let text = "a\u{301} 東京 👩‍💻 B";
    let cell = cell(text, MotionMode::Animated);
    let lines = cell.animate_lines(
        cell.inner.display_hyperlink_lines(/*width*/ 24),
        /*width*/ 24,
        Duration::ZERO,
    );

    assert!(lines[0].line.to_string().contains("a\u{301} 東京 👩‍💻"));
    assert_eq!(lines[0].width(), 24);
    assert_eq!(cell.raw_lines()[0].to_string(), text);
}

#[test]
fn settled_split_flap_stops_scheduling_animation_frames() {
    let (frame_requester, mut requests) = FrameRequester::test_channel();
    let mut cell = SplitFlapTranscriptCell::new(
        plain_transcript,
        "user",
        "GATE",
        /*previous*/ None,
        /*discarded_prefix_bytes*/ 0,
        MotionMode::Animated,
        frame_requester,
    );
    assert!(requests.try_recv().is_ok());
    cell.started_at = Instant::now() - ANIMATION_DURATION;
    for arrival in &mut cell.tile_arrivals {
        *arrival -= ANIMATION_DURATION;
    }

    assert_eq!(cell.transcript_animation_tick(), None);
    assert_eq!(
        cell.display_lines(/*width*/ 12)[0].to_string().trim_end(),
        "› GATE"
    );
    assert!(requests.try_recv().is_err());
}
