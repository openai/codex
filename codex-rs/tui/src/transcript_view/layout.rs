//! Width-specific layouts retained only for recently displayed conversation entries.
//!
//! Mutable cells refresh each frame, then share that frame's layout across measurement and paint.
//! Stable cells also invalidate when animation ticks, syntax themes, or terminal colors change.

use std::sync::Weak;

use super::*;

const MAX_CACHED_ENTRIES: usize = 64;
const MAX_CACHED_TEXT_BYTES: usize = 8 * 1024 * 1024;

/// The selected activity presentation and any live content that follows it.
pub(crate) struct ActivityTranscriptLines {
    pub(crate) activity: Vec<HyperlinkLine>,
    pub(crate) auxiliary: Vec<HyperlinkLine>,
    pub(crate) has_hidden_details: bool,
}

/// Keep disclosure controls outside selectable source without rendering hidden details.
pub(super) fn activity_layout(
    mut lines: ActivityTranscriptLines,
    width: u16,
    expanded: bool,
) -> TextLayout {
    let has_details = !lines.activity.is_empty() && (expanded || lines.has_hidden_details);
    let source_offset = (!lines.auxiliary.is_empty())
        .then(|| TextLayout::new(lines.activity.clone(), width).text().len());
    lines.activity.extend(lines.auxiliary);
    let layout = TextLayout::new(lines.activity, width);
    if !has_details {
        return layout;
    }
    let label = if expanded {
        "− Show less"
    } else {
        "+ Show details"
    }
    .to_owned();
    match source_offset {
        Some(offset) => layout.with_disclosure_control_at(label, offset),
        None => layout.with_disclosure_control(label),
    }
}

#[derive(Default)]
pub(super) struct LayoutCache {
    entries: Vec<CachedLayout>,
    frame: u64,
    render_state: Option<RenderState>,
}

#[derive(PartialEq, Eq)]
struct RenderState {
    theme: u64,
    foreground: Option<(u8, u8, u8)>,
    background: Option<(u8, u8, u8)>,
    color_level: crate::terminal_palette::StdoutColorLevel,
}

struct CachedLayout {
    source: Weak<dyn HistoryCell>,
    width: u16,
    presentation: CellPresentation,
    layout: Arc<TextLayout>,
    rendered_frame: u64,
    animation_tick: Option<u64>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct CellPresentation {
    separated: bool,
    expanded: bool,
    disclosure: bool,
}

impl TranscriptView {
    pub(super) fn layout(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        index: usize,
    ) -> Option<Arc<TextLayout>> {
        let snapshot = self.snapshot_cells();
        let cells = snapshot.as_deref().unwrap_or(cells);
        if index > cells.len() {
            return None;
        }
        let key = self.entry_key(cells, index);
        if let Some(layout) = self
            .snapshot()
            .and_then(|snapshot| snapshot.pinned.get(&key))
        {
            return Some(Arc::clone(layout));
        }
        if index == cells.len() && self.snapshot().is_some() {
            return None;
        }
        self.current_layout(cells, index)
    }

    /// Resolve the current revision without a reader's retained snapshot.
    pub(super) fn current_layout(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        index: usize,
    ) -> Option<Arc<TextLayout>> {
        let Some(cell) = cells.get(index) else {
            if index != cells.len() {
                return None;
            }
            let live = self.live.as_ref()?;
            if cells.is_empty() || self.live_continuation {
                return Some(Arc::clone(live));
            }
            let width = self.area.width.max(/*other*/ 1);
            return Some(Arc::clone(self.live_separated.get_or_insert_with(|| {
                Arc::new(live.rewrap(width).with_leading_separator())
            })));
        };
        let width = self.area.width.max(/*other*/ 1);
        // Session information belongs before all history, including pages not loaded yet.
        // Pinned displayed revisions above remain visible; hidden headers never enter the cache.
        if self.history.has_unloaded_history()
            && cell.as_any().is::<crate::history_cell::SessionInfoCell>()
        {
            return Some(Arc::new(TextLayout::new(Vec::new(), width)));
        }
        let detailed = self.detailed;
        let mode = self.mode;
        let ids = cell.activity_ids();
        let disclosure = !detailed && mode == HistoryRenderMode::Rich && !ids.is_empty();
        let expanded = disclosure && self.disclosure.is_expanded(&ids);
        if expanded {
            self.disclosure.expanded.extend(ids);
        }
        let separated = index > 0 && !cell.is_stream_continuation();
        let presentation = CellPresentation {
            separated,
            expanded,
            disclosure,
        };
        Some(self.cache.get(cell, width, presentation, || {
            if disclosure {
                activity_layout(
                    ActivityTranscriptLines {
                        activity: if expanded {
                            cell.expanded_hyperlink_lines(width)
                        } else {
                            cell.compact_hyperlink_lines(width)
                        },
                        auxiliary: Vec::new(),
                        has_hidden_details: cell.has_hidden_activity_details(width),
                    },
                    width,
                    expanded,
                )
            } else if detailed || mode == HistoryRenderMode::Rich {
                TextLayout::new(cell.retained_hyperlink_lines(width, detailed), width)
            } else {
                TextLayout::new(cell.display_hyperlink_lines_for_mode(width, mode), width)
            }
        }))
    }
}

impl LayoutCache {
    pub(super) fn begin_frame(&mut self) {
        self.frame = self.frame.wrapping_add(/*rhs*/ 1);
        let state = RenderState {
            theme: crate::render::highlight::syntax_theme_revision(),
            foreground: crate::terminal_palette::default_fg(),
            background: crate::terminal_palette::default_bg(),
            color_level: crate::terminal_palette::stdout_color_level(),
        };
        if self.render_state.as_ref() != Some(&state) {
            self.entries.clear();
            self.render_state = Some(state);
        }
    }

    pub(super) fn get(
        &mut self,
        cell: &Arc<dyn HistoryCell>,
        width: u16,
        presentation: CellPresentation,
        render: impl FnOnce() -> TextLayout,
    ) -> Arc<TextLayout> {
        let source = Arc::downgrade(cell);
        let animation_tick = cell.transcript_animation_tick();
        if let Some(index) = self.entries.iter().position(|entry| {
            entry.width == width
                && entry.presentation == presentation
                && entry.source.ptr_eq(&source)
                && (entry.rendered_frame == self.frame
                    || (cell.has_stable_transcript_height()
                        && entry.animation_tick == animation_tick))
        }) {
            let layout = Arc::clone(&self.entries[index].layout);
            if index + 1 != self.entries.len() {
                let entry = self.entries.remove(index);
                self.entries.push(entry);
            }
            return layout;
        }
        let layout = render();
        let layout = Arc::new(if presentation.separated {
            layout.with_leading_separator()
        } else {
            layout
        });
        self.entries.retain(|entry| !entry.source.ptr_eq(&source));
        self.entries.push(CachedLayout {
            source,
            width,
            presentation,
            layout: Arc::clone(&layout),
            rendered_frame: self.frame,
            animation_tick,
        });
        self.evict();
        layout
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }

    fn evict(&mut self) {
        let mut bytes = self
            .entries
            .iter()
            .map(|entry| entry.layout.text().len())
            .sum::<usize>();
        // Retain a single oversized entry rather than repeatedly laying it out while visible.
        while self.entries.len() > 1
            && (self.entries.len() > MAX_CACHED_ENTRIES || bytes > MAX_CACHED_TEXT_BYTES)
        {
            bytes -= self.entries.remove(/*index*/ 0).layout.text().len();
        }
    }
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
