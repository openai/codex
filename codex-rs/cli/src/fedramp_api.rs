use anyhow::bail;
use codex_utils_cli::CliConfigOverrides;
use std::ffi::OsString;

use super::LoginSubcommand;
use super::Subcommand;

const FEDRAMP_API_BINARY_NAME: &str = "codex-fedramp-api";
const FEDRAMP_API_DEFAULTS_TOML: &str = include_str!("fedramp_api_defaults.toml");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CodexFlavor {
    Normal,
    FedRAMPApi,
}

impl CodexFlavor {
    pub(crate) fn compiled() -> Self {
        Self::from_compiled_binary_name(option_env!("CARGO_BIN_NAME"))
    }

    fn from_compiled_binary_name(binary_name: Option<&str>) -> Self {
        match binary_name {
            Some(FEDRAMP_API_BINARY_NAME) => Self::FedRAMPApi,
            _ => Self::Normal,
        }
    }

    fn is_fedramp_api(self) -> bool {
        self == Self::FedRAMPApi
    }
}

pub(crate) fn apply(
    flavor: CodexFlavor,
    root_config_overrides: &mut CliConfigOverrides,
    subcommand: Option<&Subcommand>,
) -> anyhow::Result<()> {
    if !flavor.is_fedramp_api() {
        return Ok(());
    }

    reject_unsafe_cli_args(std::env::args_os().skip(1))?;
    reject_unsupported_subcommand(subcommand)?;
    root_config_overrides
        .raw_overrides
        .extend(blessed_config_overrides()?);
    Ok(())
}

fn blessed_config_overrides() -> anyhow::Result<Vec<String>> {
    let config: toml::Value = toml::from_str(FEDRAMP_API_DEFAULTS_TOML)?;
    let mut overrides = Vec::new();
    flatten_toml_value(&config, None, &mut overrides);
    Ok(overrides)
}

fn flatten_toml_value(value: &toml::Value, prefix: Option<&str>, overrides: &mut Vec<String>) {
    if let toml::Value::Table(table) = value {
        if table.is_empty() {
            if let Some(prefix) = prefix {
                overrides.push(format!("{prefix}={{}}"));
            }
            return;
        }

        for (key, child) in table {
            let path = match prefix {
                Some(prefix) => format!("{prefix}.{key}"),
                None => key.clone(),
            };
            flatten_toml_value(child, Some(&path), overrides);
        }
        return;
    }

    if let Some(prefix) = prefix {
        overrides.push(format!("{prefix}={value}"));
    }
}

fn reject_unsafe_cli_args(args: impl Iterator<Item = OsString>) -> anyhow::Result<()> {
    for arg in args {
        let arg = arg.to_string_lossy();
        if arg == "--" {
            break;
        }

        if matches!(
            arg.as_ref(),
            "-c" | "--config"
                | "--enable"
                | "--disable"
                | "-p"
                | "--profile"
                | "--oss"
                | "--local-provider"
                | "--remote"
                | "--remote-auth-token-env"
        ) || arg.starts_with("--config=")
            || arg.starts_with("-c")
            || arg.starts_with("--enable=")
            || arg.starts_with("--disable=")
            || arg.starts_with("--profile=")
            || arg.starts_with("-p")
            || arg.starts_with("--local-provider=")
            || arg.starts_with("--remote=")
            || arg.starts_with("--remote-auth-token-env=")
        {
            bail!("{arg} is not supported by codex-fedramp-api");
        }
    }

    Ok(())
}

fn reject_unsupported_subcommand(subcommand: Option<&Subcommand>) -> anyhow::Result<()> {
    match subcommand {
        None
        | Some(Subcommand::Exec(_))
        | Some(Subcommand::Review(_))
        | Some(Subcommand::Logout(_))
        | Some(Subcommand::Completion(_))
        | Some(Subcommand::Doctor(_))
        | Some(Subcommand::Sandbox(_))
        | Some(Subcommand::Debug(_))
        | Some(Subcommand::Execpolicy(_))
        | Some(Subcommand::Apply(_))
        | Some(Subcommand::Resume(_))
        | Some(Subcommand::Archive(_))
        | Some(Subcommand::Delete(_))
        | Some(Subcommand::Unarchive(_))
        | Some(Subcommand::Fork(_)) => Ok(()),
        Some(Subcommand::Login(login)) => {
            if matches!(login.action, Some(LoginSubcommand::Status))
                || (login.with_api_key
                    && !login.with_access_token
                    && !login.use_device_code
                    && login.api_key.is_none())
            {
                Ok(())
            } else {
                bail!("codex-fedramp-api only supports API-key login and login status");
            }
        }
        Some(Subcommand::Mcp(_))
        | Some(Subcommand::Plugin(_))
        | Some(Subcommand::McpServer(_))
        | Some(Subcommand::AppServer(_))
        | Some(Subcommand::RemoteControl(_))
        | Some(Subcommand::Update)
        | Some(Subcommand::Cloud(_))
        | Some(Subcommand::ResponsesApiProxy(_))
        | Some(Subcommand::StdioToUds(_))
        | Some(Subcommand::ExecServer(_))
        | Some(Subcommand::Features(_)) => {
            bail!("this command is not supported by codex-fedramp-api");
        }
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        Some(Subcommand::App(_)) => {
            bail!("this command is not supported by codex-fedramp-api");
        }
    }
}

#[cfg(test)]
#[path = "fedramp_api_tests.rs"]
mod tests;
