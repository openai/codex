use std::sync::atomic::Ordering;

use pretty_assertions::assert_eq;
use tokio::sync::oneshot;

use super::*;

#[tokio::test]
async fn stale_end_does_not_release_a_newer_control_generation() {
    let browser = TerminalBrowser::discover();
    browser
        .inner
        .human_control
        .store(/*val*/ true, Ordering::SeqCst);
    browser
        .inner
        .human_control_generation
        .store(/*val*/ 2, Ordering::SeqCst);

    let error = browser
        .end_human_control(HumanControlToken { generation: 1 })
        .await
        .expect_err("stale control token should be rejected");

    let generation = browser
        .inner
        .human_control_generation
        .load(Ordering::SeqCst);
    assert_eq!(error.to_string(), "browser control transition was canceled");
    assert_eq!((browser.is_human_control_active(), generation), (true, 2));
}

#[tokio::test]
async fn release_discards_its_backlog_and_preserves_a_new_generation() {
    let (sender, mut receivers) = TerminalBrowser::human_input_channel();
    for _ in 1..HUMAN_INPUT_CAPACITY {
        sender
            .input
            .try_send(QueuedHumanInput {
                generation: 1,
                input: HumanInput::Text(String::new()),
            })
            .expect("normal input should fill its reserved queue");
    }
    sender
        .input
        .try_send(QueuedHumanInput {
            generation: 3,
            input: HumanInput::Text("new generation".to_string()),
        })
        .expect("new-generation input should enter the normal queue");
    let (completion_tx, _completion_rx) = oneshot::channel();
    sender
        .control
        .send(QueuedHumanInput {
            generation: 1,
            input: HumanInput::ReleaseMouseButtons {
                completion_tx,
                after_release: HumanControlAfterRelease::Continue,
            },
        })
        .await
        .expect("release should use its reserved queue");

    let mut deferred = VecDeque::new();
    let release = next_human_input(&mut receivers, &mut deferred)
        .await
        .expect("queued release input");
    drain_human_inputs_for_generation(&mut receivers, release.generation, &mut deferred);
    assert_eq!(deferred.front().map(|queued| queued.generation), Some(3));
    let (completion_tx, _completion_rx) = oneshot::channel();
    sender
        .control
        .send(QueuedHumanInput {
            generation: 3,
            input: HumanInput::ReleaseMouseButtons {
                completion_tx,
                after_release: HumanControlAfterRelease::Continue,
            },
        })
        .await
        .expect("new-generation release should use its reserved queue");
    let release = next_human_input(&mut receivers, &mut deferred)
        .await
        .expect("queued new-generation release");
    drain_human_inputs_for_generation(&mut receivers, release.generation, &mut deferred);

    assert!(matches!(
        release.input,
        HumanInput::ReleaseMouseButtons { .. }
    ));
    assert!(deferred.is_empty());
}

#[test]
fn ending_release_deactivates_before_acknowledgement() {
    let browser = TerminalBrowser::discover();
    browser
        .inner
        .human_control
        .store(/*val*/ true, Ordering::SeqCst);
    browser
        .inner
        .human_control_generation
        .store(/*val*/ 1, Ordering::SeqCst);

    finish_human_control_after_release(
        &browser.inner,
        /*generation*/ 1,
        HumanControlAfterRelease::End,
    )
    .expect("ending release should deactivate its generation");

    let generation = browser
        .inner
        .human_control_generation
        .load(Ordering::SeqCst);
    assert_eq!((browser.is_human_control_active(), generation), (false, 2));
}
