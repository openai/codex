//! Section and page navigation share the rendered row layout, including group headings.
//! Navigation clamps at either end of the visible list.

use super::*;
use ratatui::layout::Position;
use std::ops::Range;

// The list leaves one column of padding at its right edge.
pub(super) const STATUS_COLUMNS_MIN_WIDTH: u16 = 57;

pub(super) enum TaskListRow {
    Group(usize),
    Task(usize),
    Context(usize),
    Gap,
    ShowMore,
}

pub(in crate::app::agents_overview_view) enum SectionDirection {
    Next,
    Previous,
}

impl TaskListRow {
    pub(super) fn selection_index(&self) -> Option<usize> {
        match self {
            Self::Task(index) => Some(*index),
            Self::ShowMore => Some(usize::MAX),
            Self::Group(_) | Self::Context(_) | Self::Gap => None,
        }
    }
}

pub(super) fn visible_page(
    entries: &[TaskListRow],
    selected_index: usize,
    state: &mut AgentsOverviewViewState,
) -> (usize, Range<usize>) {
    let selected = entries
        .iter()
        .position(|row| row.selection_index() == Some(selected_index))
        .unwrap_or_default();
    let height = state.page_height.max(1);
    let mut start = state.scroll.min(entries.len().saturating_sub(height));
    if selected <= start {
        start = selected;
        if height > 1 && selected > 0 && matches!(entries[selected - 1], TaskListRow::Group(_)) {
            start -= 1;
        }
    }
    let context = height > 1 && matches!(entries.get(selected + 1), Some(TaskListRow::Context(_)));
    start = start.max((selected + 1 + usize::from(context)).saturating_sub(height));
    state.scroll = start;
    (selected, start..(start + height).min(entries.len()))
}

impl AgentsOverviewView {
    pub(super) fn center_rows(
        &self,
        indices: &[usize],
        state: &AgentsOverviewViewState,
    ) -> Vec<TaskListRow> {
        let mut entries = Vec::new();
        let mut previous = None;
        for index in indices.iter().copied() {
            if index == usize::MAX {
                entries.push(TaskListRow::Gap);
                entries.push(TaskListRow::ShowMore);
                continue;
            }
            if previous.is_none_or(|previous| !self.same_group(state.grouping, previous, index)) {
                if previous.is_some() {
                    entries.push(TaskListRow::Gap);
                }
                entries.push(TaskListRow::Group(index));
            }
            entries.push(TaskListRow::Task(index));
            if state.list_width < STATUS_COLUMNS_MIN_WIDTH && state.page_height > 1 {
                entries.push(TaskListRow::Context(index));
            }
            previous = Some(index);
        }
        entries
    }

    pub(in crate::app::agents_overview_view) fn select_section(
        &mut self,
        direction: SectionDirection,
    ) {
        if self.state().rename_target.is_some() {
            return;
        }
        let indices = self.selectable_indices();
        let entries = self.center_rows(&indices, &self.state());
        let mut sections = Vec::new();
        let mut current = None;
        for row in entries {
            match row {
                TaskListRow::Group(index) => sections.push(index),
                TaskListRow::Task(index) if index == self.selected => {
                    current = sections.len().checked_sub(1)
                }
                TaskListRow::ShowMore if self.selected == usize::MAX => {
                    current = Some(sections.len())
                }
                TaskListRow::Task(_)
                | TaskListRow::Context(_)
                | TaskListRow::Gap
                | TaskListRow::ShowMore => {}
            }
        }
        let target = current.and_then(|current| match direction {
            SectionDirection::Next => current.checked_add(1),
            SectionDirection::Previous => current.checked_sub(1),
        });
        if let Some(index) = target.and_then(|target| sections.get(target)).copied() {
            self.selected = index;
            self.state().show_more_selected = false;
        }
    }

    pub(in crate::app) fn click_row(&mut self, point: Position) {
        let state = self.state();
        if state.rename_target.is_some()
            || state.help
            || state.removal.is_some()
            || state.creating_worktree
        {
            return;
        }
        let target = state
            .row_hits
            .iter()
            .find_map(|(target, rect)| rect.contains(point).then_some(*target));
        drop(state);
        let index = match target {
            Some(RowTarget::Thread(id)) => self
                .visible_indices()
                .into_iter()
                .find(|&index| self.rows[index].thread_id == id),
            Some(RowTarget::ShowMore) => self.state().more_available().then_some(usize::MAX),
            None => None,
        };
        if let Some(index) = index {
            self.selected = index;
            self.state().show_more_selected = index == usize::MAX;
            if self.state().connection_notice.is_none() {
                self.activate();
            }
        }
    }

    pub(in crate::app::agents_overview_view) fn page_selection(&mut self, action: ListAction) {
        let indices = self.selectable_indices();
        let mut state = self.state();
        if state.rename_target.is_some() || indices.is_empty() {
            return;
        }
        let entries = self.center_rows(&indices, &state);
        let (selected, page) = visible_page(&entries, self.selected, &mut state);
        let page_size = entries[page.clone()]
            .iter()
            .filter_map(TaskListRow::selection_index)
            .count()
            .max(1);
        let next = if action == ListAction::PageDown {
            let end = (page.end + page.len()).min(entries.len());
            // Keep a task's detail row with it when the next page ends at its title.
            let end = end - usize::from(matches!(entries.get(end), Some(TaskListRow::Context(_))));
            entries[selected + 1..end]
                .iter()
                .filter_map(TaskListRow::selection_index)
                .take(page_size)
                .last()
                .or_else(|| entries[end..].iter().find_map(TaskListRow::selection_index))
        } else {
            // Leave one row of overlap in case the destination needs its heading.
            let start = page
                .start
                .saturating_sub(state.page_height.saturating_sub(1));
            entries[start..selected]
                .iter()
                .rev()
                .filter_map(TaskListRow::selection_index)
                .take(page_size)
                .last()
                .or_else(|| {
                    entries[..start]
                        .iter()
                        .rev()
                        .find_map(TaskListRow::selection_index)
                })
        };
        if let Some(next) = next {
            // A second page key may arrive before we paint this selection.
            visible_page(&entries, next, &mut state);
            state.show_more_selected = next == usize::MAX;
            drop(state);
            self.selected = next;
        }
    }
}
