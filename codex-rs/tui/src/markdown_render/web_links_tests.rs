use super::WebLinkDisplay;
use crate::markdown_render::render_markdown_lines_with_width_cwd_and_hidden_link_destinations;
use crate::markdown_render::render_streaming_markdown_lines_with_width_and_cwd;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::TerminalHyperlink;
use crate::terminal_hyperlinks::visible_lines;
use codex_terminal_detection::Multiplexer;
use codex_terminal_detection::TerminalInfo;
use codex_terminal_detection::TerminalName;
use insta::assert_debug_snapshot;
use pretty_assertions::assert_eq;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Text;
use std::collections::BTreeSet;

fn terminal(name: TerminalName) -> TerminalInfo {
    TerminalInfo {
        name,
        term_program: None,
        version: None,
        term: None,
        multiplexer: None,
    }
}

fn render(markdown: &str, width: usize, display: WebLinkDisplay) -> Vec<HyperlinkLine> {
    render_markdown_lines_with_width_cwd_and_hidden_link_destinations(
        markdown,
        Some(width),
        /*cwd*/ None,
        &|destination| display.hide_destination(destination),
    )
}

#[test]
fn supporting_terminals_render_only_the_styled_label_and_keep_its_target() {
    for name in [
        TerminalName::Ghostty,
        TerminalName::Iterm2,
        TerminalName::WezTerm,
        TerminalName::Kitty,
        TerminalName::VsCode,
        TerminalName::Alacritty,
        TerminalName::WindowsTerminal,
        TerminalName::Konsole,
        TerminalName::GnomeTerminal,
        TerminalName::Vte,
    ] {
        let display = WebLinkDisplay::for_terminal(&terminal(name), /*term*/ None);
        for (markdown, label) in [
            ("[label](https://example.com)", "label".cyan().underlined()),
            (
                "[`label`](https://example.com)",
                "label".cyan().underlined(),
            ),
            (
                "[**label**](https://example.com)",
                "label".cyan().bold().underlined(),
            ),
            (
                "[*label*](https://example.com)",
                "label".cyan().italic().underlined(),
            ),
            ("[<b>](https://example.com)", "<b>".cyan().underlined()),
            (
                "[https://example.com](https://example.com)",
                "https://example.com".cyan().underlined(),
            ),
        ] {
            let label_width = label.width();
            assert_eq!(
                render(markdown, /*width*/ 80, display),
                vec![HyperlinkLine {
                    line: Line::from(label),
                    hyperlinks: vec![TerminalHyperlink::web(
                        0..label_width,
                        "https://example.com".into(),
                    )],
                }],
                "terminal: {name:?}"
            );
        }
    }
}

#[test]
fn unknown_terminals_and_multiplexers_keep_visible_destinations() {
    let mut tmux = terminal(TerminalName::Ghostty);
    tmux.multiplexer = Some(Multiplexer::Tmux { version: None });
    let mut zellij = terminal(TerminalName::Ghostty);
    zellij.multiplexer = Some(Multiplexer::Zellij { version: None });
    let markdown = "[label](https://example.com)";
    let expected = render(markdown, /*width*/ 80, WebLinkDisplay::WithDestination);
    for (terminal, term) in [
        (terminal(TerminalName::Unknown), None),
        (terminal(TerminalName::AppleTerminal), None),
        (terminal(TerminalName::Dumb), None),
        (tmux, None),
        (zellij, None),
        (terminal(TerminalName::Ghostty), Some("screen-256color")),
        (terminal(TerminalName::Ghostty), Some("tmux-256color")),
        (terminal(TerminalName::Ghostty), Some("dumb")),
    ] {
        assert_eq!(
            render(
                markdown,
                /*width*/ 80,
                WebLinkDisplay::for_terminal(&terminal, term),
            ),
            expected,
        );
    }
}

#[test]
fn label_only_policy_keeps_non_web_links_and_literal_urls_unchanged() {
    let markdown = "[file](./src/main.rs) [mail](mailto:a@example.com)\n\nhttps://example.com\n\n`https://example.com/code`";
    assert_eq!(
        render(markdown, /*width*/ 80, WebLinkDisplay::LabelOnly),
        render(markdown, /*width*/ 80, WebLinkDisplay::WithDestination),
    );
}

#[test]
fn empty_web_link_labels_keep_a_visible_destination() {
    for markdown in [
        "[](https://example.com)",
        "[ ](https://example.com)",
        "[\u{200d}](https://example.com)",
    ] {
        assert_eq!(
            render(markdown, /*width*/ 80, WebLinkDisplay::LabelOnly),
            render(markdown, /*width*/ 80, WebLinkDisplay::WithDestination),
        );
    }
}

#[test]
fn label_only_web_links_keep_targets_after_prose_and_table_wrapping() {
    let links = "[plain label](https://example.com/plain) [`code label`](https://example.com/code) [<b>](https://example.com/html)";
    for markdown in [
        links.to_string(),
        format!("| Links |\n| --- |\n| {links} |"),
    ] {
        for width in [12, 40] {
            let lines = render(&markdown, width, WebLinkDisplay::LabelOnly);
            let targets = lines
                .iter()
                .flat_map(|line| &line.hyperlinks)
                .map(|link| link.destination.as_str())
                .collect::<BTreeSet<_>>();
            assert_eq!(
                targets,
                BTreeSet::from([
                    "https://example.com/plain",
                    "https://example.com/code",
                    "https://example.com/html",
                ]),
            );
            assert!(
                lines
                    .iter()
                    .all(|line| !line.line.to_string().contains("https://"))
            );
        }
    }
}

#[test]
fn streaming_and_full_render_agree_with_label_only_links() {
    let markdown = "See [**label**](https://example.com).\n\n| Resource |\n| --- |\n| [`code`](https://example.com/code) |\n\nTrailing text.";
    let display = WebLinkDisplay::LabelOnly;
    for width in [12, 40] {
        for end in 1..=markdown.len() {
            let prefix = &markdown[..end];
            let streamed = render_streaming_markdown_lines_with_width_and_cwd(
                prefix,
                Some(width),
                /*cwd*/ None,
                &|destination| display.hide_destination(destination),
            );
            assert_eq!(streamed.lines, render(prefix, width, display));
        }
    }
}

#[test]
fn label_only_and_fallback_presentations_snapshot() {
    let markdown = "plain [plain](https://example.com) `code` [`code`](https://example.com)\n\n| Resource |\n| --- |\n| [docs](https://example.com/docs) |";
    let presentations = [WebLinkDisplay::LabelOnly, WebLinkDisplay::WithDestination]
        .map(|display| Text::from(visible_lines(render(markdown, /*width*/ 40, display))));
    assert_debug_snapshot!(presentations);
}
