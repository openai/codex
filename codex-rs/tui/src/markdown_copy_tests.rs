//! Verify escaping and bounded copy annotations through the real renderer.

use super::*;
use crate::markdown_render::render_markdown_lines_with_width_and_cwd;
use pretty_assertions::assert_eq;
use std::path::Path;

#[test]
fn escaping_tracks_block_prefixes_without_rescanning_them() {
    for (text, expected) in [
        ("  - item", "  \\- item"),
        ("  123.)", "  123\\.)"),
        ("12\u{2003}.", "12\u{2003}\\."),
        ("12 3.", "12 3."),
        ("\u{2003}12\t)", "\u{2003}12\t\\)"),
        ("12.34)", "12\\.34)"),
        ("word - text", "word - text"),
        ("\u{2003}+", "\u{2003}\\+"),
    ] {
        assert_eq!(escape(text), expected);
    }
    let digits = "1".repeat(/*n*/ 20_000);
    let punctuation = ".".repeat(/*n*/ 20_000);
    assert_eq!(
        escape(&format!("{digits}{punctuation}")),
        format!("{digits}\\{punctuation}")
    );
}

#[test]
fn deeply_nested_lazy_continuations_keep_bounded_relative_copy_prefixes() {
    let depth = 256;
    let continuation_count = 256;
    let markdown = format!(
        "{}text\n{}",
        "- ".repeat(depth),
        "a\n".repeat(continuation_count)
    );
    let lines =
        render_markdown_lines_with_width_and_cwd(&markdown, /*width*/ None, /*cwd*/ None);
    let copies = lines
        .iter()
        .filter_map(|line| line.source.as_ref())
        .filter_map(|source| source.copy.as_ref())
        .collect::<Vec<_>>();
    let continuation = "  ".repeat(MAX_COPY_DEPTH);
    let item_prefix = format!("{}- ", "  ".repeat(MAX_COPY_DEPTH - 1));
    assert_eq!(copies.len(), continuation_count + 1);
    assert!(
        copies
            .iter()
            .all(|copy| copy.continuation == continuation && copy.item_prefix == item_prefix)
    );
    assert_eq!(copies[0].prefix, item_prefix);
    assert!(copies[1..].iter().all(|copy| copy.prefix == continuation));
}

#[test]
fn code_display_padding_does_not_displace_a_retained_container() {
    let depth = MAX_COPY_DEPTH + 1;
    let indent = "  ".repeat(depth);
    let markdown = format!(
        "{}before\n\n{indent}```\n{indent}code()\n{indent}```\n",
        "- ".repeat(depth)
    );
    let lines =
        render_markdown_lines_with_width_and_cwd(&markdown, /*width*/ None, /*cwd*/ None);
    let prefixes = lines
        .iter()
        .filter_map(|line| line.source.as_ref())
        .filter_map(|source| source.copy.as_ref())
        .filter(|copy| copy.code)
        .map(|copy| copy.prefix.clone())
        .collect::<Vec<_>>();
    assert_eq!(prefixes, vec!["  ".repeat(MAX_COPY_DEPTH)]);
}

#[test]
fn link_runs_share_destinations_and_bound_nested_formatting() {
    for destination in ["https://example.com/", "/repo/"] {
        let destination = format!("{destination}{}", "a".repeat(/*n*/ 4096));
        let label = format!(
            "**prefix** {}middle{}",
            "*a ".repeat(/*n*/ 128),
            " b*".repeat(/*n*/ 128)
        );
        let markdown = format!("[{label}](<{destination}>)");
        let lines = render_markdown_lines_with_width_and_cwd(
            &markdown,
            /*width*/ None,
            Some(Path::new("/")),
        );
        let runs = lines
            .iter()
            .filter_map(|line| line.source.as_ref())
            .filter_map(|source| source.copy.as_ref())
            .flat_map(|copy| &copy.runs)
            .map(|(_, inline)| inline)
            .collect::<Vec<_>>();
        assert!(runs.len() > 1);
        assert!(runs.iter().all(|inline| inline.len() <= 5));
        let links = runs
            .iter()
            .flat_map(|inline| inline.iter())
            .filter_map(|inline| {
                if let Inline::Link(destination) = inline {
                    Some(destination)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        if destination.starts_with('/') {
            assert!(links.is_empty());
            continue;
        }
        assert!(links.len() > 1);
        assert!(links.iter().all(|link| Arc::ptr_eq(links[0], link)));
    }
}
