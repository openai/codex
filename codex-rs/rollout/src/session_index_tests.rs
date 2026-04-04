#![allow(warnings, clippy::all)]

use super::*;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::collections::HashSet;
use tempfile::TempDir;
fn write_index(path: &Path, lines: &[SessionIndexEntry]) -> std::io::Result<()> {
    let mut out = String::new();
    for entry in lines {
        out.push_str(&serde_json::to_string(entry).unwrap());
        out.push('\n');
    }
    std::fs::write(path, out)
}

#[test]
fn find_thread_id_by_name_prefers_latest_entry() -> std::io::Result<()> {
    let temp = TempDir::new()?;
    let path = session_index_path(temp.path());
    let id1 = ThreadId::new();
    let id2 = ThreadId::new();
    let lines = vec![
        SessionIndexEntry {
            id: id1,
            thread_name: "same".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        },
        SessionIndexEntry {
            id: id2,
            thread_name: "same".to_string(),
            updated_at: "2024-01-02T00:00:00Z".to_string(),
        },
    ];
    write_index(&path, &lines)?;

    let found = scan_index_from_end_by_name(&path, "same")?;
    assert_eq!(found.map(|entry| entry.id), Some(id2));
    Ok(())
}

#[tokio::test]
async fn find_thread_path_by_name_str_skips_newest_entry_without_rollout() -> std::io::Result<()> {
    // A newer unsaved name entry should not shadow an older persisted rollout with the same name.
    let temp = TempDir::new()?;
    let path = session_index_path(temp.path());
    let saved_id = ThreadId::new();
    let unsaved_id = ThreadId::new();
    let saved_rollout_path = temp
        .path()
        .join("sessions/2024/01/01")
        .join(format!("rollout-2024-01-01T00-00-00-{saved_id}.jsonl"));
    std::fs::create_dir_all(saved_rollout_path.parent().expect("rollout parent"))?;
    std::fs::write(&saved_rollout_path, "")?;
    let lines = vec![
        SessionIndexEntry {
            id: saved_id,
            thread_name: "same".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        },
        SessionIndexEntry {
            id: unsaved_id,
            thread_name: "same".to_string(),
            updated_at: "2024-01-02T00:00:00Z".to_string(),
        },
    ];
    write_index(&path, &lines)?;

    let found = find_thread_path_by_name_str(temp.path(), "same").await?;

    assert_eq!(found, Some(saved_rollout_path));
    Ok(())
}

#[tokio::test]
async fn find_thread_path_by_name_str_ignores_historical_name_after_rename() -> std::io::Result<()>
{
    let temp = TempDir::new()?;
    let path = session_index_path(temp.path());
    let renamed_id = ThreadId::new();
    let current_id = ThreadId::new();
    let current_rollout_path = temp
        .path()
        .join("sessions/2024/01/01")
        .join(format!("rollout-2024-01-01T00-00-00-{current_id}.jsonl"));
    std::fs::create_dir_all(current_rollout_path.parent().expect("rollout parent"))?;
    std::fs::write(&current_rollout_path, "")?;
    let lines = vec![
        SessionIndexEntry {
            id: renamed_id,
            thread_name: "same".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        },
        SessionIndexEntry {
            id: current_id,
            thread_name: "same".to_string(),
            updated_at: "2024-01-02T00:00:00Z".to_string(),
        },
        SessionIndexEntry {
            id: renamed_id,
            thread_name: "different".to_string(),
            updated_at: "2024-01-03T00:00:00Z".to_string(),
        },
    ];
    write_index(&path, &lines)?;

    let found = find_thread_path_by_name_str(temp.path(), "same").await?;

    assert_eq!(found, Some(current_rollout_path));
    Ok(())
}

#[test]
fn find_thread_name_by_id_prefers_latest_entry() -> std::io::Result<()> {
    let temp = TempDir::new()?;
    let path = session_index_path(temp.path());
    let id = ThreadId::new();
    let lines = vec![
        SessionIndexEntry {
            id,
            thread_name: "first".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        },
        SessionIndexEntry {
            id,
            thread_name: "second".to_string(),
            updated_at: "2024-01-02T00:00:00Z".to_string(),
        },
    ];
    write_index(&path, &lines)?;

    let found = scan_index_from_end_by_id(&path, &id)?;
    assert_eq!(
        found.map(|entry| entry.thread_name),
        Some("second".to_string())
    );
    Ok(())
}

#[test]
fn scan_index_returns_none_when_entry_missing() -> std::io::Result<()> {
    let temp = TempDir::new()?;
    let path = session_index_path(temp.path());
    let id = ThreadId::new();
    let lines = vec![SessionIndexEntry {
        id,
        thread_name: "present".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    }];
    write_index(&path, &lines)?;

    let missing_name = scan_index_from_end_by_name(&path, "missing")?;
    assert_eq!(missing_name, None);

    let missing_id = scan_index_from_end_by_id(&path, &ThreadId::new())?;
    assert_eq!(missing_id, None);
    Ok(())
}

#[tokio::test]
async fn find_thread_names_by_ids_prefers_latest_entry() -> std::io::Result<()> {
    let temp = TempDir::new()?;
    let path = session_index_path(temp.path());
    let id1 = ThreadId::new();
    let id2 = ThreadId::new();
    let lines = vec![
        SessionIndexEntry {
            id: id1,
            thread_name: "first".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        },
        SessionIndexEntry {
            id: id2,
            thread_name: "other".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        },
        SessionIndexEntry {
            id: id1,
            thread_name: "latest".to_string(),
            updated_at: "2024-01-02T00:00:00Z".to_string(),
        },
    ];
    write_index(&path, &lines)?;

    let mut ids = HashSet::new();
    ids.insert(id1);
    ids.insert(id2);

    let mut expected = HashMap::new();
    expected.insert(id1, "latest".to_string());
    expected.insert(id2, "other".to_string());

    let found = find_thread_names_by_ids(temp.path(), &ids).await?;
    assert_eq!(found, expected);
    Ok(())
}

#[test]
fn scan_index_finds_latest_match_among_mixed_entries() -> std::io::Result<()> {
    let temp = TempDir::new()?;
    let path = session_index_path(temp.path());
    let id_target = ThreadId::new();
    let id_other = ThreadId::new();
    let expected = SessionIndexEntry {
        id: id_target,
        thread_name: "target".to_string(),
        updated_at: "2024-01-03T00:00:00Z".to_string(),
    };
    let expected_other = SessionIndexEntry {
        id: id_other,
        thread_name: "target".to_string(),
        updated_at: "2024-01-02T00:00:00Z".to_string(),
    };
    // Resolution is based on append order (scan from end), not updated_at.
    let lines = vec![
        SessionIndexEntry {
            id: id_target,
            thread_name: "target".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        },
        expected_other.clone(),
        expected.clone(),
        SessionIndexEntry {
            id: ThreadId::new(),
            thread_name: "another".to_string(),
            updated_at: "2024-01-04T00:00:00Z".to_string(),
        },
    ];
    write_index(&path, &lines)?;

    let found_by_name = scan_index_from_end_by_name(&path, "target")?;
    assert_eq!(found_by_name, Some(expected.clone()));

    let found_by_id = scan_index_from_end_by_id(&path, &id_target)?;
    assert_eq!(found_by_id, Some(expected));

    let found_other_by_id = scan_index_from_end_by_id(&path, &id_other)?;
    assert_eq!(found_other_by_id, Some(expected_other));
    Ok(())
}
