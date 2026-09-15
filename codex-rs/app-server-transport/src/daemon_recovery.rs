//! Shared on-disk candidate set for managed daemon restarts.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io;
use std::path::Path;

use codex_core::path_utils::write_atomically;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct RecoverySnapshot {
    #[serde(skip)]
    pub loaded: BTreeSet<String>,
    pub interrupted: BTreeMap<String, InterruptedTurn>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct InterruptedTurn {
    pub turn_id: String,
    pub output_schema: Option<serde_json::Value>,
    pub service_tier: Option<String>,
    pub cyber_access_program: Option<codex_protocol::turn_input::CyberAccessProgram>,
}

// Old servers accept the array and skip this non-thread entry during best-effort
// restoration. Keeping metadata in the same atomic file avoids stale sidecars.
const INTERRUPTION_PREFIX: &str = "codex-interrupted-v1:";

pub fn read_snapshot(path: &Path) -> io::Result<RecoverySnapshot> {
    let mut loaded: BTreeSet<String> = match std::fs::read(path) {
        Ok(contents) => serde_json::from_slice(&contents).map_err(io::Error::other)?,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(RecoverySnapshot::default()),
        Err(err) => return Err(err),
    };
    let mut snapshot = RecoverySnapshot::default();
    loaded.retain(|entry| {
        if let Some(metadata) = entry.strip_prefix(INTERRUPTION_PREFIX) {
            if let Ok(saved) = serde_json::from_str::<RecoverySnapshot>(metadata) {
                snapshot = saved;
            }
            false
        } else {
            true
        }
    });
    snapshot.interrupted.retain(|id, _| loaded.contains(id));
    snapshot.loaded = loaded;
    Ok(snapshot)
}

pub fn read_candidates(path: &Path) -> io::Result<BTreeSet<String>> {
    Ok(read_snapshot(path)?.loaded)
}

pub fn write_candidates(path: &Path, candidates: &BTreeSet<String>) -> io::Result<()> {
    write_snapshot(
        path,
        &RecoverySnapshot {
            loaded: candidates.clone(),
            ..Default::default()
        },
    )
}

pub fn write_snapshot(path: &Path, snapshot: &RecoverySnapshot) -> io::Result<()> {
    let mut saved = snapshot.loaded.clone();
    if !snapshot.interrupted.is_empty() {
        saved.insert(format!(
            "{INTERRUPTION_PREFIX}{}",
            serde_json::to_string(snapshot).map_err(io::Error::other)?
        ));
    }
    write_atomically(
        path,
        &serde_json::to_string(&saved).map_err(io::Error::other)?,
    )
}
