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
use crate::file_search::FileSearchEvent;
use crate::i18n::t;
use crate::paste::PasteManager;
use crate::state::AppState;
use crate::state::ChatMessage;
use crate::state::FileSuggestionItem;
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
    paste_manager: &PasteManager,
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
            let raw_message = state.ui.input.take();
            if !raw_message.trim().is_empty() {
                // Resolve paste pills to content blocks for API
                let content = paste_manager.resolve_to_blocks(&raw_message);

                // Keep original text (with pills) for display in chat history
                let display_text = raw_message.clone();

                // Add user message to chat (display version)
                let msg_id = format!("user-{}", state.session.messages.len());
                state
                    .session
                    .add_message(ChatMessage::user(&msg_id, &display_text));

                // Save to history with frecency tracking
                state.ui.input.add_to_history(raw_message);
                state.ui.input.history_index = None;

                // Send to core with resolved content blocks
                let _ = command_tx
                    .send(UserCommand::SubmitInput {
                        content,
                        display_text,
                    })
                    .await;

                // Auto-scroll to bottom and reset user scroll state
                state.ui.scroll_offset = 0;
                state.ui.reset_user_scrolled();
            }
        }
        TuiCommand::Interrupt => {
            let _ = command_tx.send(UserCommand::Interrupt).await;
        }
        TuiCommand::ClearScreen => {
            // Clear chat history and reset scroll
            state.session.messages.clear();
            state.ui.scroll_offset = 0;
            state.ui.reset_user_scrolled();
            tracing::debug!("Screen cleared - chat history reset");
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
            state.ui.mark_user_scrolled();
        }
        TuiCommand::ScrollDown => {
            state.ui.scroll_offset = state.ui.scroll_offset.saturating_sub(3);
            if state.ui.scroll_offset < 0 {
                state.ui.scroll_offset = 0;
            }
            // Only mark as user scrolled if we're not at the bottom
            if state.ui.scroll_offset > 0 {
                state.ui.mark_user_scrolled();
            } else {
                // User scrolled to bottom, re-enable auto-scroll
                state.ui.reset_user_scrolled();
            }
        }
        TuiCommand::PageUp => {
            state.ui.scroll_offset = state.ui.scroll_offset.saturating_add(20);
            state.ui.mark_user_scrolled();
        }
        TuiCommand::PageDown => {
            state.ui.scroll_offset = state.ui.scroll_offset.saturating_sub(20);
            if state.ui.scroll_offset < 0 {
                state.ui.scroll_offset = 0;
            }
            if state.ui.scroll_offset > 0 {
                state.ui.mark_user_scrolled();
            } else {
                state.ui.reset_user_scrolled();
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
            match &mut state.ui.overlay {
                Some(Overlay::ModelPicker(picker)) => {
                    picker.filter.push(c);
                }
                Some(Overlay::CommandPalette(palette)) => {
                    palette.insert_char(c);
                }
                Some(Overlay::SessionBrowser(browser)) => {
                    browser.insert_char(c);
                }
                _ => {
                    state.ui.input.insert_char(c);
                }
            }
        }
        TuiCommand::DeleteBackward => match &mut state.ui.overlay {
            Some(Overlay::ModelPicker(picker)) => {
                picker.filter.pop();
            }
            Some(Overlay::CommandPalette(palette)) => {
                palette.delete_char();
            }
            Some(Overlay::SessionBrowser(browser)) => {
                browser.delete_char();
            }
            _ => {
                state.ui.input.delete_backward();
            }
        },
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
            match &mut state.ui.overlay {
                Some(Overlay::Permission(perm)) => {
                    perm.move_up();
                }
                Some(Overlay::ModelPicker(picker)) => {
                    picker.move_up();
                }
                Some(Overlay::CommandPalette(palette)) => {
                    palette.move_up();
                }
                Some(Overlay::SessionBrowser(browser)) => {
                    browser.move_up();
                }
                _ => {
                    // History navigation
                    handle_history_up(state);
                }
            }
        }
        TuiCommand::CursorDown => {
            // Handle overlay navigation or history
            match &mut state.ui.overlay {
                Some(Overlay::Permission(perm)) => {
                    perm.move_down();
                }
                Some(Overlay::ModelPicker(picker)) => {
                    picker.move_down();
                }
                Some(Overlay::CommandPalette(palette)) => {
                    palette.move_down();
                }
                Some(Overlay::SessionBrowser(browser)) => {
                    browser.move_down();
                }
                _ => {
                    // History navigation
                    handle_history_down(state);
                }
            }
        }
        TuiCommand::CursorHome => {
            state.ui.input.move_home();
        }
        TuiCommand::CursorEnd => {
            state.ui.input.move_end();
        }
        TuiCommand::WordLeft => {
            state.ui.input.move_word_left();
        }
        TuiCommand::WordRight => {
            state.ui.input.move_word_right();
        }
        TuiCommand::DeleteWordBackward => {
            state.ui.input.delete_word_backward();
        }
        TuiCommand::DeleteWordForward => {
            state.ui.input.delete_word_forward();
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
            } else if let Some(Overlay::CommandPalette(ref palette)) = state.ui.overlay {
                // Execute selected command
                if let Some(cmd) = palette.selected_command() {
                    let action = cmd.action.clone();
                    state.ui.clear_overlay();
                    execute_command_action(state, &action, command_tx).await;
                } else {
                    state.ui.clear_overlay();
                }
            } else if let Some(Overlay::SessionBrowser(ref browser)) = state.ui.overlay {
                // Load selected session
                if let Some(session) = browser.selected_session() {
                    let session_id = session.id.clone();
                    state.ui.clear_overlay();
                    tracing::info!(session_id, "Load session requested (not yet implemented)");
                } else {
                    state.ui.clear_overlay();
                }
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

        // ========== File Autocomplete ==========
        TuiCommand::SelectNextSuggestion => {
            if let Some(ref mut suggestions) = state.ui.file_suggestions {
                suggestions.move_down();
            }
        }
        TuiCommand::SelectPrevSuggestion => {
            if let Some(ref mut suggestions) = state.ui.file_suggestions {
                suggestions.move_up();
            }
        }
        TuiCommand::AcceptSuggestion => {
            if let Some(suggestions) = state.ui.file_suggestions.take() {
                if let Some(selected) = suggestions.selected_suggestion() {
                    state
                        .ui
                        .input
                        .insert_selected_path(suggestions.start_pos, &selected.path);
                }
            }
        }
        TuiCommand::DismissSuggestions => {
            state.ui.clear_file_suggestions();
        }

        // ========== Skill Autocomplete ==========
        TuiCommand::SelectNextSkillSuggestion => {
            if let Some(ref mut suggestions) = state.ui.skill_suggestions {
                suggestions.move_down();
            }
        }
        TuiCommand::SelectPrevSkillSuggestion => {
            if let Some(ref mut suggestions) = state.ui.skill_suggestions {
                suggestions.move_up();
            }
        }
        TuiCommand::AcceptSkillSuggestion => {
            if let Some(suggestions) = state.ui.skill_suggestions.take() {
                if let Some(selected) = suggestions.selected_suggestion() {
                    state
                        .ui
                        .input
                        .insert_selected_skill(suggestions.start_pos, &selected.name);
                }
            }
        }
        TuiCommand::DismissSkillSuggestions => {
            state.ui.clear_skill_suggestions();
        }

        // ========== Queue ==========
        TuiCommand::QueueInput => {
            // Queue input for later processing (Enter during streaming)
            // This also serves as real-time steering: queued commands are
            // injected into the current turn as system reminders.
            let prompt = state.ui.input.take();
            if !prompt.trim().is_empty() {
                let id = state.session.queue_command(&prompt);
                let _ = command_tx
                    .send(UserCommand::QueueCommand {
                        prompt: prompt.clone(),
                    })
                    .await;
                let count = state.session.queued_count();
                state
                    .ui
                    .toast_info(t!("toast.command_queued", count = count).to_string());
                tracing::debug!(id, count, "Command queued (also serves as steering)");
            }
        }

        // ========== External Editor ==========
        TuiCommand::OpenExternalEditor => {
            // TODO: Implement external editor support
            tracing::info!("External editor requested (not yet implemented)");
        }

        // ========== Clipboard Paste ==========
        TuiCommand::PasteFromClipboard => {
            // Handled in app.rs (needs &mut paste_manager)
        }

        // ========== Help ==========
        TuiCommand::ShowHelp => {
            state.ui.set_overlay(Overlay::Help);
        }

        // ========== Command Palette ==========
        TuiCommand::ShowCommandPalette => {
            let commands = get_default_commands();
            state.ui.set_overlay(Overlay::CommandPalette(
                crate::state::CommandPaletteOverlay::new(commands),
            ));
        }

        // ========== Session Browser ==========
        TuiCommand::ShowSessionBrowser => {
            // TODO: Load sessions from storage
            let sessions = Vec::new();
            state.ui.set_overlay(Overlay::SessionBrowser(
                crate::state::SessionBrowserOverlay::new(sessions),
            ));
        }
        TuiCommand::LoadSession(_session_id) => {
            // TODO: Implement session loading
            tracing::info!("Load session requested (not yet implemented)");
        }
        TuiCommand::DeleteSession(_session_id) => {
            // TODO: Implement session deletion
            tracing::info!("Delete session requested (not yet implemented)");
        }

        // ========== Thinking Toggle ==========
        TuiCommand::ToggleThinking => {
            state.ui.toggle_thinking();
        }

        // ========== Quit ==========
        TuiCommand::Quit => {
            state.quit();
        }
    }
}

