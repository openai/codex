use std::path::PathBuf;

use anyhow::Context;
use clap::ArgAction;
use clap::ArgGroup;
use clap::Parser;
use codex_core::config::ConfigBuilder;
use codex_core::config::edit::ConfigEditsBuilder;
use codex_core::config::find_codex_home;
use codex_utils_cli::ProfileV2Name;
use toml::Value as TomlValue;

#[derive(Debug, Parser)]
#[command(group(
    ArgGroup::new("sandbox_user")
        .required(true)
        .args(["user", "current_user"])
))]
pub(crate) struct SandboxSetupCommand {
    /// Set up the elevated Windows sandbox.
    #[arg(long = "elevated", action = ArgAction::SetTrue)]
    elevated_sandbox_level: bool,

    /// Windows user that will run Codex after managed deployment.
    #[arg(
        long = "user",
        value_name = "USER",
        conflicts_with = "current_user",
        requires = "codex_home"
    )]
    user: Option<String>,

    /// Use the current Windows user as the Codex user.
    #[arg(
        long = "current-user",
        default_value_t = false,
        conflicts_with = "user"
    )]
    current_user: bool,

    /// CODEX_HOME for the Codex user. Required with --user.
    #[arg(long = "codex-home", value_name = "DIR")]
    codex_home: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SandboxSetupLevel {
    Elevated,
}

impl SandboxSetupCommand {
    fn setup_level(&self) -> anyhow::Result<SandboxSetupLevel> {
        if self.elevated_sandbox_level {
            Ok(SandboxSetupLevel::Elevated)
        } else {
            anyhow::bail!("`codex sandbox setup` currently requires --elevated");
        }
    }
}

pub(crate) async fn run(
    cmd: SandboxSetupCommand,
    config_profile: Option<ProfileV2Name>,
    cli_overrides: Vec<(String, TomlValue)>,
) -> anyhow::Result<()> {
    match cmd.setup_level()? {
        SandboxSetupLevel::Elevated => run_elevated(cmd, config_profile, cli_overrides).await,
    }
}

pub(crate) fn parse_setup_command(
    sandbox_command: &[String],
) -> anyhow::Result<Option<SandboxSetupCommand>> {
    if sandbox_command
        .first()
        .is_none_or(|command| command != "setup")
    {
        return Ok(None);
    }

    SandboxSetupCommand::try_parse_from(sandbox_command.iter().map(String::as_str))
        .map(Some)
        .map_err(anyhow::Error::from)
}

async fn run_elevated(
    cmd: SandboxSetupCommand,
    config_profile: Option<ProfileV2Name>,
    cli_overrides: Vec<(String, TomlValue)>,
) -> anyhow::Result<()> {
    let identity = resolve_sandbox_setup_identity(&cmd)?;
    let config = ConfigBuilder::default()
        .codex_home(identity.codex_home.clone())
        .fallback_cwd(Some(identity.codex_home.clone()))
        .loader_overrides(super::loader_overrides_for_profile_at_codex_home(
            config_profile.as_ref(),
            &identity.codex_home,
        ))
        .cli_overrides(cli_overrides)
        .build()
        .await
        .context("failed to load target user's Codex config for sandbox provisioning")?;

    codex_core::windows_sandbox::run_elevated_provisioning_setup(
        identity.codex_home.as_path(),
        identity.real_user.as_str(),
        config.permissions.network.as_ref(),
    )?;
    ConfigEditsBuilder::new(identity.codex_home.as_path())
        .set_windows_sandbox_mode("elevated")
        .apply()
        .await
        .map_err(|err| {
            anyhow::anyhow!(
                "sandbox provisioning succeeded, but failed to persist elevated sandbox config: {err}"
            )
        })?;

    println!(
        "Windows elevated sandbox setup completed for {} at {}.",
        identity.real_user,
        identity.codex_home.display()
    );
    Ok(())
}

struct SandboxSetupIdentity {
    real_user: String,
    codex_home: PathBuf,
}

fn resolve_sandbox_setup_identity(
    cmd: &SandboxSetupCommand,
) -> anyhow::Result<SandboxSetupIdentity> {
    if cmd.current_user {
        let real_user = std::env::var("USERNAME")
            .or_else(|_| std::env::var("USER"))
            .map_err(|err| {
                anyhow::anyhow!("failed to determine current user from environment: {err}")
            })?;
        let codex_home = match cmd.codex_home.clone() {
            Some(codex_home) => codex_home,
            None => find_codex_home()?.to_path_buf(),
        };
        return Ok(SandboxSetupIdentity {
            real_user,
            codex_home,
        });
    }

    let real_user = cmd
        .user
        .clone()
        .ok_or_else(|| anyhow::anyhow!("--user or --current-user is required"))?;
    let codex_home = cmd
        .codex_home
        .clone()
        .ok_or_else(|| anyhow::anyhow!("--codex-home is required with --user"))?;
    Ok(SandboxSetupIdentity {
        real_user,
        codex_home,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_managed_user_identity() {
        let command = SandboxSetupCommand::try_parse_from([
            "setup",
            "--elevated",
            "--user",
            "DOMAIN\\alice",
            "--codex-home",
            r"C:\Users\alice\.codex",
        ])
        .expect("parse");

        assert!(command.elevated_sandbox_level);
        assert_eq!(command.user.as_deref(), Some(r"DOMAIN\alice"));
        assert!(!command.current_user);
        assert_eq!(
            command.codex_home.as_deref(),
            Some(std::path::Path::new(r"C:\Users\alice\.codex"))
        );
    }

    #[test]
    fn requires_explicit_user_identity() {
        let err = SandboxSetupCommand::try_parse_from(["setup", "--elevated"])
            .expect_err("parse should fail");

        assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn requires_codex_home_for_managed_user() {
        let err =
            SandboxSetupCommand::try_parse_from(["setup", "--elevated", "--user", "DOMAIN\\alice"])
                .expect_err("parse should fail");

        assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn parses_setup_from_sandbox_command_args() {
        let command = parse_setup_command(&[
            "setup".to_string(),
            "--elevated".to_string(),
            "--user".to_string(),
            r"DOMAIN\alice".to_string(),
            "--codex-home".to_string(),
            r"C:\Users\alice\.codex".to_string(),
        ])
        .expect("parse")
        .expect("setup command");

        assert_eq!(command.user.as_deref(), Some(r"DOMAIN\alice"));
    }

    #[test]
    fn ignores_non_setup_sandbox_command_args() {
        let command =
            parse_setup_command(&["echo".to_string(), "hello".to_string()]).expect("parse");

        assert!(command.is_none());
    }
}
