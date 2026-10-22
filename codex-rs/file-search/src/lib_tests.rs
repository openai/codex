#![cfg(test)]

//! Unit tests for the crate and its related test suites.

#![allow(clippy::unwrap_used)]

use super::*;
use pretty_assertions::assert_eq;
use std::fs;
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use tempfile::TempDir;

#[test]
fn verify_score_is_none_for_non_match() {
    let mut utf32buf = Vec::<char>::new();
    let line = "hello";
    let mut matcher = Matcher::new(Config::DEFAULT);
    let haystack: Utf32Str<'_> = Utf32Str::new(line, &mut utf32buf);
    let pattern = create_pattern("zzz");
    let score = pattern.score(haystack, &mut matcher);
    assert_eq!(score, None);
}

#[test]
fn tie_breakers_sort_by_path_when_scores_equal() {
    let mut matches = vec![
        (100, "b_path".to_string()),
        (100, "a_path".to_string()),
        (90, "zzz".to_string()),
    ];

    sort_matches(&mut matches);

    // Highest score first; ties broken alphabetically.
    let expected = vec![
        (100, "a_path".to_string()),
        (100, "b_path".to_string()),
        (90, "zzz".to_string()),
    ];

    assert_eq!(matches, expected);
}

#[test]
fn file_name_from_path_uses_basename() {
    assert_eq!(file_name_from_path("foo/bar.txt"), "bar.txt");
}

#[test]
fn file_name_from_path_falls_back_to_full_path() {
    assert_eq!(file_name_from_path(""), "");
}

#[derive(Default)]
pub(super) struct RecordingReporter {
    updates: Mutex<Vec<FileSearchSnapshot>>,
    complete_times: Mutex<Vec<Instant>>,
    complete_cv: Condvar,
    update_cv: Condvar,
}

impl RecordingReporter {
    fn wait_until<T, F>(
        &self,
        mutex: &Mutex<T>,
        cv: &Condvar,
        timeout: Duration,
        mut predicate: F,
    ) -> bool
    where
        F: FnMut(&T) -> bool,
    {
        let deadline = Instant::now() + timeout;
        let mut state = mutex.lock().unwrap();
        loop {
            if predicate(&state) {
                return true;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return false;
            }
            let (next_state, wait_result) = cv.wait_timeout(state, remaining).unwrap();
            state = next_state;
            if wait_result.timed_out() {
                return predicate(&state);
            }
        }
    }

    fn wait_for_complete(&self, timeout: Duration) -> bool {
        self.wait_until(
            &self.complete_times,
            &self.complete_cv,
            timeout,
            |completes| !completes.is_empty(),
        )
    }
    fn clear(&self) {
        self.updates.lock().unwrap().clear();
        self.complete_times.lock().unwrap().clear();
    }

    fn updates(&self) -> Vec<FileSearchSnapshot> {
        self.updates.lock().unwrap().clone()
    }

    fn wait_for_updates_at_least(&self, min_len: usize, timeout: Duration) -> bool {
        self.wait_until(&self.updates, &self.update_cv, timeout, |updates| {
            updates.len() >= min_len
        })
    }

    fn snapshot(&self) -> FileSearchSnapshot {
        self.updates
            .lock()
            .unwrap()
            .last()
            .cloned()
            .unwrap_or_default()
    }
}

impl SessionReporter for RecordingReporter {
    fn on_update(&self, snapshot: &FileSearchSnapshot) {
        let mut updates = self.updates.lock().unwrap();
        updates.push(snapshot.clone());
        self.update_cv.notify_all();
    }

    fn on_complete(&self) {
        {
            let mut complete_times = self.complete_times.lock().unwrap();
            complete_times.push(Instant::now());
        }
        self.complete_cv.notify_all();
    }
}

fn create_temp_tree(file_count: usize) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..file_count {
        let path = dir.path().join(format!("file-{i:04}.txt"));
        fs::write(path, format!("contents {i}")).unwrap();
    }
    dir
}

