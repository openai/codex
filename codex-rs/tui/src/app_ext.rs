//! Extension module for app.rs to minimize upstream merge conflicts.
//!
//! Contains Plan/EnterPlanMode/UserQuestion overlay construction logic.
//! Also contains batch delegation handlers for extension AppEvents.
//! Also contains spawn event handlers (StartSpawnTask, SpawnList, etc.).
//! Separated to keep app.rs modifications minimal during upstream syncs.

use crate::App;
use crate::app_event::AppEvent;
use crate::bottom_pane::ApprovalRequest;
use crate::pager_overlay::Overlay;
use codex_core::ThreadManager;
use codex_core::protocol::Op;
use codex_core::spawn_task::SpawnCommandArgs;
use codex_protocol::protocol_ext::UserQuestion;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;
use std::sync::Arc;

/// Build paragraph for Plan approval overlay.
pub fn build_plan_overlay_paragraph(
    plan_content: &str,
    plan_file_path: &str,
) -> Paragraph<'static> {
    Paragraph::new(vec![
        Line::from(vec![
            "Plan file: ".into(),
            plan_file_path.to_string().bold(),
        ]),
        Line::from(""),
        Line::from(plan_content.to_string()),
    ])
    .wrap(Wrap { trim: false })
}

/// Build paragraph for EnterPlanMode approval overlay.
pub fn build_enter_plan_mode_paragraph() -> Paragraph<'static> {
    Paragraph::new(vec![
        Line::from(
            "The LLM is requesting to enter plan mode."
                .to_string()
                .bold(),
        ),
        Line::from(""),
        Line::from("In plan mode, the LLM will:"),
        Line::from("- Explore the codebase using read-only tools"),
        Line::from("- Design an implementation approach"),
        Line::from("- Write a plan file for your review"),
        Line::from("- Ask for approval before implementing"),
    ])
    .wrap(Wrap { trim: false })
}

/// Build paragraph for UserQuestion approval overlay.
pub fn build_user_question_paragraph(questions: &[UserQuestion]) -> Paragraph<'static> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(
        "The LLM is asking for your input:".to_string().bold(),
    ));
    lines.push(Line::from(""));

    for (i, q) in questions.iter().enumerate() {
        lines.push(Line::from(format!("{}. {}", i + 1, q.question).bold()));
        lines.push(Line::from(format!("   [{}]", q.header)));
        for opt in &q.options {
            lines.push(Line::from(format!(
                "   • {} - {}",
                opt.label, opt.description
            )));
        }
        if q.multi_select {
            lines.push(Line::from("   (Multiple selections allowed)".to_string()));
        }
        lines.push(Line::from(""));
    }

    Paragraph::new(lines).wrap(Wrap { trim: false })
}

/// Build overlay for plan-related approval requests (Plan, EnterPlanMode, UserQuestion).
/// Returns Some(overlay) if the request matches, None otherwise.
pub fn build_plan_approval_overlay(request: &ApprovalRequest) -> Option<Overlay> {
    match request {
        ApprovalRequest::Plan {
            plan_content,
            plan_file_path,
        } => {
            let paragraph = build_plan_overlay_paragraph(plan_content, plan_file_path);
            Some(Overlay::new_static_with_renderables(
                vec![Box::new(paragraph)],
                "P L A N".to_string(),
            ))
        }
        ApprovalRequest::EnterPlanMode => {
            let paragraph = build_enter_plan_mode_paragraph();
            Some(Overlay::new_static_with_renderables(
                vec![Box::new(paragraph)],
                "E N T E R   P L A N   M O D E".to_string(),
            ))
        }
        ApprovalRequest::UserQuestion { questions, .. } => {
            let paragraph = build_user_question_paragraph(questions);
            Some(Overlay::new_static_with_renderables(
                vec![Box::new(paragraph)],
                "U S E R   Q U E S T I O N".to_string(),
            ))
        }
        _ => None,
    }
}

// =============================================================================
// Batch delegation handlers for extension AppEvents
// =============================================================================

/// Handle simple extension AppEvents - batch delegation pattern to minimize upstream conflicts.
/// For events that only need ChatWidget access, handle directly here.
/// Complex events (spawn task handlers) remain in app.rs as they need App-internal state.
pub fn handle_simple_ext_event(app: &mut App, event: AppEvent) {
    match event {
        AppEvent::PluginResult(text) => {
            app.chat_widget.add_info_message(text, None);
        }
        AppEvent::PluginCommandExpanded(text) => {
            app.chat_widget.submit_text_message(&text);
        }
        AppEvent::PluginCommandsLoaded(commands) => {
            app.chat_widget.on_plugin_commands_loaded(commands);
        }
        AppEvent::SetOutputStyle { style_name } => {
            app.current_output_style = style_name.clone();
            app.chat_widget.set_output_style(&style_name);
            app.chat_widget
                .add_info_message(format!("Output style set to: {style_name}"), None);
        }
        AppEvent::TogglePlanMode => {
            handle_toggle_plan_mode(app);
        }
        AppEvent::ToggleUltrathink => {
            let new_state = app.windows_sandbox.thinking_state.toggle();
            let msg = if new_state {
                "Ultrathink: ON"
            } else {
                "Ultrathink: OFF"
            };
            app.chat_widget.add_info_message(msg.to_string(), None);
        }
        AppEvent::SpawnTaskResult { message } => {
            app.chat_widget.add_info_message(message, None);
        }
        _ => {}
    }
}

