//! Goal summary for the bare `/goal` command.

use super::*;
use crate::goal_display::format_goal_elapsed_seconds;
use crate::goal_files;
use crate::status::format_tokens_compact;

impl ChatWidget {
    pub(crate) fn show_goal_summary(&mut self, goal: AppThreadGoal) {
        self.add_plain_history_lines(goal_summary_lines(&goal));
    }

    pub(crate) fn show_goal_edit_prompt(
        &mut self,
        thread_id: ThreadId,
        goal: AppThreadGoal,
        objective: String,
    ) {
        let tx = self.app_event_tx.clone();
        let status = edited_goal_status(goal.status);
        let token_budget = goal.token_budget;
        let view = CustomPromptView::new(
            "Edit goal".to_string(),
            "Type a goal objective and press Enter".to_string(),
            objective,
            /*context_label*/ None,
            Box::new(move |objective: String| {
                tx.send(AppEvent::SetThreadGoalDraft {
                    thread_id,
                    draft: goal_files::GoalDraft {
                        objective,
                        ..Default::default()
                    },
                    mode: crate::app_event::ThreadGoalSetMode::UpdateExisting {
                        status,
                        token_budget,
                    },
                });
            }),
        );
        self.bottom_pane.show_view(Box::new(view));
    }

    pub(crate) fn show_resume_paused_goal_prompt(
        &mut self,
        thread_id: ThreadId,
        objective: String,
    ) {
        let subtitle = if let Some(path) = goal_files::objective_file_path(&objective) {
            format!("Goal file: {path}")
        } else {
            format!("Goal: {objective}")
        };
        let resume_actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
            tx.send(AppEvent::SetThreadGoalStatus {
                thread_id,
                status: AppThreadGoalStatus::Active,
            });
        })];
        self.show_selection_view(SelectionViewParams {
            title: Some("Resume paused goal?".to_string()),
            subtitle: Some(subtitle),
            footer_hint: Some(standard_popup_hint_line()),
            initial_selected_idx: Some(0),
            items: vec![
                SelectionItem {
                    name: "Resume goal".to_string(),
                    description: Some("Mark it active and continue when idle".to_string()),
                    actions: resume_actions,
                    dismiss_on_select: true,
                    ..Default::default()
                },
                SelectionItem {
                    name: "Leave paused".to_string(),
                    description: Some("Keep it paused; use /goal resume later".to_string()),
                    dismiss_on_select: true,
                    ..Default::default()
                },
            ],
            ..Default::default()
        });
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

fn goal_summary_lines(goal: &AppThreadGoal) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from("Goal".bold()),
        Line::from(vec![
            "Status: ".dim(),
            goal_status_label(goal.status).to_string().into(),
        ]),
        goal_objective_line(&goal.objective),
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
    let command_hint = match goal.status {
        AppThreadGoalStatus::Active => "Commands: /goal edit, /goal pause, /goal clear",
        AppThreadGoalStatus::Paused
        | AppThreadGoalStatus::Blocked
        | AppThreadGoalStatus::UsageLimited => "Commands: /goal edit, /goal resume, /goal clear",
        AppThreadGoalStatus::BudgetLimited | AppThreadGoalStatus::Complete => {
            "Commands: /goal edit, /goal clear"
        }
    };
    lines.push(Line::default());
    lines.push(Line::from(command_hint.dim()));
    lines
}

fn goal_objective_line(objective: &str) -> Line<'static> {
    if let Some(path) = goal_files::objective_file_path(objective) {
        Line::from(vec!["Objective file: ".dim(), path.into()])
    } else {
        Line::from(vec!["Objective: ".dim(), objective.to_string().into()])
    }
}

fn goal_status_label(status: AppThreadGoalStatus) -> &'static str {
    match status {
        AppThreadGoalStatus::Active => "active",
        AppThreadGoalStatus::Paused => "paused",
        AppThreadGoalStatus::Blocked => "blocked",
        AppThreadGoalStatus::UsageLimited => "usage limited",
        AppThreadGoalStatus::BudgetLimited => "limited by budget",
        AppThreadGoalStatus::Complete => "complete",
    }
}

fn edited_goal_status(status: AppThreadGoalStatus) -> AppThreadGoalStatus {
    match status {
        AppThreadGoalStatus::Active => AppThreadGoalStatus::Active,
        AppThreadGoalStatus::Paused
        | AppThreadGoalStatus::Blocked
        | AppThreadGoalStatus::UsageLimited => status,
        AppThreadGoalStatus::BudgetLimited | AppThreadGoalStatus::Complete => {
            AppThreadGoalStatus::Active
        }
    }
}
