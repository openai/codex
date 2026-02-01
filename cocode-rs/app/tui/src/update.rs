//! State update functions.
//!
//! This module contains pure functions that update the application state
//! in response to events. Following the Elm Architecture pattern, these
//! functions take the current state and an event, and return the new state.

use cocode_protocol::LoopEvent;
use cocode_protocol::ToolResultContent;
use tokio::sync::mpsc;

use crate::command::UserCommand;
use crate::event::TuiCommand;
use crate::state::AppState;
use crate::state::ChatMessage;
use crate::state::FocusTarget;
use crate::state::ModelPickerOverlay;
use crate::state::Overlay;
use crate::state::PermissionOverlay;

/// Handle a TUI command and update the state accordingly.
///
/// This function processes high-level commands from keyboard input
/// and updates the state. It also sends commands to the core agent
/// when needed.
pub async fn handle_command(
    state: &mut AppState,
    cmd: TuiCommand,
    command_tx: &mpsc::Sender<UserCommand>,
    available_models: &[String],
) {
    match cmd {
        // ========== Mode Toggles ==========
        TuiCommand::TogglePlanMode => {
            state.toggle_plan_mode();
            let _ = command_tx
                .send(UserCommand::SetPlanMode {
                    active: state.session.plan_mode,
                })
                .await;
        }
        TuiCommand::CycleThinkingLevel => {
            state.cycle_thinking_level();
            let _ = command_tx
                .send(UserCommand::SetThinkingLevel {
                    level: state.session.thinking_level.clone(),
                })
                .await;
        }
        TuiCommand::CycleModel => {
            // Show model picker overlay
            if !available_models.is_empty() {
                state
                    .ui
                    .set_overlay(Overlay::ModelPicker(ModelPickerOverlay::new(
                        available_models.to_vec(),
                    )));
            }
        }
        TuiCommand::ShowModelPicker => {
            if !available_models.is_empty() {
                state
                    .ui
                    .set_overlay(Overlay::ModelPicker(ModelPickerOverlay::new(
                        available_models.to_vec(),
                    )));
            }
        }

        // ========== Input Actions ==========
        TuiCommand::SubmitInput => {
            let message = state.ui.input.take();
            if !message.trim().is_empty() {
                // Add user message to chat
                let msg_id = format!("user-{}", state.session.messages.len());
                state
                    .session
                    .add_message(ChatMessage::user(&msg_id, &message));

                // Save to history
                state.ui.input.history.push(message.clone());
                state.ui.input.history_index = None;

                // Send to core
                let _ = command_tx.send(UserCommand::SubmitInput { message }).await;

                // Auto-scroll to bottom
                state.ui.scroll_offset = 0;
            }
        }
        TuiCommand::Interrupt => {
            let _ = command_tx.send(UserCommand::Interrupt).await;
        }
        TuiCommand::ClearScreen => {
            // Just trigger a redraw - the terminal will handle clearing
            tracing::debug!("Clear screen requested");
        }
        TuiCommand::Cancel => {
            // Close overlay if present, otherwise clear input
            if state.has_overlay() {
                state.ui.clear_overlay();
            } else if !state.ui.input.is_empty() {
                state.ui.input.take();
            }
        }

        // ========== Navigation ==========
        TuiCommand::ScrollUp => {
            state.ui.scroll_offset = state.ui.scroll_offset.saturating_add(3);
        }
        TuiCommand::ScrollDown => {
            state.ui.scroll_offset = state.ui.scroll_offset.saturating_sub(3);
            if state.ui.scroll_offset < 0 {
                state.ui.scroll_offset = 0;
            }
        }
        TuiCommand::PageUp => {
            state.ui.scroll_offset = state.ui.scroll_offset.saturating_add(20);
        }
        TuiCommand::PageDown => {
            state.ui.scroll_offset = state.ui.scroll_offset.saturating_sub(20);
            if state.ui.scroll_offset < 0 {
                state.ui.scroll_offset = 0;
            }
        }
        TuiCommand::FocusNext => {
            state.ui.focus = match state.ui.focus {
                FocusTarget::Input => FocusTarget::Chat,
                FocusTarget::Chat => FocusTarget::ToolPanel,
                FocusTarget::ToolPanel => FocusTarget::Input,
            };
        }
        TuiCommand::FocusPrevious => {
            state.ui.focus = match state.ui.focus {
                FocusTarget::Input => FocusTarget::ToolPanel,
                FocusTarget::Chat => FocusTarget::Input,
                FocusTarget::ToolPanel => FocusTarget::Chat,
            };
        }

        // ========== Editing ==========
        TuiCommand::InsertChar(c) => {
            // Handle overlay input if present
            if let Some(Overlay::ModelPicker(ref mut picker)) = state.ui.overlay {
                picker.filter.push(c);
            } else {
                state.ui.input.insert_char(c);
            }
        }
        TuiCommand::DeleteBackward => {
            if let Some(Overlay::ModelPicker(ref mut picker)) = state.ui.overlay {
                picker.filter.pop();
            } else {
                state.ui.input.delete_backward();
            }
        }
        TuiCommand::DeleteForward => {
            state.ui.input.delete_forward();
        }
        TuiCommand::CursorLeft => {
            state.ui.input.move_left();
        }
        TuiCommand::CursorRight => {
            state.ui.input.move_right();
        }
        TuiCommand::CursorUp => {
            // Handle overlay navigation or history
            if let Some(Overlay::Permission(ref mut perm)) = state.ui.overlay {
                perm.move_up();
            } else if let Some(Overlay::ModelPicker(ref mut picker)) = state.ui.overlay {
                picker.move_up();
            } else {
                // History navigation
                handle_history_up(state);
            }
        }
        TuiCommand::CursorDown => {
            // Handle overlay navigation or history
            if let Some(Overlay::Permission(ref mut perm)) = state.ui.overlay {
                perm.move_down();
            } else if let Some(Overlay::ModelPicker(ref mut picker)) = state.ui.overlay {
                picker.move_down();
            } else {
                // History navigation
                handle_history_down(state);
            }
        }
        TuiCommand::CursorHome => {
            state.ui.input.move_home();
        }
        TuiCommand::CursorEnd => {
            state.ui.input.move_end();
        }
        TuiCommand::InsertNewline => {
            state.ui.input.insert_newline();
        }

        // ========== Approval ==========
        TuiCommand::Approve => {
            if let Some(Overlay::Permission(ref perm)) = state.ui.overlay {
                let request_id = perm.request.request_id.clone();
                let _ = command_tx
                    .send(UserCommand::ApprovalResponse {
                        request_id,
                        approved: true,
                        remember: false,
                    })
                    .await;
                state.ui.clear_overlay();
            } else if let Some(Overlay::ModelPicker(ref picker)) = state.ui.overlay {
                // Select current model
                let filtered = picker.filtered_models();
                if let Some(model) = filtered.get(picker.selected as usize) {
                    let model = model.to_string();
                    state.session.current_model = model.clone();
                    let _ = command_tx.send(UserCommand::SetModel { model }).await;
                }
                state.ui.clear_overlay();
            }
        }
        TuiCommand::Deny => {
            if let Some(Overlay::Permission(ref perm)) = state.ui.overlay {
                let request_id = perm.request.request_id.clone();
                let _ = command_tx
                    .send(UserCommand::ApprovalResponse {
                        request_id,
                        approved: false,
                        remember: false,
                    })
                    .await;
                state.ui.clear_overlay();
            }
        }
        TuiCommand::ApproveAll => {
            if let Some(Overlay::Permission(ref perm)) = state.ui.overlay {
                let request_id = perm.request.request_id.clone();
                let _ = command_tx
                    .send(UserCommand::ApprovalResponse {
                        request_id,
                        approved: true,
                        remember: true,
                    })
                    .await;
                state.ui.clear_overlay();
            }
        }

        // ========== External Editor ==========
        TuiCommand::OpenExternalEditor => {
            // TODO: Implement external editor support
            tracing::info!("External editor requested (not yet implemented)");
        }

        // ========== Quit ==========
        TuiCommand::Quit => {
            state.quit();
        }
    }
}

