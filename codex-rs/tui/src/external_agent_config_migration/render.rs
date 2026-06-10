use super::ActionMenuOption;
use super::ExternalAgentConfigMigrationScreen;
use super::FocusArea;
use crate::key_hint;
use crate::render::Insets;
use crate::render::RectExt as _;
use crate::selection_list::selection_option_row_with_dim;
use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::prelude::Stylize as _;
use ratatui::text::Line;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;

impl WidgetRef for &ExternalAgentConfigMigrationScreen {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);

        let inner_area = area.inset(Insets::vh(/*v*/ 1, /*h*/ 2));
        let error_height = u16::from(self.error.is_some());
        let fixed_height = 1u16 + 2u16 + error_height + 1u16 + 4u16 + 1u16;
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
            Constraint::Length(2),
            Constraint::Length(error_height),
            Constraint::Length(list_height),
            Constraint::Length(1),
            Constraint::Length(4),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .areas(inner_area);

        let heading = Line::from(vec!["> ".into(), "External agent config detected".bold()]);
        heading.render(header_area, buf);

        Paragraph::new(vec![
            Line::from("We found settings from another agent that you can add to this project."),
            Line::from("Select what to import"),
        ])
        .wrap(Wrap { trim: false })
        .render(intro_area, buf);

        if let Some(error) = &self.error {
            Paragraph::new(error.clone().red().to_string())
                .wrap(Wrap { trim: false })
                .render(error_area, buf);
        }

        self.render_items(list_area, buf);
        Clear.render(list_gap_area, buf);

        let [
            actions_intro_area,
            proceed_area,
            skip_area,
            skip_forever_area,
        ] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(actions_area);
        let actions_intro = format!(
            "Selected {} of {} item(s).",
            self.selected_count(),
            self.items.len()
        );
        Paragraph::new(actions_intro)
            .wrap(Wrap { trim: false })
            .render(actions_intro_area, buf);
        selection_option_row_with_dim(
            /*index*/ 0,
            ActionMenuOption::Proceed.label().to_string(),
            self.focus == FocusArea::Actions
                && self.highlighted_action == ActionMenuOption::Proceed,
            /*dim*/ self.focus != FocusArea::Actions || !self.proceed_enabled(),
        )
        .render(proceed_area, buf);
        selection_option_row_with_dim(
            /*index*/ 1,
            ActionMenuOption::Skip.label().to_string(),
            self.focus == FocusArea::Actions && self.highlighted_action == ActionMenuOption::Skip,
            /*dim*/ self.focus != FocusArea::Actions,
        )
        .render(skip_area, buf);
        selection_option_row_with_dim(
            /*index*/ 2,
            ActionMenuOption::SkipForever.label().to_string(),
            self.focus == FocusArea::Actions
                && self.highlighted_action == ActionMenuOption::SkipForever,
            /*dim*/ self.focus != FocusArea::Actions,
        )
        .render(skip_forever_area, buf);

        Line::from(vec![
            "Use ".dim(),
            key_hint::plain(KeyCode::Up).into(),
            "/".dim(),
            key_hint::plain(KeyCode::Down).into(),
            " to move, ".dim(),
            key_hint::plain(KeyCode::Char(' ')).into(),
            " to toggle, ".dim(),
            "1".cyan(),
            "/".dim(),
            "2".cyan(),
            "/".dim(),
            "3".cyan(),
            " to choose, ".dim(),
            "a".cyan(),
            "/".dim(),
            "n".cyan(),
            " for all/none".dim(),
        ])
        .render(footer_area, buf);
    }
}
