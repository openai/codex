use super::CompletedExternalAgentSessionImport;
use super::ImportedExternalAgentSessionLedger;
use super::record_completed_session_imports;
use codex_protocol::ThreadId;
use sha2::Digest;
use sha2::Sha256;
use tempfile::TempDir;

#[test]
fn empty_ledger_does_not_read_source() {
    let root = TempDir::new().expect("tempdir");
    let missing_source = root.path().join("missing-session.jsonl");

    assert!(
        !ImportedExternalAgentSessionLedger::default()
            .contains_current_source(&missing_source)
            .expect("empty ledger cannot contain sources")
    );
}

#[test]
fn completed_imports_do_not_read_source_files() {
    let root = TempDir::new().expect("tempdir");
    let codex_home = root.path().join("codex-home");
    let source_path = root.path().join("session.jsonl");
    let contents = b"session contents";
    std::fs::write(&source_path, contents).expect("source");
    let source_path = std::fs::canonicalize(&source_path).expect("canonical source");
    std::fs::remove_file(&source_path).expect("remove source");
    let imported_thread_id = ThreadId::new();

    record_completed_session_imports(
        &codex_home,
        vec![CompletedExternalAgentSessionImport {
            source_path: source_path.clone(),
            source_content_sha256: format!("{:x}", Sha256::digest(contents)),
            imported_thread_id,
        }],
    )
    .expect("record completed imports");

    let ledger = super::load_import_ledger(&codex_home).expect("ledger");
    assert_eq!(ledger.records.len(), 1);
    assert_eq!(ledger.records[0].source_path, source_path);
    assert_eq!(ledger.records[0].imported_thread_id, imported_thread_id);
    assert_eq!(ledger.records[0].source_modified_at, None);
}

#[test]
fn completed_import_refreshes_existing_record_metadata() {
    let root = TempDir::new().expect("tempdir");
    let codex_home = root.path().join("codex-home");
    let source_path = root.path().join("session.jsonl");
    let contents = b"session contents";
    std::fs::write(&source_path, contents).expect("source");
    let source_path = std::fs::canonicalize(source_path).expect("canonical source");
    let content_sha256 = format!("{:x}", Sha256::digest(contents));
    let first_thread_id = ThreadId::new();
    let second_thread_id = ThreadId::new();

    record_completed_session_imports(
        &codex_home,
        vec![CompletedExternalAgentSessionImport {
            source_path: source_path.clone(),
            source_content_sha256: content_sha256.clone(),
            imported_thread_id: first_thread_id,
        }],
    )
    .expect("record first import");
    record_completed_session_imports(
        &codex_home,
        vec![CompletedExternalAgentSessionImport {
            source_path: source_path.clone(),
            source_content_sha256: content_sha256,
            imported_thread_id: second_thread_id,
        }],
    )
    .expect("record replacement import");

    let ledger = super::load_import_ledger(&codex_home).expect("ledger");
    assert_eq!(ledger.records.len(), 1);
    assert_eq!(ledger.records[0].source_path, source_path);
    assert_eq!(ledger.records[0].imported_thread_id, second_thread_id);
    assert!(ledger.records[0].source_modified_at.is_some());
}
