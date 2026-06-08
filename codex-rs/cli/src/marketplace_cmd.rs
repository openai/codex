use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use clap::Parser;
use codex_core::config::Config;
use codex_core::config::find_codex_home;
use codex_core_plugins::PluginMarketplaceUpgradeOutcome;
use codex_core_plugins::PluginsManager;
use codex_core_plugins::marketplace::marketplace_root_dir;
use codex_core_plugins::marketplace_add::MarketplaceAddOutcome;
use codex_core_plugins::marketplace_add::MarketplaceAddRequest;
use codex_core_plugins::marketplace_add::add_marketplace;
use codex_core_plugins::marketplace_remove::MarketplaceRemoveOutcome;
use codex_core_plugins::marketplace_remove::MarketplaceRemoveRequest;
use codex_core_plugins::marketplace_remove::remove_marketplace;
use codex_utils_cli::CliConfigOverrides;
use serde::Serialize;
use std::collections::HashSet;

use crate::plugin_cmd::configured_marketplace_snapshot_issues;

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

    /// List plugin marketplaces Codex is currently considering and their roots.
    List(ListMarketplaceArgs),

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

    /// Output add result as JSON.
    #[arg(long = "json")]
    json: bool,
}

#[derive(Debug, Parser)]
#[command(bin_name = "codex plugin marketplace list")]
struct ListMarketplaceArgs {
    /// Output marketplace list as JSON.
    #[arg(long = "json")]
    json: bool,
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

    /// Output upgrade result as JSON.
    #[arg(long = "json")]
    json: bool,
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

    /// Output remove result as JSON.
    #[arg(long = "json")]
    json: bool,
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
            MarketplaceSubcommand::List(args) => run_list(overrides, args).await?,
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
        json,
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

