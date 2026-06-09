use super::App;
use crate::app_event::AppEvent;
use crate::app_event::ThreadGoalSetMode;
use crate::app_server_session::AppServerSession;
use crate::bottom_pane::SelectionAction;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use crate::bottom_pane::popup_consts::standard_popup_hint_line;
use crate::goal_display::GOAL_USAGE;
use crate::goal_display::goal_status_label;
use crate::goal_display::goal_usage_summary;
use codex_app_server_protocol::ThreadGoal;
use codex_app_server_protocol::ThreadGoalStatus;
use codex_protocol::ThreadId;

const EPHEMERAL_THREAD_GOAL_ERROR_MESSAGE: &str = concat!(
    "Goals need a saved session. This session is temporary.\n",
    "Run `codex` to start a saved session, or `codex resume` / `/resume` to reopen one.",
);

impl App {
    pub(super) async fn open_thread_goal_menu(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
    ) {
        let result = app_server.thread_goal_get(thread_id).await;
        if self.current_displayed_thread_id() != Some(thread_id) {
            return;
        }

        let response = match result {
            Ok(response) => response,
            Err(err) => {
                self.chat_widget
                    .add_error_message(thread_goal_error_message("read", &err));
                return;
            }
        };

        let Some(goal) = response.goal else {
            self.chat_widget.add_info_message(
                GOAL_USAGE.to_string(),
                Some("No goal is currently set.".to_string()),
            );
            return;
        };

        self.chat_widget.show_goal_summary(goal);
    }

    pub(super) async fn maybe_prompt_resume_paused_goal_after_resume(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
    ) {
        let result = app_server.thread_goal_get(thread_id).await;
        if self.current_displayed_thread_id() != Some(thread_id) {
            return;
        }

        let response = match result {
            Ok(response) => response,
            Err(err) => {
                tracing::warn!("failed to read thread goal after resume: {err}");
                return;
            }
        };

        let Some(goal) = response.goal else {
            return;
        };
        if matches!(
            goal.status,
            ThreadGoalStatus::Paused | ThreadGoalStatus::Blocked | ThreadGoalStatus::UsageLimited
        ) {
            self.chat_widget
                .show_resume_paused_goal_prompt(thread_id, goal.objective);
        }
    }

    pub(super) async fn open_thread_goal_editor(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: Option<ThreadId>,
    ) {
        let Some(thread_id) = thread_id else {
            self.show_no_thread_goal_to_edit();
            return;
        };

        let result = app_server.thread_goal_get(thread_id).await;
        if self.current_displayed_thread_id() != Some(thread_id) {
            return;
        }

        let response = match result {
            Ok(response) => response,
            Err(err) => {
                self.chat_widget
                    .add_error_message(thread_goal_error_message("read", &err));
                return;
            }
        };

        let Some(goal) = response.goal else {
            self.show_no_thread_goal_to_edit();
            return;
        };

        self.chat_widget.show_goal_edit_prompt(thread_id, goal);
    }

    pub(super) async fn set_thread_goal_objective(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
        objective: String,
        mode: ThreadGoalSetMode,
    ) {
        let mode = if matches!(mode, ThreadGoalSetMode::ConfirmIfExists) {
            let result = app_server.thread_goal_get(thread_id).await;
            if self.current_displayed_thread_id() != Some(thread_id) {
                return;
            }

            match result {
                Ok(response) => match response.goal.as_ref() {
                    Some(goal) if should_confirm_before_replacing_goal(goal) => {
                        self.show_replace_thread_goal_confirmation(thread_id, objective);
                        return;
                    }
                    Some(_) => ThreadGoalSetMode::ReplaceExisting,
                    None => mode,
                },
                Err(err) => {
                    self.chat_widget
                        .add_error_message(thread_goal_error_message("read", &err));
                    return;
                }
            }
        } else {
            mode
        };

        let replacing_goal = matches!(mode, ThreadGoalSetMode::ReplaceExisting);
        if replacing_goal {
            let result = app_server.thread_goal_clear(thread_id).await;

            if let Err(err) = result {
                if self.current_displayed_thread_id() != Some(thread_id) {
                    return;
                }
                self.chat_widget
                    .add_error_message(thread_goal_error_message("replace", &err));
                return;
            }
        }

        let (status, token_budget) = match mode {
            ThreadGoalSetMode::ConfirmIfExists | ThreadGoalSetMode::ReplaceExisting => {
                (ThreadGoalStatus::Active, None)
            }
            ThreadGoalSetMode::UpdateExisting {
                status,
                token_budget,
            } => (status, Some(token_budget)),
        };

        let result = app_server
            .thread_goal_set(thread_id, Some(objective), Some(status), token_budget)
            .await;
        if self.current_displayed_thread_id() != Some(thread_id) {
            return;
        }

        match result {
            Ok(response) => self.chat_widget.add_info_message(
                format!("Goal {}", goal_status_label(response.goal.status)),
                Some(goal_usage_summary(&response.goal)),
            ),
            Err(err) => {
                let action = if replacing_goal { "replace" } else { "set" };
                self.chat_widget
                    .add_error_message(thread_goal_error_message(action, &err));
            }
        }
    }

    pub(super) async fn set_thread_goal_status(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
        status: ThreadGoalStatus,
    ) {
        let result = app_server
            .thread_goal_set(
                thread_id,
                /*objective*/ None,
                Some(status),
                /*token_budget*/ None,
            )
            .await;
        if self.current_displayed_thread_id() != Some(thread_id) {
            return;
        }

        match result {
            Ok(response) => self.chat_widget.add_info_message(
                format!("Goal {}", goal_status_label(response.goal.status)),
                Some(goal_usage_summary(&response.goal)),
            ),
            Err(err) => self
                .chat_widget
                .add_error_message(thread_goal_error_message("update", &err)),
        }
    }

