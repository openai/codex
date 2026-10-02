//! Semantic copy navigation over the selection's frozen transcript revision.
//! Targets retain their original payloads and leave surrounding context when revealed.

use super::*;
use crate::history_cell::AgentMarkdownCell;
use crate::history_cell::ProposedPlanCell;
use crate::history_cell::UserHistoryCell;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use std::ops::Range;

pub(super) struct CopyMode {
    _guard: Option<Arc<crate::copy_input_guard::CopyInputGuard>>,
    pub(super) response: Option<(EntryKey, Arc<str>)>,
    pub(super) whole: bool,
    pub(super) source: Option<String>,
    pub(super) label: &'static str,
}

impl TranscriptView {
    pub(crate) fn begin_copy_mode(
        &mut self,
        cells: &[Arc<dyn HistoryCell>],
        guard: Option<Arc<crate::copy_input_guard::CopyInputGuard>>,
    ) -> bool {
        self.jump_to_latest();
        self.hold_position();
        let latest = cells.iter().rposition(is_response);
        self.copy_mode = Some(CopyMode {
            _guard: guard,
            response: latest.and_then(|index| {
                let cell = &cells[index];
                let text = if cell.as_any().is::<AgentMarkdownCell>() {
                    // Raw lines retain this cell's visible Markdown after hidden directives
                    // were removed during consolidation, independently of the latest cache.
                    let markdown = cell
                        .raw_lines()
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("\n");
                    crate::markdown_render::followup_labels(&markdown).into_owned()
                } else {
                    cell.copy_source()?.to_owned()
                };
                Some((EntryKey::cell(cell), Arc::from(text)))
            }),
            whole: true,
            source: None,
            label: "Whole response",
        });
        if !self.select_copy_response(cells) {
            self.move_copy_target(cells, KeyCode::Up);
        }
        if self.selection.is_none() {
            self.jump_to_latest();
        }
        self.copy_mode.is_some()
    }

    pub(super) fn handle_copy_mode_key(
        &mut self,
        key: KeyEvent,
        cells: &[Arc<dyn HistoryCell>],
    ) -> Option<ViewAction> {
        self.copy_mode.as_ref()?;
        let code = match crate::key_hint::normalize_key_parts(key.code, key.modifiers) {
            (KeyCode::Char('k'), KeyModifiers::NONE) => KeyCode::Up,
            (KeyCode::Char('j'), KeyModifiers::NONE) => KeyCode::Down,
            (KeyCode::Char('g'), KeyModifiers::NONE) => KeyCode::Home,
            (KeyCode::Char('g'), KeyModifiers::SHIFT) => KeyCode::End,
            _ => key.code,
        };
        match code {
            KeyCode::Up | KeyCode::Down | KeyCode::Home | KeyCode::End => {
                self.move_copy_target(cells, code)
            }
            KeyCode::Esc => self.jump_to_latest(),
            _ if key.code == KeyCode::Enter || crate::text_selection::is_copy_key(key) => {
                return Some(ViewAction::CopyAndFollow(
                    self.selected_text(cells).unwrap_or_default(),
                ));
            }
            KeyCode::PageUp | KeyCode::PageDown => return self.handle_scroll_key(key, cells),
            _ => return Some(ViewAction::Changed),
        }
        Some(ViewAction::Changed)
    }

