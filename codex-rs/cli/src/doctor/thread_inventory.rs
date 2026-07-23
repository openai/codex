//! Doctor check that compares rollout files against the SQLite thread inventory.

use super::CheckStatus;
use super::Config;
use super::DoctorCheck;
use super::DoctorIssue;
use codex_protocol::protocol::InternalSessionSource;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_state::ThreadStateAuditRow;
use codex_utils_path::normalize_for_path_comparison;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::Path;
use std::path::PathBuf;

const MAX_PARITY_SCAN_FILES: usize = 10_000;
const MAX_ROLLOUT_HEADER_LINES: usize = 64;
const SAMPLE_LIMIT: usize = 5;
const SUMMARY_LIMIT: usize = 8;
const CHECK_ID: &str = "state.rollout_db_parity";
const CHECK_CATEGORY: &str = "threads";

#[derive(Clone, Debug)]
struct RolloutAuditFile {
    path: PathBuf,
    key: PathBuf,
    archived: bool,
    thread_id: String,
}

#[derive(Default)]
struct RolloutScan {
    files: Vec<RolloutAuditFile>,
    existing_keys: HashSet<PathBuf>,
    scan_errors: Vec<String>,
    malformed_names: Vec<PathBuf>,
    reached_scan_cap: bool,
}

enum RolloutThreadId {
    Id(String),
    MalformedName,
    Unusable(String),
}

#[derive(serde::Deserialize)]
struct RolloutLineType {
    #[serde(rename = "type")]
    item_type: String,
}

impl RolloutScan {
    fn candidate_count(&self) -> usize {
        self.files.len() + self.malformed_names.len() + self.scan_errors.len()
    }

    fn reached_candidate_cap(&self) -> bool {
        self.candidate_count() >= MAX_PARITY_SCAN_FILES
    }

    fn record_malformed_name(&mut self, path: PathBuf) {
        if self.reached_candidate_cap() {
            self.reached_scan_cap = true;
            return;
        }
        self.malformed_names.push(path);
        self.reached_scan_cap = self.reached_candidate_cap();
    }

    fn record_scan_error(&mut self, message: String) {
        if self.reached_candidate_cap() {
            self.reached_scan_cap = true;
            return;
        }
        self.scan_errors.push(message);
        self.reached_scan_cap = self.reached_candidate_cap();
    }

    fn active_count(&self) -> usize {
        self.files.iter().filter(|file| !file.archived).count()
    }

    fn archived_count(&self) -> usize {
        self.files.iter().filter(|file| file.archived).count()
    }
}

pub(super) async fn thread_inventory_check(config: &Config) -> DoctorCheck {
    thread_inventory_check_for_roots(
        config.codex_home.as_path(),
        config.sqlite_config(),
        config.model_provider_id.as_str(),
    )
    .await
}

async fn thread_inventory_check_for_roots(
    codex_home: &Path,
    sqlite: &codex_state::SqliteConfig,
    default_provider: &str,
) -> DoctorCheck {
    let scan = scan_rollout_files(codex_home).await;
    let state_db_path = sqlite.state_db_path();

    let mut details = vec![
        format!("default model provider: {default_provider}"),
        format!("rollout DB active files: {}", scan.active_count()),
        format!("rollout DB archived files: {}", scan.archived_count()),
        format!("rollout DB scan errors: {}", scan.scan_errors.len()),
        format!(
            "rollout DB malformed file names: {}",
            scan.malformed_names.len()
        ),
        format!("rollout DB scan cap reached: {}", scan.reached_scan_cap),
    ];
    push_samples(
        &mut details,
        "rollout DB scan error sample",
        scan.scan_errors.iter().map(String::as_str),
    );
    push_samples(
        &mut details,
        "rollout DB malformed file sample",
        scan.malformed_names
            .iter()
            .map(|path| path.display().to_string()),
    );

    if !state_db_path.is_file() {
        details.push("rollout DB rows: skipped (state DB missing)".to_string());
        return missing_state_db_check(scan, details);
    }

    let rows = match codex_state::read_thread_state_audit_rows(sqlite).await {
        Ok(rows) => rows,
        Err(err) => {
            details.push(format!("rollout DB read error: {err}"));
            return DoctorCheck::new(
                CHECK_ID,
                CHECK_CATEGORY,
                CheckStatus::Warning,
                "state database thread inventory could not be read",
            )
            .details(details)
            .issue(
                DoctorIssue::new(
                    CheckStatus::Warning,
                    "state DB thread rows could not be queried",
                )
                .measured(err.to_string())
                .expected("readable threads table"),
            );
        }
    };

    parity_check_from_scan_and_rows(codex_home, scan, rows, details)
}

fn missing_state_db_check(scan: RolloutScan, details: Vec<String>) -> DoctorCheck {
    if scan.files.is_empty()
        && scan.scan_errors.is_empty()
        && scan.malformed_names.is_empty()
        && !scan.reached_scan_cap
    {
        return DoctorCheck::new(
            CHECK_ID,
            CHECK_CATEGORY,
            CheckStatus::Ok,
            "no rollout/state DB inventory to compare",
        )
        .details(details);
    }

    let summary = if scan.files.is_empty() {
        "rollout scan was incomplete or found bad files"
    } else {
        "state DB is missing while rollout files exist"
    };
    let mut check =
        DoctorCheck::new(CHECK_ID, CHECK_CATEGORY, CheckStatus::Warning, summary).details(details);

    if !scan.files.is_empty() {
        check = check
            .issue(
                DoctorIssue::new(
                    CheckStatus::Warning,
                    "rollout files exist but the state DB is missing",
                )
                .measured(format!("{} rollout files", scan.files.len()))
                .expected("state DB contains matching thread rows")
                .remedy("Start Codex with no state DB present so startup backfill can create it from rollout files."),
        )
            .remediation(
                "Start Codex with no state DB present so startup backfill can create it from rollout files.",
            );
    }
    if !scan.scan_errors.is_empty() || !scan.malformed_names.is_empty() || scan.reached_scan_cap {
        check = check.issue(
            DoctorIssue::new(
                CheckStatus::Warning,
                "rollout scan was incomplete or found bad files",
            )
            .measured(format!(
                "{} scan errors, {} malformed names, scan cap reached: {}",
                scan.scan_errors.len(),
                scan.malformed_names.len(),
                scan.reached_scan_cap
            ))
            .expected("rollout directories are fully scannable")
            .remedy("Check file permissions and unexpected files under CODEX_HOME sessions."),
        );
    }
    check
}

