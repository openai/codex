use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use clap::Parser;
use codex_core::config::Config;
use codex_core::config::find_codex_home;
use codex_core_plugins::ConfiguredMarketplace;
use codex_core_plugins::PluginInstallRequest;
use codex_core_plugins::PluginsConfigInput;
use codex_core_plugins::PluginsManager;
use codex_plugin::PluginId;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_cli::CliConfigOverrides;

use crate::marketplace_cmd::MarketplaceCli;

#[derive(Debug, Parser)]
#[command(bin_name = "codex plugin")]
pub struct PluginCli {
    #[clap(flatten)]
    pub config_overrides: CliConfigOverrides,

    #[command(subcommand)]
    pub subcommand: PluginSubcommand,
}

#[derive(Debug, clap::Subcommand)]
pub enum PluginSubcommand {
    /// Install a plugin from a marketplace.
    Add(AddPluginArgs),

    /// List marketplace plugins.
    List(ListPluginsArgs),

    /// Manage plugin marketplaces for Codex.
    Marketplace(MarketplaceCli),

    /// Remove an installed plugin.
    Remove(RemovePluginArgs),
}

#[derive(Debug, Parser)]
#[command(bin_name = "codex plugin add")]
pub struct AddPluginArgs {
    /// Plugin to install. Accepts <plugin> with --marketplace or <plugin>@<marketplace>.
    plugin: String,

    /// Marketplace name containing the plugin.
    #[arg(long = "marketplace", short = 'm')]
    marketplace_name: Option<String>,
}

#[derive(Debug, Parser)]
#[command(bin_name = "codex plugin list")]
pub struct ListPluginsArgs {
    /// Only list plugins in this marketplace.
    #[arg(long = "marketplace", short = 'm')]
    marketplace_name: Option<String>,
}

#[derive(Debug, Parser)]
#[command(bin_name = "codex plugin remove")]
pub struct RemovePluginArgs {
    /// Plugin to remove. Accepts <plugin> with --marketplace or <plugin>@<marketplace>.
    plugin: String,

    /// Marketplace name containing the plugin.
    #[arg(long = "marketplace", short = 'm')]
    marketplace_name: Option<String>,
}

pub async fn run_plugin_add(
    overrides: Vec<(String, toml::Value)>,
    args: AddPluginArgs,
) -> Result<()> {
    let PluginCommandContext {
        plugins_input,
        manager,
    } = load_plugin_command_context(overrides).await?;
    let PluginSelection {
        plugin_name,
        marketplace_name,
        ..
    } = parse_plugin_selection(args.plugin, args.marketplace_name)?;
    let marketplace =
        find_marketplace_for_plugin(&manager, &plugins_input, &marketplace_name, &plugin_name)?;
    let outcome = manager
        .install_plugin(PluginInstallRequest {
            plugin_name,
            marketplace_path: marketplace.path,
        })
        .await?;

    println!(
        "Added plugin `{}` from marketplace `{}`.",
        outcome.plugin_id.plugin_name, outcome.plugin_id.marketplace_name
    );
    println!(
        "Installed plugin root: {}",
        outcome.installed_path.as_path().display()
    );

    Ok(())
}

pub async fn run_plugin_list(
    overrides: Vec<(String, toml::Value)>,
    args: ListPluginsArgs,
) -> Result<()> {
    let PluginCommandContext {
        plugins_input,
        manager,
        ..
    } = load_plugin_command_context(overrides).await?;
    let current_dir = AbsolutePathBuf::try_from(std::env::current_dir()?)
        .context("failed to resolve current directory")?;
    let outcome = manager
        .list_marketplaces_for_config(&plugins_input, &[current_dir])
        .context("failed to list marketplace plugins")?;

    let marketplaces = outcome
        .marketplaces
        .into_iter()
        .filter(|marketplace| {
            args.marketplace_name
                .as_ref()
                .is_none_or(|name| marketplace.name == *name)
        })
        .collect::<Vec<_>>();

    if marketplaces.is_empty() {
        if let Some(marketplace_name) = args.marketplace_name {
            println!("No plugins found in marketplace `{marketplace_name}`.");
        } else {
            println!("No marketplace plugins found.");
        }
    } else {
        for marketplace in marketplaces {
            println!("Marketplace `{}`", marketplace.name);
            println!("Path: {}", marketplace.path.as_path().display());
            for plugin in &marketplace.plugins {
                let state = if plugin.installed && plugin.enabled {
                    "installed, enabled"
                } else if plugin.installed {
                    "installed, disabled"
                } else {
                    "not installed"
                };
                println!("  {} ({state})", plugin.id);
            }
        }
    }

    for error in outcome.errors {
        eprintln!(
            "Failed to load marketplace {}: {}",
            error.path.as_path().display(),
            error.message
        );
    }

    Ok(())
}

