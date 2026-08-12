use super::*;
use crate::history_cell::markdown_render_cache::MarkdownRenderCacheKey;
use assert_matches::assert_matches;
use pretty_assertions::assert_eq;

#[test]
fn sanitizer_borrows_clean_text_and_removes_control_sequences() {
    for (text, expected) in [
        ("clean\ttext\n", "clean\ttext\n"),
        ("\x07before", "before"),
        ("before\x07", "before"),
        ("\x1b[31mbefore", "before"),
        ("before\x1b[31m", "before"),
        ("before\x1b[31", "before"),
        ("\x07[31m", "[31m"),
        ("\x07", ""),
    ] {
        assert_matches!(
            sanitize_user_text(text.into()),
            Cow::Borrowed(sanitized) => assert_eq!(sanitized, expected)
        );
    }
    assert_matches!(
        sanitize_user_text("before\x1b[31mafter\x07".into()),
        Cow::Owned(sanitized) => assert_eq!(sanitized, "beforeafter")
    );
    assert_eq!(sanitize_user_text("é\u{85}中".into()), "é中");
    assert_eq!(sanitize_user_text("before\x1bafter".into()), "beforeafter");
}

#[test]
fn sanitizer_preserves_owned_buffer_for_clean_and_edge_trimmed_text() {
    for (text, expected) in [
        ("clean\ttext\n", "clean\ttext\n"),
        ("\x07before", "before"),
        ("before\x07", "before"),
        ("\x07before\x07", "before"),
        ("\x1b[31mbefore", "before"),
        ("before\x1b[31m", "before"),
        ("\x07", ""),
    ] {
        let owned = text.to_string();
        let original_pointer = owned.as_ptr();
        let original_capacity = owned.capacity();

        assert_matches!(sanitize_user_text(owned.into()), Cow::Owned(sanitized) => {
            assert_eq!(sanitized, expected);
            assert_eq!(sanitized.as_ptr(), original_pointer);
            assert_eq!(sanitized.capacity(), original_capacity);
        })
    }
}

#[test]
fn sanitizer_preallocates_owned_multi_fragment_text() {
    let text = "before\x1b[31mafter\x07".to_string();
    let original_length = text.len();

    assert_matches!(sanitize_user_text(text.into()), Cow::Owned(sanitized) => {
        assert_eq!(sanitized, "beforeafter");
        assert!(sanitized.capacity() >= original_length, "{} >= {}", sanitized.capacity(), original_length);
    })
}

fn replace_cached_lines(
    cell: &AgentMarkdownCell,
    update_key: impl FnOnce(&mut MarkdownRenderCacheKey),
) {
    let rendered_lines = cell
        .rendered_lines
        .as_ref()
        .expect("ordinary markdown should be cacheable");
    let mut rendered_lines = rendered_lines.cached.lock().expect("render cache lock");
    let (key, lines) = rendered_lines
        .as_mut()
        .expect("render cache should be populated");
    *lines = vec![HyperlinkLine::from("cached")];
    update_key(key);
}

#[test]
fn finalized_markdown_reuses_lines_primed_by_transcript_height() {
    let cell = AgentMarkdownCell::new("finalized **markdown**".to_string(), Path::new("/tmp"));
    let width = 48;

    assert_eq!(cell.desired_transcript_height(width), 1);
    replace_cached_lines(&cell, |_| {});

    assert_eq!(
        visible_lines(cell.transcript_hyperlink_lines(width)),
        vec![Line::from("cached")]
    );
}

#[test]
fn finalized_markdown_cache_misses_when_width_or_render_style_changes() {
    let cell = AgentMarkdownCell::new("finalized **markdown**".to_string(), Path::new("/tmp"));
    let width = 48;
    let expected = cell.display_lines(width);

    replace_cached_lines(&cell, |key| key.width = key.width.saturating_sub(1));
    assert_eq!(cell.display_lines(width), expected);

    replace_cached_lines(&cell, |key| {
        key.syntax_theme_revision = key.syntax_theme_revision.wrapping_sub(1);
    });
    assert_eq!(cell.display_lines(width), expected);

    replace_cached_lines(&cell, |key| {
        key.terminal_fg = key
            .terminal_fg
            .map_or(Some((1, 2, 3)), |(r, g, b)| Some((r ^ 1, g, b)));
    });
    assert_eq!(cell.display_lines(width), expected);
}

#[test]
fn raw_markdown_bypasses_the_rich_render_cache() {
    let source = "finalized **markdown**";
    let cell = AgentMarkdownCell::new(source.to_string(), Path::new("/tmp"));
    let width = 48;

    cell.display_lines(width);
    replace_cached_lines(&cell, |_| {});

    assert_eq!(
        cell.display_lines_for_mode(width, HistoryRenderMode::Raw),
        vec![Line::from(source)]
    );
}

#[test]
fn visualization_directives_are_not_cached() {
    for markdown in [
        "::codex-inline-vis{file=\"chart.html\"}",
        "\u{e200}visualize\u{e202}{\"path\":\"/tmp/chart.html\"}\u{e201}",
    ] {
        let cell = AgentMarkdownCell::new(markdown.to_string(), Path::new("/tmp"));

        cell.display_lines(/*width*/ 48);

        assert!(cell.rendered_lines.is_none());
    }
}
