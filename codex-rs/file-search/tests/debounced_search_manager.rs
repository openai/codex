use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use std::time::Instant;

use codex_file_search::FileSearchResults;
use codex_file_search::search_manager::DebounceConfig;
use codex_file_search::search_manager::DebouncedSearchManager;
use pretty_assertions::assert_eq;

#[test]
fn debounced_search_manager_emits_results() {
    let temp_dir = tempfile::tempdir().unwrap();
    let nested_dir = temp_dir.path().join("src");
    std::fs::create_dir_all(&nested_dir).unwrap();
    std::fs::write(nested_dir.join("gamma.rs"), "fn main() {}").unwrap();

    let captured: Arc<Mutex<Vec<(String, FileSearchResults, bool)>>> =
        Arc::new(Mutex::new(Vec::new()));
    let captured_clone = Arc::clone(&captured);

    let callback = Arc::new(
        move |query: String, results: FileSearchResults, running: bool| {
            let mut guard = captured_clone
                .lock()
                .expect("captured results mutex poisoned");
            guard.push((query, results, running));
        },
    );

    let limit = NonZeroUsize::new(10).unwrap();
    let threads = NonZeroUsize::new(2).unwrap();
    let manager = DebouncedSearchManager::new(
        temp_dir.path().to_path_buf(),
        limit,
        threads,
        false,
        Vec::new(),
        callback,
        DebounceConfig::default(),
    );

    manager.on_query("gam".to_string());

    let start = Instant::now();
    let mut saw_match = false;

    while start.elapsed() < Duration::from_secs(2) {
        {
            let guard = captured
                .lock()
                .expect("captured results mutex poisoned while reading");
            if guard
                .iter()
                .any(|(_, results, _)| results.matches.iter().any(|m| m.path.ends_with("gamma.rs")))
            {
                saw_match = true;
                break;
            }
        }
        thread::sleep(Duration::from_millis(10));
    }

    if !saw_match {
        let guard = captured
            .lock()
            .expect("captured results mutex poisoned at end of test");
        eprintln!("captured debounced results: {guard:?}");
    }

    assert!(saw_match, "debounced search did not emit expected result");
}

#[test]
fn debounced_search_manager_backspace_updates_results() {
    let temp_dir = tempfile::tempdir().unwrap();
    let nested_dir = temp_dir.path().join("src");
    std::fs::create_dir_all(&nested_dir).unwrap();
    std::fs::write(nested_dir.join("alpha.rs"), "fn alpha() {}").unwrap();
    std::fs::write(nested_dir.join("alpine.rs"), "fn alpine() {}").unwrap();
    std::fs::write(nested_dir.join("beta.rs"), "fn beta() {}").unwrap();

    let captured: Arc<Mutex<Vec<(String, FileSearchResults, bool)>>> =
        Arc::new(Mutex::new(Vec::new()));
    let captured_clone = Arc::clone(&captured);

    let callback = Arc::new(
        move |query: String, results: FileSearchResults, running: bool| {
            let mut guard = captured_clone
                .lock()
                .expect("captured results mutex poisoned");
            guard.push((query, results, running));
        },
    );

    let limit = NonZeroUsize::new(10).unwrap();
    let threads = NonZeroUsize::new(2).unwrap();
    let manager = DebouncedSearchManager::new(
        temp_dir.path().to_path_buf(),
        limit,
        threads,
        false,
        Vec::new(),
        callback,
        DebounceConfig::default(),
    );

    manager.on_query("alph".to_string());

    let alpha_paths =
        wait_for_query_paths_matching(&captured, "alph", Duration::from_secs(2), |paths| {
            paths.iter().any(|path| path.ends_with("alpha.rs"))
        })
        .expect("timed out waiting for alph results");
    assert_eq!(
        alpha_paths,
        vec!["src/alpha.rs".to_string()],
        "expected only alpha for query 'alph'"
    );

    manager.on_query("al".to_string());

    let mut backspace_paths =
        wait_for_query_paths_matching(&captured, "al", Duration::from_secs(2), |paths| {
            paths.iter().any(|path| path.ends_with("alpha.rs"))
                && paths.iter().any(|path| path.ends_with("alpine.rs"))
        })
        .expect("timed out waiting for backspace results");
    backspace_paths.sort();
    assert_eq!(
        backspace_paths,
        vec!["src/alpha.rs".to_string(), "src/alpine.rs".to_string()],
        "expected backspace to include both alpha and alpine"
    );
}

fn wait_for_query_paths_matching(
    captured: &Arc<Mutex<Vec<(String, FileSearchResults, bool)>>>,
    query: &str,
    timeout: Duration,
    predicate: impl Fn(&[String]) -> bool,
) -> Option<Vec<String>> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        {
            let guard = captured
                .lock()
                .expect("captured results mutex poisoned while reading");
            if let Some((_, results, _)) = guard.iter().rev().find(|(q, _, _)| q == query) {
                let paths: Vec<String> = results.matches.iter().map(|m| m.path.clone()).collect();
                if predicate(&paths) {
                    return Some(paths);
                }
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
    None
}
