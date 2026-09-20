//! Grouped live tasks with shared identity colors and stable title, status and age columns.
//! Current-task labels and full titles share the flexible column; metadata drops at narrow widths.
//! The final cell stays blank inside the full-width selection highlight.

use super::navigation::CenterRow;
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
        let indices = self.visible_indices();
        let mut state = self.state();
        if indices.is_empty() {
            line(
                if state.connection_notice.is_some() {
                    "Reconnecting…"
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
        let entries = self.center_rows(&indices, state.grouping);
        let selected = entries
            .iter()
            .position(|row| matches!(row, CenterRow::Task(index) if *index == self.selected))
            .unwrap_or_default();
        let padding = u16::from(area.height >= 3);
        let viewport = row(area, padding, area.height - padding * 2);
        if padding > 0 {
            let (title, status, updated) = columns(row(area, /*offset*/ 0, /*height*/ 1));
            line(
                if !state.editing_metadata() {
                    "Tasks".cyan().bold()
                } else {
                    "Tasks".dim()
                },
                title,
                buf,
            );
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
                && matches!(entries[selected - 1], CenterRow::Group(_))
            {
                selected - 1
            } else {
                selected
            };
        }
        if selected >= start + height {
            start = selected.saturating_add(/*rhs*/ 1).saturating_sub(height);
        }
        state.scroll = start;
        for (offset, entry) in entries.iter().skip(start).take(height).enumerate() {
            let index = match entry {
                CenterRow::Group(index) | CenterRow::Task(index) => *index,
                CenterRow::Gap => continue,
            };
            let task = &self.rows[index];
            let rect = row(viewport, offset as u16, /*height*/ 1);
            if matches!(entry, CenterRow::Group(_)) {
                let group = match state.grouping {
                    AgentsOverviewGrouping::Project => {
                        format_directory_display(
                            &self.project_groups[index].heading,
                            /*max_width*/ None,
                        )
                    }
                    AgentsOverviewGrouping::Status => task.group.label().to_owned(),
                    AgentsOverviewGrouping::Model => model_name(&task.thread).to_owned(),
                };
                let count = indices
                    .iter()
                    .filter(|&&candidate| self.same_group(state.grouping, candidate, index))
                    .count();
                let total = (0..self.rows.len())
                    .filter(|&candidate| self.same_group(state.grouping, candidate, index))
                    .count();
                let count = if count == total {
                    count.to_string()
                } else {
                    format!("{count} of {total}")
                };
                let group = if state.grouping == AgentsOverviewGrouping::Project {
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
            let style = if index == self.selected {
                selection_style()
            } else {
                Style::default()
            };
            buf.set_style(
                Rect {
                    width: row_width,
                    ..rect
                },
                style,
            );
            let (status, dot) = Self::status(task);
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
            if task.is_current && title.width >= 14 {
                let badge = Rect::new(
                    title.right() - 10,
                    title.y,
                    /*width*/ 10,
                    /*height*/ 1,
                );
                title.width -= 10;
                line(Line::from("  current").style(style), badge, buf);
            }
            let title_style = if index == self.selected {
                style
            } else {
                self.title_style(task.thread_id)
            };
            line(
                Line::from(display_title(&task.thread).to_owned()).style(title_style),
                title,
                buf,
            );
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
