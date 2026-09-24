//! Bounded launcher-only environment transport for policies too large for
//! Windows command lines. Every transport variable is removed before spawn.

use std::collections::HashMap;

#[cfg(any(windows, test))]
use anyhow::Context;
use anyhow::Result;
use codex_windows_sandbox::environment_transport;

use crate::MxcCommand;

pub(super) fn encode(command: &MxcCommand, env: &mut HashMap<String, String>) -> Result<()> {
    let payload = serde_json::to_string(command)?;
    environment_transport::encode(&payload, env)
}

#[cfg(any(windows, test))]
pub(super) fn decode(env: &mut HashMap<String, String>) -> Result<MxcCommand> {
    let payload = environment_transport::decode_and_scrub(env)?;
    serde_json::from_str(&payload).context("invalid MXC launcher request")
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
