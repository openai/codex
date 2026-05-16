use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use clap::Parser;
use codex_core::config::Config;
use codex_core::config::find_codex_home;
use codex_core_plugins::PluginMarketplaceUpgradeOutcome;
use codex_core_plugins::PluginsManager;
use codex_core_plugins::installed_marketplaces::marketplace_install_root;
use codex_core_plugins::installed_marketplaces::resolve_configured_marketplace_root;
use codex_core_plugins::marketplace_add::MarketplaceAddRequest;
use codex_core_plugins::marketplace_add::add_marketplace;
use codex_core_plugins::marketplace_remove::MarketplaceRemoveRequest;
use codex_core_plugins::marketplace_remove::remove_marketplace;
use codex_plugin::validate_plugin_segment;
use codex_utils_cli::CliConfigOverrides;

#[derive(Debug, Parser)]
#[command(bin_name = "codex plugin marketplace")]
pub struct MarketplaceCli {
    #[clap(flatten)]
    pub config_overrides: CliConfigOverrides,

    #[command(subcommand)]
    subcommand: MarketplaceSubcommand,
}

#[derive(Debug, clap::Subcommand)]
enum MarketplaceSubcommand {
    /// Add a local or Git marketplace to the configured marketplace sources.
    Add(AddMarketplaceArgs),

    /// List configured marketplace names and their local snapshot roots.
    List,

    /// Refresh configured Git marketplace snapshots.
    ///
    /// Omit MARKETPLACE_NAME to upgrade all configured Git marketplaces.
    Upgrade(UpgradeMarketplaceArgs),

    /// Remove a configured marketplace source by name.
    Remove(RemoveMarketplaceArgs),
}

#[derive(Debug, Parser)]
#[command(
    bin_name = "codex plugin marketplace add",
    after_help = "Examples:\n  codex plugin marketplace add ./path/to/marketplace\n  codex plugin marketplace add owner/repo --ref main\n  codex plugin marketplace add https://github.com/owner/repo --sparse plugins/foo"
)]
struct AddMarketplaceArgs {
    /// Marketplace source: a local path, owner/repo[@ref], HTTPS Git URL, or SSH Git URL.
    #[arg(value_name = "SOURCE")]
    source: String,

    /// Git ref to fetch for Git marketplace sources.
    #[arg(long = "ref", value_name = "REF")]
    ref_name: Option<String>,

    /// Sparse checkout path for Git marketplace sources. Can be repeated.
    #[arg(
        long = "sparse",
        value_name = "PATH",
        action = clap::ArgAction::Append
    )]
    sparse_paths: Vec<String>,
}

#[derive(Debug, Parser)]
#[command(
    bin_name = "codex plugin marketplace upgrade",
    after_help = "Examples:\n  codex plugin marketplace upgrade\n  codex plugin marketplace upgrade debug"
)]
struct UpgradeMarketplaceArgs {
    /// Optional configured marketplace name to upgrade. Omit to upgrade all Git marketplaces.
    #[arg(value_name = "MARKETPLACE_NAME")]
    marketplace_name: Option<String>,
}

#[derive(Debug, Parser)]
#[command(
    bin_name = "codex plugin marketplace remove",
    after_help = "Example:\n  codex plugin marketplace remove debug"
)]
struct RemoveMarketplaceArgs {
    /// Configured marketplace name to remove.
    #[arg(value_name = "MARKETPLACE_NAME")]
    marketplace_name: String,
}

impl MarketplaceCli {
    pub async fn run(self) -> Result<()> {
        let MarketplaceCli {
            config_overrides,
            subcommand,
        } = self;

        let overrides = config_overrides
            .parse_overrides()
            .map_err(anyhow::Error::msg)?;

        match subcommand {
            MarketplaceSubcommand::Add(args) => run_add(args).await?,
            MarketplaceSubcommand::List => run_list(overrides).await?,
            MarketplaceSubcommand::Upgrade(args) => run_upgrade(overrides, args).await?,
            MarketplaceSubcommand::Remove(args) => run_remove(args).await?,
        }

        Ok(())
    }
}

async fn run_add(args: AddMarketplaceArgs) -> Result<()> {
    let AddMarketplaceArgs {
        source,
        ref_name,
        sparse_paths,
    } = args;

    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
    let outcome = add_marketplace(
        codex_home.to_path_buf(),
        MarketplaceAddRequest {
            source,
            ref_name,
            sparse_paths,
        },
    )
    .await?;

    if outcome.already_added {
        println!(
            "Marketplace `{}` is already added from {}.",
            outcome.marketplace_name, outcome.source_display
        );
    } else {
        println!(
            "Added marketplace `{}` from {}.",
            outcome.marketplace_name, outcome.source_display
        );
    }
    println!(
        "Installed marketplace root: {}",
        outcome.installed_root.as_path().display()
    );

    Ok(())
}