#[test]
fn session_scanned_file_count_is_monotonic_across_queries() {
    let dir = create_temp_tree(/*file_count*/ 200);
    let reporter = Arc::new(RecordingReporter::default());
    let session = create_session(
        vec![dir.path().to_path_buf()],
        FileSearchOptions::default(),
        reporter.clone(),
        /*cancel_flag*/ None,
    )
    .expect("session");

    session.update_query("file-00");
    thread::sleep(Duration::from_millis(20));
    let first_snapshot = reporter.snapshot();
    session.update_query("file-01");
    thread::sleep(Duration::from_millis(20));
    let second_snapshot = reporter.snapshot();
    let _ = reporter.wait_for_complete(Duration::from_secs(5));
    let completed_snapshot = reporter.snapshot();

    assert!(second_snapshot.scanned_file_count >= first_snapshot.scanned_file_count);
    assert!(completed_snapshot.scanned_file_count >= second_snapshot.scanned_file_count);
}

#[test]
fn session_streams_updates_before_walk_complete() {
    let dir = create_temp_tree(/*file_count*/ 600);
    let reporter = Arc::new(RecordingReporter::default());
    let session = create_session(
        vec![dir.path().to_path_buf()],
        FileSearchOptions::default(),
        reporter.clone(),
        /*cancel_flag*/ None,
    )
    .expect("session");

    session.update_query("file-0");
    let completed = reporter.wait_for_complete(Duration::from_secs(5));

    assert!(completed);
    let updates = reporter.updates();
    assert!(updates.iter().any(|snapshot| !snapshot.walk_complete));
}

#[test]
fn session_accepts_query_updates_after_walk_complete() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("alpha.txt"), "alpha").unwrap();
    fs::write(dir.path().join("beta.txt"), "beta").unwrap();
    let reporter = Arc::new(RecordingReporter::default());
    let session = create_session(
        vec![dir.path().to_path_buf()],
        FileSearchOptions::default(),
        reporter.clone(),
        /*cancel_flag*/ None,
    )
    .expect("session");

    session.update_query("alpha");
    assert!(reporter.wait_for_complete(Duration::from_secs(5)));
    let updates_before = reporter.updates().len();

    session.update_query("beta");
    assert!(reporter.wait_for_updates_at_least(updates_before + 1, Duration::from_secs(5),));

    let updates = reporter.updates();
    let last_update = updates.last().cloned().expect("update");
    assert!(
        last_update
            .matches
            .iter()
            .any(|file_match| file_match.path.to_string_lossy().contains("beta.txt"))
    );
}

#[test]
fn session_emits_complete_when_query_changes_with_no_matches() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("alpha.txt"), "alpha").unwrap();
    fs::write(dir.path().join("beta.txt"), "beta").unwrap();
    let reporter = Arc::new(RecordingReporter::default());
    let session = create_session(
        vec![dir.path().to_path_buf()],
        FileSearchOptions::default(),
        reporter.clone(),
        /*cancel_flag*/ None,
    )
    .expect("session");

    session.update_query("asdf");
    assert!(reporter.wait_for_complete(Duration::from_secs(5)));

    let completed_snapshot = reporter.snapshot();
    assert_eq!(completed_snapshot.matches, Vec::new());
    assert_eq!(completed_snapshot.total_match_count, 0);

    reporter.clear();

    session.update_query("asdfa");
    assert!(reporter.wait_for_complete(Duration::from_secs(5)));
    assert!(!reporter.updates().is_empty());
}

#[test]
fn dropping_session_does_not_cancel_siblings_with_shared_cancel_flag() {
    let root_a = create_temp_tree(/*file_count*/ 200);
    let root_b = create_temp_tree(/*file_count*/ 4_000);
    let cancel_flag = Arc::new(AtomicBool::new(false));

    let reporter_a = Arc::new(RecordingReporter::default());
    let session_a = create_session(
        vec![root_a.path().to_path_buf()],
        FileSearchOptions::default(),
        reporter_a,
        Some(cancel_flag.clone()),
    )
    .expect("session_a");

    let reporter_b = Arc::new(RecordingReporter::default());
    let session_b = create_session(
        vec![root_b.path().to_path_buf()],
        FileSearchOptions::default(),
        reporter_b.clone(),
        Some(cancel_flag),
    )
    .expect("session_b");

    session_a.update_query("file-0");
    session_b.update_query("file-1");

    thread::sleep(Duration::from_millis(5));
    drop(session_a);

    let completed = reporter_b.wait_for_complete(Duration::from_secs(5));
    assert_eq!(completed, true);
}

