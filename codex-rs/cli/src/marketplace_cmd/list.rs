use super::metadata;
use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use codex_core::config::find_codex_home;
use codex_core::plugins::marketplace_install_root;

#[derive(Debug, Parser)]
pub(super) struct ListMarketplaceArgs {
    /// Print the configured marketplaces as JSON.
    #[arg(long)]
    json: bool,
}

pub(super) async fn run_list(args: ListMarketplaceArgs) -> Result<()> {
    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
    let install_root = marketplace_install_root(&codex_home);
    let marketplaces = metadata::configured_marketplaces(&codex_home, &install_root)?;

    if args.json {
        let json = marketplaces
            .into_iter()
            .map(|marketplace| {
                let install_metadata = marketplace.install_metadata;
                let mut json = serde_json::Map::new();
                json.insert(
                    "name".to_string(),
                    serde_json::Value::String(marketplace.name),
                );
                json.insert(
                    "sourceType".to_string(),
                    serde_json::Value::String(install_metadata.config_source_type().to_string()),
                );
                match install_metadata.config_source_type() {
                    "directory" => {
                        json.insert(
                            "path".to_string(),
                            serde_json::Value::String(install_metadata.config_source()),
                        );
                    }
                    "git" => {
                        json.insert(
                            "url".to_string(),
                            serde_json::Value::String(install_metadata.config_source()),
                        );
                    }
                    _ => {}
                }
                json.insert(
                    "installLocation".to_string(),
                    serde_json::Value::String(marketplace.install_root.display().to_string()),
                );
                serde_json::Value::Object(json)
            })
            .collect::<Vec<_>>();
        let output = serde_json::to_string_pretty(&json)?;
        println!("{output}");
        return Ok(());
    }

    if marketplaces.is_empty() {
        println!("No marketplaces configured.");
        return Ok(());
    }

    for marketplace in marketplaces {
        println!("{}", marketplace.name);
        println!(
            "  Source: {} {}",
            marketplace.install_metadata.config_source_type(),
            marketplace.install_metadata.source_display()
        );
        println!("  Install root: {}", marketplace.install_root.display());
    }

    Ok(())
}
