use super::now_unix_seconds;
use codex_protocol::ThreadId;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;
use std::collections::BTreeMap;
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
pub(crate) struct ImportedExternalAgentSessionLedger {
    records: Vec<ImportedExternalAgentSessionRecord>,
    #[serde(default)]
    detected_connector_records: Vec<DetectedExternalAgentSessionConnectorRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct DetectedExternalAgentSessionConnectorRecord {
    source_path: PathBuf,
    connector_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ImportedExternalAgentSessionRecord {
    source_path: PathBuf,
    content_sha256: String,
    imported_thread_id: ThreadId,
    imported_at: i64,
    #[serde(default)]
    source_modified_at: Option<i64>,
    #[serde(default)]
    connector_names: Vec<String>,
    #[serde(default)]
    title: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CompletedExternalAgentSessionImport {
    pub source_path: PathBuf,
    pub source_content_sha256: String,
    pub imported_thread_id: ThreadId,
    pub connector_names: Vec<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SessionImportSourceMapping {
    None,
    Unique {
        source_content_sha256: String,
        imported_thread_id: ThreadId,
    },
    Ambiguous,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedConnectorCandidate {
    pub name: String,
    pub session_count: u32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ImportedSourceState {
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
            connector_names: Vec::new(),
            title: None,
        }],
    )
}

pub(crate) fn find_existing_session_import(
    codex_home: &Path,
    source_path: &Path,
) -> io::Result<SessionImportSourceMapping> {
    let source_path = canonical_source_path(source_path)?;
    let ledger = load_import_ledger(codex_home)?;
    let mut matching_records = ledger
        .records
        .iter()
        .filter(|record| record.source_path == source_path);
    let Some(record) = matching_records.next() else {
        return Ok(SessionImportSourceMapping::None);
    };
    if matching_records.next().is_some() {
        return Ok(SessionImportSourceMapping::Ambiguous);
    }
    Ok(SessionImportSourceMapping::Unique {
        source_content_sha256: record.content_sha256.clone(),
        imported_thread_id: record.imported_thread_id,
    })
}

pub(super) fn checkpoint_existing_session_import(
    codex_home: &Path,
    source_path: &Path,
    thread_id: ThreadId,
    expected_source_content_sha256: &str,
    source_content_sha256: &str,
) -> io::Result<bool> {
    if expected_source_content_sha256 == source_content_sha256 {
        return Ok(false);
    }
    let source_path = canonical_source_path(source_path)?;
    let source_modified_at = session_modified_at(&source_path)?;
    if session_content_sha256(&source_path)? != source_content_sha256
        || source_modified_at != session_modified_at(&source_path)?
    {
        return Ok(false);
    }

    let mut ledger = load_import_ledger(codex_home)?;
    let mut matching_indices = ledger
        .records
        .iter()
        .enumerate()
        .filter_map(|(index, record)| (record.source_path == source_path).then_some(index));
    let Some(index) = matching_indices.next() else {
        return Ok(false);
    };
    if matching_indices.next().is_some() {
        return Ok(false);
    }
    let record = &mut ledger.records[index];
    if record.imported_thread_id != thread_id
        || record.content_sha256 != expected_source_content_sha256
    {
        return Ok(false);
    }
    record.content_sha256 = source_content_sha256.to_string();
    record.imported_at = now_unix_seconds();
    record.source_modified_at = source_modified_at;
    save_import_ledger(codex_home, &ledger)?;
    Ok(true)
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
            append_connector_names(&mut record.connector_names, import.connector_names);
            record.title = import.title;
            ledger.records.push(record);
            continue;
        }
        let mut connector_names = Vec::new();
        append_connector_names(&mut connector_names, import.connector_names);
        ledger.records.push(ImportedExternalAgentSessionRecord {
            source_path: import.source_path,
            content_sha256: import.source_content_sha256,
            imported_thread_id: import.imported_thread_id,
            imported_at,
            source_modified_at,
            connector_names,
            title: import.title,
        });
    }
    save_import_ledger(codex_home, &ledger)
}

pub fn record_detected_session_connectors(
    codex_home: &Path,
    connector_names_by_source_path: BTreeMap<PathBuf, Vec<String>>,
) -> io::Result<()> {
    if connector_names_by_source_path.is_empty() {
        return Ok(());
    }
    let mut ledger = load_import_ledger(codex_home)?;
    for (source_path, connector_names) in connector_names_by_source_path {
        let source_path = canonical_source_path(&source_path)?;
        if let Some(record) = ledger
            .detected_connector_records
            .iter_mut()
            .find(|record| record.source_path == source_path)
        {
            append_connector_names(&mut record.connector_names, connector_names);
            continue;
        }
        let mut detected_connector_names = Vec::new();
        append_connector_names(&mut detected_connector_names, connector_names);
        ledger
            .detected_connector_records
            .push(DetectedExternalAgentSessionConnectorRecord {
                source_path,
                connector_names: detected_connector_names,
            });
    }
    save_import_ledger(codex_home, &ledger)
}

pub fn append_imported_session_connector_names(
    codex_home: &Path,
    connector_names_by_source_path: BTreeMap<PathBuf, Vec<String>>,
) -> io::Result<()> {
    if connector_names_by_source_path.is_empty() {
        return Ok(());
    }
    let mut ledger = load_import_ledger(codex_home)?;
    for (source_path, connector_names) in connector_names_by_source_path {
        let source_path = canonical_source_path(&source_path)?;
        for record in ledger
            .records
            .iter_mut()
            .filter(|record| record.source_path == source_path)
        {
            append_connector_names(&mut record.connector_names, connector_names.clone());
        }
    }
    save_import_ledger(codex_home, &ledger)
}

pub fn read_imported_connector_candidates(
    codex_home: &Path,
) -> io::Result<Vec<ImportedConnectorCandidate>> {
    let ledger = load_import_ledger(codex_home)?;
    let mut connector_names_by_source = BTreeMap::<PathBuf, Vec<String>>::new();
    for record in ledger.detected_connector_records {
        connector_names_by_source
            .entry(record.source_path)
            .or_default()
            .extend(record.connector_names);
    }
    for record in ledger.records {
        connector_names_by_source
            .entry(record.source_path)
            .or_default()
            .extend(record.connector_names);
    }
    let mut candidates_by_name = BTreeMap::<String, ImportedConnectorCandidate>::new();
    for connector_names in connector_names_by_source.into_values() {
        let mut connector_names_by_key = BTreeMap::new();
        for name in connector_names
            .into_iter()
            .filter_map(|name| super::normalized_connector_display_name(Some(&name)))
        {
            connector_names_by_key
                .entry(name.to_lowercase())
                .or_insert(name);
        }
        for (key, name) in connector_names_by_key {
            let candidate = candidates_by_name
                .entry(key)
                .or_insert(ImportedConnectorCandidate {
                    name,
                    session_count: 0,
                });
            candidate.session_count = candidate.session_count.saturating_add(1);
        }
    }
    let mut candidates = candidates_by_name.into_values().collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(candidates)
}

fn append_connector_names(connector_names: &mut Vec<String>, additional_names: Vec<String>) {
    for name in additional_names {
        if connector_names
            .iter()
            .any(|existing_name| existing_name.eq_ignore_ascii_case(&name))
        {
            continue;
        }
        connector_names.push(name);
    }
}

impl ImportedExternalAgentSessionLedger {
    pub(crate) fn source_states(&self) -> HashMap<&Path, ImportedSourceState> {
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

    pub(crate) fn contains_current_source(&self, source_path: &Path) -> io::Result<bool> {
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

    pub(crate) fn refresh_current_source(
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

pub(crate) fn load_import_ledger(
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

pub(crate) fn save_import_ledger(
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
