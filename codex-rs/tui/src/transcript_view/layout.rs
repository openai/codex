//! Width-specific layouts retained only for recently displayed conversation entries.
//!
//! Mutable cells refresh each frame, then share that frame's layout across measurement and paint.
//! Stable cells also invalidate when animation ticks, syntax themes, or terminal colors change.

use crate::history_cell::ActivityDisclosure;
use crate::key_hint::ShortcutHint;
use crate::keymap::KeymapContext;
use std::sync::Weak;

use super::*;

const MAX_CACHED_ENTRIES: usize = 64;
const MAX_CACHED_LAYOUT_BYTES: usize = 8 * 1024 * 1024;

/// The selected activity presentation and any live content that follows it.
pub(crate) struct ActivityTranscriptLines {
    pub(crate) activity: Vec<HyperlinkLine>,
    pub(crate) auxiliary: Vec<HyperlinkLine>,
    pub(crate) disclosure: Option<ActivityDisclosure>,
}

/// Keep disclosure controls outside selectable source without rendering hidden details.
pub(super) fn activity_layout(
    mut lines: ActivityTranscriptLines,
    width: u16,
    expanded: bool,
    shortcut: Option<ShortcutHint>,
) -> TextLayout {
    let has_details = !lines.activity.is_empty() && (expanded || lines.disclosure.is_some());
    let source_offset = (!lines.auxiliary.is_empty())
        .then(|| TextLayout::new(lines.activity.clone(), width).text().len());
    lines.activity.extend(lines.auxiliary);
    let layout = TextLayout::new(lines.activity, width);
    if !has_details {
        return layout;
    }
    let label = match (expanded, lines.disclosure) {
        (true, Some(ActivityDisclosure::OutputLines(_))) => "− Show less".to_owned(),
        (true, Some(ActivityDisclosure::Generic) | None) => "− Hide details".to_owned(),
        (false, Some(ActivityDisclosure::OutputLines(count))) => {
            let noun = if count == 1 { "line" } else { "lines" };
            let mut label = format!("+ {count} {noun}");
            if let Some(shortcut) = shortcut {
                let hint = format!(" ({} to expand)", shortcut.display_label());
                let indent = usize::from(width / 4).min(/*other*/ 4);
                if Line::from(format!("{label}{hint}")).width() + indent <= usize::from(width) {
                    label.push_str(&hint);
                }
            }
            label
        }
        (false, Some(ActivityDisclosure::Generic) | None) => "+ Show details".to_owned(),
    };
    match source_offset {
        Some(offset) => layout.with_disclosure_control_at(label, offset),
        None => layout.with_disclosure_control(label),
    }
}

fn cell_content_layout(
    cell: &dyn HistoryCell,
    width: u16,
    mode: HistoryRenderMode,
    presentation: CellPresentation,
    shortcut: Option<ShortcutHint>,
) -> TextLayout {
    let mut layout = if presentation.disclosure {
        activity_layout(
            ActivityTranscriptLines {
                activity: if presentation.expanded {
                    cell.expanded_hyperlink_lines(width)
                } else {
                    cell.compact_hyperlink_lines(width)
                },
                auxiliary: Vec::new(),
                disclosure: cell.activity_disclosure(width),
            },
            width,
            presentation.expanded,
            shortcut,
        )
    } else {
        TextLayout::new(
            crate::history_cell::fullscreen_session_lines(cell, width, presentation.detailed, mode),
            width,
        )
    };
    if let Some(protocol) = presentation.image_protocol
        && let Some(path) = cell.image_path()
    {
        layout.image =
            crate::transcript_image::TranscriptImage::load(path.as_path(), width, protocol)
                .map(Arc::new);
    }
    layout
}

fn is_rich_user_prompt(cell: &dyn HistoryCell, mode: HistoryRenderMode) -> bool {
    mode == HistoryRenderMode::Rich && cell.as_any().is::<crate::history_cell::UserHistoryCell>()
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
    cell_size: Option<(u32, u32)>,
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
    shares_prompt_background: bool,
    turn_tip_space: bool,
    expanded: bool,
    disclosure: bool,
    detailed: bool,
    image_protocol: Option<crate::pets::ImageProtocol>,
}

