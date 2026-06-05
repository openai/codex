use std::collections::BTreeMap;
use std::time::Duration;

use pretty_assertions::assert_eq;
use tokio::time;

use super::*;

fn metric_tags_map(tags: Vec<(&'static str, String)>) -> BTreeMap<&'static str, String> {
    tags.into_iter().collect()
}

#[tokio::test]
async fn active_time_timeout_pauses_while_elicitation_is_pending() {
    let pause_state = ElicitationPauseState::new();
    let pause = pause_state.enter();
    tokio::spawn(async move {
        time::sleep(Duration::from_millis(75)).await;
        drop(pause);
    });

    let result = active_time_timeout(Duration::from_millis(50), pause_state.subscribe(), async {
        time::sleep(Duration::from_millis(90)).await;
        "done"
    })
    .await;

    assert_eq!(Ok("done"), result);
}

#[test]
fn initialize_metric_tags_record_success_after_retry() {
    let tags = metric_tags_map(initialize_metric_tags(
        "streamable_http",
        "success",
        /*attempts*/ 2,
        /*retry_exhausted*/ false,
        "none",
    ));

    assert_eq!(tags["transport"], "streamable_http");
    assert_eq!(tags["outcome"], "success");
    assert_eq!(tags["retried"], "true");
    assert_eq!(tags["attempts"], "2");
    assert_eq!(tags["retry_count"], "1");
    assert_eq!(tags["retry_exhausted"], "false");
    assert_eq!(tags["failure_kind"], "none");
}

#[test]
fn initialize_metric_tags_record_retry_exhaustion() {
    let tags = metric_tags_map(initialize_metric_tags(
        "streamable_http",
        "error",
        /*attempts*/ 3,
        /*retry_exhausted*/ true,
        "retry_exhausted",
    ));

    assert_eq!(tags["transport"], "streamable_http");
    assert_eq!(tags["outcome"], "error");
    assert_eq!(tags["retried"], "true");
    assert_eq!(tags["attempts"], "3");
    assert_eq!(tags["retry_count"], "2");
    assert_eq!(tags["retry_exhausted"], "true");
    assert_eq!(tags["failure_kind"], "retry_exhausted");
}
