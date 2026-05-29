use crate::now_unix_seconds;
use codex_protocol::ThreadId;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

const SESSION_IMPORT_LEDGER_FILE: &str = "external_agent_session_imports.json";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct ImportedExternalAgentSessionLedger {
    records: Vec<ImportedExternalAgentSessionRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ImportedExternalAgentSessionRecord {
    source_path: PathBuf,
    content_sha256: String,
    imported_thread_id: ThreadId,
    imported_at: i64,
}

pub fn has_current_session_been_imported(
    codex_home: &Path,
    source_path: &Path,
) -> io::Result<bool> {
    load_import_ledger(codex_home)?.contains_current_source(source_path)
}

pub fn record_imported_session(
    codex_home: &Path,
    source_path: &Path,
    imported_thread_id: ThreadId,
) -> io::Result<()> {
    let mut ledger = load_import_ledger(codex_home)?;
    let source_path = canonical_source_path(source_path)?;
    let content_sha256 = session_content_sha256(&source_path)?;
    if ledger
        .records
        .iter()
        .any(|record| record.source_path == source_path && record.content_sha256 == content_sha256)
    {
        return Ok(());
    }
    ledger.records.push(ImportedExternalAgentSessionRecord {
        source_path,
        content_sha256,
        imported_thread_id,
        imported_at: now_unix_seconds(),
    });
    save_import_ledger(codex_home, &ledger)
}

impl ImportedExternalAgentSessionLedger {
    pub(super) fn contains_current_source(&self, source_path: &Path) -> io::Result<bool> {
        let source_path = canonical_source_path(source_path)?;
        let content_sha256 = session_content_sha256(&source_path)?;
        Ok(self.records.iter().any(|record| {
            record.source_path == source_path && record.content_sha256 == content_sha256
        }))
    }
}

pub(super) fn load_import_ledger(
    codex_home: &Path,
) -> io::Result<ImportedExternalAgentSessionLedger> {
    let path = import_ledger_path(codex_home);
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Ok(ImportedExternalAgentSessionLedger::default());
        }
        Err(err) => return Err(err),
    };
    serde_json::from_str(&raw).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid external agent session import ledger: {err}"),
        )
    })
}

fn save_import_ledger(
    codex_home: &Path,
    ledger: &ImportedExternalAgentSessionLedger,
) -> io::Result<()> {
    fs::create_dir_all(codex_home)?;
    let path = import_ledger_path(codex_home);
    let raw = serde_json::to_vec_pretty(ledger).map_err(io::Error::other)?;
    fs::write(path, raw)
}

fn import_ledger_path(codex_home: &Path) -> PathBuf {
    codex_home.join(SESSION_IMPORT_LEDGER_FILE)
}

fn canonical_source_path(path: &Path) -> io::Result<PathBuf> {
    fs::canonicalize(path)
}

fn session_content_sha256(path: &Path) -> io::Result<String> {
    let contents = fs::read(path)?;
    Ok(format!("{:x}", Sha256::digest(contents)))
}