    fn move_copy_target(&mut self, cells: &[Arc<dyn HistoryCell>], direction: KeyCode) {
        let snapshot = self.snapshot_cells();
        let cells = snapshot.as_deref().unwrap_or(cells);
        self.normalize_selection(cells);
        if direction == KeyCode::End && self.select_copy_response(cells) {
            return;
        }
        let Some(mode) = &self.copy_mode else { return };
        let label = mode.label;
        let edge = matches!(direction, KeyCode::Home | KeyCode::End);
        let older = matches!(direction, KeyCode::Up | KeyCode::End);
        if mode.whole && !older && !edge {
            return;
        }
        let current = self
            .selection
            .as_ref()
            .filter(|_| !mode.whole && !edge)
            .map(|s| (s.start.index, s.start.offset..s.end.offset));
        let indices: Box<dyn Iterator<Item = usize>> = if older {
            Box::new((0..current.as_ref().map_or(cells.len(), |(index, _)| index + 1)).rev())
        } else {
            Box::new(current.as_ref().map_or(0, |(index, _)| *index)..cells.len())
        };
        for index in indices {
            let user = cells[index].as_any().downcast_ref::<UserHistoryCell>();
            if user.is_none() && !is_response(&cells[index]) {
                continue;
            }
            let Some(layout) = self.layout(cells, index) else {
                continue;
            };
            let mut ranges = if let Some(user) = user {
                user_ranges(layout.text(), &user.message)
            } else if self.mode == crate::history_cell::HistoryRenderMode::Raw {
                user_ranges(layout.text(), "")
                    .into_iter()
                    .filter(|(_, label, _)| *label != "User message")
                    .collect()
            } else {
                layout.copy_block_ranges()
            };
            ranges.sort_by_key(|(range, _, _)| (range.start, std::cmp::Reverse(range.end)));
            if let Some(source) = user
                .map(|user| user.message.as_str())
                .or_else(|| cells[index].copy_source())
            {
                let targets = crate::markdown::extract_copy_targets(source);
                let mut codes = targets
                    .iter()
                    .filter_map(|target| match target {
                        crate::markdown::CopyTarget::Code { content, .. } => Some(content),
                        crate::markdown::CopyTarget::Quote(_) => None,
                    })
                    .collect::<Vec<_>>();
                let mut depth = 0;
                let mut quotes = Vec::new();
                for (event, span) in pulldown_cmark::Parser::new(source).into_offset_iter() {
                    match event {
                        pulldown_cmark::Event::Start(pulldown_cmark::Tag::BlockQuote) => {
                            if depth == 0 {
                                quotes.push(crate::markdown_copy::quote_content(&source[span]));
                            }
                            depth += 1;
                        }
                        pulldown_cmark::Event::End(pulldown_cmark::TagEnd::BlockQuote) => {
                            depth -= 1
                        }
                        _ => {}
                    }
                }
                let normalized = |text: &str| {
                    crate::markdown::normalize_markdown_for_rendering(text)
                        .lines()
                        .map(str::trim_end)
                        .collect::<Vec<_>>()
                        .join("\n")
                        .trim()
                        .to_owned()
                };
                for (range, label, payload) in &mut ranges {
                    if *label == "Code block" {
                        // Match rendered text, not ordinal position: indented code and
                        // source-preserving tables need not have a fenced counterpart.
                        let displayed = payload.as_deref().unwrap_or(&layout.text()[range.clone()]);
                        if let Some(at) = codes
                            .iter()
                            .position(|code| normalized(code) == normalized(displayed))
                        {
                            *payload = Some(codes.remove(at).to_string());
                        }
                    } else if user.is_none()
                        && *label == "Blockquote"
                        && let Some(displayed) = payload.as_deref()
                        && let Some(at) = quotes.iter().position(|quote| {
                            let visible = quote
                                .split_inclusive('\n')
                                .map(|line| {
                                    crate::git_action_directives::strip_line_directives(line).0
                                })
                                .collect::<String>();
                            normalized(&visible) == normalized(displayed)
                                || normalized(quote) == normalized(displayed)
                        })
                    {
                        *payload = Some(
                            quotes
                                .remove(at)
                                .split_inclusive('\n')
                                .map(|line| {
                                    crate::git_action_directives::strip_line_directives(line).0
                                })
                                .collect(),
                        );
                    }
                }
            }
            let boundary =
                current
                    .as_ref()
                    .filter(|(at, _)| *at == index)
                    .and_then(|(_, range)| {
                        ranges.iter().position(|(candidate, candidate_label, _)| {
                            candidate == range && *candidate_label == label
                        })
                    });
            let target = if older {
                ranges[..boundary.unwrap_or(ranges.len())].last()
            } else {
                ranges.get(boundary.map_or(0, |at| at + 1))
            };
            if let Some((range, label, source)) = target {
                let range = range.clone();
                if let Some(mode) = &mut self.copy_mode {
                    mode.whole = false;
                    mode.label = label;
                    mode.source = source.clone();
                }
                self.select_copy_range(cells, index, range);
                return;
            }
        }
        if !older {
            self.select_copy_response(cells);
        } else if self.history.has_unloaded_history() {
            self.jump_to_entry(cells, /*index*/ 0);
        }
    }

