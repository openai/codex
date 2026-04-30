use crate::ExternalAgentSessionMigration;
use crate::ledger::load_import_ledger;
use crate::now_unix_seconds;
use crate::summarize_session;
use std::fs;
use std::io;
use std::path::Path;
use std::time::Duration;

const SESSION_IMPORT_MAX_COUNT: usize = 50;
const SESSION_IMPORT_MAX_AGE: Duration = Duration::from_secs(30 * 24 * 60 * 60);

#[derive(Debug)]
struct SessionCandidate {
    latest_timestamp: i64,
    migration: ExternalAgentSessionMigration,
}

pub fn detect_recent_sessions(
    external_agent_home: &Path,
    codex_home: &Path,
) -> io::Result<Vec<ExternalAgentSessionMigration>> {
    let projects_root = external_agent_home.join("projects");
    if !projects_root.is_dir() {
        return Ok(Vec::new());
    }

    let now = now_unix_seconds();
    let ledger = load_import_ledger(codex_home)?;
    let mut candidates = Vec::new();
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
            let Ok(Some(summary)) = summarize_session(&path) else {
                continue;
            };
            let Ok(has_been_imported) = ledger.contains_current_source(&path) else {
                continue;
            };
            if has_been_imported {
                continue;
            }
            if !is_recent_enough(now, summary.latest_timestamp) {
                continue;
            }
            let migration = summary.migration;
            if !migration.cwd.is_dir() {
                continue;
            }
            candidates.push(SessionCandidate {
                latest_timestamp: summary.latest_timestamp,
                migration,
            });
        }
    }

    candidates.sort_by(|left, right| {
        right
            .latest_timestamp
            .cmp(&left.latest_timestamp)
            .then_with(|| left.migration.path.cmp(&right.migration.path))
    });
    candidates.truncate(SESSION_IMPORT_MAX_COUNT);
    Ok(candidates
        .into_iter()
        .map(|candidate| candidate.migration)
        .collect())
}

fn is_recent_enough(now: i64, latest_timestamp: i64) -> bool {
    latest_timestamp >= now.saturating_sub(SESSION_IMPORT_MAX_AGE.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::record_imported_session;
    use codex_protocol::ThreadId;
    use serde_json::Value as JsonValue;
    use std::path::Path;
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
    fn ignores_old_sessions() {
        let root = TempDir::new().expect("tempdir");
        let external_agent_home = root.path().join(".external");
        let project_root = root.path().join("repo");
        write_session(
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

        assert!(
            detect_recent_sessions(&external_agent_home, root.path())
                .expect("detect")
                .is_empty()
        );
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
