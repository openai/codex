use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_file_search::search_manager::SearchItem;
use codex_file_search::search_manager::SearchManager;

fn push(injector: &nucleo::Injector<SearchItem>, path: &str) {
    injector.push(
        SearchItem {
            path: path.to_string(),
        },
        |item, columns| {
            columns[0] = item.path.as_str().into();
        },
    );
}

#[test]
fn search_manager_streams_results() {
    let temp_dir = tempfile::tempdir().unwrap();

    let tick_counter = Arc::new(AtomicUsize::new(0));
    let notify_counter = Arc::clone(&tick_counter);
    let notify = Arc::new(move || {
        notify_counter.fetch_add(1, Ordering::Relaxed);
    });

    let limit = NonZeroUsize::new(10).unwrap();
    let threads = NonZeroUsize::new(2).unwrap();

    let mut manager = SearchManager::new(
        "g",
        limit,
        temp_dir.path(),
        Vec::new(),
        threads,
        false,
        notify,
    )
    .unwrap();

    let injector = manager.injector();

    push(&injector, "alpha.txt");
    manager.tick(Duration::from_millis(10));
    assert!(manager.current_results().matches.is_empty());

    push(&injector, "subdir/gamma.rs");
    for _ in 0..50 {
        let status = manager.tick(Duration::from_millis(10));
        if !status.running {
            break;
        }
    }

    let final_results = manager.current_results();
    assert!(
        final_results
            .matches
            .iter()
            .any(|m| m.path.ends_with("gamma.rs")),
        "expected to find gamma.rs; results={:?}",
        final_results
            .matches
            .iter()
            .map(|m| m.path.clone())
            .collect::<Vec<_>>()
    );
    assert!(tick_counter.load(Ordering::Relaxed) >= 2);
}

#[test]
fn search_manager_walk_finds_files() {
    let temp_dir = tempfile::tempdir().unwrap();
    let nested_dir = temp_dir.path().join("subdir");
    std::fs::create_dir_all(&nested_dir).unwrap();
    std::fs::write(nested_dir.join("gamma.rs"), "fn main() {}").unwrap();
    std::fs::write(temp_dir.path().join("alpha.txt"), "alpha").unwrap();

    let notify_flag = Arc::new(AtomicUsize::new(0));
    let notify_counter = Arc::clone(&notify_flag);
    let notify = Arc::new(move || {
        notify_counter.fetch_add(1, Ordering::Relaxed);
    });

    let limit = NonZeroUsize::new(10).unwrap();
    let threads = NonZeroUsize::new(1).unwrap();

    let mut manager = SearchManager::new(
        "gam",
        limit,
        temp_dir.path(),
        Vec::new(),
        threads,
        true,
        notify,
    )
    .unwrap();

    let start = std::time::Instant::now();
    let mut found = false;

    loop {
        let status = manager.tick(Duration::from_millis(20));
        let _ = notify_flag.swap(0, Ordering::AcqRel);
        let results = manager.current_results();
        if results.matches.iter().any(|m| m.path.ends_with("gamma.rs")) {
            found = true;
            break;
        }
        if !status.running && start.elapsed() > Duration::from_secs(1) {
            break;
        }
        if start.elapsed() > Duration::from_secs(5) {
            break;
        }
    }

    assert!(
        found,
        "expected walker to find gamma.rs; matches: {:?}",
        manager
            .current_results()
            .matches
            .iter()
            .map(|m| m.path.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn search_manager_walk_includes_directories() {
    let temp_dir = tempfile::tempdir().unwrap();
    let docs_dir = temp_dir.path().join("docs");
    let nested_dir = docs_dir.join("nested");
    std::fs::create_dir_all(&nested_dir).unwrap();

    let notify_flag = Arc::new(AtomicUsize::new(0));
    let notify_counter = Arc::clone(&notify_flag);
    let notify = Arc::new(move || {
        notify_counter.fetch_add(1, Ordering::Relaxed);
    });

    let limit = NonZeroUsize::new(10).unwrap();
    let threads = NonZeroUsize::new(1).unwrap();

    let mut manager = SearchManager::new(
        "docs",
        limit,
        temp_dir.path(),
        Vec::new(),
        threads,
        false,
        notify,
    )
    .unwrap();

    let start = std::time::Instant::now();
    let mut found_directory = false;

    loop {
        let status = manager.tick(Duration::from_millis(20));
        let _ = notify_flag.swap(0, Ordering::AcqRel);
        let results = manager.current_results();
        if results.matches.iter().any(|m| m.path == "docs/") {
            found_directory = true;
            break;
        }
        if !status.running && start.elapsed() > Duration::from_secs(1) {
            break;
        }
        if start.elapsed() > Duration::from_secs(5) {
            break;
        }
    }

    assert!(
        found_directory,
        "expected walker to find directory matches; matches: {:?}",
        manager
            .current_results()
            .matches
            .iter()
            .map(|m| m.path.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
#[cfg(unix)]
fn search_manager_walk_follows_symlinked_directories() {
    let temp_dir = tempfile::tempdir().unwrap();
    let root_dir = temp_dir.path().join("root");
    std::fs::create_dir_all(&root_dir).unwrap();
    let target_dir = temp_dir.path().join("target");
    std::fs::create_dir_all(&target_dir).unwrap();
    std::fs::write(target_dir.join("gamma.rs"), "fn main() {}").unwrap();

    std::os::unix::fs::symlink(&target_dir, root_dir.join("link")).unwrap();

    let notify_flag = Arc::new(AtomicUsize::new(0));
    let notify_counter = Arc::clone(&notify_flag);
    let notify = Arc::new(move || {
        notify_counter.fetch_add(1, Ordering::Relaxed);
    });

    let limit = NonZeroUsize::new(10).unwrap();
    let threads = NonZeroUsize::new(1).unwrap();

    let mut manager =
        SearchManager::new("gamma", limit, &root_dir, Vec::new(), threads, true, notify).unwrap();

    let start = std::time::Instant::now();
    let mut found = false;

    loop {
        let status = manager.tick(Duration::from_millis(20));
        let _ = notify_flag.swap(0, Ordering::AcqRel);
        let results = manager.current_results();
        if results.matches.iter().any(|m| m.path == "link/gamma.rs") {
            found = true;
            break;
        }
        if !status.running && start.elapsed() > Duration::from_secs(1) {
            break;
        }
        if start.elapsed() > Duration::from_secs(5) {
            break;
        }
    }

    assert!(
        found,
        "expected walker to find symlinked file; matches: {:?}",
        manager
            .current_results()
            .matches
            .iter()
            .map(|m| m.path.clone())
            .collect::<Vec<_>>()
    );
}
