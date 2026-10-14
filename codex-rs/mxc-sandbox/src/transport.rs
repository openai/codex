//! Bounded launcher transport. Windows startup variables are host-owned;
//! an original-empty marker preserves the SDK's user-profile defaults.

use std::collections::HashMap;

#[cfg(any(windows, test))]
use anyhow::Context;
use anyhow::Result;
use codex_windows_sandbox::environment_transport;

use crate::MxcCommand;

pub(super) fn encode(command: &MxcCommand, env: &mut HashMap<String, String>) -> Result<()> {
    // SDK profile defaults depend on the launcher's SystemRoot.
    #[cfg(windows)]
    for key in ["SystemDrive", "LOCALAPPDATA"]
        .into_iter()
        .chain(command.default_child_env.then_some("SystemRoot"))
    {
        let value = std::env::var(key)
            .ok()
            .filter(|value| !value.is_empty())
            .with_context(|| format!("MXC requires nonempty Unicode host {key}"))?;
        env.retain(|name, _| !name.eq_ignore_ascii_case(key));
        env.insert(key.to_owned(), value);
    }
    let payload = serde_json::to_string(command)?;
    environment_transport::encode(&payload, env)
}

#[cfg(any(windows, test))]
pub(super) fn decode(env: &mut HashMap<String, String>) -> Result<MxcCommand> {
    let payload = environment_transport::decode_and_scrub(env)?;
    let command: MxcCommand =
        serde_json::from_str(&payload).context("invalid MXC launcher request")?;
    if command.default_child_env {
        env.clear();
    }
    Ok(command)
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
