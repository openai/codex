//! Bounded launcher-only environment transport for policies too large for
//! Windows command lines. Every transport variable is removed before spawn.

use std::collections::HashMap;

#[cfg(any(windows, test))]
use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;

use crate::MxcCommand;

const PREFIX: &str = "CODEX_MXC_LAUNCH_";
const COUNT: &str = "CODEX_MXC_LAUNCH_COUNT";
const LENGTH: &str = "CODEX_MXC_LAUNCH_BYTES";
const CHUNK_BYTES: usize = 4096;
const MAX_CHUNKS: usize = 256;
const MAX_BYTES: usize = 1_000_000;

fn is_transport_key(key: &str) -> bool {
    key.as_bytes()
        .get(..PREFIX.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(PREFIX.as_bytes()))
}

pub fn encode(command: &MxcCommand, env: &mut HashMap<String, String>) -> Result<()> {
    let payload = serde_json::to_string(command)?;
    ensure!(
        payload.len() <= MAX_BYTES,
        "MXC launcher payload exceeds {MAX_BYTES} bytes"
    );
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < payload.len() {
        let mut end = (start + CHUNK_BYTES).min(payload.len());
        while !payload.is_char_boundary(end) {
            end -= 1;
        }
        chunks.push(payload[start..end].to_owned());
        start = end;
    }
    ensure!(
        chunks.len() <= MAX_CHUNKS,
        "MXC launcher payload has too many chunks"
    );
    env.retain(|key, _| !is_transport_key(key));
    env.insert(COUNT.to_owned(), chunks.len().to_string());
    env.insert(LENGTH.to_owned(), payload.len().to_string());
    for (index, chunk) in chunks.into_iter().enumerate() {
        env.insert(format!("{PREFIX}{index}"), chunk);
    }
    Ok(())
}

#[cfg(any(windows, test))]
pub fn decode(env: &mut HashMap<String, String>) -> Result<MxcCommand> {
    let mut encoded = HashMap::new();
    let mut duplicate = false;
    let mut oversized = false;
    env.retain(|key, value| {
        if !is_transport_key(key) {
            return true;
        }
        if encoded.len() >= MAX_CHUNKS + 2
            || key.len() > PREFIX.len() + 32
            || value.len() > CHUNK_BYTES
        {
            oversized = true;
            return false;
        }
        duplicate |= encoded
            .insert(key.to_ascii_uppercase(), std::mem::take(value))
            .is_some();
        false
    });
    ensure!(!oversized, "oversized MXC launcher environment");
    ensure!(!duplicate, "duplicate MXC launcher environment variables");
    let count: usize = encoded
        .get(COUNT)
        .context("missing MXC launcher chunk count")?
        .parse()?;
    let length: usize = encoded
        .get(LENGTH)
        .context("missing MXC launcher payload length")?
        .parse()?;
    ensure!(
        (1..=MAX_CHUNKS).contains(&count) && length <= MAX_BYTES,
        "invalid MXC launcher payload size"
    );
    ensure!(
        encoded.len() == count + 2,
        "unexpected MXC launcher environment variables"
    );
    let mut payload = String::with_capacity(length);
    for index in 0..count {
        let chunk = encoded
            .get(&format!("{PREFIX}{index}"))
            .context("missing MXC launcher chunk")?;
        ensure!(
            !chunk.is_empty() && chunk.len() <= CHUNK_BYTES,
            "invalid MXC launcher chunk size"
        );
        ensure!(
            payload.len() + chunk.len() <= length,
            "MXC launcher payload length mismatch"
        );
        payload.push_str(chunk);
    }
    ensure!(
        payload.len() == length,
        "MXC launcher payload length mismatch"
    );
    serde_json::from_str(&payload).context("invalid MXC launcher request")
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
