use super::now_unix_seconds;
use codex_protocol::ThreadId;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;

const SESSION_IMPORT_LEDGER_FILE: &str = "external_agent_session_imports.json";
const SESSION_HASH_BUFFER_SIZE: usize = 64 * 1024;

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
    #[serde(default)]
    source_modified_at: Option<i64>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CompletedExternalAgentSessionImport {
    pub source_path: PathBuf,
    pub source_content_sha256: String,
    pub imported_thread_id: ThreadId,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ImportedSourceState {
    pub source_modified_at: Option<i64>,
    pub imported_at: i64,
}

pub fn has_current_session_been_imported(
    codex_home: &Path,
    source_path: &Path,
) -> io::Result<bool> {
    load_import_ledger(codex_home)?.contains_current_source(source_path)
}

#[cfg(test)]
pub(crate) fn record_imported_session(
    codex_home: &Path,
    source_path: &Path,
    imported_thread_id: ThreadId,
) -> io::Result<()> {
    let source_path = canonical_source_path(source_path)?;
    record_completed_session_imports(
        codex_home,
        vec![CompletedExternalAgentSessionImport {
            source_content_sha256: session_content_sha256(&source_path)?,
            source_path,
            imported_thread_id,
        }],
    )
}

pub fn record_completed_session_imports(
    codex_home: &Path,
    imports: Vec<CompletedExternalAgentSessionImport>,
) -> io::Result<()> {
    if imports.is_empty() {
        return Ok(());
    }
    let mut ledger = load_import_ledger(codex_home)?;
    let imported_at = now_unix_seconds();
    for import in imports {
        let source_modified_at = session_modified_at(&import.source_path).ok().flatten();
        if let Some(index) = ledger.records.iter().rposition(|record| {
            record.source_path == import.source_path
                && record.content_sha256 == import.source_content_sha256
        }) {
            let mut record = ledger.records.remove(index);
            record.imported_thread_id = import.imported_thread_id;
            record.imported_at = imported_at;
            record.source_modified_at = source_modified_at.or(record.source_modified_at);
            ledger.records.push(record);
            continue;
        }
        ledger.records.push(ImportedExternalAgentSessionRecord {
            source_path: import.source_path,
            content_sha256: import.source_content_sha256,
            imported_thread_id: import.imported_thread_id,
            imported_at,
            source_modified_at,
        });
    }
    save_import_ledger(codex_home, &ledger)
}

impl ImportedExternalAgentSessionLedger {
    pub(super) fn source_states(&self) -> HashMap<&Path, ImportedSourceState> {
        let mut states = HashMap::new();
        for record in &self.records {
            states.insert(
                record.source_path.as_path(),
                ImportedSourceState {
                    source_modified_at: record.source_modified_at,
                    imported_at: record.imported_at,
                },
            );
        }
        states
    }

    pub(super) fn contains_current_source(&self, source_path: &Path) -> io::Result<bool> {
        if self.records.is_empty() {
            return Ok(false);
        }
        let source_path = canonical_source_path(source_path)?;
        if !self
            .records
            .iter()
            .any(|record| record.source_path == source_path)
        {
            return Ok(false);
        }
        let content_sha256 = session_content_sha256(&source_path)?;
        Ok(self.records.iter().any(|record| {
            record.source_path == source_path && record.content_sha256 == content_sha256
        }))
    }

    pub(super) fn refresh_current_source(
        &mut self,
        source_path: &Path,
        source_modified_at: i64,
    ) -> io::Result<bool> {
        let source_path = canonical_source_path(source_path)?;
        if !self
            .records
            .iter()
            .any(|record| record.source_path == source_path)
        {
            return Ok(false);
        }
        let content_sha256 = session_content_sha256(&source_path)?;
        let Some(index) = self.records.iter().rposition(|record| {
            record.source_path == source_path && record.content_sha256 == content_sha256
        }) else {
            return Ok(false);
        };
        let mut record = self.records.remove(index);
        record.imported_at = now_unix_seconds();
        record.source_modified_at = Some(source_modified_at);
        self.records.push(record);
        Ok(true)
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

pub(super) fn save_import_ledger(
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
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; SESSION_HASH_BUFFER_SIZE];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();
    Ok(format!("{digest:x}"))
}

fn session_modified_at(path: &Path) -> io::Result<Option<i64>> {
    Ok(fs::metadata(path)?
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_nanos()).ok()))
}

#[cfg(test)]
#[path = "ledger_tests.rs"]
mod tests;
