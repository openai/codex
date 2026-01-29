//! Config command - manage configuration.

use cocode_config::ConfigManager;
use cocode_protocol::all_features;

use crate::ConfigAction;

/// Run the config command.
pub async fn run(action: ConfigAction, config: &ConfigManager) -> anyhow::Result<()> {
    match action {
        ConfigAction::Show => show_config(config),
        ConfigAction::List => list_providers(config),
        ConfigAction::Set { key, value } => set_config(&key, &value, config),
    }
}

/// Show current configuration.
fn show_config(config: &ConfigManager) -> anyhow::Result<()> {
    let (provider, model) = config.current();

    println!("Configuration");
    println!("─────────────");
    println!();
    println!("Current:");
    println!("  Provider: {provider}");
    println!("  Model:    {model}");
    println!();
    println!("Config Path: {}", config.config_path().display());
    println!();

    // Show features
    println!("Features:");
    let features = config.features();
    for spec in all_features() {
        let enabled = features.enabled(spec.id);
        let status = if enabled { "✓" } else { "✗" };
        println!("  {status} {}", spec.key);
    }

    Ok(())
}

/// List available providers and models.
fn list_providers(config: &ConfigManager) -> anyhow::Result<()> {
    println!("Providers");
    println!("─────────");
    println!();

    let providers = config.list_providers();
    if providers.is_empty() {
        println!("No providers configured.");
        return Ok(());
    }

    for provider in providers {
        let key_status = if provider.has_api_key { "✓" } else { "✗" };
        println!(
            "{} {} ({})",
            key_status, provider.name, provider.provider_type
        );

        // List models for this provider
        let models = config.list_models(&provider.name);
        for model in models {
            let ctx = model
                .context_window
                .map(|c| format!(" ({c}k ctx)"))
                .unwrap_or_default();
            println!("    - {}{}", model.id, ctx);
        }
        println!();
    }

    Ok(())
}

/// Set a configuration value.
fn set_config(key: &str, value: &str, config: &ConfigManager) -> anyhow::Result<()> {
    match key {
        "model" => {
            // Parse provider/model format
            if let Some((provider, model)) = value.split_once('/') {
                config.switch(provider, model)?;
                println!("Switched to {provider}/{model}");
            } else {
                // Just model name, use current provider
                let (current_provider, _) = config.current();
                config.switch(&current_provider, value)?;
                println!("Switched to {current_provider}/{value}");
            }
        }
        "provider" => {
            let models = config.list_models(value);
            if models.is_empty() {
                return Err(anyhow::anyhow!("No models found for provider: {value}"));
            }
            let default_model = &models[0].id;
            config.switch(value, default_model)?;
            println!("Switched to {value}/{default_model}");
        }
        _ => {
            println!("Unknown config key: {key}");
            println!();
            println!("Available keys:");
            println!("  model    - Set current model (format: provider/model or model)");
            println!("  provider - Set current provider");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_show_config() {
        let config = ConfigManager::empty();
        let result = show_config(&config);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_providers_empty() {
        let config = ConfigManager::empty();
        let result = list_providers(&config);
        assert!(result.is_ok());
    }
}
