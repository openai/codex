//! Chat command - start an interactive chat session.

use std::path::PathBuf;

use cocode_config::ConfigManager;
use cocode_protocol::ModelSpec;
use cocode_protocol::RoleSelection;
use cocode_session::Session;
use cocode_session::SessionState;
use tracing::info;
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;

use crate::output;
use crate::repl::Repl;

/// Initialize stderr logging for REPL mode.
///
/// If logging is already initialized, this will do nothing and return None.
fn init_repl_logging(config: &ConfigManager, verbose: bool) -> Option<()> {
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

    // Build stderr layer (timezone is handled inside the macro via ConfigurableTimer)
    let stderr_layer = cocode_utils_common::configure_fmt_layer!(
        fmt::layer().with_writer(std::io::stderr).compact(),
        &effective_logging,
        "warn"
    );

    match tracing_subscriber::registry().with(stderr_layer).try_init() {
        Ok(()) => Some(()),
        Err(_) => None, // Already initialized
    }
}

/// Run the chat command in REPL mode.
///
/// # Arguments
///
/// * `initial_prompt` - Optional initial prompt for non-interactive mode
/// * `title` - Optional session title
/// * `max_turns` - Optional max turns limit
/// * `config` - Configuration manager
/// * `verbose` - Enable verbose logging
pub async fn run(
    initial_prompt: Option<String>,
    title: Option<String>,
    max_turns: Option<i32>,
    config: &ConfigManager,
    verbose: bool,
) -> anyhow::Result<()> {
    // Initialize logging for REPL mode (stderr)
    let _ = init_repl_logging(config, verbose);

    info!("Starting REPL mode");

    // Get working directory
    let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    // Use current config (profile-based)
    let (provider_name, model_name) = config.current();

    // Get provider type from config
    let provider_type = config
        .resolve_provider(&provider_name)
        .map(|info| info.provider_type)
        .unwrap_or(cocode_protocol::ProviderType::OpenaiCompat);

    // Create session with the model spec
    let spec = ModelSpec::with_type(&provider_name, provider_type, &model_name);
    let selection = RoleSelection::new(spec);
    let mut session = Session::new(working_dir, selection);

    if let Some(t) = title {
        session.set_title(t);
    }
    if let Some(max) = max_turns {
        session.set_max_turns(Some(max));
    }

    // Create session state
    let mut state = SessionState::new(session, config).await?;

    // Handle initial prompt (non-interactive mode)
    if let Some(prompt) = initial_prompt {
        let result = state.run_turn(&prompt).await?;
        println!("{}", result.final_text);
        output::print_turn_summary(result.usage.input_tokens, result.usage.output_tokens);
        return Ok(());
    }

    // Interactive mode - run REPL
    let mut repl = Repl::new(&mut state);
    repl.run().await?;

    // Save session on exit (if not ephemeral)
    if !state.session.ephemeral {
        let session_id = state.session.id.clone();
        let path = cocode_session::persistence::session_file_path(&session_id);
        cocode_session::save_session_to_file(&state.session, state.history(), &path).await?;
        println!("Session saved: {session_id}");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_chat_module_compiles() {
        assert!(true);
    }
}
