use codex_core::config::Config;
use codex_core::config::types::UriBasedFileOpener;
use codex_core::features::Feature;
use codex_core::terminal::TerminalName;
use codex_core::terminal::terminal_info;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use std::fmt::Write as _;
use std::path::PathBuf;
use url::Url;

use crate::file_references::parse_local_file_reference_token;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TerminalHyperlinkSettings {
    pub(crate) enabled: bool,
    pub(crate) cwd: PathBuf,
    pub(crate) file_opener: UriBasedFileOpener,
    pub(crate) terminal_name: TerminalName,
}

impl TerminalHyperlinkSettings {
    pub(crate) fn from_config(config: &Config) -> Self {
        Self {
            enabled: config.features.enabled(Feature::TerminalHyperlinks),
            cwd: config.cwd.clone(),
            file_opener: config.file_opener,
            terminal_name: terminal_info().name,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct HyperlinkMatch {
    start: usize,
    end: usize,
    display_text: String,
    target: String,
}

pub(crate) fn linkify_lines(
    lines: &[Line<'static>],
    settings: &TerminalHyperlinkSettings,
) -> Vec<Line<'static>> {
    lines
        .iter()
        .map(|line| linkify_line(line, settings))
        .collect()
}

pub(crate) fn linkify_line(
    line: &Line<'static>,
    settings: &TerminalHyperlinkSettings,
) -> Line<'static> {
    if !settings.enabled {
        return line.clone();
    }

    let text: String = line
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect();
    let matches = detect_hyperlinks(&text, settings);
    if matches.is_empty() {
        return line.clone();
    }

    let mut spans = Vec::new();
    let mut cursor = 0usize;
    for hyperlink in &matches {
        push_original_range(line, cursor, hyperlink.start, &mut spans);
        spans.push(Span::styled(
            osc8_wrap(&hyperlink.display_text, &hyperlink.target),
            style_at_offset(line, hyperlink.start),
        ));
        cursor = hyperlink.end;
    }
    push_original_range(line, cursor, text.len(), &mut spans);

    Line::from(spans).style(line.style)
}

pub(crate) fn linkify_buffer_area(
    buf: &mut Buffer,
    area: Rect,
    settings: &TerminalHyperlinkSettings,
) {
    if !settings.enabled {
        return;
    }

    for y in area.top()..area.bottom() {
        let mut row_text = String::new();
        let mut cells = Vec::new();
        for x in area.left()..area.right() {
            let symbol = buf[(x, y)].symbol().to_string();
            let start = row_text.len();
            row_text.push_str(&symbol);
            let end = row_text.len();
            cells.push((x, start, end));
        }

        for hyperlink in detect_hyperlinks(&row_text, settings) {
            for (x, start, end) in &cells {
                if *start >= hyperlink.end || *end <= hyperlink.start {
                    continue;
                }
                let cell = &mut buf[(*x, y)];
                let symbol = cell.symbol().to_string();
                if symbol.trim().is_empty() || symbol.contains("\x1B]8;;") {
                    continue;
                }
                cell.set_symbol(&osc8_wrap(&symbol, &hyperlink.target));
            }
        }
    }
}

fn detect_hyperlinks(text: &str, settings: &TerminalHyperlinkSettings) -> Vec<HyperlinkMatch> {
    let mut matches = Vec::new();
    let mut index = 0usize;
    while index < text.len() {
        let Some(ch) = text[index..].chars().next() else {
            break;
        };
        if ch.is_whitespace() {
            index += ch.len_utf8();
            continue;
        }

        let token_start = index;
        index += ch.len_utf8();
        while index < text.len() {
            let Some(next) = text[index..].chars().next() else {
                break;
            };
            if next.is_whitespace() {
                break;
            }
            index += next.len_utf8();
        }

        let raw = &text[token_start..index];
        let (trim_leading, trim_trailing) = trimmed_token_offsets(raw);
        if trim_leading + trim_trailing >= raw.len() {
            continue;
        }

        let start = token_start + trim_leading;
        let end = index - trim_trailing;
        let token = &text[start..end];

        let Some((target, display_text)) =
            detect_url_target(token).or_else(|| detect_file_target(token, settings))
        else {
            continue;
        };

        if matches
            .last()
            .is_some_and(|previous: &HyperlinkMatch| previous.end > start)
        {
            continue;
        }

        matches.push(HyperlinkMatch {
            start,
            end,
            display_text,
            target,
        });
    }

    matches
}

fn detect_url_target(token: &str) -> Option<(String, String)> {
    let parsed = Url::parse(token).ok()?;
    matches!(parsed.scheme(), "http" | "https").then(|| (parsed.into(), token.to_string()))
}

fn detect_file_target(
    token: &str,
    settings: &TerminalHyperlinkSettings,
) -> Option<(String, String)> {
    let reference = parse_local_file_reference_token(token, &settings.cwd)?;
    if settings.terminal_name == TerminalName::VsCode {
        return None;
    }

    let file_url = Url::from_file_path(&reference.resolved_path).ok()?;
    match settings.file_opener.get_scheme() {
        Some(scheme) => {
            let suffix = file_url.as_str().strip_prefix("file://")?;
            let mut target = format!("{scheme}://file{suffix}");
            if let Some(line) = reference.line {
                let _ = write!(target, ":{line}");
                if let Some(col) = reference.col {
                    let _ = write!(target, ":{col}");
                }
            }
            Some((target, reference.display_text()))
        }
        None => Some((file_url.into(), reference.display_text())),
    }
}

fn trimmed_token_offsets(token: &str) -> (usize, usize) {
    let leading = token
        .chars()
        .take_while(|ch| matches!(ch, '(' | '[' | '{' | '<' | '"' | '\'' | '`'))
        .map(char::len_utf8)
        .sum();
    let trailing = token
        .chars()
        .rev()
        .take_while(|ch| {
            matches!(
                ch,
                ')' | ']' | '}' | '>' | '"' | '\'' | '`' | '.' | ',' | ';' | '!' | '?'
            )
        })
        .map(char::len_utf8)
        .sum();
    (leading, trailing)
}

fn osc8_wrap(text: &str, target: &str) -> String {
    let safe_target: String = target
        .chars()
        .filter(|&ch| ch != '\x1B' && ch != '\x07')
        .collect();
    if safe_target.is_empty() {
        return text.to_string();
    }
    format!("\x1B]8;;{safe_target}\x07{text}\x1B]8;;\x07")
}

fn push_original_range(
    line: &Line<'static>,
    start: usize,
    end: usize,
    spans: &mut Vec<Span<'static>>,
) {
    if start >= end {
        return;
    }

    let mut global_offset = 0usize;
    for span in &line.spans {
        let span_text = span.content.as_ref();
        let span_end = global_offset + span_text.len();
        let segment_start = start.max(global_offset);
        let segment_end = end.min(span_end);
        if segment_start < segment_end {
            spans.push(Span::styled(
                span_text[(segment_start - global_offset)..(segment_end - global_offset)]
                    .to_string(),
                span.style,
            ));
        }
        global_offset = span_end;
        if global_offset >= end {
            break;
        }
    }
}

fn style_at_offset(line: &Line<'static>, offset: usize) -> Style {
    let mut global_offset = 0usize;
    for span in &line.spans {
        let span_end = global_offset + span.content.len();
        if offset < span_end {
            return span.style;
        }
        global_offset = span_end;
    }

    line.spans
        .last()
        .map(|span| span.style)
        .unwrap_or(line.style)
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;
    use pretty_assertions::assert_eq;
    use ratatui::buffer::Buffer;
    use ratatui::style::Stylize;
    use regex_lite::Regex;
    use std::sync::LazyLock;

    static OSC8_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\x1B\]8;;[^\x07]*\x07|\x1B\]8;;\x07")
            .unwrap_or_else(|error| panic!("invalid osc8 regex: {error}"))
    });

    fn settings(cwd: PathBuf) -> TerminalHyperlinkSettings {
        TerminalHyperlinkSettings {
            enabled: true,
            cwd,
            file_opener: UriBasedFileOpener::VsCode,
            terminal_name: TerminalName::Unknown,
        }
    }

    fn strip_osc8(text: &str) -> String {
        OSC8_RE.replace_all(text, "").to_string()
    }

    #[test]
    fn linkify_line_wraps_http_url() {
        let settings = settings(PathBuf::from("/tmp"));
        let line = Line::from(vec!["See ".into(), "https://example.com/docs".cyan()]);

        let linked = linkify_line(&line, &settings);
        let text: String = linked
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        assert!(text.contains("\x1B]8;;https://example.com/docs\x07"));
    }

    #[test]
    fn linkify_line_wraps_local_file_reference() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("src/lib.rs");
        std::fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        std::fs::write(&path, "fn main() {}\n").expect("write file");

        let line = Line::from(vec!["src/lib.rs:1".cyan()]);
        let linked = linkify_line(&line, &settings(dir.path().to_path_buf()));
        let text: String = linked
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        assert!(text.contains("\x1B]8;;vscode://file"));
        assert!(text.contains("src/lib.rs:1"));
    }

    #[test]
    fn linkify_buffer_area_wraps_detected_token() {
        let settings = settings(PathBuf::from("/tmp"));
        let area = Rect::new(0, 0, 32, 1);
        let mut buf = Buffer::empty(area);
        buf.set_string(
            0,
            0,
            "https://example.com/docs",
            ratatui::style::Style::default(),
        );

        linkify_buffer_area(&mut buf, area, &settings);

        let rendered = buf[(0, 0)].symbol().to_string();
        assert!(rendered.contains("\x1B]8;;https://example.com/docs\x07"));
    }

    #[test]
    fn trims_surrounding_punctuation() {
        let settings = settings(PathBuf::from("/tmp"));
        let matches = detect_hyperlinks("(https://example.com/docs).", &settings);
        assert_eq!(matches.len(), 1);
        assert_eq!(&matches[0].target, "https://example.com/docs");
    }

    #[test]
    fn linkify_line_wraps_bare_filename_reference_from_cwd() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, "[package]\nname = \"demo\"\n").expect("write file");

        let line = Line::from(vec!["Cargo.toml:1".cyan()]);
        let linked = linkify_line(&line, &settings(dir.path().to_path_buf()));
        let text: String = linked
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        assert!(text.contains("\x1B]8;;vscode://file"));
        assert!(text.contains("Cargo.toml:1"));
    }

    #[test]
    fn linkify_line_shortens_long_workspace_paths() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir
            .path()
            .join("src/components/chat/composer/terminal_hyperlinks.rs");
        std::fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        std::fs::write(&path, "fn main() {}\n").expect("write file");

        let token = "src/components/chat/composer/terminal_hyperlinks.rs:1";
        let line = Line::from(vec![token.cyan()]);
        let linked = linkify_line(&line, &settings(dir.path().to_path_buf()));
        let text: String = linked
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        let visible = strip_osc8(&text);
        assert_eq!(visible, "src/.../composer/terminal_hyperlinks.rs:1");
    }

    #[test]
    fn linkify_line_uses_standard_file_uri_without_editor_opener() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("Cargo.toml");
        std::fs::write(&path, "[package]\nname = \"demo\"\n").expect("write file");

        let mut settings = settings(dir.path().to_path_buf());
        settings.file_opener = UriBasedFileOpener::None;

        let line = Line::from(vec!["Cargo.toml:1".cyan()]);
        let linked = linkify_line(&line, &settings);
        let text: String = linked
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        assert!(text.contains("\x1B]8;;file://"));
        assert!(text.contains("Cargo.toml:1"));
    }

    #[test]
    fn linkify_line_leaves_local_file_references_plain_in_vscode_terminal() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("src/lib.rs");
        std::fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        std::fs::write(&path, "fn main() {}\n").expect("write file");

        let mut settings = settings(dir.path().to_path_buf());
        settings.terminal_name = TerminalName::VsCode;

        let line = Line::from(vec!["src/lib.rs:1".cyan()]);
        let linked = linkify_line(&line, &settings);
        let text: String = linked
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        assert_eq!(text, "src/lib.rs:1");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn linkify_line_snapshot_captures_terminal_hyperlinks_output() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("src/lib.rs");
        std::fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        std::fs::write(&path, "fn main() {}\n").expect("write file");

        let line = Line::from(vec![
            "Open ".into(),
            "https://example.com/docs".cyan(),
            " and ".into(),
            "src/lib.rs:1".cyan(),
            ".".into(),
        ]);

        let linked = linkify_line(&line, &settings(dir.path().to_path_buf()));
        let text: String = linked
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        let normalized = text.replace(dir.path().to_string_lossy().as_ref(), "/workspace");

        assert_snapshot!("terminal_hyperlinks_linkify_line", normalized);
    }
}
