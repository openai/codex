use crate::model::ExternalAgentSessionImportLimits;
use crate::sessions::ExternalAgentSessionMigration;
use crate::sessions::ledger::load_import_ledger;
use crate::sessions::ledger::save_import_ledger;
use crate::sessions::now_unix_seconds;
use crate::sessions::records::summarize_session_with_cwd;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

pub(super) struct SessionFileCandidate {
    pub path: PathBuf,
    pub fallback_cwd: Option<PathBuf>,
}

pub(super) fn detect_recent_sessions(
    codex_home: &Path,
    candidates: impl IntoIterator<Item = SessionFileCandidate>,
    require_existing_cwd: bool,
    limits: ExternalAgentSessionImportLimits,
) -> io::Result<Vec<ExternalAgentSessionMigration>> {
    let now = now_unix_seconds();
    let mut ledger = load_import_ledger(codex_home)?;
    let source_states = ledger.source_states();
    let mut recent = BinaryHeap::new();

    for candidate in candidates {
        let Ok(metadata) = fs::metadata(&candidate.path) else {
            continue;
        };
        let Ok(modified_at) = metadata.modified() else {
            continue;
        };
        let Ok(modified_at) = modified_at.duration_since(std::time::UNIX_EPOCH) else {
            continue;
        };
        if (modified_at.as_secs() as i64) < now.saturating_sub(limits.max_age.as_secs() as i64) {
            continue;
        }
        let Ok(modified_at_nanos) = i64::try_from(modified_at.as_nanos()) else {
            continue;
        };
        let Ok(source_path) = fs::canonicalize(&candidate.path) else {
            continue;
        };
        if let Some(state) = source_states.get(source_path.as_path())
            && (state.source_modified_at == Some(modified_at_nanos)
                || state.source_modified_at.is_none()
                    && modified_at.as_secs() as i64 <= state.imported_at)
        {
            continue;
        }
        recent.push((
            Reverse(modified_at_nanos),
            candidate.path,
            candidate.fallback_cwd,
        ));
        if recent.len() > limits.max_sessions {
            recent.pop();
        }
    }

    drop(source_states);
    let mut migrations = Vec::new();
    let mut ledger_changed = false;
    for (modified_at, path, fallback_cwd) in recent.into_sorted_vec() {
        match ledger.refresh_current_source(&path, modified_at.0) {
            Ok(false) => {}
            Ok(true) => {
                ledger_changed = true;
                continue;
            }
            Err(_) => continue,
        }
        let Ok(Some(summary)) = summarize_session_with_cwd(&path, fallback_cwd.as_deref()) else {
            continue;
        };
        if require_existing_cwd && !summary.migration.cwd.is_dir() {
            continue;
        }
        migrations.push(summary.migration);
    }
    if ledger_changed {
        save_import_ledger(codex_home, &ledger)?;
    }

    Ok(migrations)
}
