//! Local daemon launch policy. Explicit embedded launches never discover or start a daemon;
//! automatic launches require a successful shared-server connection.

use super::*;

pub(super) const FAILURE_HINT: &str = "To work without the background server, rerun the same command with --no-daemon (including resume or fork and its arguments).";

pub(super) fn exclusion(
    cli: &Cli,
    cli_kv_overrides: &[(String, toml::Value)],
    loader_overrides: &LoaderOverrides,
    workload_identity_selected: bool,
    exec_server_url: Option<&std::ffi::OsStr>,
) -> Option<&'static str> {
    if cli.no_daemon {
        Some("--no-daemon")
    } else if cli.oss {
        Some("--oss")
    } else if workload_identity_selected {
        Some("workload identity")
    } else if exec_server_url.is_some() {
        Some("executor selection (CODEX_EXEC_SERVER_URL)")
    } else if cli.agents_overview {
        None
    } else if cli.config_profile_v2.is_some() {
        Some("--profile")
    } else {
        config_exclusion(
            cli_kv_overrides,
            loader_overrides,
            cli.strict_config,
            cli.bypass_hook_trust,
        )
    }
}

pub(super) fn config_exclusion(
    cli_kv_overrides: &[(String, toml::Value)],
    loader_overrides: &LoaderOverrides,
    strict_config: bool,
    bypass_hook_trust: bool,
) -> Option<&'static str> {
    // Worktree allocation is client-owned, and thread requests already forward this feature.
    if !cli_kv_overrides
        .iter()
        .all(|(key, value)| match key.as_str() {
            "features.worktrees" => value.is_bool(),
            "features" => value.as_table().is_some_and(|features| {
                features.len() == 1 && features.get("worktrees").is_some_and(toml::Value::is_bool)
            }),
            _ => false,
        })
    {
        Some("command-line configuration overrides (-c, --enable, --disable, or --search)")
    } else if !loader_overrides_are_default(loader_overrides) {
        Some("custom configuration loader")
    } else if strict_config {
        Some("--strict-config")
    } else if bypass_hook_trust {
        Some("--dangerously-bypass-hook-trust")
    } else {
        None
    }
}
