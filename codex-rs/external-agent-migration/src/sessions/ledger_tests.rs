use super::CompletedExternalAgentSessionImport;
use super::ImportedConnectorCandidate;
use super::ImportedExternalAgentSessionLedger;
use super::checkpoint_existing_session_import;
use super::read_imported_connector_candidates;
use super::record_completed_session_imports;
use codex_protocol::ThreadId;
use pretty_assertions::assert_eq;
use sha2::Digest;
use sha2::Sha256;
use std::path::Path;
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
            title: Some("Imported session".to_string()),
        }],
    )
    .expect("record completed imports");

    let ledger = super::load_import_ledger(&codex_home).expect("ledger");
    assert_eq!(ledger.records.len(), 1);
    assert_eq!(ledger.records[0].source_path, source_path);
    assert_eq!(ledger.records[0].imported_thread_id, imported_thread_id);
    assert_eq!(ledger.records[0].source_modified_at, None);
    assert_eq!(ledger.records[0].title.as_deref(), Some("Imported session"));
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
            title: Some("First title".to_string()),
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
            title: Some("Second title".to_string()),
        }],
    )
    .expect("record replacement import");

    let ledger = super::load_import_ledger(&codex_home).expect("ledger");
    assert_eq!(ledger.records.len(), 1);
    assert_eq!(ledger.records[0].source_path, source_path);
    assert_eq!(ledger.records[0].imported_thread_id, second_thread_id);
    assert!(ledger.records[0].source_modified_at.is_some());
    assert_eq!(ledger.records[0].connector_names, vec!["Slack"]);
    assert_eq!(ledger.records[0].title.as_deref(), Some("Second title"));
}

#[test]
fn checkpoint_is_compare_and_swap_and_preserves_record_metadata() {
    let root = TempDir::new().expect("tempdir");
    let codex_home = root.path().join("codex-home");
    let source_path = root.path().join("session.jsonl");
    std::fs::write(&source_path, "old").expect("initial source");
    let source_path = std::fs::canonicalize(source_path).expect("canonical source");
    let thread_id = ThreadId::new();
    let old_hash = format!("{:x}", Sha256::digest("old"));
    let new_hash = format!("{:x}", Sha256::digest("new"));
    record_completed_session_imports(
        &codex_home,
        vec![CompletedExternalAgentSessionImport {
            source_path: source_path.clone(),
            source_content_sha256: old_hash.clone(),
            imported_thread_id: thread_id,
            connector_names: vec!["Gmail".to_string()],
            title: Some("Original title".to_string()),
        }],
    )
    .expect("record initial import");
    std::fs::write(&source_path, "new").expect("updated source");
    let ledger_before = ledger_bytes(&codex_home);
    for (candidate_thread_id, expected_hash, candidate_hash) in [
        (ThreadId::new(), old_hash.as_str(), new_hash.as_str()),
        (thread_id, "stale", new_hash.as_str()),
        (thread_id, old_hash.as_str(), "not-the-source-hash"),
    ] {
        assert!(
            !checkpoint_existing_session_import(
                &codex_home,
                &source_path,
                candidate_thread_id,
                expected_hash,
                candidate_hash,
            )
            .expect("rejected checkpoint")
        );
    }
    assert_eq!(ledger_bytes(&codex_home), ledger_before);

    assert!(
        checkpoint_existing_session_import(
            &codex_home,
            &source_path,
            thread_id,
            &old_hash,
            &new_hash,
        )
        .expect("checkpoint")
    );
    let ledger = super::load_import_ledger(&codex_home).expect("updated ledger");
    let [record] = ledger.records.as_slice() else {
        panic!("checkpoint must preserve one record");
    };
    assert_eq!(
        record,
        &super::ImportedExternalAgentSessionRecord {
            source_path: source_path.clone(),
            content_sha256: new_hash,
            imported_thread_id: thread_id,
            imported_at: record.imported_at,
            source_modified_at: super::session_modified_at(&source_path).expect("source mtime"),
            connector_names: vec!["Gmail".to_string()],
            title: Some("Original title".to_string()),
        }
    );
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
                title: None,
            },
            CompletedExternalAgentSessionImport {
                source_path: first_source,
                source_content_sha256: "second-version".to_string(),
                imported_thread_id: ThreadId::new(),
                connector_names: vec!["Slack".to_string()],
                title: None,
            },
            CompletedExternalAgentSessionImport {
                source_path: second_source,
                source_content_sha256: "only-version".to_string(),
                imported_thread_id: ThreadId::new(),
                connector_names: vec!["Gmail".to_string(), "Slack".to_string()],
                title: None,
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

fn ledger_bytes(codex_home: &Path) -> Vec<u8> {
    std::fs::read(super::import_ledger_path(codex_home)).expect("ledger")
}
