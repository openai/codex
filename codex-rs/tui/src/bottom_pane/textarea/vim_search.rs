//! Draft-local search motions and operator transactions. Query entry never edits the buffer;
//! accepted searches wrap, skip atomic elements, and record patterns rather than byte offsets.

use super::TextArea;
use super::TextAreaState;
use super::VimAction;
use super::VimEditTarget;
use super::VimOperator;
use super::VimPending;
use crate::key_hint::KeyBindingListExt;
use crate::keymap::KeymapContext;
use crate::keymap::KeymapContextSet;
use crate::vim_search::SearchDirection;
use crate::vim_search::SearchQuery;
use crate::vim_search::matching_ranges;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::widgets::StatefulWidgetRef;
use ratatui::widgets::Widget;
use std::ops::Range;

#[derive(Debug, Default)]
pub(super) struct VimSearch {
    input: Option<Box<SearchInput>>,
    last: SearchQuery,
}

/// Footer query editor, boxed by its owning textarea to keep the draft and query independent.
#[derive(Debug)]
pub(crate) struct SearchInput {
    pub(crate) editor: TextArea,
    direction: SearchDirection,
}

impl SearchInput {
    pub(crate) fn render(&self, area: Rect, buf: &mut Buffer) {
        let prefix = match self.direction {
            SearchDirection::Forward => "/",
            SearchDirection::Backward => "?",
        };
        prefix.cyan().render(area, buf);
        StatefulWidgetRef::render_ref(
            &&self.editor,
            Self::query_area(area),
            buf,
            &mut TextAreaState::default(),
        );
    }

    pub(crate) fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        self.editor.cursor_pos(Self::query_area(area))
    }

    fn query_area(area: Rect) -> Rect {
        Rect {
            x: area.x.saturating_add(1),
            width: area.width.saturating_sub(1),
            ..area
        }
    }
}

enum SearchCommand {
    Start(SearchDirection),
    Next,
    Previous,
}

impl TextArea {
    pub(crate) fn keymap_contexts(&self) -> KeymapContextSet {
        let context = self.keymap_context();
        let contexts = KeymapContextSet::new(context);
        if self.vim_search_enabled
            && matches!(
                context,
                KeymapContext::VimNormal | KeymapContext::VimOperator
            )
        {
            contexts.with(KeymapContext::VimSearch)
        } else {
            contexts
        }
    }

    pub(crate) fn enable_vim_search(&mut self) {
        self.vim_search_enabled = true;
    }
    pub(crate) fn vim_query(&self) -> Option<&SearchInput> {
        self.vim_search.input.as_deref()
    }
    pub(crate) fn vim_query_mut(&mut self) -> Option<&mut SearchInput> {
        self.vim_search.input.as_deref_mut()
    }
    pub(crate) fn cancel_vim_search(&mut self) -> bool {
        self.vim_search.input.take().is_some_and(|_| {
            self.vim_pending = VimPending::None;
            true
        })
    }

    pub(crate) fn wants_vim_search_key(&self, event: KeyEvent) -> bool {
        self.vim_search.input.is_some() || self.vim_search_command(event).is_some()
    }

    fn vim_search_command(&self, event: KeyEvent) -> Option<SearchCommand> {
        use SearchCommand::Start;

        if !self.vim_search_enabled
            || !self.is_vim_normal_mode()
            || !matches!(self.vim_pending, VimPending::None | VimPending::Operator(_))
        {
            return None;
        }
        let keys = &self.vim_search_keymap;
        [
            (&keys.forward, Start(SearchDirection::Forward)),
            (&keys.backward, Start(SearchDirection::Backward)),
            (&keys.next, SearchCommand::Next),
            (&keys.previous, SearchCommand::Previous),
        ]
        .into_iter()
        .find_map(|(bindings, command)| bindings.is_pressed(event).then_some(command))
    }