    pub(super) async fn clear_thread_goal(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
    ) {
        let result = app_server.thread_goal_clear(thread_id).await;
        if self.current_displayed_thread_id() != Some(thread_id) {
            return;
        }

        match result {
            Ok(response) => {
                if response.cleared {
                    self.chat_widget
                        .add_info_message("Goal cleared".to_string(), /*hint*/ None);
                } else {
                    self.chat_widget.add_info_message(
                        "No goal to clear".to_string(),
                        Some("This thread does not currently have a goal.".to_string()),
                    );
                }
            }
            Err(err) => self
                .chat_widget
                .add_error_message(thread_goal_error_message("clear", &err)),
        }
    }

    fn show_replace_thread_goal_confirmation(&mut self, thread_id: ThreadId, objective: String) {
        let replace_objective = objective.clone();
        let replace_actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
            tx.send(AppEvent::SetThreadGoalObjective {
                thread_id,
                objective: replace_objective.clone(),
                mode: ThreadGoalSetMode::ReplaceExisting,
            });
        })];
        let items = vec![
            SelectionItem {
                name: "Replace current goal".to_string(),
                description: Some("Set the new objective and start it now".to_string()),
                actions: replace_actions,
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: "Cancel".to_string(),
                description: Some("Keep the current goal".to_string()),
                dismiss_on_select: true,
                ..Default::default()
            },
        ];
        self.chat_widget.show_selection_view(SelectionViewParams {
            title: Some("Replace goal?".to_string()),
            subtitle: Some(format!("New objective: {objective}")),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            ..Default::default()
        });
    }

    fn show_no_thread_goal_to_edit(&mut self) {
        self.chat_widget
            .add_error_message("No goal is currently set.".to_string());
        self.chat_widget.add_info_message(
            GOAL_USAGE.to_string(),
            Some("Create a goal before editing it.".to_string()),
        );
    }
}

fn thread_goal_error_message(action: &str, err: &color_eyre::Report) -> String {
    if is_ephemeral_thread_goal_error(err) {
        EPHEMERAL_THREAD_GOAL_ERROR_MESSAGE.to_string()
    } else {
        format!("Failed to {action} thread goal: {err}")
    }
}

fn is_ephemeral_thread_goal_error(err: &color_eyre::Report) -> bool {
    err.chain().any(|cause| {
        let message = cause.to_string();
        message.contains("ephemeral thread does not support goals")
            || message.contains("thread goals require a persisted thread; this thread is ephemeral")
    })
}

fn should_confirm_before_replacing_goal(goal: &ThreadGoal) -> bool {
    // Completed goals are terminal, so `/goal <objective>` can start a fresh goal
    // without asking the user to confirm replacing already-finished work.
    match goal.status {
        ThreadGoalStatus::Complete => false,
        ThreadGoalStatus::Active
        | ThreadGoalStatus::Paused
        | ThreadGoalStatus::Blocked
        | ThreadGoalStatus::UsageLimited
        | ThreadGoalStatus::BudgetLimited => true,
    }
}

#[cfg(test)]
mod tests {
    use crate::history_cell::HistoryCell;
    use pretty_assertions::assert_eq;
    use ratatui::layout::Rect;

    use super::*;

    #[test]
    fn thread_goal_error_message_explains_temporary_session() {
        let err = color_eyre::eyre::eyre!(
            "thread/goal/get failed: ephemeral thread does not support goals: thread-1"
        )
        .wrap_err("thread/goal/get failed in TUI");

        assert_eq!(
            thread_goal_error_message("read", &err),
            EPHEMERAL_THREAD_GOAL_ERROR_MESSAGE
        );
    }

    #[test]
    fn thread_goal_ephemeral_error_message_renders_snapshot() {
        let err = color_eyre::eyre::eyre!(
            "thread/goal/get failed: ephemeral thread does not support goals: thread-1"
        )
        .wrap_err("thread/goal/get failed in TUI");
        let cell = crate::history_cell::new_error_event(thread_goal_error_message("read", &err));
        let width = 72;
        let height = 6;
        let backend = crate::test_backend::VT100Backend::new(width, height);
        let mut terminal =
            crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, height - 1, width, 1));

        crate::insert_history::insert_history_lines(
            &mut terminal,
            cell.display_lines(/*width*/ width),
        )
        .expect("insert history lines");

        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn thread_goal_error_message_preserves_generic_failure_context() {
        let err =
            color_eyre::eyre::eyre!("server disappeared").wrap_err("thread/goal/get failed in TUI");

        assert_eq!(
            thread_goal_error_message("read", &err),
            "Failed to read thread goal: thread/goal/get failed in TUI"
        );
    }

    #[test]
    fn completed_goal_does_not_require_replace_confirmation() {
        assert!(!should_confirm_before_replacing_goal(&test_goal(
            ThreadGoalStatus::Complete
        )));
    }

    #[test]
    fn unfinished_goals_require_replace_confirmation() {
        for status in [
            ThreadGoalStatus::Active,
            ThreadGoalStatus::Paused,
            ThreadGoalStatus::Blocked,
            ThreadGoalStatus::UsageLimited,
            ThreadGoalStatus::BudgetLimited,
        ] {
            assert!(should_confirm_before_replacing_goal(&test_goal(status)));
        }
    }

    fn test_goal(status: ThreadGoalStatus) -> ThreadGoal {
        ThreadGoal {
            thread_id: ThreadId::new().to_string(),
            objective: "Finish the thing.".to_string(),
            status,
            token_budget: None,
            tokens_used: 0,
            time_used_seconds: 0,
            created_at: 1_776_272_400,
            updated_at: 1_776_272_460,
        }
    }
}
