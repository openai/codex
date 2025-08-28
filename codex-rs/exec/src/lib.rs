mod cli;
mod event_processor;
mod event_processor_with_human_output;
mod event_processor_with_json_output;

use std::io::IsTerminal;
use std::io::Read;
use std::path::PathBuf;

pub use cli::Cli;
use codex_core::BUILT_IN_OSS_MODEL_PROVIDER_ID;
use codex_core::ConversationManager;
use codex_core::NewConversation;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_core::util::is_inside_git_repo;
use codex_login::AuthManager;
use codex_ollama::DEFAULT_OSS_MODEL;
use codex_protocol::config_types::SandboxMode;
use event_processor_with_human_output::EventProcessorWithHumanOutput;
use event_processor_with_json_output::EventProcessorWithJsonOutput;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::event_processor::CodexStatus;
use crate::event_processor::EventProcessor;
use codex_common::path_utils::is_image_extension;
use codex_common::path_utils::normalize_pasted_path;

/// Extract image paths from the prompt text and return cleaned text + image paths
/// This mimics the TUI's intelligent image detection logic
fn extract_image_paths_from_prompt(prompt: String) -> (String, Vec<PathBuf>) {
    let mut detected_images = Vec::new();
    let mut cleaned_words = Vec::new();

    // Split prompt into words and check each for potential image paths
    for word in prompt.split_whitespace() {
        if let Some(path_buf) = normalize_pasted_path(word) {
            // Pre-filter by extension before expensive image_dimensions check
            if is_image_extension(&path_buf) {
                // Check if it's actually an image file by trying to read its dimensions
                match image::image_dimensions(&path_buf) {
                    Ok(_) => {
                        tracing::info!("Detected image path: {}", path_buf.display());
                        detected_images.push(path_buf);
                        continue; // Skip adding this word to cleaned text
                    }
                    Err(err) => {
                        tracing::debug!(
                            "Path {} looks like image but isn't: {}",
                            path_buf.display(),
                            err
                        );
                    }
                }
            }
        }
        // Not an image path, keep the original word
        cleaned_words.push(word);
    }

    let cleaned_text = cleaned_words.join(" ");
    (cleaned_text, detected_images)
}

