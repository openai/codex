//! Recognizes rollout files written before the modern rollout envelope format.
//!
//! This module is a Doctor-only structural classifier, not a compatibility
//! parser. It identifies historical files strongly enough to keep them out of
//! modern rollout/SQLite parity without deserializing their response records.
//! Recognition must not be used to index, migrate, rewrite, archive, or resume
//! these files.

use codex_protocol::ThreadId;
use serde_json::Value;
use std::io;
use std::path::Path;

const LEGACY_HEADER_KEYS: [&str; 4] = ["git", "id", "instructions", "timestamp"];

/// Returns whether `path` has the structurally valid historical rollout shape.
///
/// Callers should use this only after the modern rollout loader returns no
/// items. Recognition requires an exact historical header, agreement between
/// the header ID and filename-derived ID, and object-valued JSON for every
/// remaining nonempty record. `Ok(false)` includes malformed or truncated
/// files so Doctor can continue reporting them as scan errors rather than
/// silently treating corruption as unsupported history.
///
/// # Errors
///
/// Returns an I/O error when the rollout cannot be opened or read.
pub(super) async fn is_legacy_rollout(path: &Path) -> io::Result<bool> {
    let mut reader = codex_rollout::open_rollout_line_reader(path).await?;
    let Some(header) = next_non_empty_line(&mut reader).await? else {
        return Ok(false);
    };
    let Ok(header) = serde_json::from_str::<Value>(&header) else {
        return Ok(false);
    };
    let Some(header) = header.as_object() else {
        return Ok(false);
    };
    if header
        .keys()
        .any(|key| !LEGACY_HEADER_KEYS.contains(&key.as_str()))
    {
        return Ok(false);
    }

    let Some(thread_id) = header
        .get("id")
        .and_then(Value::as_str)
        .and_then(|id| ThreadId::from_string(id).ok())
    else {
        return Ok(false);
    };
    if header
        .get("timestamp")
        .and_then(Value::as_str)
        .is_none_or(str::is_empty)
    {
        return Ok(false);
    }
    if !matches!(
        header.get("instructions"),
        Some(Value::Null | Value::String(_))
    ) {
        return Ok(false);
    }
    if !matches!(
        header.get("git"),
        None | Some(Value::Null | Value::Object(_))
    ) {
        return Ok(false);
    }

    let Some(builder) = codex_rollout::builder_from_items(&[], path) else {
        return Ok(false);
    };
    if builder.id != thread_id {
        return Ok(false);
    }

    while let Some(line) = reader.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            return Ok(false);
        };
        if !value.is_object() {
            return Ok(false);
        }
    }

    Ok(true)
}

async fn next_non_empty_line(
    reader: &mut codex_rollout::RolloutLineReader,
) -> io::Result<Option<String>> {
    while let Some(line) = reader.next_line().await? {
        if !line.trim().is_empty() {
            return Ok(Some(line));
        }
    }
    Ok(None)
}

#[cfg(test)]
#[path = "legacy_rollout_tests.rs"]
mod tests;
