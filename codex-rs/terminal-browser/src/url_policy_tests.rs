use pretty_assertions::assert_eq;
use serde_json::json;

use super::*;

#[test]
fn observes_only_main_frame_navigation_urls() {
    let main_frame = json!({
        "method": "Page.frameNavigated",
        "params": { "frame": { "id": "main", "url": "https://example.test/next" } }
    });
    let child_frame = json!({
        "method": "Page.frameNavigated",
        "params": {
            "frame": {
                "id": "child",
                "parentId": "main",
                "url": "https://example.test/frame"
            }
        }
    });

    assert_eq!(
        observed_main_frame_url(&main_frame),
        Some("https://example.test/next")
    );
    assert_eq!(observed_main_frame_url(&child_frame), None);
}

#[test]
fn recognizes_page_load_events_for_metadata_refresh() {
    assert!(is_page_load_event(&json!({
        "method": "Page.loadEventFired",
        "params": { "timestamp": 1 }
    })));
    assert!(!is_page_load_event(&json!({
        "method": "Page.domContentEventFired",
        "params": { "timestamp": 1 }
    })));
}
