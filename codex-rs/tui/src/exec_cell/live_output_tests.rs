use super::LIVE_COMMAND_OUTPUT_LINE_HEAD_BYTES;
use super::LIVE_COMMAND_OUTPUT_LINE_TAIL_BYTES;
use super::LIVE_COMMAND_OUTPUT_MAX_BYTES;
use super::LiveCommandOutput;
use codex_ansi_escape::ansi_escape_line;
use pretty_assertions::assert_eq;

#[test]
fn keeps_all_short_lines_and_chunk_boundaries_within_the_live_byte_budget() {
    let mut output = LiveCommandOutput::default();
    for line in 1..=500 {
        output.push_str(&format!("line {line}\n"));
    }
    for chunk in ["hell", "o\r", "\n\nwor", "ld"] {
        output.push_str(chunk);
    }

    let expected: Vec<_> = (1..=500)
        .map(|line| format!("line {line}"))
        .chain(["hello".to_string(), String::new(), "world".to_string()])
        .collect();

    assert_eq!(output.total_lines(), expected.len());
    assert_eq!(output.retained_lines(), expected.len());
    assert_eq!(
        output.transcript_lines().collect::<Vec<_>>(),
        expected.iter().map(String::as_str).collect::<Vec<_>>()
    );
}

#[test]
fn switches_to_bounded_storage_after_the_byte_budget_and_preserves_split_crlf() {
    let mut output = LiveCommandOutput::default();
    let line = "x".repeat(LIVE_COMMAND_OUTPUT_MAX_BYTES);
    output.push_str(&line);
    assert_eq!(output.transcript_lines().next().expect("full line"), line);

    output.push_str("y");
    let line = output.transcript_lines().next().expect("bounded line");
    assert!(line.contains("bytes omitted"));
    assert!(line.ends_with("xy"));

    for carriage_returns in ["\r", "\r\r"] {
        let body = "x".repeat(LIVE_COMMAND_OUTPUT_MAX_BYTES - carriage_returns.len());
        let mut contiguous = LiveCommandOutput::default();
        contiguous.push_str(&format!("{body}{carriage_returns}\n"));

        let mut split = LiveCommandOutput::default();
        split.push_str(&body);
        split.push_str(carriage_returns);
        assert!(
            split
                .transcript_lines()
                .next()
                .expect("partial line")
                .ends_with('\r')
        );
        split.push_str("\n");

        assert_eq!(split.total_lines(), 1);
        assert_eq!(split.retained_lines(), 1);
        assert_eq!(
            split.lines().collect::<Vec<_>>(),
            contiguous.lines().collect::<Vec<_>>()
        );
    }
}

#[test]
fn truncated_ansi_sequence_does_not_hide_the_retained_tail() {
    let mut output = LiveCommandOutput::default();
    let prefix = "x".repeat(LIVE_COMMAND_OUTPUT_LINE_HEAD_BYTES - 2);
    let osc = format!("{prefix}\x1b]0;{}\x07visible-tail", "hidden".repeat(4_000));
    output.push_str(&osc);

    let line = output.lines().next().expect("partial line");
    let rendered = ansi_escape_line(line.as_ref())
        .spans
        .into_iter()
        .map(|span| span.content.into_owned())
        .collect::<String>();

    assert!(rendered.contains("bytes omitted"), "{rendered}");
    assert!(rendered.ends_with("visible-tail"), "{rendered}");
    assert_eq!(
        output.transcript_lines().next().expect("transcript line"),
        osc
    );
}

#[test]
fn bounds_long_no_newline_output_and_preserves_utf8_head_and_tail() {
    let mut output = LiveCommandOutput::default();
    let chunk = "🦀".repeat(1024);
    for _ in 0..600 {
        output.push_str(&chunk);
    }

    let line = output.lines().next().expect("partial line");
    let retained_bytes = LIVE_COMMAND_OUTPUT_LINE_HEAD_BYTES + LIVE_COMMAND_OUTPUT_LINE_TAIL_BYTES
        - LIVE_COMMAND_OUTPUT_LINE_HEAD_BYTES % "🦀".len()
        - LIVE_COMMAND_OUTPUT_LINE_TAIL_BYTES % "🦀".len();

    assert_eq!(output.total_lines(), 1);
    assert_eq!(output.retained_lines(), 1);
    assert!(line.starts_with("🦀🦀🦀"));
    assert!(line.ends_with("🦀🦀🦀"));
    assert!(line.contains(&format!(
        "... {} bytes omitted ...",
        600 * chunk.len() - retained_bytes
    )));
    assert!(line.len() < LIVE_COMMAND_OUTPUT_MAX_BYTES);
}

#[test]
fn retained_output_stays_within_the_live_byte_budget() {
    let mut output = LiveCommandOutput::default();
    let body = "output ".repeat(4_000);
    for line in 1..=180 {
        output.push_str(&format!("head-{line} {body} tail-{line}\n"));
    }
    output.push_str(&format!("partial-head {body} partial-tail"));

    let lines: Vec<_> = output.lines().collect();
    assert_eq!(output.total_lines(), 181);
    assert_eq!(output.retained_lines(), 101);
    assert!(lines.first().expect("head line").starts_with("head-1 "));
    assert!(lines[50].starts_with("head-131 "));
    assert!(lines.last().expect("tail line").ends_with(" partial-tail"));
    assert_eq!(
        output.transcript_lines().nth(50).expect("omission line"),
        "… +80 lines"
    );
    assert!(lines.iter().map(|line| line.len()).sum::<usize>() <= LIVE_COMMAND_OUTPUT_MAX_BYTES);
}