    pub(super) fn handle_vim_search_key(&mut self, event: KeyEvent) -> bool {
        let query = if let Some(mut input) = self.vim_search.input.take() {
            match (event.code, event.modifiers) {
                (KeyCode::Esc, _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                    self.vim_pending = VimPending::None;
                    return true;
                }
                (KeyCode::Backspace, KeyModifiers::NONE) if input.editor.is_empty() => {
                    self.vim_pending = VimPending::None;
                    return true;
                }
                (KeyCode::Enter, KeyModifiers::NONE) => {}
                _ => {
                    input.editor.input_with_keymap(event, &self.editor_keymap);
                    self.vim_search.input = Some(input);
                    return true;
                }
            }
            let text = if input.editor.is_empty() {
                self.vim_search.last.text.clone()
            } else {
                input.editor.text().to_owned()
            };
            let query = SearchQuery {
                text,
                direction: input.direction,
            };
            self.vim_search.last = query.clone();
            query
        } else {
            let Some(command) = self.vim_search_command(event) else {
                return false;
            };
            if let SearchCommand::Start(direction) = command {
                self.vim_search.input = Some(Box::new(SearchInput {
                    editor: TextArea::new(),
                    direction,
                }));
                return true;
            }
            let mut query = self.vim_search.last.clone();
            if matches!(command, SearchCommand::Previous) {
                query.direction = query.direction.reversed();
            }
            query
        };
        match std::mem::replace(&mut self.vim_pending, VimPending::None) {
            VimPending::Operator(VimOperator::Delete) => {
                self.start_vim_edit(VimAction::Delete(VimEditTarget::Search(query)));
            }
            VimPending::Operator(VimOperator::Change) => {
                self.start_vim_edit(VimAction::Change(VimEditTarget::Search(query)));
            }
            VimPending::Operator(VimOperator::Yank) => {
                self.apply_vim_search(&query, Some(VimOperator::Yank));
            }
            _ => {
                self.apply_vim_search(&query, /*operator*/ None);
            }
        }
        true
    }

    pub(super) fn apply_vim_search(
        &mut self,
        query: &SearchQuery,
        operator: Option<VimOperator>,
    ) -> bool {
        let origin = self.cursor_pos;
        // Prefer matches before wrapping; order backward matches from the end.
        let target = matching_ranges(&self.text, &query.text)
            .filter(|range| self.is_vim_command_target(range.start))
            .map(|range| range.start)
            .min_by_key(|&pos| match query.direction {
                SearchDirection::Forward => (pos <= origin, pos),
                SearchDirection::Backward => (pos >= origin, usize::MAX - pos),
            });
        let Some(target) = target else {
            return false;
        };
        if let Some(operator) = operator {
            if origin == target {
                return false;
            }
            let mut range = origin.min(target)..origin.max(target);
            let line_start = self.beginning_of_line(range.start);
            let ends_at_line_start = range.end == self.beginning_of_line(range.end);
            // Exclusive motions ending at column zero may become linewise.
            let linewise =
                ends_at_line_start && self.text[line_start..range.start].trim().is_empty();
            if linewise {
                range.start = line_start;
            } else if ends_at_line_start {
                range.end -= 1;
            }
            let start = range.start;
            if operator == VimOperator::Yank {
                self.set_cursor(start);
            }
            self.apply_vim_operator_to_range(operator, range);
            if linewise {
                self.kill_buffer_kind = super::KillBufferKind::Linewise;
                if operator == VimOperator::Change {
                    self.insert_str_at(start, "\n");
                    self.set_cursor(start);
                }
            }
        } else {
            self.set_cursor(target);
        }
        true
    }

    pub(crate) fn vim_search_highlights(&self) -> Vec<Range<usize>> {
        let query = match self.vim_query() {
            Some(input) => input.editor.text(),
            None => &self.vim_search.last.text,
        };
        matching_ranges(&self.text, query)
            .filter(|range| self.is_vim_command_target(range.start))
            .collect()
    }
}
