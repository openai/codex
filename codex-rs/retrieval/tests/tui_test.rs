//! Integration tests for the retrieval TUI.
//!
//! Tests the TUI app state machine and event handling.

use std::time::Duration;
use std::time::Instant;

use codex_retrieval::config::RetrievalConfig;
use codex_retrieval::tui::app_event::AppEvent;
use codex_retrieval::tui::app_event::ViewMode;

// Note: We can't directly test App because it requires terminal access.
// These tests verify the public API and event types work correctly.

#[test]
fn test_view_mode_navigation() {
    // Test that view mode navigation works correctly
    assert_eq!(ViewMode::Search.next(), ViewMode::Index);
    assert_eq!(ViewMode::Index.next(), ViewMode::RepoMap);
    assert_eq!(ViewMode::RepoMap.next(), ViewMode::Watch);
    assert_eq!(ViewMode::Watch.next(), ViewMode::Debug);
    assert_eq!(ViewMode::Debug.next(), ViewMode::Search);

    assert_eq!(ViewMode::Search.prev(), ViewMode::Debug);
    assert_eq!(ViewMode::Debug.prev(), ViewMode::Watch);
}

#[test]
fn test_view_mode_indices() {
    assert_eq!(ViewMode::Search.index(), 0);
    assert_eq!(ViewMode::Index.index(), 1);
    assert_eq!(ViewMode::RepoMap.index(), 2);
    assert_eq!(ViewMode::Watch.index(), 3);
    assert_eq!(ViewMode::Debug.index(), 4);

    assert_eq!(ViewMode::from_index(0), Some(ViewMode::Search));
    assert_eq!(ViewMode::from_index(4), Some(ViewMode::Debug));
    assert_eq!(ViewMode::from_index(5), None);
}

#[test]
fn test_view_mode_names() {
    assert_eq!(ViewMode::Search.name(), "Search");
    assert_eq!(ViewMode::Index.name(), "Index");
    assert_eq!(ViewMode::RepoMap.name(), "RepoMap");
    assert_eq!(ViewMode::Watch.name(), "Watch");
    assert_eq!(ViewMode::Debug.name(), "Debug");
}

#[test]
fn test_app_event_variants() {
    // Verify that all event variants can be created
    let _quit = AppEvent::Quit;
    let _tick = AppEvent::Tick;
    let _next_tab = AppEvent::NextTab;
    let _prev_tab = AppEvent::PrevTab;
    let _switch = AppEvent::SwitchView(ViewMode::Search);
    let _start_build = AppEvent::StartBuild { clean: false };
    let _stop_build = AppEvent::StopBuild;
    let _build_error = AppEvent::BuildError {
        error: "test error".to_string(),
    };
    let _build_cancelled = AppEvent::BuildCancelled;
    let _repomap_error = AppEvent::RepoMapError {
        error: "test error".to_string(),
    };
    let _watch_error = AppEvent::WatchError {
        error: "test error".to_string(),
    };
    let _search_error = AppEvent::SearchError {
        query_id: "q1".to_string(),
        error: "test error".to_string(),
    };
}

#[test]
fn test_config_has_required_fields() {
    // Test that RetrievalConfig has the fields needed by TUI
    let config = RetrievalConfig::default();

    // These should exist and have sensible defaults
    assert!(config.search.n_final > 0);
    assert!(config.indexing.watch_debounce_ms >= 0);
}

#[test]
fn test_debounce_logic() {
    // Simulate the debouncing logic used in the search handler
    const DEBOUNCE_MS: u64 = 200;

    let mut last_search_time: Option<Instant> = None;

    // First search should always succeed
    let can_search_1 = last_search_time
        .map(|t| t.elapsed() >= Duration::from_millis(DEBOUNCE_MS))
        .unwrap_or(true);
    assert!(can_search_1);
    last_search_time = Some(Instant::now());

    // Immediate second search should be blocked
    let can_search_2 = last_search_time
        .map(|t| t.elapsed() >= Duration::from_millis(DEBOUNCE_MS))
        .unwrap_or(true);
    assert!(!can_search_2);

    // After waiting, search should succeed
    std::thread::sleep(Duration::from_millis(DEBOUNCE_MS + 10));
    let can_search_3 = last_search_time
        .map(|t| t.elapsed() >= Duration::from_millis(DEBOUNCE_MS))
        .unwrap_or(true);
    assert!(can_search_3);
}