/// Execute a command action from the command palette.
async fn execute_command_action(
    state: &mut AppState,
    action: &crate::state::CommandAction,
    command_tx: &mpsc::Sender<UserCommand>,
) {
    use crate::state::CommandAction;

    match action {
        CommandAction::TogglePlanMode => {
            state.toggle_plan_mode();
            let _ = command_tx
                .send(UserCommand::SetPlanMode {
                    active: state.session.plan_mode,
                })
                .await;
        }
        CommandAction::CycleThinkingLevel => {
            state.cycle_thinking_level();
            let _ = command_tx
                .send(UserCommand::SetThinkingLevel {
                    level: state.session.thinking_level.clone(),
                })
                .await;
        }
        CommandAction::ShowModelPicker => {
            // Don't have available_models here, so just log
            tracing::info!("Model picker requested from command palette");
        }
        CommandAction::ShowHelp => {
            state.ui.set_overlay(Overlay::Help);
        }
        CommandAction::ShowSessionBrowser => {
            let sessions = Vec::new();
            state.ui.set_overlay(Overlay::SessionBrowser(
                crate::state::SessionBrowserOverlay::new(sessions),
            ));
        }
        CommandAction::ClearScreen => {
            state.session.messages.clear();
            state.ui.scroll_offset = 0;
            state.ui.reset_user_scrolled();
        }
        CommandAction::Interrupt => {
            let _ = command_tx.send(UserCommand::Interrupt).await;
        }
        CommandAction::Quit => {
            state.quit();
        }
    }
}

