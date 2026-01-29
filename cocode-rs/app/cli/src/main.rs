//! cocode - Multi-provider LLM CLI
//!
//! A command-line interface for interacting with multiple LLM providers.

mod commands;
mod output;
mod repl;

use clap::{Parser, Subcommand};
use cocode_config::ConfigManager;
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

/// Multi-provider LLM CLI
#[derive(Parser)]
#[command(name = "cocode", version, about = "Multi-provider LLM CLI")]
struct Cli {
    /// Subcommand to execute
    #[command(subcommand)]
    command: Option<Commands>,

    /// Model to use (e.g., gpt-5, claude-sonnet-4)
    #[arg(short, long, global = true)]
    model: Option<String>,

    /// Provider to use (e.g., openai, anthropic)
    #[arg(short, long, global = true)]
    provider: Option<String>,

    /// Prompt to execute (non-interactive mode)
    prompt: Option<String>,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    init_tracing(cli.verbose);

    info!("Starting cocode CLI");

    // Load configuration
    let config = ConfigManager::from_default()?;

    // Dispatch to appropriate command
    match cli.command {
        Some(Commands::Chat { title, max_turns }) => {
            commands::chat::run(
                cli.model,
                cli.provider,
                None, // No initial prompt for chat mode
                title,
                max_turns,
                &config,
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
                // Non-interactive mode: run single prompt
                commands::chat::run(
                    cli.model,
                    cli.provider,
                    Some(prompt),
                    None,
                    Some(1), // Single turn for prompt mode
                    &config,
                )
                .await
            } else {
                // Interactive mode: start chat
                commands::chat::run(cli.model, cli.provider, None, None, None, &config).await
            }
        }
    }
}

/// Initialize tracing with appropriate filters.
fn init_tracing(verbose: bool) {
    let filter = if verbose {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,cocode=debug"))
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"))
    };

    tracing_subscriber::registry()
        .with(fmt::layer().with_target(false).compact())
        .with(filter)
        .init();
}
