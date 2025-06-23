use clap::Parser;

use codex_common::CliConfigOverrides;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;

/// Print the list of models available for the configured (or overridden)
/// provider.
#[derive(Debug, Parser)]
pub struct ListModelsCli {
    /// Optional provider override. When set this value is used instead of the
    /// `model_provider_id` configured in `~/.codex/config.toml`.
    #[clap(long)]
    pub provider: Option<String>,

    /// Arbitrary `-c key=value` overrides that apply **in addition** to the
    /// `--provider` flag.
    #[clap(flatten)]
    pub config_overrides: CliConfigOverrides,
}

impl ListModelsCli {
    pub async fn run(self) -> anyhow::Result<()> {
        // Compose strongly-typed overrides. The provider flag, if specified,
        // is translated into the corresponding field inside `ConfigOverrides`.
        let overrides = ConfigOverrides {
            model: None,
            config_profile: None,
            approval_policy: None,
            sandbox_policy: None,
            cwd: None,
            model_provider: self.provider.clone(),
            codex_linux_sandbox_exe: None,
        };

        // Parse the raw `-c` overrides early so we can bail with a useful
        // error message if the user supplied an invalid value.
        let cli_kv_overrides = self
            .config_overrides
            .parse_overrides()
            .map_err(anyhow::Error::msg)?;

        // Load the merged configuration.
        let cfg = Config::load_with_cli_overrides(cli_kv_overrides, overrides)?;

        // Retrieve the model list.
        let models = codex_common::fetch_available_models(cfg.model_provider).await?;

        for m in models {
            println!("{m}");
        }

        Ok(())
    }
}
