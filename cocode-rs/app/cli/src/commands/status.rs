//! Status command - show current configuration.

use cocode_config::ConfigManager;
use cocode_protocol::all_features;

/// Run the status command.
pub async fn run(config: &ConfigManager) -> anyhow::Result<()> {
    let (provider, model) = config.current();

    println!("Current Configuration");
    println!("─────────────────────");
    println!("Provider: {provider}");
    println!("Model:    {model}");

    // Show config path
    let config_path = config.config_path();
    println!("Config:   {}", config_path.display());

    // Show features summary
    let features = config.features();
    let enabled_count = all_features()
        .filter(|spec| features.enabled(spec.id))
        .count();
    println!("Features: {enabled_count} enabled");

    Ok(())
}
