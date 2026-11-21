//! Section and page navigation share the rendered row layout, including group headings.
//! Navigation clamps at either end of the visible list.

use super::*;
use ratatui::layout::Position;

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

impl AgentsOverviewView {
    pub(super) fn center_rows(
        &self,
        indices: &[usize],
        grouping: AgentsOverviewGrouping,
    ) -> Vec<TaskListRow> {
        let mut entries = Vec::new();
        let mut previous = None;
        for index in indices.iter().copied() {
            if index == usize::MAX {
                entries.push(TaskListRow::Gap);
                entries.push(TaskListRow::ShowMore);
                continue;
            }
            if previous.is_none_or(|previous| !self.same_group(grouping, previous, index)) {
                if previous.is_some() {
                    entries.push(TaskListRow::Gap);
                }
                entries.push(TaskListRow::Group(index));
            }
            entries.push(TaskListRow::Task(index));
            if self.dock_focused.is_some() {
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
        let entries = self.center_rows(&indices, self.state().grouping);
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
        let forward = action == ListAction::PageDown;
        let entries = self.center_rows(&indices, state.grouping);
        let tasks = entries
            .iter()
            .enumerate()
            .filter_map(|(position, row)| Some((position, row.selection_index()?)))
            .collect::<Vec<_>>();
        let current = tasks
            .iter()
            .find(|(_, index)| *index == self.selected)
            .map(|(position, _)| *position)
            .unwrap_or(tasks[0].0);
        let height = state.page_height.max(/*other*/ 1);
        let target = if forward {
            current.saturating_add(height)
        } else {
            current.saturating_sub(height)
        };
        let selected = if forward {
            tasks
                .partition_point(|(position, _)| *position < target)
                .min(tasks.len() - 1)
        } else {
            tasks
                .partition_point(|(position, _)| *position <= target)
                .saturating_sub(1)
        };
        let selected = tasks[selected].1;
        let scroll = state.scroll;
        state.scroll = (if forward {
            scroll.saturating_add(height)
        } else {
            scroll.saturating_sub(height)
        })
        .min(entries.len().saturating_sub(height));
        drop(state);
        self.selected = selected;
        self.state().show_more_selected = selected == usize::MAX;
    }
}
