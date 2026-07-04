use super::*;
use crate::app::test_support::make_test_app;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn retirement_tombstones_are_bounded() {
    let mut app = make_test_app().await;

    for _ in 0..=RETIRED_THREAD_ID_CAPACITY {
        app.retire_thread(ThreadId::new());
    }

    assert_eq!(app.retired_thread_ids.len(), RETIRED_THREAD_ID_CAPACITY);
}
