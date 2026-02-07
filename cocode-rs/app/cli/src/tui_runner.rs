//! TUI runner - integrates cocode-tui with the CLI.
//!
//! This module provides the bridge between the CLI and the TUI,
//! setting up channels and running the TUI event loop.

use std::fs::OpenOptions;
use std::path::PathBuf;

use cocode_config::ConfigManager;
use cocode_protocol::LoopError;
use cocode_protocol::LoopEvent;
use cocode_protocol::ModelSpec;
use cocode_protocol::ProviderType;
use cocode_protocol::RoleSelection;
use cocode_protocol::SubmissionId;
use cocode_protocol::TokenUsage;
use cocode_protocol::model::ModelRole;
use cocode_session::Session;
use cocode_tui::App;
use cocode_tui::AppConfig;
use cocode_tui::UserCommand;
use cocode_tui::create_channels;
use tokio::sync::mpsc;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;

/// Initialize file logging for TUI mode.
///
/// Logs are written to `~/.cocode/log/cocode-tui.log`.
/// Returns a WorkerGuard that must be kept alive for the duration of the program.
fn init_tui_logging(config: &ConfigManager, verbose: bool) -> Option<WorkerGuard> {
    // Get logging config
    let logging_config = config.logging_config();
    let common_logging = logging_config
        .map(|c| c.to_common_logging())
        .unwrap_or_default();

    // Override level if verbose flag is set
    let effective_logging = if verbose {
        cocode_utils_common::LoggingConfig {
            level: "info,cocode=debug".to_string(),
            ..common_logging
        }
    } else {
        common_logging
    };

    // Create log directory
    let log_dir = cocode_config::log_dir();
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!("Warning: Could not create log directory {log_dir:?}: {e}");
        return None;
    }

    // Open log file with append mode and restrictive permissions
    let mut log_file_opts = OpenOptions::new();
    log_file_opts.create(true).append(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        log_file_opts.mode(0o600);
    }

    let log_path = log_dir.join("cocode-tui.log");
    let log_file = match log_file_opts.open(&log_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Warning: Could not open log file {log_path:?}: {e}");
            return None;
        }
    };

    // Wrap file in non-blocking writer
    let (non_blocking, guard) = tracing_appender::non_blocking(log_file);

    // Build file layer (timezone is handled inside the macro via ConfigurableTimer)
    let file_layer = cocode_utils_common::configure_fmt_layer!(
        fmt::layer().with_writer(non_blocking).with_ansi(false),
        &effective_logging,
        "info"
    );

    match tracing_subscriber::registry().with(file_layer).try_init() {
        Ok(()) => Some(guard),
        Err(_) => None, // Already initialized
    }
}

/// Run the TUI interface.
///
/// This sets up the TUI with channels for communicating with the agent loop.
pub async fn run_tui(
    title: Option<String>,
    config: &ConfigManager,
    verbose: bool,
    system_prompt_suffix: Option<String>,
) -> anyhow::Result<()> {
    // Initialize file logging for TUI mode
    let _logging_guard = init_tui_logging(config, verbose);

    info!("Starting TUI mode");

    // Get current model/provider from config
    let (provider_name, model_name) = config.current();

    // Get available models for the picker
    let available_models = config
        .list_models(&provider_name)
        .iter()
        .map(|m| m.id.clone())
        .collect::<Vec<_>>();

    // Get working directory
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    // Create TUI config
    let tui_config = AppConfig {
        model: model_name.clone(),
        available_models: if available_models.is_empty() {
            vec![model_name.clone()]
        } else {
            available_models
        },
        cwd: cwd.clone(),
    };

    // Create channels for TUI-Agent communication
    let (agent_tx, agent_rx, command_tx, command_rx) = create_channels(256);

    // Create and run the TUI
    let mut app = App::new(agent_rx, command_tx.clone(), tui_config)
        .map_err(|e| anyhow::anyhow!("Failed to create TUI: {e}"))?;

    // Spawn a task to handle user commands and drive the agent
    let agent_handle = tokio::spawn(run_agent_driver(
        command_rx,
        agent_tx,
        config.clone(),
        provider_name,
        model_name,
        title,
        cwd,
        system_prompt_suffix,
    ));

    // Run the TUI (blocks until exit)
    let tui_result = app.run().await;

    // Wait for agent driver to finish
    let _ = agent_handle.await;

    tui_result.map_err(|e| anyhow::anyhow!("TUI error: {e}"))
}

