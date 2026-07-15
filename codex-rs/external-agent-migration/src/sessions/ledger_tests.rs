use super::CompletedExternalAgentSessionImport;
use super::ImportedConnectorCandidate;
use super::ImportedExternalAgentSessionLedger;
use super::read_imported_connector_candidates;
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
            connector_names: Vec::new(),
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
            connector_names: vec!["Gmail".to_string()],
        }],
    )
    .expect("record first import");
    record_completed_session_imports(
        &codex_home,
        vec![CompletedExternalAgentSessionImport {
            source_path: source_path.clone(),
            source_content_sha256: content_sha256,
            imported_thread_id: second_thread_id,
            connector_names: vec!["Slack".to_string()],
        }],
    )
    .expect("record replacement import");

    let ledger = super::load_import_ledger(&codex_home).expect("ledger");
    assert_eq!(ledger.records.len(), 1);
    assert_eq!(ledger.records[0].source_path, source_path);
    assert_eq!(ledger.records[0].imported_thread_id, second_thread_id);
    assert!(ledger.records[0].source_modified_at.is_some());
    assert_eq!(ledger.records[0].connector_names, vec!["Slack"]);
}

#[test]
fn connector_candidates_use_latest_import_for_each_source() {
    let root = TempDir::new().expect("tempdir");
    let codex_home = root.path().join("codex-home");
    let first_source = root.path().join("first.jsonl");
    let second_source = root.path().join("second.jsonl");

    record_completed_session_imports(
        &codex_home,
        vec![
            CompletedExternalAgentSessionImport {
                source_path: first_source.clone(),
                source_content_sha256: "first-version".to_string(),
                imported_thread_id: ThreadId::new(),
                connector_names: vec!["Gmail".to_string()],
            },
            CompletedExternalAgentSessionImport {
                source_path: first_source,
                source_content_sha256: "second-version".to_string(),
                imported_thread_id: ThreadId::new(),
                connector_names: vec!["Slack".to_string()],
            },
            CompletedExternalAgentSessionImport {
                source_path: second_source,
                source_content_sha256: "only-version".to_string(),
                imported_thread_id: ThreadId::new(),
                connector_names: vec!["Gmail".to_string(), "Slack".to_string()],
            },
        ],
    )
    .expect("record imports");

    assert_eq!(
        read_imported_connector_candidates(&codex_home).expect("read connector candidates"),
        vec![
            ImportedConnectorCandidate {
                name: "Gmail".to_string(),
                session_count: 1,
            },
            ImportedConnectorCandidate {
                name: "Slack".to_string(),
                session_count: 2,
            },
        ]
    );
}