impl TranscriptView {
    fn content_presentation(
        &mut self,
        cell: &dyn HistoryCell,
        full_content: bool,
        mode: HistoryRenderMode,
    ) -> CellPresentation {
        let ids = cell.activity_ids();
        let disclosure = !self.detailed && mode == HistoryRenderMode::Rich && !ids.is_empty();
        let manually_expanded = self.disclosure.is_expanded(&ids);
        let reveal_image = ids.is_empty() || manually_expanded || self.detailed;
        if manually_expanded {
            self.disclosure.expanded.extend(ids);
        }
        CellPresentation {
            separated: false,
            shares_prompt_background: false,
            turn_tip_space: false,
            expanded: disclosure && (full_content || manually_expanded),
            disclosure,
            detailed: full_content,
            image_protocol: self
                .image_protocol
                .filter(|_| mode == HistoryRenderMode::Rich && reveal_image),
        }
    }

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
        if let Some(layout) = self.search.match_layout(key) {
            return Some(layout);
        }
        self.base_layout(cells, index)
    }

    /// Resolve manual disclosure and retained revisions, without Find's temporary expansion.
    /// Call with the snapshot's cells, if present.
    pub(super) fn base_layout(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        index: usize,
    ) -> Option<Arc<TextLayout>> {
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
        self.entry_layout(cells, index, self.detailed)
    }

    pub(super) fn entry_layout(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        index: usize,
        full_content: bool,
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
        let mode = self.mode;
        let mut presentation = self.content_presentation(cell.as_ref(), full_content, mode);
        let shortcut = self
            .disclosure
            .keymap
            .primary_hint(KeymapContext::Global, "open_transcript");
        let shares_prompt_background = is_rich_user_prompt(cell.as_ref(), mode)
            && (0..index)
                .rev()
                .find(|&previous| {
                    let previous = &cells[previous];
                    let key = EntryKey::cell(previous);
                    if let Some(layout) = self.search.match_layout(key).or_else(|| {
                        self.snapshot()
                            .and_then(|snapshot| snapshot.pinned.get(&key))
                            .cloned()
                    }) {
                        return layout.row_count() > 0;
                    }
                    let previous_presentation =
                        self.content_presentation(previous.as_ref(), self.detailed, mode);
                    self.cache
                        .has_visible_content(previous, width, previous_presentation, || {
                            cell_content_layout(
                                previous.as_ref(),
                                width,
                                mode,
                                previous_presentation,
                                shortcut,
                            )
                        })
                })
                .is_some_and(|previous| is_rich_user_prompt(cells[previous].as_ref(), mode));
        presentation.separated =
            index > 0 && !cell.is_stream_continuation() && !shares_prompt_background;
        presentation.shares_prompt_background = shares_prompt_background;
        presentation.turn_tip_space = self.turn_tip_key == Some(EntryKey::cell(cell));
        Some(self.cache.get(cell, width, presentation, || {
            let mut layout =
                cell_content_layout(cell.as_ref(), width, mode, presentation, shortcut);
            if shares_prompt_background {
                layout = layout.without_leading_blank_row();
            }
            layout
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
            cell_size: crate::transcript_image::terminal_cell_size(),
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
        let layout = if presentation.separated {
            layout.with_leading_separator()
        } else {
            layout
        };
        let layout = Arc::new(if presentation.turn_tip_space {
            layout.with_leading_spacer()
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

    fn has_visible_content(
        &mut self,
        cell: &Arc<dyn HistoryCell>,
        width: u16,
        presentation: CellPresentation,
        render: impl FnOnce() -> TextLayout,
    ) -> bool {
        let source = Arc::downgrade(cell);
        let animation_tick = cell.transcript_animation_tick();
        if let Some(index) = self.entries.iter().position(|entry| {
            entry.width == width
                && entry.presentation.expanded == presentation.expanded
                && entry.presentation.disclosure == presentation.disclosure
                && entry.presentation.detailed == presentation.detailed
                && entry.presentation.image_protocol == presentation.image_protocol
                && entry.source.ptr_eq(&source)
                && (entry.rendered_frame == self.frame
                    || (cell.has_stable_transcript_height()
                        && entry.animation_tick == animation_tick))
        }) {
            let visible = self.entries[index].layout.row_count() > 0;
            if index + 1 != self.entries.len() {
                let entry = self.entries.remove(index);
                self.entries.push(entry);
            }
            return visible;
        }
        self.get(cell, width, presentation, render).row_count() > 0
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }

    fn evict(&mut self) {
        let retained_bytes = |entry: &CachedLayout| {
            entry.layout.text().len()
                + entry
                    .layout
                    .image
                    .as_ref()
                    .map_or(0, |image| image.pixels.len())
        };
        let mut bytes = self.entries.iter().map(retained_bytes).sum::<usize>();
        // Retain a single oversized entry rather than repeatedly laying it out while visible.
        while self.entries.len() > 1
            && (self.entries.len() > MAX_CACHED_ENTRIES || bytes > MAX_CACHED_LAYOUT_BYTES)
        {
            bytes -= retained_bytes(&self.entries.remove(/*index*/ 0));
        }
    }
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
