//! Workspace-confined OSC 8 file links for spoken assistant transcript cells.

use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::TerminalHyperlink;
use crate::terminal_hyperlinks::TrustedWorkspaceFile;
use crate::width::display_width;
use ratatui::style::Modifier;
use ratatui::text::Span;
use std::path::Path;
use std::sync::Arc;

const MAX_ARTIFACT_CANDIDATES: usize = 16;

pub(super) fn annotate_spoken_artifacts(lines: &mut [HyperlinkLine], cwd: &Path) {
    let mut inspected = 0;
    let mut source_patches = Vec::new();
    'lines: for line in &mut *lines {
        let text = line
            .line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        let mut search_from = 0;
        for token in text.split_ascii_whitespace() {
            let Some(relative_start) = text[search_from..].find(token) else {
                continue;
            };
            let token_start = search_from + relative_start;
            search_from = token_start + token.len();
            let leading = token.trim_start_matches(['(', '[', '{', '<', '\'', '"', '`']);
            let candidate =
                leading.trim_end_matches([')', ']', '}', '>', '\'', '"', '`', ',', ';', '.', '!']);
            let Some(filename) = candidate.rsplit('/').next() else {
                continue;
            };
            if !filename.contains('.') {
                continue;
            }
            let mut path = candidate;
            for _ in 0..2 {
                if let Some((without_suffix, suffix)) = path.rsplit_once(':')
                    && !suffix.is_empty()
                    && suffix.bytes().all(|byte| byte.is_ascii_digit())
                {
                    path = without_suffix;
                }
            }
            inspected += 1;
            if inspected > MAX_ARTIFACT_CANDIDATES {
                break 'lines;
            }
            let Some(file) = TrustedWorkspaceFile::validate(cwd, path) else {
                continue;
            };
            let byte_start = token_start + token.len() - leading.len();
            let byte_end = byte_start + candidate.len();
            let columns = display_width(&text[..byte_start])..display_width(&text[..byte_end]);
            if line.hyperlinks.iter().any(|existing| {
                existing.columns.start < columns.end && columns.start < existing.columns.end
            }) {
                continue;
            }
            line.hyperlinks
                .push(TerminalHyperlink::trusted_workspace_file(columns, file));
            if let Some(source) = &line.source {
                let start = byte_start.max(source.prefix_bytes);
                let end = byte_end.min(source.prefix_bytes + source.range.len());
                if start < end {
                    source_patches.push((
                        Arc::clone(&source.text),
                        source.range.start + start - source.prefix_bytes
                            ..source.range.start + end - source.prefix_bytes,
                    ));
                }
            }

            let mut offset = 0;
            for span in std::mem::take(&mut line.line.spans) {
                let style = span.style;
                let content = span.content.into_owned();
                let start = byte_start.saturating_sub(offset).min(content.len());
                let end = byte_end.saturating_sub(offset).min(content.len());
                offset += content.len();
                if start >= end {
                    line.line.spans.push(Span::styled(content, style));
                    continue;
                }
                if start > 0 {
                    line.line
                        .spans
                        .push(Span::styled(content[..start].to_string(), style));
                }
                line.line.spans.push(Span::styled(
                    content[start..end].to_string(),
                    style.add_modifier(Modifier::UNDERLINED),
                ));
                if end < content.len() {
                    line.line
                        .spans
                        .push(Span::styled(content[end..].to_string(), style));
                }
            }
        }
        line.hyperlinks.sort_by_key(|link| link.columns.start);
    }

    // Every wrapped fragment must retain all annotations on its shared logical line.
    while let Some((text, range)) = source_patches.pop() {
        let mut ranges = vec![range];
        source_patches.retain(|(other, range)| {
            if Arc::ptr_eq(&text, other) {
                ranges.push(range.clone());
                false
            } else {
                true
            }
        });
        let mut sources = lines
            .iter_mut()
            .filter_map(|line| line.source.as_mut())
            .filter(|source| Arc::ptr_eq(&source.text, &text));
        let Some(first) = sources.next() else {
            continue;
        };
        let mut styles = Vec::new();
        for (span, style) in first.styles.iter() {
            let mut boundaries = vec![span.start, span.end];
            boundaries.extend(
                ranges
                    .iter()
                    .flat_map(|range| [range.start, range.end])
                    .filter(|offset| span.start < *offset && *offset < span.end),
            );
            boundaries.sort_unstable();
            boundaries.dedup();
            for pair in boundaries.windows(/*size*/ 2) {
                let range = pair[0]..pair[1];
                let style = if ranges
                    .iter()
                    .any(|patch| patch.start < range.end && range.start < patch.end)
                {
                    style.add_modifier(Modifier::UNDERLINED)
                } else {
                    *style
                };
                styles.push((range, style));
            }
        }
        first.styles = styles.into();
        for source in sources {
            source.styles = Arc::clone(&first.styles);
        }
    }
}