pub async fn run_main(cli: Cli, codex_linux_sandbox_exe: Option<PathBuf>) -> anyhow::Result<()> {
    let Cli {
        images,
        model: model_cli_arg,
        oss,
        config_profile,
        full_auto,
        dangerously_bypass_approvals_and_sandbox,
        cwd,
        skip_git_repo_check,
        color,
        last_message_file,
        json: json_mode,
        sandbox_mode: sandbox_mode_cli_arg,
        prompt,
        config_overrides,
    } = cli;

    // Determine the prompt based on CLI arg and/or stdin.
    let prompt = match prompt {
        Some(p) if p != "-" => p,
        // Either `-` was passed or no positional arg.
        maybe_dash => {
            // When no arg (None) **and** stdin is a TTY, bail out early – unless the
            // user explicitly forced reading via `-`.
            let force_stdin = matches!(maybe_dash.as_deref(), Some("-"));

            if std::io::stdin().is_terminal() && !force_stdin {
                eprintln!(
                    "No prompt provided. Either specify one as an argument or pipe the prompt into stdin."
                );
                std::process::exit(1);
            }

            // Ensure the user knows we are waiting on stdin, as they may
            // have gotten into this state by mistake. If so, and they are not
            // writing to stdin, Codex will hang indefinitely, so this should
            // help them debug in that case.
            if !force_stdin {
                eprintln!("Reading prompt from stdin...");
            }
            let mut buffer = String::new();
            if let Err(e) = std::io::stdin().read_to_string(&mut buffer) {
                eprintln!("Failed to read prompt from stdin: {e}");
                std::process::exit(1);
            } else if buffer.trim().is_empty() {
                eprintln!("No prompt provided via stdin.");
                std::process::exit(1);
            }
            buffer
        }
    };

    let (stdout_with_ansi, stderr_with_ansi) = match color {
        cli::Color::Always => (true, true),
        cli::Color::Never => (false, false),
        cli::Color::Auto => (
            std::io::stdout().is_terminal(),
            std::io::stderr().is_terminal(),
        ),
    };

    // TODO(mbolin): Take a more thoughtful approach to logging.
    let default_level = "error";
    let _ = tracing_subscriber::fmt()
        // Fallback to the `default_level` log filter if the environment
        // variable is not set _or_ contains an invalid value
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .or_else(|_| EnvFilter::try_new(default_level))
                .unwrap_or_else(|_| EnvFilter::new(default_level)),
        )
        .with_ansi(stderr_with_ansi)
        .with_writer(std::io::stderr)
        .try_init();

    let sandbox_mode = if full_auto {
        Some(SandboxMode::WorkspaceWrite)
    } else if dangerously_bypass_approvals_and_sandbox {
        Some(SandboxMode::DangerFullAccess)
    } else {
        sandbox_mode_cli_arg.map(Into::<SandboxMode>::into)
    };

    // When using `--oss`, let the bootstrapper pick the model (defaulting to
    // gpt-oss:20b) and ensure it is present locally. Also, force the built‑in
    // `oss` model provider.
    let model = if let Some(model) = model_cli_arg {
        Some(model)
    } else if oss {
        Some(DEFAULT_OSS_MODEL.to_owned())
    } else {
        None // No model specified, will use the default.
    };

    let model_provider = if oss {
        Some(BUILT_IN_OSS_MODEL_PROVIDER_ID.to_string())
    } else {
        None // No specific model provider override.
    };

    // Load configuration and determine approval policy
    let overrides = ConfigOverrides {
        model,
        config_profile,
        // This CLI is intended to be headless and has no affordances for asking
        // the user for approval.
        approval_policy: Some(AskForApproval::Never),
        sandbox_mode,
        cwd: cwd.map(|p| p.canonicalize().unwrap_or(p)),
        model_provider,
        codex_linux_sandbox_exe,
        base_instructions: None,
        include_plan_tool: None,
        include_apply_patch_tool: None,
        include_view_image_tool: None,
        disable_response_storage: oss.then_some(true),
        show_raw_agent_reasoning: oss.then_some(true),
        tools_web_search_request: None,
    };
    // Parse `-c` overrides.
    let cli_kv_overrides = match config_overrides.parse_overrides() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error parsing -c overrides: {e}");
            std::process::exit(1);
        }
    };

    let config = Config::load_with_cli_overrides(cli_kv_overrides, overrides)?;
    let mut event_processor: Box<dyn EventProcessor> = if json_mode {
        Box::new(EventProcessorWithJsonOutput::new(last_message_file.clone()))
    } else {
        Box::new(EventProcessorWithHumanOutput::create_with_ansi(
            stdout_with_ansi,
            &config,
            last_message_file.clone(),
        ))
    };

    if oss {
        codex_ollama::ensure_oss_ready(&config)
            .await
            .map_err(|e| anyhow::anyhow!("OSS setup failed: {e}"))?;
    }

    // Print the effective configuration and prompt so users can see what Codex
    // is using.
    event_processor.print_config_summary(&config, &prompt);

    if !skip_git_repo_check && !is_inside_git_repo(&config.cwd.to_path_buf()) {
        eprintln!("Not inside a trusted directory and --skip-git-repo-check was not specified.");
        std::process::exit(1);
    }

    let conversation_manager = ConversationManager::new(AuthManager::shared(
        config.codex_home.clone(),
        config.preferred_auth_method,
    ));
    let NewConversation {
        conversation_id: _,
        conversation,
        session_configured,
    } = conversation_manager.new_conversation(config).await?;
    info!("Codex initialized with event: {session_configured:?}");

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Event>();
    {
        let conversation = conversation.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {
                        tracing::debug!("Keyboard interrupt");
                        // Immediately notify Codex to abort any in‑flight task.
                        conversation.submit(Op::Interrupt).await.ok();

                        // Exit the inner loop and return to the main input prompt. The codex
                        // will emit a `TurnInterrupted` (Error) event which is drained later.
                        break;
                    }
                    res = conversation.next_event() => match res {
                        Ok(event) => {
                            debug!("Received event: {event:?}");

                            let is_shutdown_complete = matches!(event.msg, EventMsg::ShutdownComplete);
                            if let Err(e) = tx.send(event) {
                                error!("Error sending event: {e:?}");
                                break;
                            }
                            if is_shutdown_complete {
                                info!("Received shutdown event, exiting event loop.");
                                break;
                            }
                        },
                        Err(e) => {
                            error!("Error receiving event: {e:?}");
                            break;
                        }
                    }
                }
            }
        });
    }

    // Parse the prompt for image paths and combine with --image flag images
    let mut all_images = images;
    let (prompt_text, detected_images) = extract_image_paths_from_prompt(prompt);
    all_images.extend(detected_images);

    // Build input items: images first, then text
    let mut items: Vec<InputItem> = Vec::new();

    // Add images as InputItems
    for path in all_images {
        items.push(InputItem::LocalImage { path });
    }

    // Add text content
    if !prompt_text.trim().is_empty() {
        items.push(InputItem::Text { text: prompt_text });
    }

    // Send all content together in a single request (no separate image handling to avoid race conditions)
    let initial_task_id = conversation.submit(Op::UserInput { items }).await?;
    info!("Sent input with event ID: {initial_task_id}");

    // Run the loop until the task is complete.
    while let Some(event) = rx.recv().await {
        let shutdown: CodexStatus = event_processor.process_event(event);
        match shutdown {
            CodexStatus::Running => continue,
            CodexStatus::InitiateShutdown => {
                conversation.submit(Op::Shutdown).await?;
            }
            CodexStatus::Shutdown => {
                break;
            }
        }
    }

    Ok(())
}
