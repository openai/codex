//! Goal summary and action menu for the bare `/goal` command.

use super::*;
use crate::goal_display::format_goal_elapsed_seconds;
use crate::status::format_tokens_compact;
use ratatui::widgets::Wrap;

impl ChatWidget {
    pub(crate) fn open_goal_menu(&mut self, thread_id: ThreadId, goal: AppThreadGoal) {
        let items = goal_menu_items(thread_id, goal.status);
        self.bottom_pane.show_selection_view(SelectionViewParams {
            header: Box::new(goal_menu_header(&goal)),
            items,
            footer_hint: Some(standard_popup_hint_line()),
            ..Default::default()
        });
        self.request_redraw();
    }

    pub(crate) fn on_thread_goal_cleared(&mut self, thread_id: &str) {
        if self
            .thread_id
            .is_some_and(|active_thread_id| active_thread_id.to_string() == thread_id)
        {
            self.current_goal_status = None;
            self.update_collaboration_mode_indicator();
        }
    }
}

fn goal_menu_items(thread_id: ThreadId, status: AppThreadGoalStatus) -> Vec<SelectionItem> {
    let mut items = Vec::new();
    match status {
        AppThreadGoalStatus::Active => items.push(goal_status_item(
            thread_id,
            "Pause goal",
            "Stop automatic goal continuation until you unpause it.",
            AppThreadGoalStatus::Paused,
        )),
        AppThreadGoalStatus::Paused => items.push(goal_status_item(
            thread_id,
            "Unpause goal",
            "Resume automatic goal continuation.",
            AppThreadGoalStatus::Active,
        )),
        AppThreadGoalStatus::BudgetLimited | AppThreadGoalStatus::Complete => {}
    }
    items.push(SelectionItem {
        name: "Clear goal".to_string(),
        description: Some("Remove the current goal.".to_string()),
        actions: vec![Box::new(move |tx| {
            tx.send(AppEvent::ClearThreadGoal { thread_id });
        })],
        dismiss_on_select: true,
        ..Default::default()
    });
    items.push(SelectionItem {
        name: "Cancel".to_string(),
        description: Some("Keep the goal unchanged.".to_string()),
        dismiss_on_select: true,
        ..Default::default()
    });
    items
}

fn goal_status_item(
    thread_id: ThreadId,
    name: &'static str,
    description: &'static str,
    status: AppThreadGoalStatus,
) -> SelectionItem {
    SelectionItem {
        name: name.to_string(),
        description: Some(description.to_string()),
        actions: vec![Box::new(move |tx| {
            tx.send(AppEvent::SetThreadGoalStatus { thread_id, status });
        })],
        dismiss_on_select: true,
        ..Default::default()
    }
}

fn goal_menu_header(goal: &AppThreadGoal) -> ColumnRenderable<'static> {
    let mut lines = vec![
        Line::from("Goal".bold()),
        Line::from(vec![
            "Status: ".dim(),
            goal_status_label(goal.status).to_string().into(),
        ]),
        Line::from(vec!["Objective: ".dim(), goal.objective.clone().into()]),
        Line::from(vec![
            "Time used: ".dim(),
            format_goal_elapsed_seconds(goal.time_used_seconds).into(),
        ]),
        Line::from(vec![
            "Tokens used: ".dim(),
            format_tokens_compact(goal.tokens_used).into(),
        ]),
    ];
    if let Some(token_budget) = goal.token_budget {
        lines.push(Line::from(vec![
            "Token budget: ".dim(),
            format_tokens_compact(token_budget).into(),
        ]));
    }
    ColumnRenderable::with([
        Box::new(Paragraph::new(lines).wrap(Wrap { trim: false })) as Box<dyn Renderable>
    ])
}

fn goal_status_label(status: AppThreadGoalStatus) -> &'static str {
    match status {
        AppThreadGoalStatus::Active => "active",
        AppThreadGoalStatus::Paused => "paused",
        AppThreadGoalStatus::BudgetLimited => "limited by budget",
        AppThreadGoalStatus::Complete => "complete",
    }
}
