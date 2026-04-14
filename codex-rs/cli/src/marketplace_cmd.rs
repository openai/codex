use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use codex_core::config::find_codex_home;
use codex_core::plugins::MarketplaceAddRequest;
use codex_core::plugins::add_marketplace;
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
    /// Add a remote marketplace repository.
    Add(AddMarketplaceArgs),
}

#[derive(Debug, Parser)]
struct AddMarketplaceArgs {
    /// Marketplace source. Supports owner/repo[@ref], HTTP(S) Git URLs, SSH URLs,
    /// local filesystem paths, or direct marketplace.json URLs.
    source: String,

    /// Git ref to check out. Overrides any @ref or #ref suffix in SOURCE.
    #[arg(long = "ref", value_name = "REF")]
    ref_name: Option<String>,

    /// Sparse-checkout path to use while cloning git sources. Repeat to include multiple paths.
    #[arg(
        long = "sparse",
        value_name = "PATH",
        action = clap::ArgAction::Append
    )]
    sparse_paths: Vec<String>,
}

impl MarketplaceCli {
    pub async fn run(self) -> Result<()> {
        let MarketplaceCli {
            config_overrides,
            subcommand,
        } = self;

        // Validate overrides now. This command writes to CODEX_HOME only; marketplace discovery
        // happens from that cache root after the next plugin/list or app-server start.
        config_overrides
            .parse_overrides()
            .map_err(anyhow::Error::msg)?;

        match subcommand {
            MarketplaceSubcommand::Add(args) => run_add(args).await?,
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
}
