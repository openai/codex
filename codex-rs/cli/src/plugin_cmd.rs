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
use codex_core_plugins::installed_marketplaces::marketplace_install_root;
use codex_core_plugins::installed_marketplaces::resolve_configured_marketplace_root;
use codex_core_plugins::marketplace::MarketplaceListError;
use codex_core_plugins::marketplace::find_marketplace_manifest_path;
use codex_plugin::PluginId;
use codex_plugin::validate_plugin_segment;
use codex_utils_cli::CliConfigOverrides;
use std::path::PathBuf;

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
    /// Install a plugin from a configured marketplace snapshot.
    ///
    /// Pass either `PLUGIN@MARKETPLACE` or pass `PLUGIN` with
    /// `--marketplace MARKETPLACE`.
    Add(AddPluginArgs),

    /// List plugins available from configured marketplace snapshots.
    List(ListPluginsArgs),

    /// Add, list, upgrade, or remove configured plugin marketplaces.
    Marketplace(MarketplaceCli),

    /// Remove an installed plugin from local config and cache.
    ///
    /// Pass either `PLUGIN@MARKETPLACE` or pass `PLUGIN` with
    /// `--marketplace MARKETPLACE`.
    Remove(RemovePluginArgs),
}

#[derive(Debug, Parser)]
#[command(
    bin_name = "codex plugin add",
    after_help = "Examples:\n  codex plugin add sample@debug\n  codex plugin add sample --marketplace debug"
)]
pub struct AddPluginArgs {
    /// Plugin selector to install: either PLUGIN@MARKETPLACE or PLUGIN with --marketplace.
    #[arg(value_name = "PLUGIN[@MARKETPLACE]")]
    plugin: String,

    /// Configured marketplace name to use when PLUGIN does not include @MARKETPLACE.
    #[arg(long = "marketplace", short = 'm', value_name = "MARKETPLACE")]
    marketplace_name: Option<String>,
}

#[derive(Debug, Parser)]
#[command(
    bin_name = "codex plugin list",
    after_help = "Examples:\n  codex plugin list\n  codex plugin list --marketplace debug"
)]
pub struct ListPluginsArgs {
    /// Only list plugins from this configured marketplace name.
    #[arg(long = "marketplace", short = 'm', value_name = "MARKETPLACE")]
    marketplace_name: Option<String>,
}

#[derive(Debug, Parser)]
#[command(
    bin_name = "codex plugin remove",
    after_help = "Examples:\n  codex plugin remove sample@debug\n  codex plugin remove sample --marketplace debug"
)]
pub struct RemovePluginArgs {
    /// Plugin selector to remove: either PLUGIN@MARKETPLACE or PLUGIN with --marketplace.
    #[arg(value_name = "PLUGIN[@MARKETPLACE]")]
    plugin: String,

    /// Marketplace name to use when PLUGIN does not include @MARKETPLACE.
    #[arg(long = "marketplace", short = 'm', value_name = "MARKETPLACE")]
    marketplace_name: Option<String>,
}

pub async fn run_plugin_add(
    overrides: Vec<(String, toml::Value)>,
    args: AddPluginArgs,
) -> Result<()> {
    let PluginCommandContext {
        codex_home,
        plugins_input,
        manager,
    } = load_plugin_command_context(overrides).await?;
    let PluginSelection {
        plugin_name,
        marketplace_name,
        ..
    } = parse_plugin_selection(args.plugin, args.marketplace_name)?;
    let marketplace = find_marketplace_for_plugin(
        &manager,
        codex_home.as_path(),
        &plugins_input,
        &marketplace_name,
        &plugin_name,
    )?;
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
        codex_home,
        plugins_input,
        manager,
        ..
    } = load_plugin_command_context(overrides).await?;
    let outcome = manager
        .list_marketplaces_for_config(&plugins_input, &[])
        .context("failed to list marketplace plugins")?;
    ensure_configured_marketplace_snapshots_loaded(
        codex_home.as_path(),
        &plugins_input,
        &outcome.errors,
        /*marketplace_name*/ None,
    )?;

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
    codex_home: PathBuf,
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
        codex_home: codex_home.to_path_buf(),
        plugins_input,
        manager,
    })
}

struct PluginSelection {
    plugin_name: String,
    marketplace_name: String,
    plugin_key: String,
}

impl PluginSelection {
    fn from_plugin_id(plugin_id: PluginId) -> Self {
        let plugin_key = plugin_id.as_key();
        Self {
            plugin_name: plugin_id.plugin_name,
            marketplace_name: plugin_id.marketplace_name,
            plugin_key,
        }
    }
}

