//! Grouped live tasks with shared identity colors and stable title, status and age columns.
//! Full titles use the flexible column; metadata drops at narrow widths.
//! The final cell stays blank inside the full-width selection highlight.

use super::navigation::TaskListRow;
use super::render::line;
use super::render::row;
use super::*;
use crate::bottom_pane::selection_style;
use crate::resume_picker::format_relative_time;
use crate::status::format_directory_display;

fn columns(area: Rect) -> (Rect, Rect, Rect) {
    let metadata = if area.width >= 56 { 24 } else { 0 };
    let gutter = area.width.min(/*other*/ 4);
    let title = Rect::new(
        area.x + gutter,
        area.y,
        area.width - gutter - metadata,
        area.height,
    );
    let updated = Rect::new(
        area.right() - metadata.min(/*other*/ 9),
        area.y,
        metadata.min(/*other*/ 9),
        area.height,
    );
    let status = Rect::new(
        title.right() + metadata.min(/*other*/ 2),
        area.y,
        if metadata == 24 { 11 } else { 0 },
        area.height,
    );
    (title, status, updated)
}

impl AgentsOverviewView {
    pub(super) fn render_center_rows(&self, area: Rect, buf: &mut Buffer) {
        let row_width = area.width;
        let area = Rect {
            width: row_width.saturating_sub(/*rhs*/ 1),
            ..area
        };
        if area.is_empty() {
            return;
        }
        let indices = self.selectable_indices();
        let mut state = self.state();
        if indices.is_empty() {
            line(
                if state.connection_notice.is_some() {
                    "Reconnecting…"
                } else if state.history.is_some() {
                    "No matching tasks"
                } else if state.loading && self.rows.is_empty() {
                    "Loading tasks…"
                } else if state.refresh_failed {
                    "Could not load tasks"
                } else if self.rows.is_empty() {
                    "No tasks yet"
                } else {
                    "No matching tasks"
                }
                .dim(),
                area,
                buf,
            );
            return;
        }
        let selected_style = if self.dock_focused == Some(false) {
            crate::style::quiet_workspace_style()
        } else {
            selection_style()
        };
        let entries = self.center_rows(&indices, state.grouping);
        let selected = entries
            .iter()
            .position(|row| row.selection_index() == Some(self.selected))
            .unwrap_or_default();
        let padding = u16::from(area.height >= 3);
        let viewport = row(area, padding, area.height - padding * 2);
        if padding > 0 && self.dock_focused.is_none() {
            let (title, status, updated) = columns(row(area, /*offset*/ 0, /*height*/ 1));
            line("Tasks".dim(), title, buf);
            line("Status".dim(), status, buf);
            Line::from("Updated".dim())
                .right_aligned()
                .render(updated, buf);
        }
        let reference = chrono::Utc::now();
        let height = usize::from(viewport.height);
        state.page_height = height;
        let mut start = state.scroll.min(entries.len().saturating_sub(height));
        if selected < start {
            start = if height > 1
                && selected > 0
                && matches!(entries[selected - 1], TaskListRow::Group(_))
            {
                selected - 1
            } else {
                selected
            };
        }
        let last_selected_row =
            if height > 1 && matches!(entries.get(selected + 1), Some(TaskListRow::Context(_))) {
                selected + 1
            } else {
                selected
            };
        if last_selected_row >= start + height {
            start = last_selected_row.saturating_add(1).saturating_sub(height);
        }
        state.scroll = start;
        for (offset, entry) in entries.iter().skip(start).take(height).enumerate() {
            let rect = row(viewport, offset as u16, /*height*/ 1);
            let background = Rect {
                width: row_width,
                ..rect
            };
            let index = match entry {
                TaskListRow::Group(index) | TaskListRow::Task(index) => *index,
                TaskListRow::Context(index) => {
                    let task = &self.rows[*index];
                    state
                        .row_hits
                        .push((RowTarget::Thread(task.thread_id), background));
                    let style = if *index == self.selected {
                        selected_style
                    } else {
                        Style::default().dim()
                    };
                    buf.set_style(background, style);
                    let (status, _) = Self::status(task);
                    let branch = task
                        .thread
                        .git_info
                        .as_ref()
                        .and_then(|git| git.branch.as_deref());
                    let detail = match branch {
                        Some(branch) => format!("    {status} · {branch}"),
                        None => format!("    {status}"),
                    };
                    line(Line::from(detail).style(style), rect, buf);
                    continue;
                }
                TaskListRow::Gap => continue,
                TaskListRow::ShowMore => {
                    state.row_hits.push((RowTarget::ShowMore, background));
                    let selected = self.selected == usize::MAX;
                    let style = if selected {
                        selected_style
                    } else {
                        Style::default()
                    };
                    buf.set_style(background, style);
                    let label = if let Some(search) = &state.history {
                        if search.loading {
                            "Searching history…"
                        } else if search.failed {
                            "Show more (retry)"
                        } else {
                            "Show more matches"
                        }
                    } else if state.loading {
                        "Loading more…"
                    } else if state.refresh_failed {
                        "Show more (retry)"
                    } else {
                        "Show more"
                    };
                    let marker = if selected { "›" } else { " " };
                    line(
                        Line::from(format!("{marker}   {label}")).style(style),
                        rect,
                        buf,
                    );
                    continue;
                }
            };
            let task = &self.rows[index];
            if matches!(entry, TaskListRow::Group(_)) {
                let group = match state.grouping {
                    AgentsOverviewGrouping::Status => task.group.label().to_owned(),
                    _ if self.is_pinned(index) => "Pinned".to_owned(),
                    AgentsOverviewGrouping::Project => format_directory_display(
                        &self.project_groups[index].heading,
                        /*max_width*/ None,
                    ),
                    AgentsOverviewGrouping::Directory => format_directory_display(
                        self.rows[index].thread.cwd.as_path(),
                        /*max_width*/ None,
                    ),
                    AgentsOverviewGrouping::Model => model_name(&task.thread).to_owned(),
                };
                let count = indices
                    .iter()
                    .filter(|&&candidate| {
                        candidate != usize::MAX && self.same_group(state.grouping, candidate, index)
                    })
                    .count();
                let total = (0..self.rows.len())
                    .filter(|&i| state.includes_row(&self.rows[i]))
                    .filter(|&candidate| self.same_group(state.grouping, candidate, index))
                    .count();
                let count = if count == total {
                    count.to_string()
                } else {
                    format!("{count} of {total}")
                };
                let group = if matches!(
                    state.grouping,
                    AgentsOverviewGrouping::Project | AgentsOverviewGrouping::Directory
                ) && !self.is_pinned(index)
                {
                    crate::text_formatting::center_truncate_path(
                        &group,
                        usize::from(rect.width)
                            .saturating_sub(count.width() + 2)
                            .min(/*other*/ 64),
                    )
                } else {
                    group
                };
                line(format!("{group}  {count}").dim(), rect, buf);
                continue;
            }
            state
                .row_hits
                .push((RowTarget::Thread(task.thread_id), background));
            let style = if index == self.selected {
                selected_style
            } else {
                Style::default()
            };
            buf.set_style(background, style);
            let (status, mut dot) = Self::status(task);
            let idle = matches!(
                task.group,
                AgentsOverviewGroup::Ready | AgentsOverviewGroup::Finished
            );
            if task.unread || idle {
                dot = crate::style::task_marker(task.unread, style);
            } else if index == self.selected {
                dot.style = style;
            }
            line(
                Line::from(if index == self.selected { "›" } else { " " }).style(style),
                rect,
                buf,
            );
            line(
                Line::from(dot),
                Rect::new(
                    rect.x + rect.width.min(/*other*/ 2),
                    rect.y,
                    rect.width.saturating_sub(/*rhs*/ 2).min(/*other*/ 1),
                    /*height*/ 1,
                ),
                buf,
            );
            let (mut title, status_area, updated) = columns(rect);
            if task.has_voice && title.width >= 8 {
                let badge = Rect::new(
                    title.right() - 8,
                    title.y,
                    /*width*/ 8,
                    /*height*/ 1,
                );
                title.width -= 8;
                line(Line::from("  voice").style(style), badge, buf);
            }
            let title_style = if index == self.selected {
                style
            } else {
                self.title_style(task.thread_id)
            };
            let mut prefix = String::new();
            if state.grouping == AgentsOverviewGrouping::Status && self.is_pinned(index) {
                prefix.push_str("📌 ");
            }
            // On narrow rows the result dot shares the only slot for the working or alert icon.
            if task.unread && !idle && status_area.is_empty() {
                prefix.push_str(status);
                prefix.push_str(": ");
            }
            let prefix = Span::from(prefix);
            let task_title = Line::from(display_title(&task.thread).to_owned()).style(title_style);
            let mut task_title = task.label_style.render(
                task_title,
                usize::from(title.width).saturating_sub(prefix.width()),
            );
            if !prefix.content.is_empty() {
                task_title.spans.insert(0, prefix);
            }
            line(task_title, title, buf);
            line(Line::from(status).style(style), status_area, buf);
            if !updated.is_empty() {
                let age = format_relative_time(
                    reference,
                    chrono::DateTime::from_timestamp(task.thread.updated_at, /*nsecs*/ 0),
                );
                Line::from(age)
                    .style(if index == self.selected {
                        style
                    } else {
                        style.dim()
                    })
                    .right_aligned()
                    .render(updated, buf);
            }
        }
        if padding > 0 && start > 0 {
            line("↑".dim(), row(area, /*offset*/ 0, /*height*/ 1), buf);
        }
        if padding > 0 && start + height < entries.len() {
            line(
                "↓".dim(),
                row(
                    area,
                    area.height.saturating_sub(/*rhs*/ 1),
                    /*height*/ 1,
                ),
                buf,
            );
        }
    }
}
