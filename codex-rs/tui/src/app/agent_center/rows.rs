//! Grouped live tasks with shared identity colors and stable title, status and age columns.
//! Narrow lists put status and branch on a second row.
//! The final cell stays blank inside the full-width selection highlight.

use super::navigation::STATUS_COLUMNS_MIN_WIDTH;
use super::navigation::TaskListRow;
use super::navigation::visible_page;
use super::render::line;
use super::render::row;
use super::*;
use crate::bottom_pane::selection_style;
use crate::resume_picker::format_relative_time;
use crate::status::format_directory_display;

fn columns(area: Rect, widths: Option<(u16, u16)>) -> (Rect, Rect, Rect) {
    let gutter = area.width.min(/*other*/ 4);
    let (status_width, updated_width, gap) = match widths {
        Some((status, updated)) => (status, updated, 2),
        None => (0, 0, 0),
    };
    let updated = Rect::new(
        area.right() - updated_width,
        area.y,
        updated_width,
        area.height,
    );
    let status = Rect::new(
        updated.x - gap - status_width,
        area.y,
        status_width,
        area.height,
    );
    let title_x = area.x + gutter;
    let title = Rect::new(title_x, area.y, status.x - gap - title_x, area.height);
    (title, status, updated)
}

impl AgentsOverviewView {
    pub(super) fn render_center_rows(
        &self,
        area: Rect,
        buf: &mut Buffer,
        progress_in_footer: bool,
    ) {
        let row_width = area.width;
        let area = Rect {
            width: row_width.saturating_sub(/*rhs*/ 1),
            ..area
        };
        let indices = self.selectable_indices();
        let mut state = self.state();
        let padding = u16::from(area.height >= 3);
        let viewport = row(area, padding, area.height - padding * 2);
        state.list_width = viewport.width;
        state.page_height = usize::from(viewport.height);
        if area.is_empty() {
            return;
        }
        if indices.is_empty() {
            line(
                if state.connection_notice.is_some() {
                    "Reconnecting…"
                } else if state.history.is_some() {
                    "No matching tasks"
                } else if state.loading && self.rows.is_empty() {
                    if progress_in_footer {
                        ""
                    } else {
                        "Loading tasks…"
                    }
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
        let opening = state.opening_task.map(|task| task.thread_id);
        let status_for = |task: &AgentsOverviewRow| {
            let (status, dot) = Self::status(task);
            let label = if opening == Some(task.thread_id) {
                "Opening…"
            } else {
                status
            };
            (label, dot)
        };
        let entries = self.center_rows(&indices, &state);
        let reference = chrono::Utc::now();
        let age = |task: &AgentsOverviewRow| {
            format_relative_time(
                reference,
                chrono::DateTime::from_timestamp(task.thread.updated_at, /*nsecs*/ 0),
            )
        };
        let widths = (state.list_width >= STATUS_COLUMNS_MIN_WIDTH).then(|| {
            let mut status_width = "Status".width();
            let mut updated_width = "Updated".width();
            for task in indices.iter().filter_map(|&index| self.rows.get(index)) {
                status_width = status_width.max(status_for(task).0.width());
                updated_width = updated_width.max(age(task).width());
            }
            (status_width.min(11) as u16, updated_width.min(9) as u16)
        });
        if padding > 0 && self.dock_focused.is_none() {
            let (title, status, updated) = columns(row(area, /*offset*/ 0, /*height*/ 1), widths);
            line("Tasks".dim(), title, buf);
            line("Status".dim(), status, buf);
            Line::from("Updated".dim())
                .right_aligned()
                .render(updated, buf);
        }
        let (_, page) = visible_page(&entries, self.selected, &mut state);
        let has_previous = page.start > 0;
        let has_next = page.end < entries.len();
        for (offset, entry) in entries[page].iter().enumerate() {
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
                    let (status, _) = status_for(task);
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
            let (status, mut dot) = status_for(task);
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
            let (mut title, status_area, updated) = columns(rect, widths);
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
                Line::from(age(task))
                    .style(if index == self.selected {
                        style
                    } else {
                        style.dim()
                    })
                    .right_aligned()
                    .render(updated, buf);
            }
        }
        if padding > 0 && has_previous {
            line("↑".dim(), row(area, /*offset*/ 0, /*height*/ 1), buf);
        }
        if padding > 0 && has_next {
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
