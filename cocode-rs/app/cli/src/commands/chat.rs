//! Chat command - start an interactive chat session.

use std::path::PathBuf;

use cocode_config::ConfigManager;
use cocode_protocol::ProviderType;
use cocode_session::{Session, SessionState};

use crate::output;
use crate::repl::Repl;

/// Run the chat command.
///
/// # Arguments
///
/// * `model` - Optional model override
/// * `provider` - Optional provider override
/// * `initial_prompt` - Optional initial prompt for non-interactive mode
/// * `title` - Optional session title
/// * `max_turns` - Optional max turns limit
/// * `config` - Configuration manager
pub async fn run(
    model: Option<String>,
    provider: Option<String>,
    initial_prompt: Option<String>,
    title: Option<String>,
    max_turns: Option<i32>,
    config: &ConfigManager,
) -> anyhow::Result<()> {
    // Get working directory
    let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    // Resolve provider and model
    let (resolved_provider, resolved_model) =
        resolve_provider_model(provider.as_deref(), model.as_deref(), config)?;

    // Get provider type from config (resolves correctly from registered providers)
    let provider_type = config
        .resolve_provider(&resolved_provider)
        .map(|info| info.provider_type)
        .unwrap_or(ProviderType::OpenaiCompat);

    // Create session
    let mut session = Session::new(working_dir, &resolved_model, provider_type);
    session.provider = resolved_provider.clone();

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

/// Resolve provider and model from options and config.
fn resolve_provider_model(
    provider: Option<&str>,
    model: Option<&str>,
    config: &ConfigManager,
) -> anyhow::Result<(String, String)> {
    match (provider, model) {
        // Both specified
        (Some(p), Some(m)) => Ok((p.to_string(), m.to_string())),

        // Only provider - use its default model
        (Some(p), None) => {
            let models = config.list_models(p);
            if models.is_empty() {
                return Err(anyhow::anyhow!("No models found for provider: {p}"));
            }
            Ok((p.to_string(), models[0].id.clone()))
        }

        // Only model - infer provider from current config
        (None, Some(m)) => {
            let (current_provider, _) = config.current();
            Ok((current_provider, m.to_string()))
        }

        // Neither - use current config
        (None, None) => {
            let (p, m) = config.current();
            Ok((p, m))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_provider_model_both_specified() {
        let config = ConfigManager::empty();
        let result = resolve_provider_model(Some("openai"), Some("gpt-4"), &config);
        assert!(result.is_ok());
        let (provider, model) = result.unwrap();
        assert_eq!(provider, "openai");
        assert_eq!(model, "gpt-4");
    }

    #[test]
    fn test_resolve_provider_model_neither_specified() {
        let config = ConfigManager::empty();
        let result = resolve_provider_model(None, None, &config);
        assert!(result.is_ok());
    }
}
