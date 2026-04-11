use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use clap::Args;
use clap::Parser;
use clap::Subcommand;
use clap::ValueEnum;
use codex_bundles::BundleChannel;
use codex_bundles::BundleSelection;
use codex_bundles::InstallBundleOptions;
use codex_bundles::InstallStatus;

#[derive(Debug, Parser)]
pub struct BundlesCli {
    #[command(subcommand)]
    subcommand: BundlesSubcommand,
}

#[derive(Debug, Subcommand)]
enum BundlesSubcommand {
    /// Install and configure a Codex runtime bundle.
    Install(BundlesInstallCommand),

    /// List installed Codex runtime bundles.
    List(BundlesListCommand),
}

#[derive(Debug, Args)]
#[command(group(
    clap::ArgGroup::new("selection")
        .args(["channel", "version"])
        .multiple(false)
))]
struct BundlesInstallCommand {
    /// Runtime manifest file.
    #[arg(short = 'F', long = "file", value_name = "FILE")]
    file: PathBuf,

    /// Named release channel to install.
    #[arg(long = "channel", value_enum)]
    channel: Option<BundleChannelArg>,

    /// Exact bundle version to install from the manifest's versions map.
    #[arg(long = "version", value_name = "VERSION")]
    version: Option<String>,

    /// Runtime artifact name to install.
    #[arg(long = "artifact", hide = true, default_value = codex_bundles::DEFAULT_ARTIFACT_NAME)]
    artifact: String,

    /// Override the runtime install root.
    #[arg(long = "install-root", hide = true, value_name = "DIR")]
    install_root: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct BundlesListCommand {
    /// Show installed versions and runtime paths.
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,

    /// Override the runtime install root.
    #[arg(long = "install-root", hide = true, value_name = "DIR")]
    install_root: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum BundleChannelArg {
    Latest,
    Alpha,
}

impl BundlesCli {
    pub async fn run(self) -> Result<()> {
        match self.subcommand {
            BundlesSubcommand::Install(command) => run_install(command).await,
            BundlesSubcommand::List(command) => run_list(command).await,
        }
    }
}

async fn run_install(command: BundlesInstallCommand) -> Result<()> {
    let config = codex_bundles::read_runtimes_config(&command.file).await?;
    let selection = match (command.channel, command.version) {
        (Some(channel), None) => BundleSelection::Channel(channel.into()),
        (None, Some(version)) => BundleSelection::Version(version),
        (None, None) => BundleSelection::Channel(BundleChannel::Latest),
        (Some(_), Some(_)) => unreachable!("clap enforces mutually exclusive selection args"),
    };
    let mut options = InstallBundleOptions::for_current_target()?;
    options.artifact_name = command.artifact;
    if let Some(install_root) = command.install_root {
        options.install_root = install_root;
    }

    let result = codex_bundles::install_bundle(&config, selection, options).await?;
    let codex_home = codex_bundles::codex_home().context("failed to resolve Codex home")?;
    codex_bundles::write_codex_runtime_config(&codex_home, &result.paths).await?;

    match result.status {
        InstallStatus::AlreadyCurrent => {
            println!(
                "{} {} already current at {}",
                result.artifact_name,
                result.bundle_version,
                result.runtime_root.display()
            );
        }
        InstallStatus::Installed => {
            println!(
                "{} {} installed at {}",
                result.artifact_name,
                result.bundle_version,
                result.runtime_root.display()
            );
        }
    }
    Ok(())
}

async fn run_list(command: BundlesListCommand) -> Result<()> {
    let install_root = match command.install_root {
        Some(install_root) => install_root,
        None => codex_bundles::default_install_root()?,
    };
    let bundles = codex_bundles::list_installed_bundles(&install_root).await?;
    if command.verbose {
        for bundle in bundles {
            let current = if bundle.current { "*" } else { " " };
            let valid = if bundle.valid { "valid" } else { "invalid" };
            println!(
                "{current} {} {} {valid} {}",
                bundle.artifact_name,
                bundle.bundle_version,
                bundle.runtime_root.display()
            );
        }
    } else {
        let mut artifacts = std::collections::BTreeMap::<String, Option<String>>::new();
        for bundle in bundles {
            let entry = artifacts.entry(bundle.artifact_name).or_insert(None);
            if bundle.current {
                *entry = Some(bundle.bundle_version);
            }
        }
        for (artifact_name, current_version) in artifacts {
            let current = current_version
                .map(|version| format!(" ({version})"))
                .unwrap_or_default();
            println!("{artifact_name}{current}");
        }
    }
    Ok(())
}

impl From<BundleChannelArg> for BundleChannel {
    fn from(value: BundleChannelArg) -> Self {
        match value {
            BundleChannelArg::Latest => BundleChannel::Latest,
            BundleChannelArg::Alpha => BundleChannel::Alpha,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_accepts_uppercase_file_short_with_version() {
        let cli = BundlesCli::try_parse_from([
            "codex-bundles",
            "install",
            "-F",
            "manifest.json",
            "--version",
            "2026.03.26.1",
        ])
        .expect("bundles install parses");

        let BundlesSubcommand::Install(command) = cli.subcommand else {
            panic!("expected install command");
        };
        assert_eq!(command.file, PathBuf::from("manifest.json"));
        assert_eq!(command.version.as_deref(), Some("2026.03.26.1"));
        assert!(command.channel.is_none());
    }

    #[test]
    fn install_rejects_lowercase_file_short() {
        let err = BundlesCli::try_parse_from(["codex-bundles", "install", "-f", "manifest.json"])
            .expect_err("lowercase file short should be rejected");

        assert!(err.to_string().contains("unexpected argument '-f'"));
    }

    #[test]
    fn install_rejects_channel_and_version_together() {
        let err = BundlesCli::try_parse_from([
            "codex-bundles",
            "install",
            "-F",
            "manifest.json",
            "--channel",
            "latest",
            "--version",
            "2026.03.26.1",
        ])
        .expect_err("channel and version should conflict");

        assert!(err.to_string().contains("cannot be used with"));
    }

    #[test]
    fn list_accepts_verbose_flag() {
        let cli = BundlesCli::try_parse_from(["codex-bundles", "list", "-v"])
            .expect("bundles list parses");

        let BundlesSubcommand::List(command) = cli.subcommand else {
            panic!("expected list command");
        };
        assert!(command.verbose);
    }
}
