use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use clap::Parser;
use codex_core::config::Config;
use codex_core::config::find_codex_home;
use codex_core::plugins::PluginMarketplaceUpgradeOutcome;
use codex_core::plugins::PluginsManager;
use codex_core_plugins::marketplace_add::MarketplaceAddRequest;
use codex_core_plugins::marketplace_add::add_marketplace;
use codex_core_plugins::marketplace_remove::MarketplaceRemoveRequest;
use codex_core_plugins::marketplace_remove::remove_marketplace;
use codex_utils_cli::CliConfigOverrides;

#[derive(Debug, Parser)]
pub struct MarketplaceCli {
    #[clap(flatten)]
    pub config_overrides: CliConfigOverrides,

    #[command(subcommand)]
    subcommand: MarketplaceSubcommand,
}

#[derive(Debug, clap::Subcommand)]
enum MarketplaceSubcommand {
    Add(AddMarketplaceArgs),
    Upgrade(UpgradeMarketplaceArgs),
    Remove(RemoveMarketplaceArgs),
}

#[derive(Debug, Parser)]
struct AddMarketplaceArgs {
    /// Marketplace source. Supports owner/repo[@ref], HTTP(S) Git URLs, SSH URLs,
    /// or local marketplace root directories.
    source: String,

    #[arg(long = "ref", value_name = "REF")]
    ref_name: Option<String>,

    #[arg(
        long = "sparse",
        value_name = "PATH",
        action = clap::ArgAction::Append
    )]
    sparse_paths: Vec<String>,
}

#[derive(Debug, Parser)]
struct UpgradeMarketplaceArgs {
    marketplace_name: Option<String>,
}

#[derive(Debug, Parser)]
struct RemoveMarketplaceArgs {
    /// Configured marketplace name to remove.
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
    let outcome = manager
        .upgrade_configured_marketplaces_for_config(&config, marketplace_name.as_deref())
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
