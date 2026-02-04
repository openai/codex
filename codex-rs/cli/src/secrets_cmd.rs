use std::io::IsTerminal;
use std::io::Read;
use std::io::Write;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use clap::Parser;
use codex_common::CliConfigOverrides;
use codex_core::config::Config;
use codex_core::secrets::SecretName;
use codex_core::secrets::SecretScope;
use codex_core::secrets::SecretsManager;
use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;
use crossterm::terminal::disable_raw_mode;
use crossterm::terminal::enable_raw_mode;

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
    /// Update an existing secret value.
    Edit(SecretsEditArgs),
    /// List secret names (values are never displayed).
    List(SecretsListArgs),
    /// Delete a secret value.
    Delete(SecretsDeleteArgs),
}

#[derive(Debug, Parser)]
pub struct SecretsScopeArgs {
    /// Use the global scope (default).
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
pub struct SecretsEditArgs {
    /// Secret name (A-Z, 0-9, underscore only).
    pub name: String,

    /// New secret value. Prefer piping via stdin to avoid shell history.
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
            SecretsSubcommand::Set(args) => run_set(&manager, args),
            SecretsSubcommand::Edit(args) => run_edit(&manager, args),
            SecretsSubcommand::List(args) => run_list(&manager, args),
            SecretsSubcommand::Delete(args) => run_delete(&manager, args),
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

fn run_set(manager: &SecretsManager, args: SecretsSetArgs) -> Result<()> {
    let name = SecretName::new(&args.name)?;
    let scope = resolve_scope(&args.scope)?;
    let value = resolve_value(&args.name, args.value)?;
    manager.set(&scope, &name, &value)?;
    println!("Stored {name} in {}.", scope_label(&scope));
    Ok(())
}

fn run_edit(manager: &SecretsManager, args: SecretsEditArgs) -> Result<()> {
    let name = SecretName::new(&args.name)?;
    let scope = resolve_scope(&args.scope)?;
    let exists = manager.get(&scope, &name)?.is_some();
    if !exists {
        bail!(
            "No secret named {name} found in {}. Use `codex secrets set {name}` to create it.",
            scope_label(&scope)
        );
    }
    let value = resolve_value(&args.name, args.value)?;
    manager.set(&scope, &name, &value)?;
    println!("Updated {name} in {}.", scope_label(&scope));
    Ok(())
}

fn run_list(manager: &SecretsManager, args: SecretsListArgs) -> Result<()> {
    let scope_filter = match (args.scope.global, args.scope.environment_id.as_deref()) {
        (true, _) => SecretScope::Global,
        (false, Some(env_id)) => SecretScope::environment(env_id.to_string())?,
        (false, None) => SecretScope::Global,
    };

    let mut entries = manager.list(None)?;
    entries.retain(|entry| entry.scope == scope_filter);

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

fn run_delete(manager: &SecretsManager, args: SecretsDeleteArgs) -> Result<()> {
    let name = SecretName::new(&args.name)?;
    let scope = resolve_scope(&args.scope)?;
    let removed = manager.delete(&scope, &name)?;
    if removed {
        println!("Deleted {name} from {}.", scope_label(&scope));
    } else {
        println!("No secret named {name} found in {}.", scope_label(&scope));
    }
    Ok(())
}

fn resolve_scope(scope_args: &SecretsScopeArgs) -> Result<SecretScope> {
    if scope_args.global {
        return Ok(SecretScope::Global);
    }
    if let Some(env_id) = scope_args.environment_id.as_deref() {
        return SecretScope::environment(env_id.to_string());
    }
    Ok(SecretScope::Global)
}

fn resolve_value(display_name: &str, explicit: Option<String>) -> Result<String> {
    if let Some(value) = explicit {
        return Ok(value);
    }

    if std::io::stdin().is_terminal() {
        return prompt_secret_value(display_name);
    }

    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .context("failed to read secret value from stdin")?;
    let trimmed = buf.trim_end_matches(['\n', '\r']).to_string();
    anyhow::ensure!(!trimmed.is_empty(), "secret value must not be empty");
    Ok(trimmed)
}

fn prompt_secret_value(display_name: &str) -> Result<String> {
    print!("Enter value for {display_name} (experimental): ");
    std::io::stdout()
        .flush()
        .context("failed to flush stdout before prompt")?;

    enable_raw_mode().context("failed to enable raw mode for secret prompt")?;
    let _raw_mode_guard = RawModeGuard;

    let mut value = String::new();

    loop {
        let event = crossterm::event::read().context("failed to read secret input")?;
        let Event::Key(key_event) = event else {
            continue;
        };

        match key_event.code {
            KeyCode::Enter => {
                println!();
                break;
            }
            KeyCode::Backspace => {
                if value.pop().is_some() {
                    print!("\u{8} \u{8}");
                    std::io::stdout()
                        .flush()
                        .context("failed to flush stdout after backspace")?;
                }
            }
            KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                println!();
                bail!("secret input cancelled");
            }
            KeyCode::Esc => {
                println!();
                bail!("secret input cancelled");
            }
            KeyCode::Char(ch) if !key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                value.push(ch);
                print!("*");
                std::io::stdout()
                    .flush()
                    .context("failed to flush stdout after input")?;
            }
            _ => {}
        }
    }