fn parse_plugin_selection(
    plugin: String,
    marketplace_name: Option<String>,
) -> Result<PluginSelection> {
    match (PluginId::parse(&plugin), marketplace_name) {
        (Ok(plugin_id), None) => Ok(PluginSelection::from_plugin_id(plugin_id)),
        (Ok(plugin_id), Some(marketplace_name)) => {
            if plugin_id.marketplace_name != marketplace_name {
                bail!(
                    "plugin id `{}` belongs to marketplace `{}`, but --marketplace specified `{}`",
                    plugin,
                    plugin_id.marketplace_name,
                    marketplace_name
                );
            }
            Ok(PluginSelection::from_plugin_id(plugin_id))
        }
        (Err(_), Some(marketplace_name)) => Ok(PluginSelection::from_plugin_id(PluginId::new(
            plugin,
            marketplace_name,
        )?)),
        (Err(_), None) => {
            bail!("plugin requires --marketplace unless passed as <plugin>@<marketplace>")
        }
    }
}

fn find_marketplace_for_plugin(
    manager: &PluginsManager,
    codex_home: &std::path::Path,
    plugins_input: &PluginsConfigInput,
    marketplace_name: &str,
    plugin_name: &str,
) -> Result<ConfiguredMarketplace> {
    let outcome = manager
        .list_marketplaces_for_config(plugins_input, &[])
        .context("failed to list marketplace plugins")?;
    ensure_configured_marketplace_snapshots_loaded(
        codex_home,
        plugins_input,
        &outcome.errors,
        Some(marketplace_name),
    )?;
    let matches = outcome
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

struct ConfiguredMarketplaceSnapshotIssue {
    marketplace_name: String,
    path: PathBuf,
    message: String,
}

fn ensure_configured_marketplace_snapshots_loaded(
    codex_home: &std::path::Path,
    plugins_input: &PluginsConfigInput,
    load_errors: &[MarketplaceListError],
    marketplace_name: Option<&str>,
) -> Result<()> {
    let issues = configured_marketplace_snapshot_issues(
        codex_home,
        plugins_input,
        load_errors,
        marketplace_name,
    );
    if issues.is_empty() {
        return Ok(());
    }

    let issue_lines = issues
        .iter()
        .map(|issue| {
            format!(
                "- `{}` at {}: {}",
                issue.marketplace_name,
                issue.path.display(),
                issue.message
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    bail!("failed to load configured marketplace snapshot(s):\n{issue_lines}");
}

fn configured_marketplace_snapshot_issues(
    codex_home: &std::path::Path,
    plugins_input: &PluginsConfigInput,
    load_errors: &[MarketplaceListError],
    marketplace_name: Option<&str>,
) -> Vec<ConfiguredMarketplaceSnapshotIssue> {
    let Some(user_layer) = plugins_input.config_layer_stack.get_active_user_layer() else {
        return Vec::new();
    };
    let Some(configured_marketplaces) = user_layer
        .config
        .get("marketplaces")
        .and_then(toml::Value::as_table)
    else {
        return Vec::new();
    };

    let default_install_root = marketplace_install_root(codex_home);
    let mut manifest_paths = Vec::new();
    let mut issues = Vec::new();
    for (configured_name, marketplace) in configured_marketplaces {
        if marketplace_name.is_some_and(|name| configured_name != name) {
            continue;
        }
        if !marketplace.is_table()
            || validate_plugin_segment(configured_name, "marketplace name").is_err()
        {
            continue;
        }
        let Some(root) = resolve_configured_marketplace_root(
            configured_name,
            marketplace,
            &default_install_root,
        ) else {
            continue;
        };
        match find_marketplace_manifest_path(&root) {
            Some(path) => manifest_paths.push((configured_name.clone(), path)),
            None => issues.push(ConfiguredMarketplaceSnapshotIssue {
                marketplace_name: configured_name.clone(),
                path: root,
                message: "marketplace root does not contain a supported manifest".to_string(),
            }),
        }
    }

    for error in load_errors {
        if let Some((configured_name, _)) = manifest_paths
            .iter()
            .find(|(_, path)| path.as_path() == error.path.as_path())
        {
            issues.push(ConfiguredMarketplaceSnapshotIssue {
                marketplace_name: configured_name.clone(),
                path: error.path.to_path_buf(),
                message: error.message.clone(),
            });
        }
    }
    issues
}
