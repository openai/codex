//! TUI runner - integrates cocode-tui with the CLI.
//!
//! This module provides the bridge between the CLI and the TUI,
//! setting up channels and running the TUI event loop.

use std::path::PathBuf;

use cocode_config::ConfigManager;
use cocode_protocol::LoopError;
use cocode_protocol::LoopEvent;
use cocode_protocol::ProviderType;
use cocode_protocol::TokenUsage;
use cocode_protocol::model::ModelRole;
use cocode_session::Session;
use cocode_tui::App;
use cocode_tui::AppConfig;
use cocode_tui::UserCommand;
use cocode_tui::create_channels;
use tokio::sync::mpsc;
use tracing::error;
use tracing::info;
use tracing::warn;

/// Run the TUI interface.
///
/// This sets up the TUI with channels for communicating with the agent loop.
pub async fn run_tui(title: Option<String>, config: &ConfigManager) -> anyhow::Result<()> {
    info!("Starting TUI mode");

    // Get current model/provider from config
    let (provider_name, model_name) = config.current();

    // Get available models for the picker
    let available_models = config
        .list_models(&provider_name)
        .iter()
        .map(|m| m.id.clone())
        .collect::<Vec<_>>();

    // Create TUI config
    let tui_config = AppConfig {
        model: model_name.clone(),
        available_models: if available_models.is_empty() {
            vec![model_name.clone()]
        } else {
            available_models
        },
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
) {
    info!("Agent driver started");

    // Get working directory
    let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    // Get provider type
    let provider_type = config
        .resolve_provider(&provider_name)
        .map(|info| info.provider_type)
        .unwrap_or(cocode_protocol::ProviderType::OpenaiCompat);

    // Create session
    let mut session = Session::new(working_dir.clone(), &model_name, provider_type);
    session.provider = provider_name.clone();
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

    // Track plan mode state locally (Session doesn't have plan_mode field yet)
    let plan_file = working_dir.join(".cocode/plan.md");

    // Handle commands from TUI
    let mut turn_counter = 0;
    while let Some(command) = command_rx.recv().await {
        match command {
            UserCommand::SubmitInput { message } => {
                info!(input_len = message.len(), "Processing user input");

                turn_counter += 1;
                let turn_id = format!("turn-{turn_counter}");

                // Signal turn start
                let _ = event_tx
                    .send(LoopEvent::TurnStarted {
                        turn_id: turn_id.clone(),
                        turn_number: turn_counter,
                    })
                    .await;

                // Signal request start
                let _ = event_tx.send(LoopEvent::StreamRequestStart).await;

                // Run turn and stream events
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
                // Update runtime config
                if let Err(e) = config.switch_thinking_level(ModelRole::Main, level.clone()) {
                    warn!(error = %e, "Failed to update thinking level in config");
                }
                // Update session state
                state.switch_thinking_level(ModelRole::Main, level);
            }
            UserCommand::SetModel { model } => {
                info!(model, "Model changed");
                // Parse model spec (format: "provider/model" or just "model")
                let (new_provider, new_model) = if model.contains('/') {
                    let parts: Vec<&str> = model.splitn(2, '/').collect();
                    (parts[0].to_string(), parts[1].to_string())
                } else {
                    // Use current provider
                    (state.provider().to_string(), model.clone())
                };

                // Validate and switch in config
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

                // Resolve provider type for the new model
                let new_provider_type = config
                    .resolve_provider(&new_provider)
                    .map(|info| info.provider_type)
                    .unwrap_or(ProviderType::OpenaiCompat);

                // Create new session with the new model
                let mut new_session =
                    Session::new(working_dir.clone(), &new_model, new_provider_type);
                new_session.provider = new_provider.clone();

                // Recreate session state
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
        }
    }

    info!("Agent driver stopped");
}

/// Run a turn and send events to TUI.
///
/// This uses real streaming via the AgentLoop, forwarding all events
/// to the TUI through the provided event channel.
async fn run_turn_with_events(
    state: &mut cocode_session::SessionState,
    input: &str,
    event_tx: &mpsc::Sender<LoopEvent>,
    _turn_id: &str,
) -> anyhow::Result<TokenUsage> {
    // Use real streaming via SessionState::run_turn_streaming
    // The AgentLoop sends events directly to the TUI via event_tx
    let result = state.run_turn_streaming(input, event_tx.clone()).await?;

    Ok(result.usage)
}