    anyhow::ensure!(!value.is_empty(), "secret value must not be empty");
    Ok(value)
}

struct RawModeGuard;

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
    }
}

fn scope_label(scope: &SecretScope) -> String {
    match scope {
        SecretScope::Global => "global".to_string(),
        SecretScope::Environment(env_id) => format!("env/{env_id}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_core::config::ConfigBuilder;
    use codex_core::config::ConfigOverrides;
    use codex_keyring_store::tests::MockKeyringStore;
    use pretty_assertions::assert_eq;
    use std::sync::Arc;

    async fn test_config(codex_home: &std::path::Path, cwd: &std::path::Path) -> Result<Config> {
        let overrides = ConfigOverrides {
            cwd: Some(cwd.to_path_buf()),
            ..Default::default()
        };
        Ok(ConfigBuilder::default()
            .codex_home(codex_home.to_path_buf())
            .harness_overrides(overrides)
            .build()
            .await?)
    }

    #[tokio::test]
    async fn edit_updates_existing_secret() -> Result<()> {
        let codex_home = tempfile::tempdir().context("temp codex home")?;
        let cwd = tempfile::tempdir().context("temp cwd")?;
        let config = test_config(codex_home.path(), cwd.path()).await?;
        let keyring = Arc::new(MockKeyringStore::default());
        let manager = SecretsManager::new_with_keyring_store(
            config.codex_home.clone(),
            config.secrets_backend,
            keyring,
        );

        let scope = SecretScope::Global;
        let name = SecretName::new("TEST_SECRET")?;
        manager.set(&scope, &name, "before")?;

        run_edit(
            &manager,
            SecretsEditArgs {
                name: "TEST_SECRET".to_string(),
                value: Some("after".to_string()),
                scope: SecretsScopeArgs {
                    global: true,
                    environment_id: None,
                },
            },
        )?;

        assert_eq!(manager.get(&scope, &name)?, Some("after".to_string()));
        Ok(())
    }

    #[tokio::test]
    async fn edit_missing_secret_errors() -> Result<()> {
        let codex_home = tempfile::tempdir().context("temp codex home")?;
        let cwd = tempfile::tempdir().context("temp cwd")?;
        let config = test_config(codex_home.path(), cwd.path()).await?;
        let keyring = Arc::new(MockKeyringStore::default());
        let manager = SecretsManager::new_with_keyring_store(
            config.codex_home.clone(),
            config.secrets_backend,
            keyring,
        );

        let err = run_edit(
            &manager,
            SecretsEditArgs {
                name: "TEST_SECRET".to_string(),
                value: Some("after".to_string()),
                scope: SecretsScopeArgs {
                    global: true,
                    environment_id: None,
                },
            },
        )
        .expect_err("edit should fail when secret is missing");

        let message = err.to_string();
        assert!(message.contains("No secret named TEST_SECRET found in global."));
        assert!(message.contains("codex secrets set TEST_SECRET"));
        Ok(())
    }

    #[test]
    fn resolve_scope_defaults_to_global() -> Result<()> {
        let scope = resolve_scope(&SecretsScopeArgs {
            global: false,
            environment_id: None,
        })?;

        assert_eq!(scope, SecretScope::Global);
        Ok(())
    }
}