#[test]
fn session_emits_updates_when_query_changes() {
    let dir = create_temp_tree(/*file_count*/ 200);
    let reporter = Arc::new(RecordingReporter::default());
    let session = create_session(
        vec![dir.path().to_path_buf()],
        FileSearchOptions::default(),
        reporter.clone(),
        /*cancel_flag*/ None,
    )
    .expect("session");

    session.update_query("zzzzzzzz");
    let completed = reporter.wait_for_complete(Duration::from_secs(5));
    assert!(completed);

    reporter.clear();

    session.update_query("zzzzzzzzq");
    let completed = reporter.wait_for_complete(Duration::from_secs(5));
    assert!(completed);

    let updates = reporter.updates();
    assert_eq!(updates.len(), 1);
}

#[test]
fn run_returns_matches_for_query() {
    let dir = create_temp_tree(/*file_count*/ 40);
    let options = FileSearchOptions {
        limit: NonZero::new(20).unwrap(),
        exclude: Vec::new(),
        threads: NonZero::new(2).unwrap(),
        compute_indices: false,
        respect_gitignore: true,
    };
    let results = run(
        "file-000",
        vec![dir.path().to_path_buf()],
        options,
        /*cancel_flag*/ None,
    )
    .expect("run ok");

    assert!(!results.matches.is_empty());
    assert!(results.total_match_count >= results.matches.len());
    assert!(
        results
            .matches
            .iter()
            .any(|m| m.path.to_string_lossy().contains("file-0000.txt"))
    );
}

#[test]
fn run_returns_directory_matches_for_query() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("docs/guides")).unwrap();
    fs::write(dir.path().join("docs/guides/intro.md"), "intro").unwrap();
    fs::write(dir.path().join("docs/readme.md"), "readme").unwrap();

    let results = run(
        "guides",
        vec![dir.path().to_path_buf()],
        FileSearchOptions {
            limit: NonZero::new(20).unwrap(),
            exclude: Vec::new(),
            threads: NonZero::new(2).unwrap(),
            compute_indices: false,
            respect_gitignore: true,
        },
        /*cancel_flag*/ None,
    )
    .expect("run ok");

    assert!(results.matches.iter().any(|m| {
        m.path == std::path::Path::new("docs").join("guides")
            && m.match_type == MatchType::Directory
    }));
}

#[cfg(unix)]
#[test]
fn run_classifies_followed_directory_symlink_as_directory() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("guides")).unwrap();
    symlink(dir.path().join("guides"), dir.path().join("guides-link")).unwrap();

    let results = run(
        "guides-link",
        vec![dir.path().to_path_buf()],
        FileSearchOptions::default(),
        /*cancel_flag*/ None,
    )
    .expect("run ok");

    assert!(results.matches.iter().any(|file_match| {
        file_match.path == Path::new("guides-link") && file_match.match_type == MatchType::Directory
    }));
}

#[test]
fn cancel_exits_run() {
    let dir = create_temp_tree(/*file_count*/ 200);
    let cancel_flag = Arc::new(AtomicBool::new(true));
    let search_dir = dir.path().to_path_buf();
    let options = FileSearchOptions {
        compute_indices: false,
        ..Default::default()
    };
    let (tx, rx) = std::sync::mpsc::channel();

    let handle = thread::spawn(move || {
        let result = run("file-", vec![search_dir], options, Some(cancel_flag));
        let _ = tx.send(result);
    });

    let result = rx
        .recv_timeout(Duration::from_secs(2))
        .expect("run should exit after cancellation");
    handle.join().unwrap();

    let results = result.expect("run ok");
    assert_eq!(results.matches, Vec::new());
    assert_eq!(results.total_match_count, 0);
}