/// Get the default list of commands for the command palette.
fn get_default_commands() -> Vec<crate::state::CommandItem> {
    use crate::state::CommandAction;
    use crate::state::CommandItem;

    vec![
        CommandItem {
            name: t!("palette.toggle_plan_mode").to_string(),
            description: t!("palette.toggle_plan_mode_desc").to_string(),
            shortcut: Some("Tab".to_string()),
            action: CommandAction::TogglePlanMode,
        },
        CommandItem {
            name: t!("palette.cycle_thinking").to_string(),
            description: t!("palette.cycle_thinking_desc").to_string(),
            shortcut: Some("Ctrl+T".to_string()),
            action: CommandAction::CycleThinkingLevel,
        },
        CommandItem {
            name: t!("palette.switch_model").to_string(),
            description: t!("palette.switch_model_desc").to_string(),
            shortcut: Some("Ctrl+M".to_string()),
            action: CommandAction::ShowModelPicker,
        },
        CommandItem {
            name: t!("palette.show_help").to_string(),
            description: t!("palette.show_help_desc").to_string(),
            shortcut: Some("?".to_string()),
            action: CommandAction::ShowHelp,
        },
        CommandItem {
            name: t!("palette.session_browser").to_string(),
            description: t!("palette.session_browser_desc").to_string(),
            shortcut: Some("Ctrl+S".to_string()),
            action: CommandAction::ShowSessionBrowser,
        },
        CommandItem {
            name: t!("palette.clear_screen").to_string(),
            description: t!("palette.clear_screen_desc").to_string(),
            shortcut: Some("Ctrl+L".to_string()),
            action: CommandAction::ClearScreen,
        },
        CommandItem {
            name: t!("palette.interrupt").to_string(),
            description: t!("palette.interrupt_desc").to_string(),
            shortcut: Some("Ctrl+C".to_string()),
            action: CommandAction::Interrupt,
        },
        CommandItem {
            name: t!("palette.quit").to_string(),
            description: t!("palette.quit_desc").to_string(),
            shortcut: Some("Ctrl+Q".to_string()),
            action: CommandAction::Quit,
        },
    ]
}