/// Agent driver that handles user commands and sends events to TUI.
async fn run_agent_driver(
    mut command_rx: mpsc::Receiver<UserCommand>,
    event_tx: mpsc::Sender<LoopEvent>,
    config: ConfigManager,
    provider_name: String,
    model_name: String,
    title: Option<String>,
    working_dir: PathBuf,
    system_prompt_suffix: Option<String>,
) {
    info!("Agent driver started");

    // Get provider type
    let provider_type = config
        .resolve_provider(&provider_name)
        .map(|info| info.provider_type)
        .unwrap_or(cocode_protocol::ProviderType::OpenaiCompat);

    // Create session with model spec
    let spec = ModelSpec::with_type(&provider_name, provider_type, &model_name);
    let selection = RoleSelection::new(spec);
    let mut session = Session::new(working_dir.clone(), selection);
    if let Some(t) = title {
        session.set_title(t);
    }

    // Create session state
    let state_result = cocode_session::SessionState::new(session, &config).await;
    let mut state = match state_result {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to create session: {e}");
            let _ = event_tx
                .send(LoopEvent::Error {
                    error: LoopError {
                        code: "session_error".to_string(),
                        message: format!("Failed to create session: {e}"),
                        recoverable: false,
                    },
                })
                .await;
            return;
        }
    };

    // Set system prompt suffix if provided
    if let Some(suffix) = system_prompt_suffix {
        state.set_system_prompt_suffix(suffix);
    }

    let plan_file = working_dir.join(".cocode/plan.md");

    // Track current correlation ID for turn-related events.
    // This can be used in future to wrap LoopEvents with CorrelatedEvent
    // for request-response tracking.
    #[allow(unused_assignments)]
    let mut _current_correlation_id: Option<SubmissionId> = None;

    let mut turn_counter = 0;
    while let Some(command) = command_rx.recv().await {
        // Generate correlation ID for commands that trigger turns
        let correlation_id = if command.triggers_turn() {
            let id = SubmissionId::new();
            debug!(correlation_id = %id, "Generated correlation ID for command");
            Some(id)
        } else {
            None
        };

        match command {
            UserCommand::SubmitInput {
                content,
                display_text,
            } => {
                // Extract text content for the agent (for now, we concatenate text blocks)
                // TODO: Support multimodal content in the agent
                let message: String = content
                    .iter()
                    .filter_map(|block| block.as_text())
                    .collect::<Vec<_>>()
                    .join("");

                info!(
                    input_len = message.len(),
                    display_len = display_text.len(),
                    content_blocks = content.len(),
                    correlation_id = ?correlation_id.as_ref().map(|id| id.as_str()),
                    "Processing user input"
                );

                // Track the correlation ID for this turn's events
                _current_correlation_id = correlation_id.clone();

                turn_counter += 1;
                let turn_id = format!("turn-{turn_counter}");

                let _ = event_tx
                    .send(LoopEvent::TurnStarted {
                        turn_id: turn_id.clone(),
                        turn_number: turn_counter,
                    })
                    .await;

                let _ = event_tx.send(LoopEvent::StreamRequestStart).await;

                match run_turn_with_events(&mut state, &message, &event_tx, &turn_id).await {
                    Ok(usage) => {
                        let _ = event_tx
                            .send(LoopEvent::StreamRequestEnd {
                                usage: usage.clone(),
                            })
                            .await;
                        let _ = event_tx
                            .send(LoopEvent::TurnCompleted { turn_id, usage })
                            .await;
                    }
                    Err(e) => {
                        error!("Turn failed: {e}");
                        let _ = event_tx
                            .send(LoopEvent::Error {
                                error: LoopError {
                                    code: "turn_error".to_string(),
                                    message: e.to_string(),
                                    recoverable: true,
                                },
                            })
                            .await;
                    }
                }
            }
            UserCommand::Interrupt => {
                info!("Interrupt requested");
                state.cancel();
                let _ = event_tx.send(LoopEvent::Interrupted).await;
            }
            UserCommand::Shutdown => {
                info!("Shutdown requested");
                break;
            }
            UserCommand::SetPlanMode { active } => {
                info!(active, "Plan mode changed");
                if active {
                    let _ = event_tx
                        .send(LoopEvent::PlanModeEntered {
                            plan_file: plan_file.clone(),
                        })
                        .await;
                } else {
                    let _ = event_tx
                        .send(LoopEvent::PlanModeExited { approved: false })
                        .await;
                }
            }
            UserCommand::SetThinkingLevel { level } => {
                info!(?level, "Thinking level changed");
                if let Err(e) = config.switch_thinking_level(ModelRole::Main, level.clone()) {
                    warn!(error = %e, "Failed to update thinking level in config");
                }
                state.switch_thinking_level(ModelRole::Main, level);
            }
            UserCommand::SetModel { model } => {
                info!(model, "Model changed");
                let (new_provider, new_model) = if model.contains('/') {
                    let parts: Vec<&str> = model.splitn(2, '/').collect();
                    (parts[0].to_string(), parts[1].to_string())
                } else {
                    (state.provider().to_string(), model.clone())
                };

                if let Err(e) = config.switch(&new_provider, &new_model) {
                    error!(error = %e, "Failed to switch model");
                    let _ = event_tx
                        .send(LoopEvent::Error {
                            error: LoopError {
                                code: "model_switch_error".to_string(),
                                message: format!("Failed to switch model: {e}"),
                                recoverable: true,
                            },
                        })
                        .await;
                    continue;
                }

                let new_provider_type = config
                    .resolve_provider(&new_provider)
                    .map(|info| info.provider_type)
                    .unwrap_or(ProviderType::OpenaiCompat);

                let spec = ModelSpec::with_type(&new_provider, new_provider_type, &new_model);
                let selection = RoleSelection::new(spec);
                let new_session = Session::new(working_dir.clone(), selection);

                match cocode_session::SessionState::new(new_session, &config).await {
                    Ok(new_state) => {
                        state = new_state;
                        info!(
                            provider = new_provider,
                            model = new_model,
                            "Model switched successfully"
                        );
                    }
                    Err(e) => {
                        error!(error = %e, "Failed to create session with new model");
                        let _ = event_tx
                            .send(LoopEvent::Error {
                                error: LoopError {
                                    code: "model_switch_error".to_string(),
                                    message: format!(
                                        "Failed to create session with new model: {e}"
                                    ),
                                    recoverable: true,
                                },
                            })
                            .await;
                    }
                }
            }
            UserCommand::ApprovalResponse {
                request_id,
                approved,
                remember,
            } => {
                info!(request_id, approved, remember, "Approval response received");
                let _ = event_tx
                    .send(LoopEvent::ApprovalResponse {
                        request_id,
                        approved,
                    })
                    .await;
            }
            UserCommand::ExecuteSkill { name, args } => {
                info!(
                    name, args,
                    correlation_id = ?correlation_id.as_ref().map(|id| id.as_str()),
                    "Skill execution requested"
                );

                // Track the correlation ID for this turn's events
                _current_correlation_id = correlation_id.clone();

                let message = if args.is_empty() {
                    format!("/{name}")
                } else {
                    format!("/{name} {args}")
                };

                turn_counter += 1;
                let turn_id = format!("turn-{turn_counter}");

                let _ = event_tx
                    .send(LoopEvent::TurnStarted {
                        turn_id: turn_id.clone(),
                        turn_number: turn_counter,
                    })
                    .await;

                let _ = event_tx.send(LoopEvent::StreamRequestStart).await;

                match run_turn_with_events(&mut state, &message, &event_tx, &turn_id).await {
                    Ok(usage) => {
                        let _ = event_tx
                            .send(LoopEvent::StreamRequestEnd {
                                usage: usage.clone(),
                            })
                            .await;
                        let _ = event_tx
                            .send(LoopEvent::TurnCompleted { turn_id, usage })
                            .await;
                    }
                    Err(e) => {
                        error!("Skill execution failed: {e}");
                        let _ = event_tx
                            .send(LoopEvent::Error {
                                error: LoopError {
                                    code: "skill_error".to_string(),
                                    message: e.to_string(),
                                    recoverable: true,
                                },
                            })
                            .await;
                    }
                }
            }
            UserCommand::QueueCommand { prompt } => {
                // Queue command for real-time steering and post-idle processing.
                // The command is:
                // 1. Injected as `<system-reminder>User sent: {message}</system-reminder>` for steering
                // 2. Executed as a new user turn after the current turn completes
                let id = state.queue_command(&prompt);
                info!(
                    prompt_len = prompt.len(),
                    queued_count = state.queued_count(),
                    "Command queued for steering and post-idle execution"
                );
                let preview = if prompt.len() > 30 {
                    format!("{}...", &prompt[..30])
                } else {
                    prompt.clone()
                };
                let _ = event_tx
                    .send(LoopEvent::CommandQueued { id, preview })
                    .await;
            }
            UserCommand::ClearQueues => {
                state.clear_queued_commands();
                info!("Cleared all queued commands");
                let _ = event_tx
                    .send(LoopEvent::QueueStateChanged { queued: 0 })
                    .await;
            }
        }
    }

    info!("Agent driver stopped");
}

async fn run_turn_with_events(
    state: &mut cocode_session::SessionState,
    input: &str,
    event_tx: &mpsc::Sender<LoopEvent>,
    _turn_id: &str,
) -> anyhow::Result<TokenUsage> {
    let result = state.run_turn_streaming(input, event_tx.clone()).await?;
    Ok(result.usage)
}