fn parity_check_from_scan_and_rows(
    codex_home: &Path,
    scan: RolloutScan,
    rows: Vec<ThreadStateAuditRow>,
    mut details: Vec<String>,
) -> DoctorCheck {
    let rollout_by_key = scan
        .files
        .iter()
        .map(|file| (file.key.clone(), file))
        .collect::<HashMap<_, _>>();
    let mut rows_by_key: HashMap<PathBuf, Vec<&ThreadStateAuditRow>> = HashMap::new();
    for row in &rows {
        rows_by_key
            .entry(rollout_path_key(&row.rollout_path))
            .or_default()
            .push(row);
    }

    let missing_active = missing_rollout_paths(&scan.files, &rows_by_key, /*archived*/ false);
    let missing_archived = missing_rollout_paths(&scan.files, &rows_by_key, /*archived*/ true);
    let scan_complete = !scan.reached_scan_cap;
    let stale_rows = if scan_complete {
        rows.iter()
            .filter(|row| {
                !scan
                    .existing_keys
                    .contains(&rollout_path_key(&row.rollout_path))
                    && !row.rollout_path.is_file()
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let archive_mismatches = if scan_complete {
        rows.iter()
            .filter_map(|row| {
                let expected_archived = rollout_by_key
                    .get(&rollout_path_key(&row.rollout_path))
                    .map(|file| file.archived)
                    .or_else(|| {
                        (scan
                            .existing_keys
                            .contains(&rollout_path_key(&row.rollout_path))
                            || row.rollout_path.is_file())
                        .then(|| archived_from_rollout_path(codex_home, &row.rollout_path))
                        .flatten()
                    })?;
                (expected_archived != row.archived).then_some(row)
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let duplicate_rollout_thread_ids = duplicate_rollout_thread_ids(&scan.files);
    let duplicate_db_paths = duplicate_db_paths(&rows_by_key);
    let archived_rows = rows.iter().filter(|row| row.archived).count();
    let active_rows = rows.len() - archived_rows;

    details.extend([
        format!("rollout DB rows: {}", rows.len()),
        format!("rollout DB active rows: {active_rows}"),
        format!("rollout DB archived rows: {archived_rows}"),
        format!("rollout DB missing active rows: {}", missing_active.len()),
        format!(
            "rollout DB missing archived rows: {}",
            missing_archived.len()
        ),
        format!(
            "rollout DB stale rows: {}",
            count_or_skipped(stale_rows.len(), scan_complete)
        ),
        format!(
            "rollout DB archive mismatches: {}",
            count_or_skipped(archive_mismatches.len(), scan_complete)
        ),
        format!(
            "rollout DB duplicate rollout thread ids: {}",
            duplicate_rollout_thread_ids.len()
        ),
        format!(
            "rollout DB duplicate DB paths: {}",
            duplicate_db_paths.len()
        ),
        format!(
            "rollout DB model providers: {}",
            count_summary(rows.iter().map(|row| row.model_provider.as_str()))
        ),
        format!(
            "rollout DB sources: {}",
            count_summary(rows.iter().map(|row| source_category(&row.source)))
        ),
    ]);
    push_path_samples(
        &mut details,
        "rollout DB missing active sample",
        missing_active.iter().copied(),
    );
    push_path_samples(
        &mut details,
        "rollout DB missing archived sample",
        missing_archived.iter().copied(),
    );
    push_path_samples(
        &mut details,
        "rollout DB stale row sample",
        stale_rows.iter().map(|row| row.rollout_path.as_path()),
    );
    push_path_samples(
        &mut details,
        "rollout DB archive mismatch sample",
        archive_mismatches
            .iter()
            .map(|row| row.rollout_path.as_path()),
    );
    push_samples(
        &mut details,
        "rollout DB duplicate rollout thread id sample",
        duplicate_rollout_thread_ids.iter().map(String::as_str),
    );
    push_path_samples(
        &mut details,
        "rollout DB duplicate DB path sample",
        duplicate_db_paths.iter().map(PathBuf::as_path),
    );

    let status = if scan.scan_errors.is_empty()
        && scan.malformed_names.is_empty()
        && !scan.reached_scan_cap
        && missing_active.is_empty()
        && missing_archived.is_empty()
        && stale_rows.is_empty()
        && archive_mismatches.is_empty()
        && duplicate_rollout_thread_ids.is_empty()
        && duplicate_db_paths.is_empty()
    {
        CheckStatus::Ok
    } else {
        CheckStatus::Warning
    };

    let summary = if status == CheckStatus::Ok {
        "rollout files and state DB thread inventory agree"
    } else {
        "rollout files and state DB thread inventory differ"
    };
    let mut check = DoctorCheck::new(CHECK_ID, CHECK_CATEGORY, status, summary).details(details);

    if !missing_active.is_empty() || !missing_archived.is_empty() {
        check = check.issue(
            DoctorIssue::new(
                CheckStatus::Warning,
                "rollout files are missing from the state DB",
            )
            .measured(format!(
                "{} active, {} archived",
                missing_active.len(),
                missing_archived.len()
            ))
            .expected("every rollout file has a matching threads row"),
        );
    }
    if !stale_rows.is_empty() {
        check = check.issue(
            DoctorIssue::new(
                CheckStatus::Warning,
                "state DB rows point at missing or unusable rollout files",
            )
            .measured(format!("{} stale rows", stale_rows.len()))
            .expected("every state DB rollout path is a file on disk"),
        );
    }
    if !archive_mismatches.is_empty() {
        check = check.issue(
            DoctorIssue::new(
                CheckStatus::Warning,
                "state DB archive flags disagree with rollout file locations",
            )
            .measured(format!("{} mismatched rows", archive_mismatches.len()))
            .expected(
                "rows under archived_sessions are archived and rows under sessions are active",
            ),
        );
    }
    if !duplicate_rollout_thread_ids.is_empty() || !duplicate_db_paths.is_empty() {
        check = check.issue(
            DoctorIssue::new(
                CheckStatus::Warning,
                "duplicate thread inventory entries found",
            )
            .measured(format!(
                "{} duplicate rollout thread ids, {} duplicate DB paths",
                duplicate_rollout_thread_ids.len(),
                duplicate_db_paths.len()
            ))
            .expected("one rollout path and thread id per thread")
            .remedy("Attach the doctor report to a bug report so support can inspect samples."),
        );
    }
    if !scan.scan_errors.is_empty() || !scan.malformed_names.is_empty() || scan.reached_scan_cap {
        check = check.issue(
            DoctorIssue::new(
                CheckStatus::Warning,
                "rollout scan was incomplete or found bad files",
            )
            .measured(format!(
                "{} scan errors, {} malformed names, scan cap reached: {}",
                scan.scan_errors.len(),
                scan.malformed_names.len(),
                scan.reached_scan_cap
            ))
            .expected("rollout directories are fully scannable")
            .remedy("Check file permissions and unexpected files under CODEX_HOME sessions."),
        );
    }
    check
}

async fn scan_rollout_files(codex_home: &Path) -> RolloutScan {
    let mut scan = RolloutScan::default();
    scan_rollout_root(
        &codex_home.join("sessions"),
        /*archived*/ false,
        &mut scan,
    )
    .await;
    scan_rollout_root(
        &codex_home.join("archived_sessions"),
        /*archived*/ true,
        &mut scan,
    )
    .await;
    scan
}

async fn scan_rollout_root(root: &Path, archived: bool, scan: &mut RolloutScan) {
    let mut dirs = vec![root.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        if scan.reached_scan_cap {
            return;
        }
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => {
                scan.record_scan_error(format!("{} ({err})", dir.display()));
                continue;
            }
        };
        for entry in entries {
            if scan.reached_scan_cap {
                return;
            }
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    scan.record_scan_error(format!("{} ({err})", dir.display()));
                    continue;
                }
            };
            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(err) => {
                    scan.record_scan_error(format!("{} ({err})", path.display()));
                    continue;
                }
            };
            if file_type.is_dir() {
                dirs.push(path);
                continue;
            }
            let logical_path = codex_rollout::plain_rollout_path(&path);
            if !file_type.is_file()
                || !is_rollout_file(&logical_path)
                || (path != logical_path && logical_path.is_file())
            {
                continue;
            }
            if scan.candidate_count() >= MAX_PARITY_SCAN_FILES {
                scan.reached_scan_cap = true;
                return;
            }
            let key = path_key(&logical_path);
            scan.existing_keys.insert(key.clone());
            let thread_id = match thread_id_from_rollout(&path).await {
                RolloutThreadId::Id(thread_id) => thread_id,
                RolloutThreadId::MalformedName => {
                    scan.record_malformed_name(path.clone());
                    continue;
                }
                RolloutThreadId::Unusable(reason) => {
                    scan.record_scan_error(format!("{} ({reason})", path.display()));
                    continue;
                }
            };
            scan.files.push(RolloutAuditFile {
                key,
                path,
                archived,
                thread_id,
            });
        }
    }
}

async fn thread_id_from_rollout(path: &Path) -> RolloutThreadId {
    let mut lines = match codex_rollout::open_rollout_line_reader(path).await {
        Ok(lines) => lines,
        Err(err) => return RolloutThreadId::Unusable(err.to_string()),
    };
    let mut has_legacy_item = false;

    for _ in 0..MAX_ROLLOUT_HEADER_LINES {
        let line = match lines.next_line().await {
            Ok(Some(line)) if line.trim().is_empty() => continue,
            Ok(Some(line)) => line,
            Ok(None) => break,
            Err(err) => return RolloutThreadId::Unusable(err.to_string()),
        };
        let item_type = match serde_json::from_str::<RolloutLineType>(line.trim()) {
            Ok(line) => line.item_type,
            Err(_) => continue,
        };
        if item_type == "session_meta" {
            return match serde_json::from_str::<RolloutLine>(line.trim()) {
                Ok(line) => match line.item {
                    RolloutItem::SessionMeta(session_meta) => {
                        RolloutThreadId::Id(session_meta.meta.id.to_string())
                    }
                    _ => RolloutThreadId::Unusable(format!(
                        "rollout at {} has invalid session metadata",
                        path.display()
                    )),
                },
                Err(_) => RolloutThreadId::Unusable(format!(
                    "rollout at {} has invalid session metadata",
                    path.display()
                )),
            };
        }
        if !has_legacy_item {
            has_legacy_item = serde_json::from_str::<RolloutLine>(line.trim()).is_ok();
        }
    }

    if !has_legacy_item {
        return RolloutThreadId::Unusable(format!(
            "rollout at {} has no usable header record",
            path.display()
        ));
    }
    // Legacy rollouts can omit session metadata, so use the validated filename fallback after
    // the bounded prefix without retaining the first item or loading the full history.
    let logical_path = codex_rollout::plain_rollout_path(path);
    codex_rollout::builder_from_items(&[], &logical_path)
        .map(|builder| RolloutThreadId::Id(builder.id.to_string()))
        .unwrap_or(RolloutThreadId::MalformedName)
}

fn is_rollout_file(path: &Path) -> bool {
    path.extension() == Some(OsStr::new("jsonl"))
        && path
            .file_name()
            .and_then(OsStr::to_str)
            .is_some_and(|name| name.starts_with("rollout-"))
}

fn count_or_skipped(count: usize, complete: bool) -> String {
    if complete {
        count.to_string()
    } else {
        "skipped (scan cap reached)".to_string()
    }
}

fn path_key(path: &Path) -> PathBuf {
    normalize_for_path_comparison(path).unwrap_or_else(|_| path.to_path_buf())
}

fn rollout_path_key(path: &Path) -> PathBuf {
    path_key(&codex_rollout::plain_rollout_path(path))
}

fn archived_from_rollout_path(codex_home: &Path, path: &Path) -> Option<bool> {
    let key = path_key(path);
    if key.starts_with(path_key(&codex_home.join("archived_sessions"))) {
        return Some(true);
    }
    if key.starts_with(path_key(&codex_home.join("sessions"))) {
        return Some(false);
    }
    None
}

fn missing_rollout_paths<'a>(
    files: &'a [RolloutAuditFile],
    rows_by_key: &HashMap<PathBuf, Vec<&ThreadStateAuditRow>>,
    archived: bool,
) -> Vec<&'a Path> {
    files
        .iter()
        .filter(|file| file.archived == archived && !has_matching_thread_row(file, rows_by_key))
        .map(|file| file.path.as_path())
        .collect()
}

