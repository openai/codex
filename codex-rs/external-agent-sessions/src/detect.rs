use crate::ExternalAgentSessionMigration;
use crate::ledger::load_import_ledger;
use crate::ledger::save_import_ledger;
use crate::now_unix_seconds;
use crate::summarize_session;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::fs;
use std::io;
use std::path::Path;
use std::time::Duration;

const SESSION_IMPORT_MAX_COUNT: usize = 50;
const SESSION_IMPORT_MAX_AGE: Duration = Duration::from_secs(30 * 24 * 60 * 60);

pub fn detect_recent_sessions(
    external_agent_home: &Path,
    codex_home: &Path,
) -> io::Result<Vec<ExternalAgentSessionMigration>> {
    let projects_root = external_agent_home.join("projects");
    if !projects_root.is_dir() {
        return Ok(Vec::new());
    }

    let now = now_unix_seconds();
    let mut ledger = load_import_ledger(codex_home)?;
    let source_states = ledger.source_states();
    let mut file_candidates = BinaryHeap::with_capacity(SESSION_IMPORT_MAX_COUNT + 1);
    for project_entry in fs::read_dir(projects_root)? {
        let Ok(project_entry) = project_entry else {
            continue;
        };
        let project_path = project_entry.path();
        if !project_path.is_dir() {
            continue;
        }
        let Ok(entries) = fs::read_dir(project_path) else {
            continue;
        };
        for entry in entries {
            let Ok(entry) = entry else {
                continue;
            };
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
                continue;
            }
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            let Ok(modified_at) = metadata.modified() else {
                continue;
            };
            let Ok(modified_at) = modified_at.duration_since(std::time::UNIX_EPOCH) else {
                continue;
            };
            if (modified_at.as_secs() as i64)
                < now.saturating_sub(SESSION_IMPORT_MAX_AGE.as_secs() as i64)
            {
                continue;
            }
            let Ok(modified_at_nanos) = i64::try_from(modified_at.as_nanos()) else {
                continue;
            };
            let Ok(source_path) = fs::canonicalize(&path) else {
                continue;
            };
            if let Some(state) = source_states.get(source_path.as_path())
                && (state.source_modified_at == Some(modified_at_nanos)
                    || state.source_modified_at.is_none()
                        && modified_at.as_secs() as i64 <= state.imported_at)
            {
                continue;
            }
            file_candidates.push((Reverse(modified_at_nanos), path));
            if file_candidates.len() > SESSION_IMPORT_MAX_COUNT {
                file_candidates.pop();
            }
        }
    }

    drop(source_states);
    let file_candidates = file_candidates.into_sorted_vec();
    let mut migrations = Vec::new();
    let mut ledger_changed = false;
    for (modified_at, path) in file_candidates {
        match ledger.refresh_current_source(&path, modified_at.0) {
            Ok(false) => {}
            Ok(true) => {
                ledger_changed = true;
                continue;
            }
            Err(_) => continue,
        }
        let Ok(Some(summary)) = summarize_session(&path) else {
            continue;
        };
        let migration = summary.migration;
        if !migration.cwd.is_dir() {
            continue;
        }
        migrations.push(migration);
    }
    if ledger_changed {
        save_import_ledger(codex_home, &ledger)?;
    }

    Ok(migrations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::record_imported_session;
    use codex_protocol::ThreadId;
    use serde_json::Value as JsonValue;
    use std::fs::FileTimes;
    use std::fs::OpenOptions;
    use std::path::Path;
    use std::time::SystemTime;
    use tempfile::TempDir;

    #[test]
    fn detects_recent_sessions_with_existing_roots() {
        let root = TempDir::new().expect("tempdir");
        let external_agent_home = root.path().join(".external");
        let project_root = root.path().join("repo");
        let session_path = write_session(
            &external_agent_home,
            &project_root,
            "session.jsonl",
            &[
                record("user", "hello there", project_root.as_path()),
                record("assistant", "ack", project_root.as_path()),
            ],
        );

        let sessions = detect_recent_sessions(&external_agent_home, root.path()).expect("detect");

        assert_eq!(
            sessions,
            vec![ExternalAgentSessionMigration {
                path: session_path,
                cwd: project_root,
                title: Some("hello there".to_string()),
            }]
        );
    }

    #[test]
    fn prefers_latest_custom_title_over_first_user_message() {
        let root = TempDir::new().expect("tempdir");
        let external_agent_home = root.path().join(".external");
        let project_root = root.path().join("repo");
        let session_path = write_session(
            &external_agent_home,
            &project_root,
            "session.jsonl",
            &[
                record("user", "hello there", project_root.as_path()),
                custom_title_record("first title"),
                custom_title_record("final title"),
            ],
        );

        let sessions = detect_recent_sessions(&external_agent_home, root.path()).expect("detect");

        assert_eq!(
            sessions,
            vec![ExternalAgentSessionMigration {
                path: session_path,
                cwd: project_root,
                title: Some("final title".to_string()),
            }]
        );
    }

    #[test]
    fn detects_ai_title_over_first_user_message() {
        let root = TempDir::new().expect("tempdir");
        let external_agent_home = root.path().join(".external");
        let project_root = root.path().join("repo");
        let session_path = write_session(
            &external_agent_home,
            &project_root,
            "session.jsonl",
            &[
                record("user", "hello there", project_root.as_path()),
                ai_title_record("generated by source app"),
            ],
        );

        let sessions = detect_recent_sessions(&external_agent_home, root.path()).expect("detect");

        assert_eq!(
            sessions,
            vec![ExternalAgentSessionMigration {
                path: session_path,
                cwd: project_root,
                title: Some("generated by source app".to_string()),
            }]
        );
    }

    #[test]
    fn prefers_custom_title_over_later_ai_title() {
        let root = TempDir::new().expect("tempdir");
        let external_agent_home = root.path().join(".external");
        let project_root = root.path().join("repo");
        let session_path = write_session(
            &external_agent_home,
            &project_root,
            "session.jsonl",
            &[
                record("user", "hello there", project_root.as_path()),
                custom_title_record("custom title"),
                ai_title_record("generated title"),
            ],
        );

        let sessions = detect_recent_sessions(&external_agent_home, root.path()).expect("detect");

        assert_eq!(
            sessions,
            vec![ExternalAgentSessionMigration {
                path: session_path,
                cwd: project_root,
                title: Some("custom title".to_string()),
            }]
        );
    }

    #[test]
    fn uses_file_modification_time_for_recency() {
        let root = TempDir::new().expect("tempdir");
        let external_agent_home = root.path().join(".external");
        let project_root = root.path().join("repo");
        let session_path = write_session(
            &external_agent_home,
            &project_root,
            "session.jsonl",
            &[record_at(
                "user",
                "hello",
                &project_root,
                "2020-01-01T00:00:00Z",
            )],
        );

        let sessions = detect_recent_sessions(&external_agent_home, root.path()).expect("detect");

        assert_eq!(
            sessions,
            vec![ExternalAgentSessionMigration {
                path: session_path,
                cwd: project_root,
                title: Some("hello".to_string()),
            }]
        );
    }

    #[test]
    fn ignores_sessions_with_old_file_modification_time() {
        let root = TempDir::new().expect("tempdir");
        let external_agent_home = root.path().join(".external");
        let project_root = root.path().join("repo");
        let session_path = write_session(
            &external_agent_home,
            &project_root,
            "session.jsonl",
            &[record("user", "hello", &project_root)],
        );
        set_modified_at(
            &session_path,
            SystemTime::UNIX_EPOCH + Duration::from_secs(/*secs*/ 1),
        );

        assert!(
            detect_recent_sessions(&external_agent_home, root.path())
                .expect("detect")
                .is_empty()
        );
    }

    #[test]
    fn detects_sessions_in_batches() {
        let root = TempDir::new().expect("tempdir");
        let external_agent_home = root.path().join(".external");
        let project_root = root.path().join("repo");
        let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let modified_at = SystemTime::now();
        let mut expected = Vec::new();
        for index in 0..=SESSION_IMPORT_MAX_COUNT {
            let file_name = format!("{index:02}-session.jsonl");
            let title = format!("session {index}");
            let path = write_session(
                &external_agent_home,
                &project_root,
                &file_name,
                &[record_at("user", &title, &project_root, &timestamp)],
            );
            set_modified_at(
                &path,
                modified_at - Duration::from_secs(/*secs*/ index as u64),
            );
            expected.push(ExternalAgentSessionMigration {
                path,
                cwd: project_root.clone(),
                title: Some(title),
            });
        }
        let oldest_session = expected.pop().expect("oldest session");
        let mut all_sessions = expected.clone();
        all_sessions.push(oldest_session.clone());

        let sessions = detect_recent_sessions(&external_agent_home, root.path()).expect("detect");

        assert_eq!(sessions, expected);
        for session in sessions {
            record_imported_session(root.path(), &session.path, ThreadId::new())
                .expect("record import");
        }

        let sessions = detect_recent_sessions(&external_agent_home, root.path()).expect("detect");

        assert_eq!(sessions, vec![oldest_session.clone()]);
        for session in sessions {
            record_imported_session(root.path(), &session.path, ThreadId::new())
                .expect("record import");
        }

        let changed_at = SystemTime::now()
            + Duration::from_secs(/*secs*/ SESSION_IMPORT_MAX_COUNT as u64 + 1);
        for (index, session) in all_sessions.iter().enumerate() {
            let title = session.title.as_deref().expect("session title");
            std::fs::write(
                &session.path,
                jsonl(&[
                    record("user", title, &project_root),
                    record("assistant", "updated", &project_root),
                ]),
            )
            .expect("update session");
            set_modified_at(
                &session.path,
                changed_at - Duration::from_secs(/*secs*/ index as u64),
            );
        }

        let sessions = detect_recent_sessions(&external_agent_home, root.path()).expect("detect");

        assert_eq!(sessions, expected);
        for session in sessions {
            record_imported_session(root.path(), &session.path, ThreadId::new())
                .expect("record import");
        }

        let sessions = detect_recent_sessions(&external_agent_home, root.path()).expect("detect");

        assert_eq!(sessions, vec![oldest_session]);
    }

    #[test]
    fn skips_already_imported_current_session_versions() {
        let root = TempDir::new().expect("tempdir");
        let external_agent_home = root.path().join(".external");
        let project_root = root.path().join("repo");
        let session_path = write_session(
            &external_agent_home,
            &project_root,
            "session.jsonl",
            &[record("user", "hello there", project_root.as_path())],
        );

        record_imported_session(root.path(), &session_path, ThreadId::new())
            .expect("record import");

        assert!(
            detect_recent_sessions(&external_agent_home, root.path())
                .expect("detect")
                .is_empty()
        );
    }

    #[test]
    fn redetects_sessions_when_source_contents_change_after_import() {
        let root = TempDir::new().expect("tempdir");
        let external_agent_home = root.path().join(".external");
        let project_root = root.path().join("repo");
        let session_path = write_session(
            &external_agent_home,
            &project_root,
            "session.jsonl",
            &[record("user", "hello there", project_root.as_path())],
        );
        record_imported_session(root.path(), &session_path, ThreadId::new())
            .expect("record import");

        std::fs::write(
            &session_path,
            jsonl(&[
                record("user", "hello there", project_root.as_path()),
                record("assistant", "new reply", project_root.as_path()),
            ]),
        )
        .expect("update session");

        let sessions = detect_recent_sessions(&external_agent_home, root.path()).expect("detect");
        assert_eq!(
            sessions,
            vec![ExternalAgentSessionMigration {
                path: session_path,
                cwd: project_root,
                title: Some("hello there".to_string()),
            }]
        );
    }

    fn write_session(
        external_agent_home: &Path,
        project_root: &Path,
        file_name: &str,
        records: &[JsonValue],
    ) -> std::path::PathBuf {
        let projects_dir = external_agent_home.join("projects").join("repo");
        std::fs::create_dir_all(project_root).expect("project root");
        std::fs::create_dir_all(&projects_dir).expect("projects dir");
        let session_path = projects_dir.join(file_name);
        std::fs::write(&session_path, jsonl(records)).expect("session");
        session_path
    }

    fn set_modified_at(path: &Path, modified_at: SystemTime) {
        OpenOptions::new()
            .write(true)
            .open(path)
            .expect("open session")
            .set_times(FileTimes::new().set_modified(modified_at))
            .expect("set session modified time");
    }

    fn record(role: &str, text: &str, cwd: &Path) -> JsonValue {
        let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        record_at(role, text, cwd, &timestamp)
    }

    fn record_at(role: &str, text: &str, cwd: &Path, timestamp: &str) -> JsonValue {
        serde_json::json!({
            "type": role,
            "cwd": cwd,
            "timestamp": timestamp,
            "message": { "content": text }
        })
    }

    fn custom_title_record(title: &str) -> JsonValue {
        serde_json::json!({
            "type": "custom-title",
            "customTitle": title,
        })
    }

    fn ai_title_record(title: &str) -> JsonValue {
        serde_json::json!({
            "type": "ai-title",
            "aiTitle": title,
        })
    }

    fn jsonl(records: &[JsonValue]) -> String {
        records
            .iter()
            .map(JsonValue::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    }
}