/// Regression test for #3493: a parent directory's `.gitignore` with `*`
/// must not suppress files discovered inside a child "repo" directory.
///
/// The fixture intentionally omits `git init` so that no `.git` directory
/// exists. With `require_git(true)`, the walker skips all gitignore
/// processing, making the parent's broad ignore harmless.
#[test]
fn parent_gitignore_outside_repo_does_not_hide_repo_files() {
    let temp = tempfile::tempdir().unwrap();
    let parent = temp.path().join("home");
    let repo = parent.join("repo");
    fs::create_dir_all(repo.join(".vscode")).unwrap();

    fs::write(parent.join(".gitignore"), "*\n!.gitignore\n").unwrap();
    fs::write(
        repo.join(".gitignore"),
        ".vscode/*\n!.vscode/\n!.vscode/settings.json\n!package.json\n",
    )
    .unwrap();
    fs::write(repo.join("package.json"), "{ \"name\": \"demo\" }\n").unwrap();
    fs::write(repo.join(".vscode/settings.json"), "{ \"editor\": true }\n").unwrap();

    let respect_results = run(
        "package",
        vec![repo.clone()],
        FileSearchOptions {
            limit: NonZero::new(20).unwrap(),
            exclude: Vec::new(),
            threads: NonZero::new(2).unwrap(),
            compute_indices: false,
            respect_gitignore: true,
        },
        /*cancel_flag*/ None,
    )
    .expect("run ok");
    assert!(
        respect_results
            .matches
            .iter()
            .any(|m| m.path.as_path() == Path::new("package.json"))
    );

    let nested_file_results = run(
        "settings",
        vec![repo],
        FileSearchOptions {
            limit: NonZero::new(20).unwrap(),
            exclude: Vec::new(),
            threads: NonZero::new(2).unwrap(),
            compute_indices: false,
            respect_gitignore: true,
        },
        /*cancel_flag*/ None,
    )
    .expect("run ok");
    assert!(
        nested_file_results
            .matches
            .iter()
            .any(|m| m.path.as_path() == Path::new(".vscode/settings.json"))
    );
}

#[test]
fn git_repo_still_respects_local_gitignore_when_enabled() {
    let temp = tempfile::tempdir().unwrap();
    let parent = temp.path().join("home");
    let repo = parent.join("repo");
    fs::create_dir_all(repo.join(".vscode")).unwrap();

    fs::write(parent.join(".gitignore"), "*\n!.gitignore\n").unwrap();
    fs::write(
        repo.join(".gitignore"),
        ".vscode/*\n!.vscode/\n!.vscode/settings.json\n!package.json\n",
    )
    .unwrap();
    fs::write(repo.join("package.json"), "{ \"name\": \"demo\" }\n").unwrap();
    fs::write(repo.join(".vscode/settings.json"), "{ \"editor\": true }\n").unwrap();
    fs::write(
        repo.join(".vscode/extensions.json"),
        "{ \"extensions\": [] }\n",
    )
    .unwrap();

    fs::create_dir_all(repo.join(".git")).unwrap();

    let package_results = run(
        "package",
        vec![repo.clone()],
        FileSearchOptions {
            limit: NonZero::new(20).unwrap(),
            exclude: Vec::new(),
            threads: NonZero::new(2).unwrap(),
            compute_indices: false,
            respect_gitignore: true,
        },
        /*cancel_flag*/ None,
    )
    .expect("run ok");
    assert!(
        package_results
            .matches
            .iter()
            .any(|m| m.path.as_path() == Path::new("package.json"))
    );

    let ignored_results = run(
        "extensions.json",
        vec![repo.clone()],
        FileSearchOptions {
            limit: NonZero::new(20).unwrap(),
            exclude: Vec::new(),
            threads: NonZero::new(2).unwrap(),
            compute_indices: false,
            respect_gitignore: true,
        },
        /*cancel_flag*/ None,
    )
    .expect("run ok");
    assert!(
        !ignored_results
            .matches
            .iter()
            .any(|m| m.path.as_path() == Path::new(".vscode/extensions.json"))
    );

    let whitelisted_results = run(
        "settings.json",
        vec![repo],
        FileSearchOptions {
            limit: NonZero::new(20).unwrap(),
            exclude: Vec::new(),
            threads: NonZero::new(2).unwrap(),
            compute_indices: false,
            respect_gitignore: true,
        },
        /*cancel_flag*/ None,
    )
    .expect("run ok");
    assert!(
        whitelisted_results
            .matches
            .iter()
            .any(|m| m.path.as_path() == Path::new(".vscode/settings.json"))
    );
}

#[path = "matcher_tests.rs"]
mod matcher_tests;