fn has_matching_thread_row(
    file: &RolloutAuditFile,
    rows_by_key: &HashMap<PathBuf, Vec<&ThreadStateAuditRow>>,
) -> bool {
    let Some(rows) = rows_by_key.get(&file.key) else {
        return false;
    };
    rows.iter().any(|row| row.id == file.thread_id.as_str())
}

fn duplicate_rollout_thread_ids(files: &[RolloutAuditFile]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut duplicates = HashSet::new();
    for thread_id in files.iter().map(|file| file.thread_id.as_str()) {
        if !seen.insert(thread_id) {
            duplicates.insert(thread_id.to_string());
        }
    }
    let mut duplicates = duplicates.into_iter().collect::<Vec<_>>();
    duplicates.sort();
    duplicates
}

fn duplicate_db_paths(rows_by_key: &HashMap<PathBuf, Vec<&ThreadStateAuditRow>>) -> Vec<PathBuf> {
    let mut paths = rows_by_key
        .iter()
        .filter(|(_, rows)| rows.len() > 1)
        .map(|(path, _)| path.clone())
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn source_category(source: &str) -> &'static str {
    let parsed = serde_json::from_str::<SessionSource>(source)
        .or_else(|_| serde_json::from_value(serde_json::Value::String(source.to_string())));
    let Ok(source) = parsed else {
        return "unparsable";
    };

    match source {
        SessionSource::Cli => "cli",
        SessionSource::VSCode => "vscode",
        SessionSource::Exec => "exec",
        SessionSource::Mcp => "mcp",
        SessionSource::Custom(_) => "custom",
        SessionSource::Internal(InternalSessionSource::MemoryConsolidation) => {
            "internal:memory_consolidation"
        }
        SessionSource::SubAgent(SubAgentSource::Review) => "subagent:review",
        SessionSource::SubAgent(SubAgentSource::Compact) => "subagent:compact",
        SessionSource::SubAgent(SubAgentSource::ThreadSpawn { .. }) => "subagent:thread_spawn",
        SessionSource::SubAgent(SubAgentSource::MemoryConsolidation) => {
            "subagent:memory_consolidation"
        }
        SessionSource::SubAgent(SubAgentSource::Other(_)) => "subagent:other",
        SessionSource::Unknown => "unknown",
    }
}

