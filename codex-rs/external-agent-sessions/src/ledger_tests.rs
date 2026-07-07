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
            .contains_source_identity(&missing_source, /*source_session_id*/ None)
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
            source_session_id: None,
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
            source_session_id: None,
            source_content_sha256: content_sha256.clone(),
            imported_thread_id: first_thread_id,
        }],
    )
    .expect("record first import");
    record_completed_session_imports(
        &codex_home,
        vec![CompletedExternalAgentSessionImport {
            source_path: source_path.clone(),
            source_session_id: None,
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

#[test]
fn stable_session_id_deduplicates_moved_and_changed_sources() {
    let root = TempDir::new().expect("tempdir");
    let codex_home = root.path().join("codex-home");
    let first_path = root.path().join("first-session.jsonl");
    let second_path = root.path().join("moved-session.jsonl");
    std::fs::write(&first_path, "first contents").expect("first source");
    std::fs::write(&second_path, "updated contents").expect("moved source");
    let first_path = std::fs::canonicalize(first_path).expect("canonical first source");
    let second_path = std::fs::canonicalize(second_path).expect("canonical moved source");
    let source_session_id = "source-session-id";
    let first_thread_id = ThreadId::new();
    let second_thread_id = ThreadId::new();

    record_completed_session_imports(
        &codex_home,
        vec![CompletedExternalAgentSessionImport {
            source_path: first_path,
            source_session_id: Some(source_session_id.to_string()),
            source_content_sha256: format!("{:x}", Sha256::digest(b"first contents")),
            imported_thread_id: first_thread_id,
        }],
    )
    .expect("record first import");

    let ledger = super::load_import_ledger(&codex_home).expect("ledger");
    assert!(
        ledger
            .contains_source_identity(&second_path, Some(source_session_id))
            .expect("match moved source")
    );

    record_completed_session_imports(
        &codex_home,
        vec![CompletedExternalAgentSessionImport {
            source_path: second_path.clone(),
            source_session_id: Some(source_session_id.to_string()),
            source_content_sha256: format!("{:x}", Sha256::digest(b"updated contents")),
            imported_thread_id: second_thread_id,
        }],
    )
    .expect("record moved import");

    let ledger = super::load_import_ledger(&codex_home).expect("updated ledger");
    assert_eq!(ledger.records.len(), 1);
    assert_eq!(ledger.records[0].source_path, second_path);
    assert_eq!(
        ledger.records[0].source_session_id.as_deref(),
        Some(source_session_id)
    );
    assert_eq!(ledger.records[0].imported_thread_id, second_thread_id);
}

#[test]
fn legacy_ledger_uses_source_filename_as_session_id() {
    let root = TempDir::new().expect("tempdir");
    let codex_home = root.path().join("codex-home");
    let source_session_id = "source-session-id";
    let first_path = root.path().join(format!("{source_session_id}.jsonl"));
    let moved_path = root.path().join("moved-session.jsonl");
    std::fs::write(&first_path, "first contents").expect("first source");
    std::fs::write(&moved_path, "first contents").expect("moved source");
    super::record_imported_session(&codex_home, &first_path, ThreadId::new())
        .expect("record legacy import");
    let moved_path = std::fs::canonicalize(moved_path).expect("canonical moved source");

    let ledger = super::load_import_ledger(&codex_home).expect("ledger");
    assert!(
        ledger
            .contains_source_identity(&moved_path, Some(source_session_id))
            .expect("match legacy source")
    );
}
