//! cocode - Multi-provider LLM CLI
//!
//! A command-line interface for interacting with multiple LLM providers.
//!
//! This binary uses the arg0 dispatcher for single-binary deployment,
//! supporting apply_patch and sandbox invocation via PATH hijacking.

mod commands;
mod output;
mod repl;
mod tui_runner;

use std::path::PathBuf;

use clap::Parser;
use clap::Subcommand;
use cocode_config::ConfigManager;

/// Multi-provider LLM CLI
#[derive(Parser)]
#[command(name = "cocode", version, about = "Multi-provider LLM CLI")]
struct Cli {
    /// Subcommand to execute
    #[command(subcommand)]
    command: Option<Commands>,

    /// Configuration profile to use
    #[arg(short, long, global = true)]
    profile: Option<String>,

    /// Prompt to execute (non-interactive mode)
    prompt: Option<String>,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Disable TUI mode (use simple REPL instead)
    #[arg(long, global = true)]
    no_tui: bool,

    /// Append additional text to the end of the system prompt.
    #[arg(long, global = true)]
    system_prompt_suffix: Option<String>,
}

/// Config subcommands
#[derive(Subcommand)]
pub enum ConfigAction {
    /// Show current configuration
    Show,
    /// List available providers and models
    List,
    /// Set a configuration value
    Set {
        /// Configuration key (model, provider)
        key: String,
        /// Value to set
        value: String,
    },
}

/// Available subcommands
#[derive(Subcommand)]
enum Commands {
    /// Start an interactive chat session
    Chat {
        /// Session title
        #[arg(short, long)]
        title: Option<String>,

        /// Maximum turns before stopping
        #[arg(long)]
        max_turns: Option<i32>,
    },

    /// Configure providers and settings
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Resume a previous session
    Resume {
        /// Session ID to resume
        session_id: String,
    },

    /// List sessions
    Sessions {
        /// Show all sessions (including completed)
        #[arg(short, long)]
        all: bool,
    },

    /// Show current model and provider
    Status,
}

fn main() -> anyhow::Result<()> {
    // Use arg0 dispatcher for single-binary deployment.
    // This handles:
    // - argv[0] dispatch: apply_patch, cocode-linux-sandbox
    // - argv[1] hijack: --cocode-run-as-apply-patch
    // - PATH setup with symlinks for subprocess integration
    // - dotenv loading from ~/.cocode/.env
    cocode_arg0::arg0_dispatch_or_else(cli_main)
}

/// Main CLI entry point (runs inside Tokio runtime created by arg0).
///
/// Note: Logging is NOT initialized here. Instead:
/// - TUI mode: Initializes file logging in tui_runner.rs
/// - REPL mode: Initializes stderr logging in commands/chat.rs
async fn cli_main(_sandbox_exe: Option<PathBuf>) -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Load configuration first
    let config = ConfigManager::from_default()?;

    // Apply profile if specified
    if let Some(profile) = &cli.profile {
        match config.set_profile(profile) {
            Ok(true) => {
                // Profile applied successfully - will log after tracing is initialized
            }
            Ok(false) => {
                eprintln!("Warning: Profile '{profile}' not found in config, using defaults");
            }
            Err(e) => {
                eprintln!("Error setting profile: {e}");
            }
        }
    }

    // Dispatch to appropriate command
    match cli.command {
        Some(Commands::Chat { title, max_turns }) => {
            run_interactive(
                None, // No initial prompt for chat mode
                title,
                max_turns,
                &config,
                cli.no_tui,
                cli.verbose,
                cli.system_prompt_suffix,
            )
            .await
        }
        Some(Commands::Config { action }) => commands::config::run(action, &config).await,
        Some(Commands::Resume { session_id }) => commands::resume::run(&session_id, &config).await,
        Some(Commands::Sessions { all }) => commands::sessions::run(all, &config).await,
        Some(Commands::Status) => commands::status::run(&config).await,
        None => {
            // No subcommand - either run prompt or start interactive chat
            if let Some(prompt) = cli.prompt {
                // Non-interactive mode: run single prompt (always uses REPL mode)
                run_interactive(
                    Some(prompt),
                    None,
                    Some(1), // Single turn for prompt mode
                    &config,
                    true, // Force no-tui for single prompt
                    cli.verbose,
                    cli.system_prompt_suffix,
                )
                .await
            } else {
                // Interactive mode: start chat (use TUI by default)
                run_interactive(
                    None,
                    None,
                    None,
                    &config,
                    cli.no_tui,
                    cli.verbose,
                    cli.system_prompt_suffix,
                )
                .await
            }
        }
    }
}

/// Run interactive mode (TUI or REPL).
async fn run_interactive(
    initial_prompt: Option<String>,
    title: Option<String>,
    max_turns: Option<i32>,
    config: &ConfigManager,
    no_tui: bool,
    verbose: bool,
    system_prompt_suffix: Option<String>,
) -> anyhow::Result<()> {
    // For single prompt or explicit --no-tui, use REPL mode
    if initial_prompt.is_some() || no_tui {
        return commands::chat::run(
            initial_prompt,
            title,
            max_turns,
            config,
            verbose,
            system_prompt_suffix,
        )
        .await;
    }

    // Interactive mode: use TUI
    tui_runner::run_tui(title, config, verbose, system_prompt_suffix).await
}
