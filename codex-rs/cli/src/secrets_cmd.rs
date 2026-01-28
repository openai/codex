use std::io::IsTerminal;
use std::io::Read;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use clap::Parser;
use codex_common::CliConfigOverrides;
use codex_core::config::Config;
use codex_core::secrets::SecretName;
use codex_core::secrets::SecretScope;
use codex_core::secrets::SecretsManager;
use codex_core::secrets::environment_id_from_cwd;

#[derive(Debug, Parser)]
pub struct SecretsCli {
    #[clap(flatten)]
    pub config_overrides: CliConfigOverrides,

    #[command(subcommand)]
    pub subcommand: SecretsSubcommand,
}

#[derive(Debug, clap::Subcommand)]
pub enum SecretsSubcommand {
    /// Store or update a secret value.
    Set(SecretsSetArgs),
    /// List secret names (values are never displayed).
    List(SecretsListArgs),
    /// Delete a secret value.
    Delete(SecretsDeleteArgs),
}

#[derive(Debug, Parser)]
pub struct SecretsScopeArgs {
    /// Use the global scope instead of the current environment scope.
    #[arg(long, default_value_t = false, conflicts_with = "environment_id")]
    pub global: bool,

    /// Explicit environment identifier for scoping the secret.
    #[arg(long = "env")]
    pub environment_id: Option<String>,
}

#[derive(Debug, Parser)]
pub struct SecretsSetArgs {
    /// Secret name (A-Z, 0-9, underscore only).
    pub name: String,

    /// Secret value. Prefer piping via stdin to avoid shell history.
    #[arg(long)]
    pub value: Option<String>,

    #[clap(flatten)]
    pub scope: SecretsScopeArgs,
}

#[derive(Debug, Parser)]
pub struct SecretsListArgs {
    #[clap(flatten)]
    pub scope: SecretsScopeArgs,
}

#[derive(Debug, Parser)]
pub struct SecretsDeleteArgs {
    /// Secret name (A-Z, 0-9, underscore only).
    pub name: String,

    #[clap(flatten)]
    pub scope: SecretsScopeArgs,
}

impl SecretsCli {
    pub async fn run(self) -> Result<()> {
        let config = load_config(self.config_overrides).await?;
        let manager = SecretsManager::new(config.codex_home.clone(), config.secrets_backend);
        match self.subcommand {
            SecretsSubcommand::Set(args) => run_set(&config, &manager, args),
            SecretsSubcommand::List(args) => run_list(&config, &manager, args),
            SecretsSubcommand::Delete(args) => run_delete(&config, &manager, args),
        }
    }
}

async fn load_config(cli_config_overrides: CliConfigOverrides) -> Result<Config> {
    let cli_overrides = cli_config_overrides
        .parse_overrides()
        .map_err(anyhow::Error::msg)?;
    Config::load_with_cli_overrides(cli_overrides)
        .await
        .context("failed to load configuration")
}

fn run_set(config: &Config, manager: &SecretsManager, args: SecretsSetArgs) -> Result<()> {
    let name = SecretName::new(&args.name)?;
    let scope = resolve_scope(config, &args.scope)?;
    let value = resolve_value(args.value)?;
    manager.set(&scope, &name, &value)?;
    println!("Stored {name} in {}.", scope_label(&scope));
    Ok(())
}

fn run_list(config: &Config, manager: &SecretsManager, args: SecretsListArgs) -> Result<()> {
    let default_scope = default_environment_scope(config)?;
    let scope_filter = match (args.scope.global, args.scope.environment_id.as_deref()) {
        (true, _) => Some(SecretScope::Global),
        (false, Some(env_id)) => Some(SecretScope::environment(env_id.to_string())?),
        (false, None) => None,
    };

    let mut entries = manager.list(None)?;
    entries.retain(|entry| match scope_filter.as_ref() {
        Some(scope) => &entry.scope == scope,
        None => entry.scope == SecretScope::Global || entry.scope == default_scope,
    });

    entries.sort_by(|a, b| {
        scope_label(&a.scope)
            .cmp(&scope_label(&b.scope))
            .then(a.name.cmp(&b.name))
    });

    if entries.is_empty() {
        println!("No secrets found.");
        return Ok(());
    }

    for entry in entries {
        println!("{} {}", scope_label(&entry.scope), entry.name);
    }

    Ok(())
}

fn run_delete(config: &Config, manager: &SecretsManager, args: SecretsDeleteArgs) -> Result<()> {
    let name = SecretName::new(&args.name)?;
    let scope = resolve_scope(config, &args.scope)?;
    let removed = manager.delete(&scope, &name)?;
    if removed {
        println!("Deleted {name} from {}.", scope_label(&scope));
    } else {
        println!("No secret named {name} found in {}.", scope_label(&scope));
    }
    Ok(())
}

fn resolve_scope(config: &Config, scope_args: &SecretsScopeArgs) -> Result<SecretScope> {
    if scope_args.global {
        return Ok(SecretScope::Global);
    }
    if let Some(env_id) = scope_args.environment_id.as_deref() {
        return SecretScope::environment(env_id.to_string());
    }
    default_environment_scope(config)
}

fn default_environment_scope(config: &Config) -> Result<SecretScope> {
    let environment_id = environment_id_from_cwd(&config.cwd);
    SecretScope::environment(environment_id)
}

fn resolve_value(explicit: Option<String>) -> Result<String> {
    if let Some(value) = explicit {
        return Ok(value);
    }

    if std::io::stdin().is_terminal() {
        bail!("secret value must be provided via --value or piped stdin");
    }

    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .context("failed to read secret value from stdin")?;
    let trimmed = buf.trim_end_matches(['\n', '\r']).to_string();
    anyhow::ensure!(!trimmed.is_empty(), "secret value must not be empty");
    Ok(trimmed)
}

fn scope_label(scope: &SecretScope) -> String {
    match scope {
        SecretScope::Global => "global".to_string(),
        SecretScope::Environment(env_id) => format!("env/{env_id}"),
    }
}