/// Handle input history navigation (up arrow).
fn handle_history_up(state: &mut AppState) {
    let history_len = state.ui.input.history_len();
    if history_len == 0 {
        return;
    }

    let new_index = match state.ui.input.history_index {
        None => Some(0), // Start from most recent (history is sorted by frecency)
        Some(idx) if (idx as usize) < history_len - 1 => Some(idx + 1),
        Some(idx) => Some(idx),
    };

    if let Some(idx) = new_index {
        // Clone text to avoid borrow issues
        let text = state
            .ui
            .input
            .history_text(idx as usize)
            .map(|s| s.to_string());
        if let Some(text) = text {
            state.ui.input.set_text(text);
            state.ui.input.history_index = Some(idx);
        }
    }
}

/// Handle input history navigation (down arrow).
fn handle_history_down(state: &mut AppState) {
    let history_len = state.ui.input.history_len();
    if history_len == 0 {
        return;
    }

    match state.ui.input.history_index {
        Some(idx) if idx > 0 => {
            let new_idx = idx - 1;
            // Clone text to avoid borrow issues
            let text = state
                .ui
                .input
                .history_text(new_idx as usize)
                .map(|s| s.to_string());
            if let Some(text) = text {
                state.ui.input.set_text(text);
                state.ui.input.history_index = Some(new_idx);
            }
        }
        Some(_) | None => {
            // At the most recent or not in history, clear input
            state.ui.input.take();
            state.ui.input.history_index = None;
        }
    }
}

