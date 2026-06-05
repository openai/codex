use std::time::Duration;

use pretty_assertions::assert_eq;
use tokio::time;

use super::*;

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