    if json {
        let output = JsonMarketplaceAddOutput::from_outcome(outcome);
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonMarketplaceAddOutput {
    marketplace_name: String,
    installed_root: String,
    already_added: bool,
}

impl JsonMarketplaceAddOutput {
    fn from_outcome(outcome: MarketplaceAddOutcome) -> Self {
        Self {
            marketplace_name: outcome.marketplace_name,
            installed_root: outcome.installed_root.as_path().display().to_string(),
            already_added: outcome.already_added,
        }
    }
}

async fn run_list(overrides: Vec<(String, toml::Value)>, args: ListMarketplaceArgs) -> Result<()> {
    let config = Config::load_with_cli_overrides(overrides)
        .await
        .context("failed to load configuration")?;
    let manager = PluginsManager::new(config.codex_home.to_path_buf());
    let plugins_input = config.plugins_config_input();
    let marketplace_listing = manager
        .discover_marketplaces_for_config(&plugins_input, &[])
        .context("failed to list plugin marketplaces")?;
    let mut load_issues = configured_marketplace_snapshot_issues(
        config.codex_home.as_path(),
        &plugins_input,
        &marketplace_listing.errors,
        /*marketplace_name*/ None,
    );
    let mut issue_paths = load_issues
        .iter()
        .map(|issue| issue.path.clone())
        .collect::<HashSet<_>>();
    for error in &marketplace_listing.errors {
        if issue_paths.insert(error.path.to_path_buf()) {
            load_issues.push(crate::plugin_cmd::ConfiguredMarketplaceSnapshotIssue {
                marketplace_name: error.path.display().to_string(),
                path: error.path.to_path_buf(),
                message: error.message.clone(),
            });
        }
    }
    if !load_issues.is_empty() {
        let issue_lines = load_issues
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
        bail!("failed to load marketplace(s):\n{issue_lines}");
    }
    let marketplaces = marketplace_listing.marketplaces;
    if args.json {
        let output = JsonMarketplaceListOutput::from_marketplaces(marketplaces);
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    if marketplaces.is_empty() {
        println!("No plugin marketplaces in scope.");
        return Ok(());
    }

    let mut seen_roots = HashSet::new();
    let mut rows = Vec::new();
    for marketplace in marketplaces {
        let Ok(root) = marketplace_root_dir(&marketplace.path) else {
            continue;
        };
        if !seen_roots.insert(root.clone()) {
            continue;
        }
        rows.push((marketplace.name, root));
    }

    let marketplace_width = rows
        .iter()
        .map(|(name, _)| name.len())
        .max()
        .unwrap_or("MARKETPLACE".len())
        .max("MARKETPLACE".len());

    println!("{:<marketplace_width$}  ROOT", "MARKETPLACE");
    for (marketplace_name, root) in rows {
        println!(
            "{:<marketplace_width$}  {}",
            marketplace_name,
            root.display()
        );
    }

    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonMarketplaceListOutput {
    marketplaces: Vec<JsonMarketplaceListEntry>,
}

impl JsonMarketplaceListOutput {
    fn from_marketplaces(marketplaces: Vec<codex_core_plugins::marketplace::Marketplace>) -> Self {
        let mut seen_roots = HashSet::new();
        let marketplaces = marketplaces
            .into_iter()
            .filter_map(|marketplace| {
                let root = marketplace_root_dir(&marketplace.path).ok()?;
                if !seen_roots.insert(root.clone()) {
                    return None;
                }
                Some(JsonMarketplaceListEntry {
                    name: marketplace.name,
                    root: root.display().to_string(),
                })
            })
            .collect();

        Self { marketplaces }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonMarketplaceListEntry {
    name: String,
    root: String,
}

async fn run_upgrade(
    overrides: Vec<(String, toml::Value)>,
    args: UpgradeMarketplaceArgs,
) -> Result<()> {
    let UpgradeMarketplaceArgs {
        marketplace_name,
        json,
    } = args;
    let config = Config::load_with_cli_overrides(overrides)
        .await
        .context("failed to load configuration")?;
    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
    let manager = PluginsManager::new(codex_home.to_path_buf());
    let plugins_input = config.plugins_config_input();
    let outcome = manager
        .upgrade_configured_marketplaces_for_config(&plugins_input, marketplace_name.as_deref())
        .map_err(anyhow::Error::msg)?;
    if json {
        print_upgrade_outcome_json(&outcome)
    } else {
        print_upgrade_outcome(&outcome, marketplace_name.as_deref())
    }
}

async fn run_remove(args: RemoveMarketplaceArgs) -> Result<()> {
    let RemoveMarketplaceArgs {
        marketplace_name,
        json,
    } = args;
    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
    let outcome = remove_marketplace(
        codex_home.to_path_buf(),
        MarketplaceRemoveRequest { marketplace_name },
    )
    .await?;

    if json {
        let output = JsonMarketplaceRemoveOutput::from_outcome(outcome);
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    println!("Removed marketplace `{}`.", outcome.marketplace_name);
    if let Some(installed_root) = outcome.removed_installed_root {
        println!(
            "Removed installed marketplace root: {}",
            installed_root.as_path().display()
        );
    }

    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonMarketplaceRemoveOutput {
    marketplace_name: String,
    installed_root: Option<String>,
}

impl JsonMarketplaceRemoveOutput {
    fn from_outcome(outcome: MarketplaceRemoveOutcome) -> Self {
        Self {
            marketplace_name: outcome.marketplace_name,
            installed_root: outcome
                .removed_installed_root
                .map(|root| root.as_path().display().to_string()),
        }
    }
}

fn print_upgrade_outcome_json(outcome: &PluginMarketplaceUpgradeOutcome) -> Result<()> {
    for error in &outcome.errors {
        eprintln!(
            "Failed to upgrade marketplace `{}`: {}",
            error.marketplace_name, error.message
        );
    }
    if !outcome.all_succeeded() {
        bail!("{} upgrade failure(s) occurred.", outcome.errors.len());
    }

    let output = JsonMarketplaceUpgradeOutput::from_outcome(outcome);
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonMarketplaceUpgradeOutput {
    selected_marketplaces: Vec<String>,
    upgraded_roots: Vec<String>,
    errors: Vec<JsonMarketplaceUpgradeError>,
}

impl JsonMarketplaceUpgradeOutput {
    fn from_outcome(outcome: &PluginMarketplaceUpgradeOutcome) -> Self {
        Self {
            selected_marketplaces: outcome.selected_marketplaces.clone(),
            upgraded_roots: outcome
                .upgraded_roots
                .iter()
                .map(|root| root.display().to_string())
                .collect(),
            errors: outcome
                .errors
                .iter()
                .map(|error| JsonMarketplaceUpgradeError {
                    marketplace_name: error.marketplace_name.clone(),
                    message: error.message.clone(),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonMarketplaceUpgradeError {
    marketplace_name: String,
    message: String,
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
