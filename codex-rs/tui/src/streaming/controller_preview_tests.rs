use super::*;
use crate::terminal_hyperlinks::visible_lines;
use pretty_assertions::assert_eq;

#[test]
fn unfinished_link_destinations_do_not_expand_the_preview() {
    let cwd = std::env::temp_dir();
    for destination in [
        "https://example.com/a/very/long/path/to/the/guide",
        "https://example.com/a_(nested_(path))",
        r"https://example.com/escaped\)parenthesis",
        "<https://example.com/unbalanced)parenthesis>",
        " <https://example.com/unbalanced)parenthesis>",
        "https://example.com/guide \"a title with ) parentheses\"",
        "https://example.com/guide (x < y)",
    ] {
        let mut controller = StreamController::new(Some(24), &cwd, HistoryRenderMode::Rich);
        controller.push("See [documentation]");
        let label = controller.current_tail_lines();
        // Split at every character, including the opening delimiter and escapes.
        for character in format!("({destination}").chars() {
            controller.push(&character.to_string());
            assert_eq!(controller.current_tail_lines(), label);
        }
        controller.push(") and continue.");
        assert_eq!(
            controller.current_tail_lines(),
            render_source(
                &format!("See [documentation]({destination}) and continue."),
                Some(24),
                &cwd,
                HistoryRenderMode::Rich,
                /*inline_visualization_context*/ None,
            ),
        );
    }
    // The preview's byte limit must not drop the opener and expose a long URL tail.
    let mut controller = StreamController::new(Some(24), &cwd, HistoryRenderMode::Rich);
    controller.push("See [documentation]");
    let label = controller.current_tail_lines();
    controller.push(&format!("(https://example.com/{}", "界".repeat(3000)));
    controller.push("more-path");
    assert_eq!(controller.current_tail_lines(), label);
}

#[test]
fn unfinished_links_flush_on_newline_or_completion() {
    let cwd = std::env::temp_dir();
    let source = "See [documentation](https://example.com/a/very/long/path";
    for ending in ["", "\n"] {
        let mut controller = StreamController::new(Some(24), &cwd, HistoryRenderMode::Rich);
        controller.push(source);
        insta::assert_debug_snapshot!(
            "unfinished_link_preview",
            visible_lines(controller.current_tail_lines()),
        );
        controller.set_width(Some(32));
        assert_eq!(
            visible_lines(controller.current_tail_lines()),
            vec![Line::from("See [documentation]")],
        );
        controller.push(ending);
        let (queued, _) = controller.on_commit_tick_batch(usize::MAX);
        let (remaining, finalized_source) = controller.finalize();
        let displayed = queued
            .into_iter()
            .chain(remaining)
            .flat_map(|cell| cell.display_lines(/*width*/ 34))
            .collect::<Vec<_>>();
        let expected = history_cell::AgentMessageCell::new_hyperlink_lines(
            render_source(
                source,
                Some(32),
                &cwd,
                HistoryRenderMode::Rich,
                /*inline_visualization_context*/ None,
            ),
            /*is_first_line*/ true,
        );
        assert_eq!(displayed, expected.display_lines(/*width*/ 34));
        assert_eq!(finalized_source, Some(format!("{source}\n")));
    }
}