pub async fn run_plugin_remove(
    overrides: Vec<(String, toml::Value)>,
    args: RemovePluginArgs,
) -> Result<()> {
    let PluginCommandContext { manager, .. } = load_plugin_command_context(overrides).await?;
    let selection = parse_plugin_selection(args.plugin, args.marketplace_name)?;

    manager.uninstall_plugin(selection.plugin_key).await?;
    println!(
        "Removed plugin `{}` from marketplace `{}`.",
        selection.plugin_name, selection.marketplace_name
    );

    Ok(())
}

struct PluginCommandContext {
    plugins_input: PluginsConfigInput,
    manager: PluginsManager,
}

async fn load_plugin_command_context(
    overrides: Vec<(String, toml::Value)>,
) -> Result<PluginCommandContext> {
    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
    let config = Config::load_with_cli_overrides(overrides)
        .await
        .context("failed to load configuration")?;
    let plugins_input = config.plugins_config_input();
    let manager = PluginsManager::new(codex_home.to_path_buf());
    Ok(PluginCommandContext {
        plugins_input,
        manager,
    })
}

struct PluginSelection {
    plugin_name: String,
    marketplace_name: String,
    plugin_key: String,
}

fn parse_plugin_selection(
    plugin: String,
    marketplace_name: Option<String>,
) -> Result<PluginSelection> {
    match (PluginId::parse(&plugin), marketplace_name) {
        (Ok(plugin_id), None) => {
            let plugin_key = plugin_id.as_key();
            Ok(PluginSelection {
                plugin_name: plugin_id.plugin_name,
                marketplace_name: plugin_id.marketplace_name,
                plugin_key,
            })
        }
        (Ok(plugin_id), Some(marketplace_name)) => {
            if plugin_id.marketplace_name != marketplace_name {
                bail!(
                    "plugin id `{}` belongs to marketplace `{}`, but --marketplace specified `{}`",
                    plugin,
                    plugin_id.marketplace_name,
                    marketplace_name
                );
            }
            let plugin_key = plugin_id.as_key();
            Ok(PluginSelection {
                plugin_name: plugin_id.plugin_name,
                marketplace_name: plugin_id.marketplace_name,
                plugin_key,
            })
        }
        (Err(_), Some(marketplace_name)) => {
            let plugin_id = PluginId::new(plugin, marketplace_name)?;
            let plugin_key = plugin_id.as_key();
            Ok(PluginSelection {
                plugin_name: plugin_id.plugin_name,
                marketplace_name: plugin_id.marketplace_name,
                plugin_key,
            })
        }
        (Err(_), None) => {
            bail!("plugin requires --marketplace unless passed as <plugin>@<marketplace>")
        }
    }
}

fn find_marketplace_for_plugin(
    manager: &PluginsManager,
    plugins_input: &PluginsConfigInput,
    marketplace_name: &str,
    plugin_name: &str,
) -> Result<ConfiguredMarketplace> {
    let current_dir = AbsolutePathBuf::try_from(std::env::current_dir()?)
        .context("failed to resolve current directory")?;
    let matches = manager
        .list_marketplaces_for_config(plugins_input, &[current_dir])
        .context("failed to list marketplace plugins")?
        .marketplaces
        .into_iter()
        .filter(|marketplace| marketplace.name == marketplace_name)
        .filter(|marketplace| {
            marketplace
                .plugins
                .iter()
                .any(|plugin| plugin.name == plugin_name)
        })
        .collect::<Vec<_>>();

    match matches.as_slice() {
        [] => bail!("plugin `{plugin_name}` was not found in marketplace `{marketplace_name}`"),
        [marketplace] => Ok(marketplace.clone()),
        _ => bail!(
            "plugin `{plugin_name}` in marketplace `{marketplace_name}` matched multiple marketplace roots"
        ),
    }
}
