pub(crate) mod debug_sandbox;
mod exit_status;
pub(crate) mod login;

use clap::Parser;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_cli::CliConfigOverrides;
use codex_utils_cli::ProfileV2Name;
use std::path::PathBuf;

pub use debug_sandbox::run_command_under_landlock;
pub use debug_sandbox::run_command_under_seatbelt;
pub use debug_sandbox::run_command_under_windows_sandbox;
pub use login::read_access_token_from_stdin;
pub use login::read_api_key_from_stdin;
pub use login::run_login_status;
pub use login::run_login_with_access_token;
pub use login::run_login_with_api_key;
pub use login::run_login_with_chatgpt;
pub use login::run_login_with_device_code;
pub use login::run_login_with_device_code_fallback_to_browser;
pub use login::run_logout;

// These command structs share common sandbox options, but remain separate
// because each host backend has a slightly different option surface.
#[derive(Debug, Parser)]
pub struct SeatbeltCommand {
    /// Named permissions profile to apply from the active configuration stack.
    #[arg(long = "permissions-profile", value_name = "NAME")]
    pub permissions_profile: Option<String>,

    /// Layer $CODEX_HOME/<name>.config.toml on top of the base user config.
    #[arg(long = "profile", short = 'p')]
    pub config_profile: Option<ProfileV2Name>,

    /// Working directory used for profile resolution and command execution.
    #[arg(
        short = 'C',
        long = "cd",
        value_name = "DIR",
        requires = "permissions_profile"
    )]
    pub cwd: Option<PathBuf>,

    /// Include managed requirements while resolving an explicit permissions profile.
    #[arg(
        long = "include-managed-config",
        default_value_t = false,
        requires = "permissions_profile"
    )]
    pub include_managed_config: bool,

    /// Allow the sandboxed command to look up this Mach service name. Repeat to allow multiple services.
    #[arg(long = "allow-mach-service", value_name = "SERVICE", value_parser = parse_non_empty_string)]
    pub allow_mach_services: Vec<String>,

    /// Allow the sandboxed command to send AppleEvents to this destination bundle ID. Repeat to allow multiple destinations.
    #[arg(long = "allow-appleevent-destination", value_name = "BUNDLE_ID", value_parser = parse_non_empty_string)]
    pub allow_appleevent_bundle_ids: Vec<String>,

    /// Allow the sandboxed command to use LaunchServices open APIs.
    #[arg(long = "allow-lsopen", default_value_t = false)]
    pub allow_lsopen: bool,

    /// Allow the sandboxed command to bind/connect AF_UNIX sockets rooted at this path. Relative paths are resolved against the current directory. Repeat to allow multiple paths.
    #[arg(long = "allow-unix-socket", value_parser = parse_allow_unix_socket_path)]
    pub allow_unix_sockets: Vec<AbsolutePathBuf>,

    /// While the command runs, capture macOS sandbox denials via `log stream` and print them after exit
    #[arg(long = "log-denials", default_value_t = false)]
    pub log_denials: bool,

    #[clap(skip)]
    pub config_overrides: CliConfigOverrides,

    /// Full command args to run under seatbelt.
    #[arg(trailing_var_arg = true)]
    pub command: Vec<String>,
}

fn parse_allow_unix_socket_path(raw: &str) -> Result<AbsolutePathBuf, String> {
    AbsolutePathBuf::relative_to_current_dir(raw)
        .map_err(|err| format!("invalid path {raw}: {err}"))
}

fn parse_non_empty_string(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        Err("value must not be empty".to_string())
    } else {
        Ok(trimmed.to_string())
    }
}

#[derive(Debug, Parser)]
pub struct LandlockCommand {
    /// Named permissions profile to apply from the active configuration stack.
    #[arg(long = "permissions-profile", value_name = "NAME")]
    pub permissions_profile: Option<String>,

    /// Layer $CODEX_HOME/<name>.config.toml on top of the base user config.
    #[arg(long = "profile", short = 'p')]
    pub config_profile: Option<ProfileV2Name>,

    /// Working directory used for profile resolution and command execution.
    #[arg(
        short = 'C',
        long = "cd",
        value_name = "DIR",
        requires = "permissions_profile"
    )]
    pub cwd: Option<PathBuf>,

    /// Include managed requirements while resolving an explicit permissions profile.
    #[arg(
        long = "include-managed-config",
        default_value_t = false,
        requires = "permissions_profile"
    )]
    pub include_managed_config: bool,

    #[clap(skip)]
    pub config_overrides: CliConfigOverrides,

    /// Full command args to run under the Linux sandbox.
    #[arg(trailing_var_arg = true)]
    pub command: Vec<String>,
}

#[derive(Debug, Parser)]
pub struct WindowsCommand {
    /// Named permissions profile to apply from the active configuration stack.
    #[arg(long = "permissions-profile", value_name = "NAME")]
    pub permissions_profile: Option<String>,

    /// Layer $CODEX_HOME/<name>.config.toml on top of the base user config.
    #[arg(long = "profile", short = 'p')]
    pub config_profile: Option<ProfileV2Name>,

    /// Working directory used for profile resolution and command execution.
    #[arg(
        short = 'C',
        long = "cd",
        value_name = "DIR",
        requires = "permissions_profile"
    )]
    pub cwd: Option<PathBuf>,

    /// Include managed requirements while resolving an explicit permissions profile.
    #[arg(
        long = "include-managed-config",
        default_value_t = false,
        requires = "permissions_profile"
    )]
    pub include_managed_config: bool,

    #[clap(skip)]
    pub config_overrides: CliConfigOverrides,

    /// Full command args to run under Windows restricted token sandbox.
    #[arg(trailing_var_arg = true)]
    pub command: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::SeatbeltCommand;
    use clap::Parser;
    use pretty_assertions::assert_eq;

    #[test]
    fn seatbelt_command_parses_additional_allowlist_flags() {
        let command = SeatbeltCommand::try_parse_from([
            "seatbelt",
            "--allow-mach-service",
            "com.apple.foo",
            "--allow-mach-service",
            "com.apple.bar",
            "--allow-appleevent-destination",
            "com.apple.finder",
            "--allow-lsopen",
            "--allow-unix-socket",
            "/tmp/codex.sock",
            "--",
            "/bin/echo",
            "hi",
        ])
        .expect("parse");

        assert_eq!(
            command.allow_mach_services,
            vec!["com.apple.foo".to_string(), "com.apple.bar".to_string()]
        );
        assert_eq!(
            command.allow_appleevent_bundle_ids,
            vec!["com.apple.finder".to_string()]
        );
        assert!(command.allow_lsopen);
        assert_eq!(command.allow_unix_sockets.len(), 1);
        assert_eq!(
            command.command,
            vec!["/bin/echo".to_string(), "hi".to_string()]
        );
    }
}