/// Handle a file search event.
///
/// This function processes results from the file search manager
/// and updates the autocomplete suggestions.
pub fn handle_file_search_event(state: &mut AppState, event: FileSearchEvent) {
    match event {
        FileSearchEvent::SearchResult {
            query,
            start_pos: _,
            suggestions,
        } => {
            // Only update if we're still showing suggestions for this query
            if let Some(ref current) = state.ui.file_suggestions {
                if current.query == query {
                    let items: Vec<FileSuggestionItem> = suggestions
                        .into_iter()
                        .map(|s| FileSuggestionItem {
                            path: s.path,
                            display_text: s.display_text,
                            score: s.score,
                            match_indices: s.match_indices,
                            is_directory: s.is_directory,
                        })
                        .collect();
                    state.ui.update_file_suggestions(items);
                }
            }
        }
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
            // Clear previous thinking duration when starting a new turn
            state.ui.clear_thinking_duration();
            // Reset thinking tokens for new turn
            state.session.reset_thinking_tokens();
        }
        LoopEvent::TurnCompleted { turn_id, usage } => {
            // Stop thinking timer if still running
            if state.ui.is_thinking() {
                state.ui.stop_thinking();
            }
            // Finalize the streaming message
            if let Some(streaming) = state.ui.streaming.take() {
                let mut message = ChatMessage::assistant(&turn_id, &streaming.content);
                if !streaming.thinking.is_empty() {
                    message.thinking = Some(streaming.thinking);
                }
                message.complete();
                state.session.add_message(message);
            }
            // Track reasoning/thinking tokens
            if let Some(reasoning_tokens) = usage.reasoning_tokens {
                state.session.add_thinking_tokens(reasoning_tokens as i32);
            }
            state.session.update_tokens(usage);
        }

        // ========== Content Streaming ==========
        LoopEvent::TextDelta { delta, .. } => {
            // When we get the first text delta, thinking is done
            if state.ui.is_thinking() {
                state.ui.stop_thinking();
            }
            state.ui.append_streaming(&delta);
        }
        LoopEvent::ThinkingDelta { delta, .. } => {
            // Start thinking timer on first thinking delta
            state.ui.start_thinking();
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
            // Track reasoning/thinking tokens separately
            if let Some(reasoning_tokens) = usage.reasoning_tokens {
                state.session.add_thinking_tokens(reasoning_tokens as i32);
            }
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

        // ========== Subagent Events ==========
        LoopEvent::SubagentSpawned {
            agent_id,
            agent_type,
            description,
        } => {
            state
                .session
                .start_subagent(agent_id, agent_type, description);
        }
        LoopEvent::SubagentProgress { agent_id, progress } => {
            state.session.update_subagent_progress(&agent_id, progress);
        }
        LoopEvent::SubagentCompleted { agent_id, result } => {
            state.session.complete_subagent(&agent_id, result);
            // Cleanup old completed subagents
            state.session.cleanup_completed_subagents(5);
        }
        LoopEvent::SubagentBackgrounded {
            agent_id,
            output_file,
        } => {
            state.session.background_subagent(&agent_id, output_file);
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
        LoopEvent::ContextUsageWarning {
            percent_left,
            estimated_tokens,
            warning_threshold,
        } => {
            // Format tokens with k/M suffix
            let format_tokens = |n: i32| -> String {
                if n >= 1_000_000 {
                    format!("{:.1}M", n as f64 / 1_000_000.0)
                } else if n >= 1_000 {
                    format!("{:.0}k", n as f64 / 1_000.0)
                } else {
                    n.to_string()
                }
            };
            // Calculate remaining tokens from threshold vs used
            let remain = format_tokens(warning_threshold.saturating_sub(estimated_tokens));
            let total = format_tokens(warning_threshold);
            let percent = (percent_left * 100.0) as i32;
            let msg = t!(
                "toast.context_warning",
                percent = percent,
                remain = remain,
                total = total
            )
            .to_string();
            state.ui.toast_warning(msg);
            tracing::debug!(percent_left, "Context usage warning");
        }
        LoopEvent::CompactionStarted => {
            state.ui.toast_info(t!("toast.compacting").to_string());
            state.session.is_compacting = true;
            tracing::debug!("Compaction started");
        }
        LoopEvent::CompactionCompleted {
            removed_messages,
            summary_tokens,
        } => {
            let msg = t!(
                "toast.compacted",
                messages = removed_messages,
                tokens = summary_tokens
            )
            .to_string();
            state.ui.toast_success(msg);
            state.session.is_compacting = false;
            tracing::info!(removed_messages, summary_tokens, "Compaction completed");
        }
        LoopEvent::CompactionFailed { error, .. } => {
            state
                .ui
                .toast_error(t!("toast.compaction_failed", error = error).to_string());
            state.session.is_compacting = false;
            tracing::error!(error, "Compaction failed");
        }

        // ========== Model Fallback ==========
        LoopEvent::ModelFallbackStarted { from, to, reason } => {
            let msg = t!("toast.model_fallback", from = from, to = to).to_string();
            state.ui.toast_warning(msg);
            state.session.fallback_model = Some(to.clone());
            tracing::info!(from, to, reason, "Model fallback started");
        }
        LoopEvent::ModelFallbackCompleted => {
            state.session.fallback_model = None;
            tracing::debug!("Model fallback completed");
        }

        // ========== Queue ==========
        LoopEvent::CommandQueued { id, preview } => {
            tracing::debug!(id, preview, "Command queued (from core)");
        }
        LoopEvent::CommandDequeued { id } => {
            // Remove from local queue if present
            state.session.queued_commands.retain(|c| c.id != id);
            tracing::debug!(id, "Command dequeued");
        }
        LoopEvent::QueueStateChanged { queued } => {
            tracing::debug!(queued, "Queue state changed");
        }

        // ========== MCP Events ==========
        LoopEvent::McpStartupUpdate { server, status } => {
            use cocode_protocol::McpStartupStatus;
            match status {
                McpStartupStatus::Ready => {
                    state
                        .ui
                        .toast_success(t!("toast.mcp_ready", server = server).to_string());
                }
                McpStartupStatus::Failed => {
                    state
                        .ui
                        .toast_error(t!("toast.mcp_failed", server = server).to_string());
                }
                _ => {}
            }
        }
        LoopEvent::McpStartupComplete { servers, failed } => {
            if !servers.is_empty() {
                let count = servers.len();
                state
                    .ui
                    .toast_success(t!("toast.mcp_connected", count = count).to_string());
            }
            for (name, error) in failed {
                state
                    .ui
                    .toast_error(t!("toast.mcp_error", name = name, error = error).to_string());
            }
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
        use crate::state::HistoryEntry;

        let mut state = create_test_state();
        // Add history entries - they're sorted by frecency (most recent first)
        state.ui.input.history = vec![
            HistoryEntry::new("second"), // Index 0 - most recent
            HistoryEntry::new("first"),  // Index 1 - older
        ];

        // Navigate up (goes to older entries, index increases)
        handle_history_up(&mut state);
        assert_eq!(state.ui.input.text(), "second");
        assert_eq!(state.ui.input.history_index, Some(0));

        handle_history_up(&mut state);
        assert_eq!(state.ui.input.text(), "first");
        assert_eq!(state.ui.input.history_index, Some(1));

        // Navigate down (goes to newer entries, index decreases)
        handle_history_down(&mut state);
        assert_eq!(state.ui.input.text(), "second");
        assert_eq!(state.ui.input.history_index, Some(0));

        handle_history_down(&mut state);
        assert!(state.ui.input.is_empty());
        assert_eq!(state.ui.input.history_index, None);
    }
}