fn handle_toggle_plan_mode(app: &mut App) {
    if app.chat_widget.is_plan_mode() {
        // Exit plan mode
        app.chat_widget.submit_op(Op::SetPlanMode {
            active: false,
            plan_file_path: None,
        });
        app.chat_widget
            .add_info_message("Exiting plan mode...".to_string(), None);
    } else if app.chat_widget.thread_id().is_some() {
        // Enter plan mode
        app.chat_widget.submit_op(Op::SetPlanMode {
            active: true,
            plan_file_path: None,
        });
        app.chat_widget
            .add_info_message("Entering plan mode...".to_string(), None);
    }
}

// =============================================================================
// Spawn event handlers - moved from app.rs to minimize upstream conflicts
// =============================================================================

/// Handle spawn-related AppEvents.
/// Async function to handle all spawn task events in one place.
pub async fn handle_spawn_event(app: &mut App, event: AppEvent) {
    match event {
        AppEvent::StartSpawnTask { args } => {
            handle_start_spawn_task(app, args);
        }
        AppEvent::SpawnListRequest => {
            handle_spawn_list(app);
        }
        AppEvent::SpawnStatusRequest { task_id } => {
            handle_spawn_status(app, &task_id).await;
        }
        AppEvent::SpawnKillRequest { task_id } => {
            handle_spawn_kill(app, &task_id).await;
        }
        AppEvent::SpawnDropRequest { task_id } => {
            handle_spawn_drop(app, &task_id).await;
        }
        AppEvent::SpawnMergeRequest { task_ids, prompt } => {
            handle_spawn_merge(app, task_ids, prompt).await;
        }
        _ => {}
    }
}

fn handle_start_spawn_task(app: &mut App, args: SpawnCommandArgs) {
    use codex_core::spawn_task::agent::SpawnAgent;
    use codex_core::spawn_task::agent::SpawnAgentContext;
    use codex_core::spawn_task::agent::SpawnAgentParams;
    use codex_core::spawn_task::plan_fork::read_plan_content;
    use codex_core::subagent::ApprovalMode;
    use codex_core::subagent::expect_session_state;
    use codex_protocol::config_types::PlanModeApprovalPolicy;

    let task_id = args
        .name
        .clone()
        .unwrap_or_else(|| format!("spawn-{}", chrono::Utc::now().timestamp_millis()));

    let loop_condition = match args.loop_condition {
        Some(cond) => cond,
        None => {
            app.chat_widget
                .add_info_message("Spawn error: missing --iter or --time".to_string(), None);
            return;
        }
    };

    let prompt = match args.prompt {
        Some(p) => p,
        None => {
            app.chat_widget
                .add_info_message("Spawn error: missing --prompt".to_string(), None);
            return;
        }
    };

    // Read parent plan content if:
    // 1. --detach is NOT set
    // 2. Parent has a plan file that exists and is non-empty
    let forked_plan_content = if !args.detach {
        app.chat_widget.thread_id().and_then(|conv_id| {
            let stores = expect_session_state(&conv_id);
            match stores.get_plan_file_path() {
                Some(plan_path) => {
                    let content = read_plan_content(&plan_path);
                    if let Some(ref c) = content {
                        tracing::info!(
                            task_id = %task_id,
                            content_len = c.len(),
                            "Forking parent plan to spawn task"
                        );
                    } else {
                        tracing::debug!(
                            task_id = %task_id,
                            path = %plan_path.display(),
                            "Parent plan file empty or unreadable, skipping fork"
                        );
                    }
                    content
                }
                None => {
                    tracing::debug!(task_id = %task_id, "No parent plan file, skipping fork");
                    None
                }
            }
        })
    } else {
        tracing::debug!(task_id = %task_id, "Plan fork disabled (--detach)");
        None
    };

    let params = SpawnAgentParams {
        task_id: task_id.clone(),
        loop_condition: loop_condition.clone(),
        prompt: prompt.clone(),
        cwd: app.config.cwd.to_path_buf(),
        custom_loop_prompt: None,
        approval_mode: ApprovalMode::DontAsk,
        model_override: args.model.clone(),
        forked_plan_content,
        plan_mode_approval_policy: PlanModeApprovalPolicy::AutoApprove,
    };

    let context = SpawnAgentContext {
        auth_manager: app.auth_manager.clone(),
        models_manager: app.server.get_models_manager(),
        skills_manager: app.server.skills_manager(),
        config: app.config.clone(),
        codex_home: app.config.codex_home.clone(),
        lsp_manager: app.server.get_lsp_manager(),
    };

    let agent = SpawnAgent::new(params, context);
    let tx = app.app_event_tx.clone();

    // Spawn async task to run the agent
    tokio::spawn(async move {
        use codex_core::spawn_task::SpawnTask;

        // Save initial metadata
        let metadata = agent.metadata();
        if let Err(e) = codex_core::spawn_task::save_metadata(&metadata.cwd, &metadata).await {
            tracing::warn!(error = %e, "Failed to save spawn task metadata");
        }

        // Start the agent
        let handle = Box::new(agent).spawn();
        let result = handle.await;

        let message = match result {
            Ok(task_result) => {
                format!(
                    "Spawn task '{}' completed: {} iterations succeeded, {} failed",
                    task_result.task_id,
                    task_result.iterations_completed,
                    task_result.iterations_failed
                )
            }
            Err(e) => {
                format!("Spawn task failed: {e}")
            }
        };

        tx.send(AppEvent::SpawnTaskResult { message });
    });

    app.chat_widget.add_info_message(
        format!(
            "Started spawn task '{}' with {} - {}",
            task_id,
            loop_condition.display(),
            prompt
        ),
        None,
    );
}

