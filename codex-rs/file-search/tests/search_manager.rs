use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_file_search::SearchItem;
use codex_file_search::SearchManager;

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

    let limit = std::num::NonZero::new(10).unwrap();
    let threads = std::num::NonZero::new(1).unwrap();

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