    /// Keep fitting targets inside a central band, scrolling only to the nearest margin.
    /// Oversized targets show their beginning with context above rather than clipping it.
    pub(super) fn reveal_copy_context(&mut self, cells: &[Arc<dyn HistoryCell>]) {
        if self.copy_mode.is_none() {
            return;
        }
        let Some(selection) = &self.selection else {
            return;
        };
        let (start, end) = (selection.start, selection.end);
        let index = self.resolve(cells, start);
        let Some(layout) = self.layout(cells, index) else {
            return;
        };
        let first = layout.row_for_offset(start.offset);
        // Match block painting: whole-entry highlights include final padding.
        let finish = if start.offset == 0 && end.offset == layout.text().len() {
            end.offset
        } else {
            end.offset.saturating_sub(/*rhs*/ 1)
        };
        let last = layout.row_for_offset(finish);
        let height = usize::from(self.area.height);
        if height == 0 {
            return;
        }
        let target_height = last.saturating_sub(first) + 1;
        // Reserve a quarter of the viewport, reducing the margin for taller targets.
        let margin = (height / 4)
            .max(1)
            .min(height.saturating_sub(target_height) / 2);
        let bottom_margin = (height / 4)
            .max(1)
            .min(height.saturating_sub(target_height).div_ceil(2));
        let top = self.start(cells);
        let inner_top = self.move_rows(cells, top.0, top.1, margin as isize);
        let inner_bottom = self.move_rows(
            cells,
            top.0,
            top.1,
            height.saturating_sub(bottom_margin + 1) as isize,
        );
        if target_height < height && (index, first) >= inner_top && (index, last) <= inner_bottom {
            return;
        }
        let inset = if target_height >= height {
            height / 4
        } else if (index, first) < inner_top {
            margin
        } else {
            height - bottom_margin - target_height
        };
        let (index, row) = self.move_rows(cells, index, first, -(inset as isize));
        if let Some(layout) = self.layout(cells, index) {
            let offset = layout.position_at(row, /*column*/ 0);
            self.position = Position::Reading(Anchor {
                key: self.entry_key(cells, index),
                index,
                offset,
                row_bias: layout.row_for_offset(offset) as isize - row as isize,
            });
        }
    }

    fn select_copy_response(&mut self, cells: &[Arc<dyn HistoryCell>]) -> bool {
        let Some(mode) = &mut self.copy_mode else {
            return false;
        };
        let Some((key, _)) = &mode.response else {
            return false;
        };
        let Some(index) = cells.iter().position(|cell| EntryKey::cell(cell) == *key) else {
            return false;
        };
        mode.whole = true;
        mode.source = None;
        mode.label = "Whole response";
        let Some(layout) = self.layout(cells, index) else {
            return false;
        };
        self.select_copy_range(cells, index, 0..layout.text().len());
        true
    }
}

fn is_response(cell: &Arc<dyn HistoryCell>) -> bool {
    cell.as_any().is::<AgentMarkdownCell>() || cell.as_any().is::<ProposedPlanCell>()
}

fn user_ranges(text: &str, message: &str) -> Vec<(Range<usize>, &'static str, Option<String>)> {
    use pulldown_cmark::Event;
    use pulldown_cmark::Tag;
    use pulldown_cmark::TagEnd;
    let mut ranges = Vec::new();
    if !text.trim().is_empty() {
        ranges.push((
            0..text.len(),
            "User message",
            (!message.is_empty())
                .then(|| crate::history_cell::sanitize_user_text(message.into()).into_owned()),
        ));
    }
    let mut depth = 0;
    let mut code: Option<(Range<usize>, String)> = None;
    for (event, range) in pulldown_cmark::Parser::new(text).into_offset_iter() {
        match event {
            Event::Start(Tag::BlockQuote) => {
                if depth == 0 {
                    let source = message
                        .is_empty()
                        .then(|| crate::markdown_copy::quote_content(&text[range.clone()]));
                    ranges.push((range, "Blockquote", source));
                }
                depth += 1;
            }
            Event::End(TagEnd::BlockQuote) => depth -= 1,
            Event::Start(Tag::CodeBlock(_)) => {
                code = Some((range.start..range.start, String::new()))
            }
            Event::Text(text) => {
                if let Some((source, content)) = code.as_mut() {
                    content.push_str(&text);
                    if source.start == source.end {
                        source.start = range.start;
                    }
                    source.end = range.end;
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some((range, content)) = code.take().filter(|(range, _)| !range.is_empty()) {
                    ranges.push((range, "Code block", Some(content)));
                }
            }
            _ => {}
        }
    }
    ranges
}

#[cfg(test)]
#[path = "copy_mode_tests.rs"]
mod tests;
