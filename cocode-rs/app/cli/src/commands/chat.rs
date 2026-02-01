//! Chat command - start an interactive chat session.

use std::path::PathBuf;

use cocode_config::ConfigManager;
use cocode_session::Session;
use cocode_session::SessionState;

use crate::output;
use crate::repl::Repl;

/// Run the chat command in REPL mode.
///
/// # Arguments
///
/// * `initial_prompt` - Optional initial prompt for non-interactive mode
/// * `title` - Optional session title
/// * `max_turns` - Optional max turns limit
/// * `config` - Configuration manager
pub async fn run(
    initial_prompt: Option<String>,
    title: Option<String>,
    max_turns: Option<i32>,
    config: &ConfigManager,
) -> anyhow::Result<()> {
    // Get working directory
    let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    // Use current config (profile-based)
    let (provider_name, model_name) = config.current();

    // Get provider type from config
    let provider_type = config
        .resolve_provider(&provider_name)
        .map(|info| info.provider_type)
        .unwrap_or(cocode_protocol::ProviderType::OpenaiCompat);

    // Create session
    let mut session = Session::new(working_dir, &model_name, provider_type);
    session.provider = provider_name;

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