fn count_summary<I, V>(values: I) -> String
where
    I: Iterator<Item = V>,
    V: Into<String>,
{
    let mut counts = BTreeMap::<String, usize>::new();
    for value in values {
        *counts.entry(value.into()).or_default() += 1;
    }
    if counts.is_empty() {
        return "none".to_string();
    }

    let mut entries = counts.into_iter().collect::<Vec<_>>();
    entries.sort_by(|(left_value, left_count), (right_value, right_count)| {
        right_count
            .cmp(left_count)
            .then_with(|| left_value.cmp(right_value))
    });
    let omitted_categories = entries.len().saturating_sub(SUMMARY_LIMIT);
    let omitted_rows = entries
        .iter()
        .skip(SUMMARY_LIMIT)
        .map(|(_, count)| count)
        .sum::<usize>();
    let mut parts = entries
        .into_iter()
        .take(SUMMARY_LIMIT)
        .map(|(value, count)| format!("{value}={count}"))
        .collect::<Vec<_>>();
    if omitted_categories > 0 {
        parts.push(format!(
            "other={omitted_rows} across {omitted_categories} categories"
        ));
    }
    parts.join(", ")
}

fn push_path_samples<'a>(
    details: &mut Vec<String>,
    label: &str,
    paths: impl Iterator<Item = &'a Path>,
) {
    push_samples(details, label, paths.map(|path| path.display().to_string()));
}

