#![cfg(test)]

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn unconfirmed_copy_reveals_path_and_feedback_expires() {
    let mut actions = PathActions::default();
    actions.copy_result(&Ok(CopyStatus::Pending(7)));
    actions.finish_copy(&(7, Ok(CopyStatus::Unconfirmed)), /*visible*/ true);
    assert_eq!(actions.scroll, Some(0));

    let frames = FrameRequester::test_dummy();
    assert_eq!(
        actions.feedback_hint(&frames).map(|line| line.to_string()),
        Some("Copy unconfirmed".to_owned()),
    );

    let Some(CopyFeedback::Finished { expires_at, .. }) = &mut actions.feedback else {
        panic!("copy completed");
    };
    *expires_at = Instant::now();
    assert_eq!(actions.feedback_hint(&frames), None);
}

#[test]
fn dismissal_keeps_copy_feedback_without_reopening_the_path() {
    let mut actions = PathActions {
        scroll: Some(0),
        ..Default::default()
    };
    actions.copy_result(&Ok(CopyStatus::Pending(7)));
    actions.dismiss();
    actions.finish_copy(&(7, Ok(CopyStatus::Unconfirmed)), /*visible*/ true);
    assert_eq!(actions.scroll, None);
    assert!(matches!(
        actions.feedback,
        Some(CopyFeedback::Finished {
            message: "Copy unconfirmed",
            ..
        })
    ));
}