async fn run_list(overrides: Vec<(String, toml::Value)>) -> Result<()> {
    let config = Config::load_with_cli_overrides(overrides)
        .await
        .context("failed to load configuration")?;
    let configured_marketplaces = config
        .config_layer_stack
        .get_active_user_layer()
        .and_then(|layer| layer.config.get("marketplaces"))
        .and_then(toml::Value::as_table);
    let Some(configured_marketplaces) = configured_marketplaces else {
        println!("No configured plugin marketplaces.");
        return Ok(());
    };

    if configured_marketplaces.is_empty() {
        println!("No configured plugin marketplaces.");
        return Ok(());
    }

    let default_install_root = marketplace_install_root(config.codex_home.as_path());
    for (marketplace_name, marketplace) in configured_marketplaces {
        if !marketplace.is_table() {
            eprintln!("Ignoring invalid marketplace `{marketplace_name}`: expected table.");
            continue;
        }
        if let Err(err) = validate_plugin_segment(marketplace_name, "marketplace name") {
            eprintln!("Ignoring invalid marketplace `{marketplace_name}`: {err}.");
            continue;
        }
        let root = resolve_configured_marketplace_root(
            marketplace_name,
            marketplace,
            default_install_root.as_path(),
        )
        .map(|root| root.display().to_string())
        .unwrap_or_else(|| "<invalid source>".to_string());
        println!("{marketplace_name}\t{root}");
    }

    Ok(())
}

async fn run_upgrade(
    overrides: Vec<(String, toml::Value)>,
    args: UpgradeMarketplaceArgs,
) -> Result<()> {
    let UpgradeMarketplaceArgs { marketplace_name } = args;
    let config = Config::load_with_cli_overrides(overrides)
        .await
        .context("failed to load configuration")?;
    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
    let manager = PluginsManager::new(codex_home.to_path_buf());
    let plugins_input = config.plugins_config_input();
    let outcome = manager
        .upgrade_configured_marketplaces_for_config(&plugins_input, marketplace_name.as_deref())
        .map_err(anyhow::Error::msg)?;
    print_upgrade_outcome(&outcome, marketplace_name.as_deref())
}

async fn run_remove(args: RemoveMarketplaceArgs) -> Result<()> {
    let RemoveMarketplaceArgs { marketplace_name } = args;
    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
    let outcome = remove_marketplace(
        codex_home.to_path_buf(),
        MarketplaceRemoveRequest { marketplace_name },
    )
    .await?;

    println!("Removed marketplace `{}`.", outcome.marketplace_name);
    if let Some(installed_root) = outcome.removed_installed_root {
        println!(
            "Removed installed marketplace root: {}",
            installed_root.as_path().display()
        );
    }

    Ok(())
}

fn print_upgrade_outcome(
    outcome: &PluginMarketplaceUpgradeOutcome,
    marketplace_name: Option<&str>,
) -> Result<()> {
    for error in &outcome.errors {
        eprintln!(
            "Failed to upgrade marketplace `{}`: {}",
            error.marketplace_name, error.message
        );
    }
    if !outcome.all_succeeded() {
        bail!("{} upgrade failure(s) occurred.", outcome.errors.len());
    }

    let selection_label = marketplace_name.unwrap_or("all configured Git marketplaces");
    if outcome.selected_marketplaces.is_empty() {
        println!("No configured Git marketplaces to upgrade.");
    } else if outcome.upgraded_roots.is_empty() {
        if marketplace_name.is_some() {
            println!("Marketplace `{selection_label}` is already up to date.");
        } else {
            println!("All configured Git marketplaces are already up to date.");
        }
    } else if marketplace_name.is_some() {
        println!("Upgraded marketplace `{selection_label}` to the latest configured revision.");
        for root in &outcome.upgraded_roots {
            println!("Installed marketplace root: {}", root.display());
        }
    } else {
        println!("Upgraded {} marketplace(s).", outcome.upgraded_roots.len());
        for root in &outcome.upgraded_roots {
            println!("Installed marketplace root: {}", root.display());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn sparse_paths_parse_before_or_after_source() {
        let sparse_before_source =
            AddMarketplaceArgs::try_parse_from(["add", "--sparse", "plugins/foo", "owner/repo"])
                .unwrap();
        assert_eq!(sparse_before_source.source, "owner/repo");
        assert_eq!(sparse_before_source.sparse_paths, vec!["plugins/foo"]);

        let sparse_after_source =
            AddMarketplaceArgs::try_parse_from(["add", "owner/repo", "--sparse", "plugins/foo"])
                .unwrap();
        assert_eq!(sparse_after_source.source, "owner/repo");
        assert_eq!(sparse_after_source.sparse_paths, vec!["plugins/foo"]);

        let repeated_sparse = AddMarketplaceArgs::try_parse_from([
            "add",
            "--sparse",
            "plugins/foo",
            "--sparse",
            "skills/bar",
            "owner/repo",
        ])
        .unwrap();
        assert_eq!(repeated_sparse.source, "owner/repo");
        assert_eq!(
            repeated_sparse.sparse_paths,
            vec!["plugins/foo", "skills/bar"]
        );
    }

    #[test]
    fn upgrade_subcommand_parses_optional_marketplace_name() {
        let upgrade_all = UpgradeMarketplaceArgs::try_parse_from(["upgrade"]).unwrap();
        assert_eq!(upgrade_all.marketplace_name, None);

        let upgrade_one = UpgradeMarketplaceArgs::try_parse_from(["upgrade", "debug"]).unwrap();
        assert_eq!(upgrade_one.marketplace_name.as_deref(), Some("debug"));
    }

    #[test]
    fn remove_subcommand_parses_marketplace_name() {
        let remove = RemoveMarketplaceArgs::try_parse_from(["remove", "debug"]).unwrap();
        assert_eq!(remove.marketplace_name, "debug");
    }
}