fn handle_spawn_list(app: &mut App) {
    use crate::spawn_command_ext::format_task_list;
    use crate::spawn_command_ext::list_task_metadata_sync;

    match list_task_metadata_sync(&app.config.codex_home) {
        Ok(tasks) => {
            let output = if tasks.is_empty() {
                "No spawn tasks found.".to_string()
            } else {
                format!("Spawn Tasks:{}", format_task_list(&tasks))
            };
            app.chat_widget.add_info_message(output, None);
        }
        Err(e) => {
            app.chat_widget
                .add_info_message(format!("Failed to list tasks: {e}"), None);
        }
    }
}

async fn handle_spawn_status(app: &mut App, task_id: &str) {
    use crate::spawn_command_ext::format_task_status;
    use codex_core::spawn_task::load_metadata;

    match load_metadata(&app.config.codex_home, task_id).await {
        Ok(metadata) => {
            let output = format_task_status(&metadata);
            app.chat_widget.add_info_message(output, None);
        }
        Err(e) => {
            app.chat_widget
                .add_info_message(format!("Task '{}' not found: {e}", task_id), None);
        }
    }
}

async fn handle_spawn_kill(app: &mut App, task_id: &str) {
    use codex_core::spawn_task::SpawnTaskManager;

    let manager = SpawnTaskManager::with_options(
        app.config.codex_home.clone(),
        app.config.cwd.to_path_buf(),
        SpawnTaskManager::DEFAULT_MAX_CONCURRENT,
        app.server.get_lsp_manager(),
    );

    match manager.kill(task_id).await {
        Ok(()) => {
            app.chat_widget
                .add_info_message(format!("Task '{}' cancelled.", task_id), None);
        }
        Err(e) => {
            app.chat_widget
                .add_info_message(format!("Failed to kill task: {e}"), None);
        }
    }
}

async fn handle_spawn_drop(app: &mut App, task_id: &str) {
    use codex_core::spawn_task::SpawnTaskManager;

    let manager = SpawnTaskManager::with_options(
        app.config.codex_home.clone(),
        app.config.cwd.to_path_buf(),
        SpawnTaskManager::DEFAULT_MAX_CONCURRENT,
        app.server.get_lsp_manager(),
    );

    match manager.drop(task_id).await {
        Ok(()) => {
            app.chat_widget
                .add_info_message(format!("Task '{}' dropped.", task_id), None);
        }
        Err(e) => {
            app.chat_widget
                .add_info_message(format!("Failed to drop task: {e}"), None);
        }
    }
}

async fn handle_spawn_merge(app: &mut App, task_ids: Vec<String>, prompt: Option<String>) {
    use codex_core::spawn_task::MergeRequest;
    use codex_core::spawn_task::build_merge_prompt;
    use codex_core::spawn_task::load_metadata;

    // Load metadata for all tasks
    let mut tasks_metadata = Vec::new();
    for task_id in &task_ids {
        match load_metadata(&app.config.codex_home, task_id).await {
            Ok(metadata) => tasks_metadata.push(metadata),
            Err(e) => {
                app.chat_widget
                    .add_info_message(format!("Task '{}' not found: {e}", task_id), None);
                return;
            }
        }
    }

    // Build merge request (prompt -> query for MergeRequest)
    let request = MergeRequest {
        task_ids: task_ids.clone(),
        query: prompt,
    };

    // Build merge prompt
    let prompt = build_merge_prompt(&request, &tasks_metadata, None);

    // Send as user message to the agent
    app.chat_widget.add_info_message(
        format!(
            "Merging {} task(s): {}",
            task_ids.len(),
            task_ids.join(", ")
        ),
        None,
    );

    // Submit merge prompt to agent via chat widget
    app.chat_widget.submit_text_message(&prompt);
}

// =============================================================================
// LSP shutdown on exit
// =============================================================================

/// Shutdown LSP servers on TUI exit.
///
/// Called before the TUI exits to ensure all LSP server processes are
/// properly terminated and don't become zombies.
pub async fn shutdown_lsp_on_exit(thread_manager: &Arc<ThreadManager>) {
    if let Some(lsp_manager) = thread_manager.get_lsp_manager() {
        tracing::info!("Shutting down LSP servers on exit");
        lsp_manager.shutdown_all().await;
    }
}
