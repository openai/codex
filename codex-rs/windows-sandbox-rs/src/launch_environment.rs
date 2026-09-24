//! Invocation-scoped transport for Windows sandbox payloads that exceed argv.
//! Chunks stay below the per-variable limit; no process-global environment is mutated.

#[cfg(windows)]
use anyhow::Result;

#[cfg(windows)]
use crate::environment_transport;

#[cfg(windows)]
pub(crate) const ARG: &str = "--launch-payload-env";

/// Keep small launches compatible with the existing argv protocol.
pub(crate) fn needs_environment(payload: &str) -> bool {
    payload.encode_utf16().count() > 24_000
}

#[cfg(windows)]
pub(crate) fn configure_command(command: &mut std::process::Command, payload: &str) -> Result<()> {
    // A wrapper may itself carry a payload. Never inherit its chunks into setup.
    for (key, _) in std::env::vars_os() {
        if environment_transport::is_key(&key) {
            command.env_remove(key);
        }
    }
    if needs_environment(payload) {
        let mut env = std::collections::HashMap::new();
        environment_transport::encode(payload, &mut env)?;
        command.arg(ARG).envs(env);
    } else {
        command.arg(payload);
    }
    Ok(())
}

#[cfg(test)]
#[path = "launch_environment_tests.rs"]
mod tests;