#[test]
fn link_preview_respects_code_escapes_and_raw_mode() {
    let cwd = std::env::temp_dir();
    for source in [
        r"See \[label](https://example.com/unfinished",
        r"See [label\](https://example.com/unfinished",
        "See `[label](https://example.com/unfinished`",
        "See ``[la`bel](https://example.com/unfinished``",
        "See [label](https://example.com/complete) and prose",
    ] {
        let mut controller = StreamController::new(Some(24), &cwd, HistoryRenderMode::Rich);
        for character in source.chars() {
            controller.push(&character.to_string());
        }
        assert_eq!(
            controller.current_tail_lines(),
            render_source(
                source,
                Some(24),
                &cwd,
                HistoryRenderMode::Rich,
                /*inline_visualization_context*/ None,
            ),
            "{source}",
        );
    }
    let mut controller = StreamController::new(Some(24), &cwd, HistoryRenderMode::Rich);
    controller.push("See `[code](literal` then [label](https://example.com/unfinished");
    assert_eq!(
        controller.current_tail_lines(),
        render_source(
            "See `[code](literal` then [label]",
            Some(24),
            &cwd,
            HistoryRenderMode::Rich,
            /*inline_visualization_context*/ None,
        ),
    );

    let source = "See [label](https://example.com/unfinished";
    let mut controller = StreamController::new(Some(24), &cwd, HistoryRenderMode::Raw);
    controller.push(source);
    assert_eq!(
        controller.current_tail_lines(),
        render_source(
            source,
            Some(24),
            &cwd,
            HistoryRenderMode::Raw,
            /*inline_visualization_context*/ None,
        ),
    );
}

#[test]
fn link_holdback_stays_rich_only_when_a_pipe_freezes_the_preview() {
    let cwd = std::env::temp_dir();
    let source = "See [label](https://example.com";
    let mut controller = StreamController::new(Some(40), &cwd, HistoryRenderMode::Rich);
    controller.push(source);
    let label = controller.current_tail_lines();
    controller.push("|suffix)");
    assert_eq!(controller.current_tail_lines(), label);
    controller.set_render_mode(HistoryRenderMode::Raw);
    assert_eq!(
        controller.current_tail_lines(),
        render_source(
            source,
            Some(40),
            &cwd,
            HistoryRenderMode::Raw,
            /*inline_visualization_context*/ None,
        ),
    );
    controller.set_render_mode(HistoryRenderMode::Rich);
    assert_eq!(controller.current_tail_lines(), label);
}

#[test]
fn source_only_changes_refresh_the_preview_and_active_tail() {
    let cwd = std::env::temp_dir();
    let mut controller = StreamController::new(Some(5), &cwd, HistoryRenderMode::Rich);
    controller.push("hello");
    let before = controller.current_tail_lines();

    // An entity preserves trailing whitespace that Markdown otherwise trims at EOF.
    assert!(controller.push("&#32;"));
    let after = controller.current_tail_lines();
    assert_eq!(visible_lines(before.clone()), visible_lines(after.clone()));
    assert_eq!(
        after[0].source,
        render_source(
            "hello&#32;",
            Some(5),
            &cwd,
            HistoryRenderMode::Rich,
            /*inline_visualization_context*/ None,
        )[0]
        .source,
    );
    assert_ne!(
        history_cell::StreamingAgentTailCell::new(before.clone(), /*is_first_line*/ true),
        history_cell::StreamingAgentTailCell::new(after.clone(), /*is_first_line*/ true),
    );
    assert_ne!(
        history_cell::StreamingPlanTailCell::new(before, /*is_stream_continuation*/ false),
        history_cell::StreamingPlanTailCell::new(after, /*is_stream_continuation*/ false),
    );
}

#[test]
fn unterminated_prose_reflows_and_finishes_without_duplication() {
    let cwd = std::env::temp_dir();
    for ending in ["", "\n"] {
        let mut controller = StreamController::new(Some(32), &cwd, HistoryRenderMode::Rich);
        let mut source = String::from("`numbers` ");
        controller.push(&source);
        for number in 100..200 {
            let delta = format!("{number} ");
            source.push_str(&delta);
            controller.push(&delta);
            assert_eq!(controller.queued_lines(), 0);
            assert_eq!(
                controller.current_tail_lines(),
                render_source(
                    &source,
                    Some(32),
                    &cwd,
                    HistoryRenderMode::Rich,
                    /*inline_visualization_context*/ None
                ),
            );
        }

        controller.push(" | pending structure");
        controller.set_width(Some(48));
        assert_eq!(controller.queued_lines(), 0);
        assert_eq!(
            controller.current_tail_lines(),
            render_source(
                &source,
                Some(48),
                &cwd,
                HistoryRenderMode::Rich,
                /*inline_visualization_context*/ None
            ),
        );
        source.push_str(" | pending structure");
        controller.push(ending);
        let (queued, _) = controller.on_commit_tick_batch(usize::MAX);
        let (remaining, finalized_source) = controller.finalize();
        let displayed = queued
            .into_iter()
            .chain(remaining)
            .flat_map(|cell| cell.display_lines(/*width*/ 50))
            .collect::<Vec<_>>();
        let expected = history_cell::AgentMessageCell::new_hyperlink_lines(
            render_source(
                &source,
                Some(48),
                &cwd,
                HistoryRenderMode::Rich,
                /*inline_visualization_context*/ None,
            ),
            /*is_first_line*/ true,
        );
        assert_eq!(displayed, expected.display_lines(/*width*/ 50));
        assert_eq!(finalized_source, Some(format!("{source}\n")));
        assert!(!controller.has_live_tail());
    }
}

