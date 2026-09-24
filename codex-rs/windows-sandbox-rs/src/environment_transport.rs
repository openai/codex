//! Bounded launcher-only environment transport for payloads too large for
//! Windows command lines.

use std::collections::HashMap;
use std::ffi::OsStr;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;

const PREFIX: &str = "CODEX_SANDBOX_LAUNCH_";
const COUNT: &str = "CODEX_SANDBOX_LAUNCH_COUNT";
const LENGTH: &str = "CODEX_SANDBOX_LAUNCH_BYTES";
const CHUNK_BYTES: usize = 16 * 1024;
const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_CHUNKS: usize = MAX_BYTES / (CHUNK_BYTES - 3) + 1;

/// Whether `key` belongs to the shared sandbox launch transport namespace.
pub fn is_key(key: &OsStr) -> bool {
    environment_key_starts_with(key, PREFIX)
}

/// Split a UTF-8 payload into bounded environment variables.
///
/// Existing transport variables are removed only after the complete payload
/// has passed validation.
pub fn encode(payload: &str, env: &mut HashMap<String, String>) -> Result<()> {
    ensure!(
        !payload.is_empty() && payload.len() <= MAX_BYTES,
        "sandbox launch payload must contain 1..={MAX_BYTES} bytes"
    );
    ensure!(
        !payload.contains('\0'),
        "sandbox launch payload contains a NUL"
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
        "sandbox launch payload has too many chunks"
    );
    env.retain(|key, _| !is_key(key.as_ref()));
    env.insert(COUNT.to_owned(), chunks.len().to_string());
    env.insert(LENGTH.to_owned(), payload.len().to_string());
    for (index, chunk) in chunks.into_iter().enumerate() {
        env.insert(format!("{PREFIX}{index}"), chunk);
    }
    Ok(())
}

/// Reconstruct a payload without inspecting unrelated non-Unicode variables.
pub fn decode<K: AsRef<OsStr>, V: AsRef<OsStr>>(
    env: impl IntoIterator<Item = (K, V)>,
) -> Result<String> {
    let mut encoded = HashMap::new();
    for (key, value) in env {
        let key = key.as_ref();
        if !is_key(key) {
            continue;
        }
        let key = canonical_key(key)?;
        let value = value
            .as_ref()
            .to_str()
            .context("non-Unicode sandbox launch environment value")?;
        ensure!(
            encoded.len() < MAX_CHUNKS + 2
                && key.len() <= PREFIX.len() + 32
                && value.len() <= CHUNK_BYTES,
            "oversized sandbox launch environment"
        );
        ensure!(
            encoded.insert(key, value.to_owned()).is_none(),
            "duplicate sandbox launch environment variables"
        );
    }
    let count: usize = encoded
        .get(COUNT)
        .context("missing sandbox launch chunk count")?
        .parse()?;
    let length: usize = encoded
        .get(LENGTH)
        .context("missing sandbox launch payload length")?
        .parse()?;
    ensure!(
        (1..=MAX_CHUNKS).contains(&count) && length <= MAX_BYTES,
        "invalid sandbox launch payload size"
    );
    ensure!(
        encoded.len() == count + 2,
        "unexpected sandbox launch environment variables"
    );
    let mut payload = String::with_capacity(length);
    for index in 0..count {
        let chunk = encoded
            .get(&format!("{PREFIX}{index}"))
            .context("missing sandbox launch chunk")?;
        ensure!(!chunk.is_empty(), "invalid sandbox launch chunk size");
        ensure!(
            payload.len() + chunk.len() <= length,
            "sandbox launch payload length mismatch"
        );
        payload.push_str(chunk);
    }
    ensure!(
        payload.len() == length,
        "sandbox launch payload length mismatch"
    );
    Ok(payload)
}

/// Remove every transport variable from `env`, even when decoding fails.
pub fn decode_and_scrub(env: &mut HashMap<String, String>) -> Result<String> {
    let payload = decode(env.iter());
    env.retain(|key, _| !is_key(key.as_ref()));
    payload
}

fn canonical_key(key: &OsStr) -> Result<String> {
    if environment_key_eq(key, COUNT) {
        return Ok(COUNT.to_owned());
    }
    if environment_key_eq(key, LENGTH) {
        return Ok(LENGTH.to_owned());
    }
    let key = key
        .to_str()
        .context("non-Unicode sandbox launch environment key")?;
    Ok(format!(
        "{PREFIX}{}",
        key.chars().skip(PREFIX.len()).collect::<String>()
    ))
}

#[cfg(windows)]
fn environment_key_eq(key: &OsStr, expected: &str) -> bool {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CompareStringOrdinal(
            left: *const u16,
            left_len: i32,
            right: *const u16,
            right_len: i32,
            ignore_case: i32,
        ) -> i32;
    }
    let key: Vec<u16> = key.encode_wide().collect();
    let expected: Vec<u16> = expected.encode_utf16().collect();
    key.len() == expected.len()
        && unsafe {
            CompareStringOrdinal(
                key.as_ptr(),
                key.len() as i32,
                expected.as_ptr(),
                expected.len() as i32,
                /*ignore_case*/ 1,
            ) == 2
        }
}

#[cfg(not(windows))]
fn environment_key_eq(key: &OsStr, expected: &str) -> bool {
    key.as_encoded_bytes()
        .eq_ignore_ascii_case(expected.as_bytes())
}

#[cfg(windows)]
fn environment_key_starts_with(key: &OsStr, prefix: &str) -> bool {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::ffi::OsStringExt;
    let key_prefix: Vec<u16> = key
        .encode_wide()
        .take(prefix.encode_utf16().count())
        .collect();
    environment_key_eq(&OsString::from_wide(&key_prefix), prefix)
}

#[cfg(not(windows))]
fn environment_key_starts_with(key: &OsStr, prefix: &str) -> bool {
    key.as_encoded_bytes()
        .get(..prefix.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(prefix.as_bytes()))
}

#[cfg(test)]
#[path = "environment_transport_tests.rs"]
mod tests;
