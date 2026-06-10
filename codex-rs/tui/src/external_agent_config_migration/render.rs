use super::*;
use crate::key_hint;
use crate::line_truncation::truncate_line_with_ellipsis_if_overflow;
use crate::render::Insets;
use crate::render::RectExt as _;
use crate::selection_list::selection_option_row_with_dim;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;

impl ExternalAgentConfigMigrationScreen {
    fn render_items(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        let rows = self.build_render_lines();
        let visible_rows = area.height as usize;
        let mut start_idx = self.scroll_top.min(rows.len().saturating_sub(1));
        if let Some(selected_item_idx) = self.selected_item_idx {
            let selected_render_idx = self.selected_render_line_index(selected_item_idx);
            if selected_render_idx < start_idx {
                start_idx = selected_render_idx;
            } else if visible_rows > 0 {
                let bottom = start_idx + visible_rows - 1;
                if selected_render_idx > bottom {
                    start_idx = selected_render_idx + 1 - visible_rows;
                }
            }
        }

        let mut y = area.y;
        for entry in rows.iter().skip(start_idx).take(visible_rows) {
            if y >= area.y + area.height {
                break;
            }

            let selected =
                self.focus == FocusArea::Items && self.selected_item_idx == entry.item_idx;
            let mut line = entry.line.clone();
            if selected {
                if let Some(cursor) = line.spans.first_mut() {
                    cursor.content = "› ".into();
                }
                line.spans.iter_mut().for_each(|span| {
                    span.style = span.style.cyan().bold();
                });
            } else if entry.kind != RenderLineKind::Item && !line.spans.is_empty() {
                line.spans.iter_mut().for_each(|span| {
                    span.style = span.style.dim();
                });
            }
            let line = truncate_line_with_ellipsis_if_overflow(line, area.width as usize);
            line.render(
                Rect {
                    x: area.x,
                    y,
                    width: area.width,
                    height: 1,
                },
                buf,
            );
            y = y.saturating_add(1);
        }
    }
}

impl WidgetRef for &ExternalAgentConfigMigrationScreen {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);

        let inner_area = area.inset(Insets::vh(/*v*/ 1, /*h*/ 2));
        let error_height = u16::from(self.error.is_some());
        let intro_lines = match self.view {
            MigrationView::Summary => vec![
                Line::from("Bring over your setup, current project, and recent chats."),
                Line::from("Codex may add files to your current project folder."),
                Line::from("Your existing agent setup will not be changed."),
                Line::from("Cloud-hosted chat data cannot be imported."),
            ],
            MigrationView::Customize => vec![
                Line::from("Choose the items to import."),
                Line::from("Codex may add files to your current project folder."),
                Line::from("Your existing agent setup will not be changed."),
            ],
        };
        let intro_height = intro_lines.len() as u16;
        let actions = self.available_actions();
        let actions_height = actions.len() as u16 + 1;
        let fixed_height = 1u16 + intro_height + error_height + 1u16 + actions_height + 1u16;
        let list_height =
            self.render_line_count()
                .max(1)
                .min(inner_area.height.saturating_sub(fixed_height) as usize) as u16;
        let [
            header_area,
            intro_area,
            error_area,
            list_area,
            list_gap_area,
            actions_area,
            footer_area,
            _spacer_area,
        ] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(intro_height),
            Constraint::Length(error_height),
            Constraint::Length(list_height),
            Constraint::Length(1),
            Constraint::Length(actions_height),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .areas(inner_area);

        let title = match self.view {
            MigrationView::Summary => "Import from another coding agent",
            MigrationView::Customize => "Choose what to import",
        };
        let heading = Line::from(vec!["> ".into(), title.bold()]);
        heading.render(header_area, buf);

        Paragraph::new(intro_lines)
            .wrap(Wrap { trim: false })
            .render(intro_area, buf);

        if let Some(error) = &self.error {
            Paragraph::new(error.clone().red().to_string())
                .wrap(Wrap { trim: false })
                .render(error_area, buf);
        }

        self.render_items(list_area, buf);
        Clear.render(list_gap_area, buf);

        let action_areas = Layout::vertical(std::iter::repeat_n(
            Constraint::Length(1),
            actions.len() + 1,
        ))
        .split(actions_area);
        let item_label = if self.items.len() == 1 {
            "item"
        } else {
            "items"
        };
        let actions_intro = format!(
            "Selected {} of {} {item_label}.",
            self.selected_count(),
            self.items.len()
        );
        Paragraph::new(actions_intro)
            .wrap(Wrap { trim: false })
            .render(action_areas[0], buf);
        for (idx, action) in actions.iter().enumerate() {
            selection_option_row_with_dim(
                idx,
                action.label().to_string(),
                self.focus == FocusArea::Actions && self.highlighted_action == *action,
                /*dim*/ self.focus != FocusArea::Actions,
            )
            .render(action_areas[idx + 1], buf);
        }

        let footer = match self.view {
            MigrationView::Summary => Line::from(vec![
                "Use ".dim(),
                key_hint::plain(KeyCode::Up).into(),
                "/".dim(),
                key_hint::plain(KeyCode::Down).into(),
                " to move, ".dim(),
                key_hint::plain(KeyCode::Enter).into(),
                " to select, ".dim(),
                "c".cyan(),
                " to customize".dim(),
            ]),
            MigrationView::Customize if self.focus == FocusArea::Actions => Line::from(vec![
                "Press ".dim(),
                key_hint::plain(KeyCode::Enter).into(),
                " to continue, ".dim(),
                key_hint::plain(KeyCode::Up).into(),
                "/".dim(),
                key_hint::plain(KeyCode::Down).into(),
                " to move, ".dim(),
                "b".cyan(),
                " to go back".dim(),
            ]),
            MigrationView::Customize => Line::from(vec![
                "Use ".dim(),
                key_hint::plain(KeyCode::Up).into(),
                "/".dim(),
                key_hint::plain(KeyCode::Down).into(),
                " to move, ".dim(),
                key_hint::plain(KeyCode::Char(' ')).into(),
                " to toggle, ".dim(),
                "b".cyan(),
                " to go back".dim(),
            ]),
        };
        footer.render(footer_area, buf);
    }
}