#[test]
fn partial_markdown_structures_remain_newline_gated() {
    let cwd = std::env::temp_dir();
    for (committed, partial) in [
        ("", "| partial header"),
        ("", "```rust"),
        ("", "> ~~~rust"),
        ("```rust\n", "let partial = 1;"),
        ("- ```rust\n", "  let partial = 1;"),
        ("> - ```rust\n", ">   let partial = 1;"),
        ("    indented code\n", "    partial code"),
        ("```markdown\n", "partial markdown fence"),
        ("| A | B |\n| --- | --- |\n", "partial row"),
    ] {
        let mut controller = StreamController::new(Some(40), &cwd, HistoryRenderMode::Rich);
        controller.push(committed);
        let tail = controller.current_tail_lines();
        let queued = controller.queued_lines();
        controller.push(partial);
        assert_eq!(controller.current_tail_lines(), tail);
        assert_eq!(controller.queued_lines(), queued);
    }
}

#[test]
fn long_unicode_preview_keeps_recent_text_and_finalizes_full_source() {
    let cwd = std::env::temp_dir();
    let mut controller = StreamController::new(Some(40), &cwd, HistoryRenderMode::Rich);
    controller.push(&"🦀 ".repeat(3000));
    controller.push("latest text");
    let preview = visible_lines(controller.current_tail_lines());
    assert_eq!(preview.first(), Some(&Line::from("…")));
    let text = preview
        .iter()
        .map(Line::to_string)
        .collect::<Vec<_>>()
        .join(" ");
    assert!(text.trim_end().ends_with("latest text"), "{text}");
    assert_eq!(controller.queued_lines(), 0);
    let (_, source) = controller.finalize();
    assert_eq!(source, Some(format!("{}latest text\n", "🦀 ".repeat(3000))));
}

#[test]
fn prose_preview_resumes_after_fenced_code() {
    let cwd = std::env::temp_dir();
    let mut controller = StreamController::new(Some(40), &cwd, HistoryRenderMode::Rich);
    controller.push("```rust\nlet x = 1;\n```\n");
    controller.on_commit_tick_batch(usize::MAX);
    controller.push("prose after code");
    assert_eq!(
        visible_lines(controller.current_tail_lines()),
        vec![Line::from("prose after code")],
    );
}

#[test]
fn prose_after_a_table_is_previewed_until_another_table_starts() {
    let cwd = std::env::temp_dir();
    let mut controller = StreamController::new(Some(40), &cwd, HistoryRenderMode::Rich);
    controller.push("| A | B |\n| --- | --- |\n| a | b |\n\n");
    controller.push("prose after the table");
    let tail = visible_lines(controller.current_tail_lines());
    assert_eq!(tail.last(), Some(&Line::from("prose after the table")));
    assert_eq!(controller.queued_lines(), 0);

    controller.push("\n\n| C | D |\n| --- | --- |\n");
    let tail = controller.current_tail_lines();
    controller.push("partial row");
    assert_eq!(controller.current_tail_lines(), tail);
}