fn push_samples<I, V>(details: &mut Vec<String>, label: &str, values: I)
where
    I: Iterator<Item = V>,
    V: ToString,
{
    for value in values.take(SAMPLE_LIMIT) {
        details.push(format!("{label}: {}", value.to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::ThreadId;
    use codex_utils_absolute_path::test_support::PathExt;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[tokio::test]
    async fn thread_inventory_check_ok_when_rollouts_match_db() {
        let fixture = Fixture::new().await;
        let active_path = fixture.write_rollout(
            /*archived*/ false,
            "2025-01-02T10-00-00",
            "00000000-0000-0000-0000-000000000001",
        );
        let archived_path = fixture.write_rollout(
            /*archived*/ true,
            "2025-01-02T11-00-00",
            "00000000-0000-0000-0000-000000000002",
        );
        fixture
            .insert_thread_row(
                "00000000-0000-0000-0000-000000000001",
                active_path.as_path(),
                /*archived*/ false,
            )
            .await;
        fixture
            .insert_thread_row(
                "00000000-0000-0000-0000-000000000002",
                archived_path.as_path(),
                /*archived*/ true,
            )
            .await;

        let check = thread_inventory_check_for_roots(
            fixture.codex_home.path(),
            &fixture.sqlite(),
            "test-provider",
        )
        .await;

        assert_eq!(check.status, CheckStatus::Ok);
        assert_eq!(check.category, CHECK_CATEGORY);
        assert_detail(&check, "rollout DB missing active rows", "0");
        assert_detail(&check, "rollout DB missing archived rows", "0");
        assert_detail(&check, "rollout DB stale rows", "0");
        assert_detail(&check, "rollout DB archive mismatches", "0");
    }

    #[tokio::test]
    async fn thread_inventory_check_warns_for_missing_stale_and_mismatched_rows() {
        let fixture = Fixture::new().await;
        let missing_path = fixture.write_rollout(
            /*archived*/ false,
            "2025-01-02T10-00-00",
            "00000000-0000-0000-0000-000000000001",
        );
        let mismatched_path = fixture.write_rollout(
            /*archived*/ true,
            "2025-01-02T11-00-00",
            "00000000-0000-0000-0000-000000000002",
        );
        let stale_path = fixture
            .codex_home
            .path()
            .join("sessions/2025/01/02/rollout-2025-01-02T12-00-00-00000000-0000-0000-0000-000000000003.jsonl");
        fixture
            .insert_thread_row(
                "00000000-0000-0000-0000-000000000002",
                mismatched_path.as_path(),
                /*archived*/ false,
            )
            .await;
        fixture
            .insert_thread_row(
                "00000000-0000-0000-0000-000000000003",
                stale_path.as_path(),
                /*archived*/ false,
            )
            .await;

        let check = thread_inventory_check_for_roots(
            fixture.codex_home.path(),
            &fixture.sqlite(),
            "test-provider",
        )
        .await;

        assert_eq!(check.status, CheckStatus::Warning);
        assert_eq!(check.issues.len(), 3);
        assert_detail(&check, "rollout DB missing active rows", "1");
        assert_detail(&check, "rollout DB stale rows", "1");
        assert_detail(&check, "rollout DB archive mismatches", "1");
        assert_eq!(check.remediation, None);
        assert!(check.issues.iter().all(|issue| {
            !issue
                .remedy
                .as_deref()
                .is_some_and(|remedy| remedy.starts_with("Restart Codex"))
        }));
        let missing_sample = check
            .details
            .iter()
            .find_map(|detail| detail.strip_prefix("rollout DB missing active sample: "))
            .expect("missing active sample");
        assert_eq!(Path::new(missing_sample), missing_path.as_path());
    }

    #[tokio::test]
    async fn thread_inventory_check_uses_metadata_id_when_filename_and_db_disagree() {
        let fixture = Fixture::new().await;
        let filename_id = "00000000-0000-0000-0000-000000000001";
        let metadata_id = ThreadId::from_string("00000000-0000-0000-0000-000000000002")
            .expect("metadata thread id");
        let path =
            fixture.write_rollout(/*archived*/ false, "2025-01-02T10-00-00", filename_id);
        let contents = std::fs::read_to_string(&path).expect("rollout file");
        let mut rollout_line =
            serde_json::from_str::<RolloutLine>(contents.trim()).expect("rollout line");
        let RolloutItem::SessionMeta(session_meta) = &mut rollout_line.item else {
            panic!("expected session metadata");
        };
        session_meta.meta.session_id = metadata_id.into();
        session_meta.meta.id = metadata_id;
        session_meta.meta.timestamp = "not-a-timestamp".to_string();
        let contents = serde_json::to_string(&rollout_line).expect("rollout line");
        std::fs::write(&path, format!("{contents}\n")).expect("rollout file");
        fixture
            .insert_thread_row(filename_id, path.as_path(), /*archived*/ false)
            .await;

        let check = thread_inventory_check_for_roots(
            fixture.codex_home.path(),
            &fixture.sqlite(),
            "test-provider",
        )
        .await;

        assert_eq!(check.status, CheckStatus::Warning);
        assert_detail(&check, "rollout DB scan errors", "0");
        assert_detail(&check, "rollout DB missing active rows", "1");
        assert_detail(&check, "rollout DB stale rows", "0");
    }

    #[tokio::test]
    async fn thread_inventory_check_ignores_invalid_utf8_after_session_metadata() {
        let fixture = Fixture::new().await;
        let thread_id = "00000000-0000-0000-0000-000000000001";
        let path = fixture.write_rollout(/*archived*/ false, "2025-01-02T10-00-00", thread_id);
        let mut contents = std::fs::read(&path).expect("rollout file");
        contents.resize(contents.len() + 128 * 1024, 0xff);
        contents.push(b'\n');
        std::fs::write(&path, contents).expect("rollout file");
        fixture
            .insert_thread_row(thread_id, path.as_path(), /*archived*/ false)
            .await;

        let check = thread_inventory_check_for_roots(
            fixture.codex_home.path(),
            &fixture.sqlite(),
            "test-provider",
        )
        .await;

        assert_eq!(check.status, CheckStatus::Ok);
        assert_detail(&check, "rollout DB scan errors", "0");
        assert_detail(&check, "rollout DB missing active rows", "0");
    }

    #[tokio::test]
    async fn thread_inventory_check_matches_compressed_rollouts_to_canonical_db_paths() {
        let fixture = Fixture::new().await;
        let active_id = "00000000-0000-0000-0000-000000000001";
        let archived_id = "00000000-0000-0000-0000-000000000002";
        let active_path =
            fixture.write_rollout(/*archived*/ false, "2025-01-02T10-00-00", active_id);
        let archived_path =
            fixture.write_rollout(/*archived*/ true, "2025-01-02T11-00-00", archived_id);
        compress_rollout(&active_path);
        compress_rollout(&archived_path);
        fixture
            .insert_thread_row(active_id, active_path.as_path(), /*archived*/ false)
            .await;
        fixture
            .insert_thread_row(archived_id, archived_path.as_path(), /*archived*/ true)
            .await;

        let check = thread_inventory_check_for_roots(
            fixture.codex_home.path(),
            &fixture.sqlite(),
            "test-provider",
        )
        .await;

        assert_eq!(check.status, CheckStatus::Ok);
        assert_detail(&check, "rollout DB active files", "1");
        assert_detail(&check, "rollout DB archived files", "1");
        assert_detail(&check, "rollout DB scan errors", "0");
        assert_detail(&check, "rollout DB missing active rows", "0");
        assert_detail(&check, "rollout DB missing archived rows", "0");
        assert_detail(&check, "rollout DB stale rows", "0");
    }

    #[tokio::test]
    async fn thread_inventory_check_prefers_plain_rollout_over_compressed_sibling() {
        let fixture = Fixture::new().await;
        let thread_id = "00000000-0000-0000-0000-000000000001";
        let path = fixture.write_rollout(/*archived*/ false, "2025-01-02T10-00-00", thread_id);
        compress_rollout(&path);
        fixture.write_rollout(/*archived*/ false, "2025-01-02T10-00-00", thread_id);
        fixture
            .insert_thread_row(thread_id, path.as_path(), /*archived*/ false)
            .await;

        let check = thread_inventory_check_for_roots(
            fixture.codex_home.path(),
            &fixture.sqlite(),
            "test-provider",
        )
        .await;

        assert_eq!(check.status, CheckStatus::Ok);
        assert_detail(&check, "rollout DB active files", "1");
        assert_detail(&check, "rollout DB duplicate rollout thread ids", "0");
        assert_detail(&check, "rollout DB stale rows", "0");
    }

    #[tokio::test]
    async fn thread_inventory_check_uses_compressed_metadata_id_and_legacy_filename_fallback() {
        let fixture = Fixture::new().await;
        let filename_id = "00000000-0000-0000-0000-000000000001";
        let metadata_id = ThreadId::from_string("00000000-0000-0000-0000-000000000002")
            .expect("metadata thread id");
        let metadata_path =
            fixture.write_rollout(/*archived*/ false, "2025-01-02T10-00-00", filename_id);
        let contents = std::fs::read_to_string(&metadata_path).expect("rollout file");
        let mut rollout_line =
            serde_json::from_str::<RolloutLine>(contents.trim()).expect("rollout line");
        let RolloutItem::SessionMeta(session_meta) = &mut rollout_line.item else {
            panic!("expected session metadata");
        };
        session_meta.meta.session_id = metadata_id.into();
        session_meta.meta.id = metadata_id;
        let contents = serde_json::to_string(&rollout_line).expect("rollout line");
        std::fs::write(&metadata_path, format!("{contents}\n")).expect("rollout file");
        compress_rollout(&metadata_path);
        fixture
            .insert_thread_row(
                filename_id,
                metadata_path.as_path(),
                /*archived*/ false,
            )
            .await;

        let legacy_id = "00000000-0000-0000-0000-000000000003";
        let legacy_path = fixture.codex_home.path().join(format!(
            "sessions/2025/01/02/rollout-2025-01-02T11-00-00-{legacy_id}.jsonl"
        ));
        std::fs::write(
            &legacy_path,
            "{\"timestamp\":\"2025-01-02T11:00:00Z\",\"type\":\"compacted\",\"payload\":{\"message\":\"legacy history\"}}\n",
        )
        .expect("legacy rollout");
        compress_rollout(&legacy_path);
        fixture
            .insert_thread_row(legacy_id, legacy_path.as_path(), /*archived*/ false)
            .await;

        let check = thread_inventory_check_for_roots(
            fixture.codex_home.path(),
            &fixture.sqlite(),
            "test-provider",
        )
        .await;

        assert_eq!(check.status, CheckStatus::Warning);
        assert_detail(&check, "rollout DB active files", "2");
        assert_detail(&check, "rollout DB malformed file names", "0");
        assert_detail(&check, "rollout DB scan errors", "0");
        assert_detail(&check, "rollout DB missing active rows", "1");
        assert_detail(&check, "rollout DB stale rows", "0");
    }

    #[tokio::test]
    async fn thread_inventory_check_reports_corrupt_compressed_rollout_without_stale_row() {
        let fixture = Fixture::new().await;
        let thread_id = "00000000-0000-0000-0000-000000000001";
        let path = fixture.write_rollout(/*archived*/ false, "2025-01-02T10-00-00", thread_id);
        let compressed_path = path.with_file_name(format!(
            "{}.zst",
            path.file_name()
                .expect("rollout file name")
                .to_string_lossy()
        ));
        std::fs::write(&compressed_path, "not zstd").expect("corrupt compressed rollout");
        std::fs::remove_file(&path).expect("remove plain rollout");
        fixture
            .insert_thread_row(thread_id, path.as_path(), /*archived*/ false)
            .await;

        let check = thread_inventory_check_for_roots(
            fixture.codex_home.path(),
            &fixture.sqlite(),
            "test-provider",
        )
        .await;

        assert_eq!(check.status, CheckStatus::Warning);
        assert_detail(&check, "rollout DB scan errors", "1");
        assert_detail(&check, "rollout DB stale rows", "0");
    }

    #[tokio::test]
    async fn thread_inventory_check_ignores_compression_temp_files() {
        let fixture = Fixture::new().await;
        let temp_path = fixture.codex_home.path().join(
            "sessions/2025/01/02/rollout-2025-01-02T10-00-00-00000000-0000-0000-0000-000000000001.jsonl.zst.compress.1.0.tmp",
        );
        std::fs::create_dir_all(temp_path.parent().expect("rollout temp parent"))
            .expect("rollout temp dir");
        std::fs::write(temp_path, "not a completed rollout").expect("rollout temp file");

        let check = thread_inventory_check_for_roots(
            fixture.codex_home.path(),
            &fixture.sqlite(),
            "test-provider",
        )
        .await;

        assert_eq!(check.status, CheckStatus::Ok);
        assert_detail(&check, "rollout DB active files", "0");
        assert_detail(&check, "rollout DB scan errors", "0");
    }

    #[tokio::test]
    async fn thread_id_from_rollout_falls_back_for_legacy_non_meta_record() {
        let home = TempDir::new().expect("temp dir");
        let thread_id = "00000000-0000-0000-0000-000000000001";
        let valid_path = home
            .path()
            .join(format!("rollout-2025-01-02T10-00-00-{thread_id}.jsonl"));
        let malformed_path = home.path().join("rollout-not-a-valid-name.jsonl");
        let legacy_line = serde_json::json!({
            "timestamp": "2025-01-02T10:00:00Z",
            "type": "compacted",
            "payload": {"message": "legacy history"},
        });
        std::fs::write(&valid_path, format!("not-json\n{legacy_line}\n")).expect("legacy rollout");
        std::fs::write(&malformed_path, format!("{legacy_line}\n")).expect("legacy rollout");

        assert!(matches!(
            thread_id_from_rollout(&valid_path).await,
            RolloutThreadId::Id(id) if id == thread_id
        ));
        assert!(matches!(
            thread_id_from_rollout(&malformed_path).await,
            RolloutThreadId::MalformedName
        ));
    }

    #[tokio::test]
    async fn thread_id_from_rollout_uses_metadata_after_pre_header_record() {
        let fixture = Fixture::new().await;
        let filename_id = "00000000-0000-0000-0000-000000000001";
        let metadata_id = ThreadId::from_string("00000000-0000-0000-0000-000000000002")
            .expect("metadata thread id");
        let path =
            fixture.write_rollout(/*archived*/ false, "2025-01-02T10-00-00", filename_id);
        let contents = std::fs::read_to_string(&path).expect("rollout file");
        let mut rollout_line =
            serde_json::from_str::<RolloutLine>(contents.trim()).expect("rollout line");
        let RolloutItem::SessionMeta(session_meta) = &mut rollout_line.item else {
            panic!("expected session metadata");
        };
        session_meta.meta.session_id = metadata_id.into();
        session_meta.meta.id = metadata_id;
        let response_item = serde_json::json!({
            "timestamp": "2025-01-02T10:00:00Z",
            "type": "response_item",
            "payload": {
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "before metadata"}],
            },
        });
        let session_meta = serde_json::to_string(&rollout_line).expect("rollout line");
        std::fs::write(&path, format!("{response_item}\n{session_meta}\n")).expect("rollout file");

        assert!(matches!(
            thread_id_from_rollout(&path).await,
            RolloutThreadId::Id(id) if id == metadata_id.to_string()
        ));
    }

    #[tokio::test]
    async fn thread_id_from_rollout_uses_metadata_at_header_line_limit() {
        let fixture = Fixture::new().await;
        let filename_id = "00000000-0000-0000-0000-000000000001";
        let metadata_id = ThreadId::from_string("00000000-0000-0000-0000-000000000002")
            .expect("metadata thread id");
        let path =
            fixture.write_rollout(/*archived*/ false, "2025-01-02T10-00-00", filename_id);
        let contents = std::fs::read_to_string(&path).expect("rollout file");
        let mut rollout_line =
            serde_json::from_str::<RolloutLine>(contents.trim()).expect("rollout line");
        let RolloutItem::SessionMeta(session_meta) = &mut rollout_line.item else {
            panic!("expected session metadata");
        };
        session_meta.meta.session_id = metadata_id.into();
        session_meta.meta.id = metadata_id;
        let legacy_line = serde_json::json!({
            "timestamp": "2025-01-02T10:00:00Z",
            "type": "compacted",
            "payload": {
                "message": "legacy history",
                "replacement_history": [{
                    "type": "message",
                    "role": "assistant",
                    "content": [{
                        "type": "output_text",
                        "text": "x".repeat(/*n*/ 1024 * 1024),
                    }],
                }],
            },
        });
        let mut lines = vec![legacy_line.to_string(); MAX_ROLLOUT_HEADER_LINES - 1];
        lines.push(serde_json::to_string(&rollout_line).expect("rollout line"));
        std::fs::write(&path, format!("{}\n", lines.join("\n"))).expect("rollout file");

        assert!(matches!(
            thread_id_from_rollout(&path).await,
            RolloutThreadId::Id(id) if id == metadata_id.to_string()
        ));
    }

    #[tokio::test]
    async fn thread_id_from_rollout_stops_after_legacy_header_line_limit() {
        let home = TempDir::new().expect("temp dir");
        let thread_id = "00000000-0000-0000-0000-000000000001";
        let path = home
            .path()
            .join(format!("rollout-2025-01-02T10-00-00-{thread_id}.jsonl"));
        let legacy_line = serde_json::json!({
            "timestamp": "2025-01-02T10:00:00Z",
            "type": "compacted",
            "payload": {"message": "legacy history"},
        });
        let mut contents = format!(
            "{}\n",
            vec![legacy_line.to_string(); MAX_ROLLOUT_HEADER_LINES].join("\n")
        )
        .into_bytes();
        contents.extend([0xff, 0xfe, b'\n']);
        std::fs::write(&path, contents).expect("legacy rollout");

        assert!(matches!(
            thread_id_from_rollout(&path).await,
            RolloutThreadId::Id(id) if id == thread_id
        ));
    }

    #[tokio::test]
    async fn thread_id_from_rollout_rejects_unusable_headers() {
        let home = TempDir::new().expect("temp dir");
        let empty_path = home
            .path()
            .join("rollout-2025-01-02T10-00-00-00000000-0000-0000-0000-000000000001.jsonl");
        let invalid_path = home
            .path()
            .join("rollout-2025-01-02T10-00-01-00000000-0000-0000-0000-000000000002.jsonl");
        let invalid_utf8_path = home
            .path()
            .join("rollout-2025-01-02T10-00-02-00000000-0000-0000-0000-000000000003.jsonl");
        let unknown_history_mode_path = home
            .path()
            .join("rollout-2025-01-02T10-00-03-00000000-0000-0000-0000-000000000004.jsonl");
        let missing_path = home
            .path()
            .join("rollout-2025-01-02T10-00-04-00000000-0000-0000-0000-000000000005.jsonl");
        std::fs::write(&empty_path, " \n\t\n").expect("empty rollout");
        std::fs::write(&invalid_path, "not-json\n{also-invalid}\n").expect("invalid rollout");
        std::fs::write(&invalid_utf8_path, [0xff, 0xfe, b'\n']).expect("invalid utf8 rollout");
        let unknown_history_mode = serde_json::json!({
            "timestamp": "2025-01-02T10:00:03Z",
            "type": "session_meta",
            "payload": {
                "session_id": "00000000-0000-0000-0000-000000000004",
                "id": "00000000-0000-0000-0000-000000000004",
                "timestamp": "2025-01-02T10:00:03Z",
                "cwd": ".",
                "originator": "test",
                "cli_version": "test",
                "source": "cli",
                "model_provider": "test-provider",
                "history_mode": "future",
            },
        });
        std::fs::write(
            &unknown_history_mode_path,
            format!(
                "{unknown_history_mode}\n{}\n",
                serde_json::json!({
                    "timestamp": "2025-01-02T10:00:04Z",
                    "type": "session_meta",
                    "payload": {
                        "session_id": "00000000-0000-0000-0000-000000000005",
                        "id": "00000000-0000-0000-0000-000000000005",
                        "timestamp": "2025-01-02T10:00:04Z",
                        "cwd": ".",
                        "originator": "test",
                        "cli_version": "test",
                        "source": "cli",
                        "model_provider": "test-provider",
                    },
                })
            ),
        )
        .expect("unknown history mode rollout");

        for path in [
            &empty_path,
            &invalid_path,
            &invalid_utf8_path,
            &unknown_history_mode_path,
            &missing_path,
        ] {
            assert!(
                matches!(
                    thread_id_from_rollout(path).await,
                    RolloutThreadId::Unusable(_)
                ),
                "{} should be unusable",
                path.display()
            );
        }
    }

    struct Fixture {
        codex_home: TempDir,
        sqlite_home: TempDir,
    }

    fn compress_rollout(path: &Path) {
        let compressed_path = path.with_file_name(format!(
            "{}.zst",
            path.file_name()
                .expect("rollout file name")
                .to_string_lossy()
        ));
        let contents = std::fs::read(path).expect("rollout file");
        let compressed =
            zstd::stream::encode_all(contents.as_slice(), /*level*/ 3).expect("compress rollout");
        std::fs::write(compressed_path, compressed).expect("compressed rollout");
        std::fs::remove_file(path).expect("remove plain rollout");
    }

    impl Fixture {
        async fn new() -> Self {
            let codex_home = TempDir::new().expect("codex home");
            let sqlite_home = TempDir::new().expect("sqlite home");
            let _runtime = codex_state::StateRuntime::init(
                codex_state::SqliteConfig::new_for_testing(sqlite_home.path().abs()),
                "test-provider".to_string(),
            )
            .await
            .expect("state runtime");
            Self {
                codex_home,
                sqlite_home,
            }
        }

        fn sqlite(&self) -> codex_state::SqliteConfig {
            codex_state::SqliteConfig::new_for_testing(self.sqlite_home.path().abs())
        }

        fn write_rollout(&self, archived: bool, timestamp: &str, thread_id: &str) -> PathBuf {
            let root = if archived {
                self.codex_home.path().join("archived_sessions")
            } else {
                self.codex_home.path().join("sessions/2025/01/02")
            };
            std::fs::create_dir_all(&root).expect("rollout dir");
            let path = root.join(format!("rollout-{timestamp}-{thread_id}.jsonl"));
            let parsed_thread_id = ThreadId::from_string(thread_id).expect("thread id");
            let rollout_line = RolloutLine {
                timestamp: timestamp.to_string(),
                ordinal: None,
                item: RolloutItem::SessionMeta(codex_protocol::protocol::SessionMetaLine {
                    meta: codex_protocol::protocol::SessionMeta {
                        session_id: parsed_thread_id.into(),
                        id: parsed_thread_id,
                        timestamp: timestamp.to_string(),
                        cwd: self.codex_home.path().to_path_buf(),
                        originator: "test".to_string(),
                        cli_version: "test".to_string(),
                        source: SessionSource::Cli,
                        model_provider: Some("test-provider".to_string()),
                        ..Default::default()
                    },
                    git: None,
                }),
            };
            let contents = serde_json::to_string(&rollout_line).expect("rollout line");
            std::fs::write(&path, format!("{contents}\n")).expect("rollout file");
            path
        }

        async fn insert_thread_row(&self, id: &str, rollout_path: &Path, archived: bool) {
            let sqlite = self.sqlite();
            let pool = sqlite
                .open_read_write_pool(&sqlite.state_db_path())
                .await
                .expect("sqlite pool");
            sqlx::query(
                r#"
INSERT INTO threads (
    id,
    rollout_path,
    created_at,
    updated_at,
    source,
    model_provider,
    cwd,
    title,
    sandbox_policy,
    approval_mode,
    archived,
    archived_at
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(id)
            .bind(rollout_path.display().to_string())
            .bind(1_i64)
            .bind(1_i64)
            .bind("cli")
            .bind("test-provider")
            .bind(self.codex_home.path().display().to_string())
            .bind("test title")
            .bind("read-only")
            .bind("on-request")
            .bind(if archived { 1_i64 } else { 0_i64 })
            .bind(archived.then_some(1_i64))
            .execute(&pool)
            .await
            .expect("insert thread row");
            pool.close().await;
        }
    }

    fn assert_detail(check: &DoctorCheck, label: &str, expected: &str) {
        let prefix = format!("{label}: ");
        let actual = check
            .details
            .iter()
            .find_map(|detail| detail.strip_prefix(&prefix))
            .expect("detail should exist");
        assert_eq!(actual, expected);
    }

    #[test]
    fn source_category_coarsens_structured_sources() {
        assert_eq!(source_category("cli"), "cli");
        assert_eq!(
            source_category(r#"{"subagent":"memory_consolidation"}"#),
            "subagent:memory_consolidation"
        );
        assert_eq!(
            source_category(
                r#"{"subagent":{"thread_spawn":{"parent_thread_id":"00000000-0000-0000-0000-000000000001","depth":2}}}"#,
            ),
            "subagent:thread_spawn"
        );
    }

    #[test]
    fn count_summary_caps_distinct_values() {
        let summary = count_summary(["a", "b", "c", "d", "e", "f", "g", "h", "i"].into_iter());

        assert_eq!(
            summary,
            "a=1, b=1, c=1, d=1, e=1, f=1, g=1, h=1, other=1 across 1 categories"
        );
    }
}