/// Handle input history navigation (up arrow).
fn handle_history_up(state: &mut AppState) {
    let history = &state.ui.input.history;
    if history.is_empty() {
        return;
    }

    let new_index = match state.ui.input.history_index {
        None => Some((history.len() as i32) - 1),
        Some(idx) if idx > 0 => Some(idx - 1),
        Some(idx) => Some(idx),
    };

    if let Some(idx) = new_index {
        if let Some(text) = history.get(idx as usize) {
            state.ui.input.set_text(text.clone());
            state.ui.input.history_index = Some(idx);
        }
    }
}

/// Handle input history navigation (down arrow).
fn handle_history_down(state: &mut AppState) {
    let history = &state.ui.input.history;
    if history.is_empty() {
        return;
    }

    match state.ui.input.history_index {
        Some(idx) if (idx as usize) < history.len() - 1 => {
            let new_idx = idx + 1;
            if let Some(text) = history.get(new_idx as usize) {
                state.ui.input.set_text(text.clone());
                state.ui.input.history_index = Some(new_idx);
            }
        }
        Some(_) => {
            // At the end of history, clear input
            state.ui.input.take();
            state.ui.input.history_index = None;
        }
        None => {}
    }
}

/// Handle an event from the core agent loop.
///
/// This function processes events from the agent and updates the
/// application state accordingly. It handles streaming content,
/// tool execution updates, and other agent lifecycle events.
pub fn handle_agent_event(state: &mut AppState, event: LoopEvent) {
    match event {
        // ========== Turn Lifecycle ==========
        LoopEvent::TurnStarted { turn_id, .. } => {
            state.ui.start_streaming(turn_id);
        }
        LoopEvent::TurnCompleted { turn_id, usage } => {
            // Finalize the streaming message
            if let Some(streaming) = state.ui.streaming.take() {
                let mut message = ChatMessage::assistant(&turn_id, &streaming.content);
                if !streaming.thinking.is_empty() {
                    message.thinking = Some(streaming.thinking);
                }
                message.complete();
                state.session.add_message(message);
            }
            state.session.update_tokens(usage);
        }

        // ========== Content Streaming ==========
        LoopEvent::TextDelta { delta, .. } => {
            state.ui.append_streaming(&delta);
        }
        LoopEvent::ThinkingDelta { delta, .. } => {
            state.ui.append_streaming_thinking(&delta);
        }

        // ========== Tool Execution ==========
        LoopEvent::ToolUseStarted { call_id, name, .. } => {
            state.session.start_tool(call_id, name);
        }
        LoopEvent::ToolProgress { call_id, progress } => {
            if let Some(msg) = progress.message {
                state.session.update_tool_progress(&call_id, msg);
            }
        }
        LoopEvent::ToolUseCompleted {
            call_id,
            output,
            is_error,
        } => {
            let output_str = match output {
                ToolResultContent::Text(s) => s,
                ToolResultContent::Structured(v) => v.to_string(),
            };
            state.session.complete_tool(&call_id, output_str, is_error);
            // Cleanup old completed tools
            state.session.cleanup_completed_tools(10);
        }

        // ========== Permission ==========
        LoopEvent::ApprovalRequired { request } => {
            state
                .ui
                .set_overlay(Overlay::Permission(PermissionOverlay::new(request)));
        }

        // ========== Token Usage ==========
        LoopEvent::StreamRequestEnd { usage } => {
            state.session.update_tokens(usage);
        }

        // ========== Plan Mode ==========
        LoopEvent::PlanModeEntered { plan_file } => {
            state.session.plan_mode = true;
            state.session.plan_file = Some(plan_file);
        }
        LoopEvent::PlanModeExited { .. } => {
            state.session.plan_mode = false;
            state.session.plan_file = None;
        }

        // ========== Errors ==========
        LoopEvent::Error { error } => {
            state
                .ui
                .set_overlay(Overlay::Error(format!("{}: {}", error.code, error.message)));
        }
        LoopEvent::Interrupted => {
            // Stop streaming if active
            state.ui.stop_streaming();
            tracing::info!("Operation interrupted");
        }

        // ========== Context/Compaction ==========
        LoopEvent::CompactionStarted => {
            tracing::debug!("Compaction started");
        }
        LoopEvent::CompactionCompleted {
            removed_messages,
            summary_tokens,
        } => {
            tracing::info!(removed_messages, summary_tokens, "Compaction completed");
        }

        // Other events we don't need to handle in the TUI
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::MessageRole;
    use cocode_protocol::TokenUsage;

    fn create_test_state() -> AppState {
        AppState::new()
    }

    #[test]
    fn test_handle_agent_event_turn_started() {
        let mut state = create_test_state();

        handle_agent_event(
            &mut state,
            LoopEvent::TurnStarted {
                turn_id: "turn-1".to_string(),
                turn_number: 1,
            },
        );

        assert!(state.is_streaming());
        assert_eq!(
            state.ui.streaming.as_ref().map(|s| s.turn_id.as_str()),
            Some("turn-1")
        );
    }

    #[test]
    fn test_handle_agent_event_text_delta() {
        let mut state = create_test_state();
        state.ui.start_streaming("turn-1".to_string());

        handle_agent_event(
            &mut state,
            LoopEvent::TextDelta {
                turn_id: "turn-1".to_string(),
                delta: "Hello ".to_string(),
            },
        );
        handle_agent_event(
            &mut state,
            LoopEvent::TextDelta {
                turn_id: "turn-1".to_string(),
                delta: "World".to_string(),
            },
        );

        assert_eq!(
            state.ui.streaming.as_ref().map(|s| s.content.as_str()),
            Some("Hello World")
        );
    }

    #[test]
    fn test_handle_agent_event_turn_completed() {
        let mut state = create_test_state();
        state.ui.start_streaming("turn-1".to_string());
        state.ui.append_streaming("Test content");

        handle_agent_event(
            &mut state,
            LoopEvent::TurnCompleted {
                turn_id: "turn-1".to_string(),
                usage: TokenUsage::new(100, 50),
            },
        );

        assert!(!state.is_streaming());
        assert_eq!(state.session.messages.len(), 1);
        assert_eq!(state.session.messages[0].content, "Test content");
        assert_eq!(state.session.messages[0].role, MessageRole::Assistant);
    }

    #[test]
    fn test_handle_agent_event_tool_lifecycle() {
        let mut state = create_test_state();

        handle_agent_event(
            &mut state,
            LoopEvent::ToolUseStarted {
                call_id: "call-1".to_string(),
                name: "bash".to_string(),
            },
        );

        assert_eq!(state.session.tool_executions.len(), 1);
        assert_eq!(state.session.tool_executions[0].name, "bash");

        handle_agent_event(
            &mut state,
            LoopEvent::ToolUseCompleted {
                call_id: "call-1".to_string(),
                output: ToolResultContent::Text("Success".to_string()),
                is_error: false,
            },
        );

        assert_eq!(
            state.session.tool_executions[0].output,
            Some("Success".to_string())
        );
    }

    #[test]
    fn test_handle_history_up_down() {
        let mut state = create_test_state();
        state.ui.input.history = vec!["first".to_string(), "second".to_string()];

        // Navigate up
        handle_history_up(&mut state);
        assert_eq!(state.ui.input.text(), "second");
        assert_eq!(state.ui.input.history_index, Some(1));

        handle_history_up(&mut state);
        assert_eq!(state.ui.input.text(), "first");
        assert_eq!(state.ui.input.history_index, Some(0));

        // Navigate down
        handle_history_down(&mut state);
        assert_eq!(state.ui.input.text(), "second");
        assert_eq!(state.ui.input.history_index, Some(1));

        handle_history_down(&mut state);
        assert!(state.ui.input.is_empty());
        assert_eq!(state.ui.input.history_index, None);
    }
}
