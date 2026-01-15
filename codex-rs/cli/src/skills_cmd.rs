use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use codex_common::CliConfigOverrides;
use codex_core::config::Config;
use codex_core::config::edit::ConfigEdit;
use codex_core::config::edit::ConfigEditsBuilder;
use codex_utils_absolute_path::AbsolutePathBuf;

/// Subcommands:
/// - `enable`  — enable a skill by SKILL.md path
/// - `disable` — disable a skill by SKILL.md path
#[derive(Debug, Parser)]
pub struct SkillsCli {
    #[clap(flatten)]
    pub config_overrides: CliConfigOverrides,

    #[command(subcommand)]
    pub subcommand: SkillsSubcommand,
}

#[derive(Debug, clap::Subcommand)]
pub enum SkillsSubcommand {
    /// Enable a skill by SKILL.md path.
    Enable(SkillPathArgs),
    /// Disable a skill by SKILL.md path.
    Disable(SkillPathArgs),
}

#[derive(Debug, Parser)]
pub struct SkillPathArgs {
    /// Path to the skill's SKILL.md file (stored as an absolute path).
    pub path: PathBuf,
}

impl SkillsCli {
    pub async fn run(self) -> Result<()> {
        let SkillsCli {
            config_overrides,
            subcommand,
        } = self;

        match subcommand {
            SkillsSubcommand::Enable(args) => {
                run_set_enabled(&config_overrides, args.path, true).await?;
            }
            SkillsSubcommand::Disable(args) => {
                run_set_enabled(&config_overrides, args.path, false).await?;
            }
        }

        Ok(())
    }
}

async fn run_set_enabled(
    config_overrides: &CliConfigOverrides,
    path: PathBuf,
    enabled: bool,
) -> Result<()> {
    let overrides = config_overrides
        .parse_overrides()
        .map_err(anyhow::Error::msg)?;
    let config = Config::load_with_cli_overrides(overrides)
        .await
        .context("failed to load configuration")?;
    let absolute_path = AbsolutePathBuf::try_from(path.as_path())
        .with_context(|| format!("failed to resolve skill path {path}", path = path.display()))?
        .into_path_buf();

    ConfigEditsBuilder::new(&config.codex_home)
        .with_edits([ConfigEdit::SetSkillConfig {
            path: absolute_path.clone(),
            enabled,
        }])
        .apply()
        .await
        .with_context(|| {
            format!(
                "failed to write skill override to {codex_home}",
                codex_home = config.codex_home.display()
            )
        })?;

    if enabled {
        println!("Enabled skill at {path}.", path = absolute_path.display());
    } else {
        println!("Disabled skill at {path}.", path = absolute_path.display());
    }

    Ok(())
}
