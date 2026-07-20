//! Incremental markdown rendering for in-flight transcript streams.
//!
//! Completed top-level blocks are retained while the final block stays mutable, avoiding repeated
//! rendering of the stable prefix as newline-bearing deltas arrive.

use crate::history_cell::HistoryRenderMode;
use crate::history_cell::raw_lines_from_source;
use crate::inline_visualization::DIRECTIVE_PREFIX;
use crate::inline_visualization::InlineVisualizationContext;
use crate::markdown::render_markdown_agent_with_links_cwd_and_visualizations;
use crate::markdown::render_streaming_markdown_agent_with_links_and_cwd;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::plain_hyperlink_lines;
use ratatui::text::Line;
use std::path::Path;

/// Incremental render state split at source and rendered-line boundaries.
///
/// The prefix before both boundaries is immutable; only the final top-level Markdown block is
/// re-rendered as committed source arrives.
pub(super) struct StreamingRender {
    pub(super) lines: Vec<HyperlinkLine>,
    /// Source prefix containing only completed top-level markdown blocks.
    stable_source_len: usize,
    /// Rendered-line boundary corresponding to `stable_source_len`.
    stable_rendered_len: usize,
    /// Reference-style link definitions can affect any earlier or later markdown block.
    has_reference_link_definition: bool,
    /// Inline visualization directives require source-wide rewriting once one is committed.
    has_inline_visualization_directive: bool,
}

impl StreamingRender {
    pub(super) fn new() -> Self {
        Self {
            lines: Vec::with_capacity(64),
            stable_source_len: 0,
            stable_rendered_len: 0,
            has_reference_link_definition: false,
            has_inline_visualization_directive: false,
        }
    }

    pub(super) fn clear(&mut self) {
        self.lines.clear();
        self.stable_source_len = 0;
        self.stable_rendered_len = 0;
        self.has_reference_link_definition = false;
        self.has_inline_visualization_directive = false;
    }

    /// Re-render the full source and reset both stable-prefix boundaries.
    ///
    /// This is used when width or render mode changes, and whenever source-wide rendering state
    /// makes retaining previously rendered blocks unsafe.
    pub(super) fn recompute(
        &mut self,
        source: &str,
        width: Option<usize>,
        cwd: &Path,
        render_mode: HistoryRenderMode,
        inline_visualization_context: Option<&InlineVisualizationContext>,
    ) {
        self.has_inline_visualization_directive = source.contains(DIRECTIVE_PREFIX);
        self.lines = match (render_mode, inline_visualization_context) {
            (HistoryRenderMode::Rich, None) if !self.has_inline_visualization_directive => {
                let rendered =
                    render_streaming_markdown_agent_with_links_and_cwd(source, width, Some(cwd));
                self.has_reference_link_definition = rendered.has_reference_link_definition;
                rendered.lines
            }
            _ => {
                self.has_reference_link_definition = false;
                render_source(
                    source,
                    width,
                    cwd,
                    render_mode,
                    inline_visualization_context,
                )
            }
        };
        self.stable_source_len = 0;
        self.stable_rendered_len = 0;
    }

    /// Append newly committed source while retaining only the final markdown block as mutable.
    ///
    /// The final top-level block can still change meaning when another line arrives (for example,
    /// list tightness, a setext heading, a fenced block, or a table). Earlier top-level blocks are
    /// rendered once and retained. Reference-style link definitions and inline visualization
    /// rewriting fall back to a full render because they can affect source-wide rendering state.
    pub(super) fn append(
        &mut self,
        raw_source: &str,
        committed_source: &str,
        width: Option<usize>,
        cwd: &Path,
        render_mode: HistoryRenderMode,
        inline_visualization_context: Option<&InlineVisualizationContext>,
    ) {
        if render_mode == HistoryRenderMode::Raw {
            self.lines
                .extend(plain_hyperlink_lines(raw_lines_from_source(
                    committed_source,
                )));
            return;
        }

        self.has_inline_visualization_directive |= committed_source.contains(DIRECTIVE_PREFIX);
        if self.has_inline_visualization_directive {
            self.recompute(
                raw_source,
                width,
                cwd,
                render_mode,
                inline_visualization_context,
            );
            return;
        }

        if self.has_reference_link_definition {
            self.recompute(
                raw_source,
                width,
                cwd,
                render_mode,
                inline_visualization_context,
            );
            return;
        }

        let pending_source = &raw_source[self.stable_source_len..];
        let pending =
            render_streaming_markdown_agent_with_links_and_cwd(pending_source, width, Some(cwd));
        if pending.has_reference_link_definition {
            self.has_reference_link_definition = true;
            self.recompute(
                raw_source,
                width,
                cwd,
                render_mode,
                inline_visualization_context,
            );
            return;
        }

        let mut newly_stable_rendered_len = None;
        if let Some(boundary) = pending.last_top_level_block_start {
            let newly_stable_source = &pending_source[..boundary];
            let newly_stable = render_source(
                newly_stable_source,
                width,
                cwd,
                render_mode,
                inline_visualization_context,
            );
            self.stable_source_len += boundary;
            newly_stable_rendered_len = Some(newly_stable.len());
        }

        self.lines.truncate(self.stable_rendered_len);
        if !self.lines.is_empty()
            && (!pending.lines.is_empty() || !pending_source.trim().is_empty())
            && !pending.first_top_level_block_is_html
        {
            self.lines.push(HyperlinkLine::new(Line::default()));
        }
        let pending_render_start = self.lines.len();
        self.lines.extend(pending.lines);
        if let Some(newly_stable_rendered_len) = newly_stable_rendered_len {
            self.stable_rendered_len = pending_render_start + newly_stable_rendered_len;
        }
    }
}

pub(super) fn render_source(
    source: &str,
    width: Option<usize>,
    cwd: &Path,
    render_mode: HistoryRenderMode,
    inline_visualization_context: Option<&InlineVisualizationContext>,
) -> Vec<HyperlinkLine> {
    match render_mode {
        HistoryRenderMode::Rich => render_markdown_agent_with_links_cwd_and_visualizations(
            source,
            width,
            Some(cwd),
            inline_visualization_context,
        ),
        HistoryRenderMode::Raw => plain_hyperlink_lines(raw_lines_from_source(source)),
    }
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
