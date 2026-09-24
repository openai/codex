//! Verifies that widget fixtures own and clean up their isolated Codex homes.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn widget_homes_remain_isolated_until_their_owners_drop() {
    let (first, _first_events, _first_ops) = make_chatwidget_manual(/*model_override*/ None).await;
    let (second, _second_events, _second_ops) =
        make_chatwidget_manual(/*model_override*/ None).await;
    let first_home = first.config.codex_home.clone();
    let second_home = second.config.codex_home.clone();
    assert_ne!(first_home, second_home);

    std::fs::create_dir_all(first_home.join("log")).expect("first fixture directory");
    std::fs::write(first_home.join("log/test.log"), "first").expect("first fixture file");
    std::fs::write(second_home.join("test.log"), "second").expect("second fixture file");

    drop(first);
    assert!(
        !first_home.exists(),
        "first fixture should be removed recursively"
    );
    assert_eq!(
        std::fs::read_to_string(second_home.join("test.log")).expect("second fixture remains"),
        "second"
    );

    drop(second);
    assert!(!second_home.exists(), "second fixture should be removed");
}
