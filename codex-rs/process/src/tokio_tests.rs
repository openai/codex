use super::Child;
use super::CommandExt as _;
use crate::UNJOINED_CHILD_MESSAGE;
use crate::test_support;
use crate::test_support::STDERR_TEXT;
use crate::test_support::STDOUT_TEXT;
#[cfg(debug_assertions)]
use crate::test_support::panic_message;
use ::tokio as tokio_crate;
use std::ops::DerefMut;
#[cfg(debug_assertions)]
use std::panic::AssertUnwindSafe;
#[cfg(debug_assertions)]
use std::panic::catch_unwind;
use std::process::Stdio;
use std::time::Duration;
use tokio_crate::io::AsyncReadExt;
use tokio_crate::process::Command;
use tokio_crate::time::Instant;
use tokio_crate::time::sleep;
use tokio_crate::time::timeout;

#[tokio_crate::test]
async fn wait_disarms_bomb_and_can_be_repeated() {
    let mut child = command("exit-success")
        .spawn_managed()
        .expect("spawn helper");
    assert!(child.bomb.is_armed());

    let status = child.wait().await.expect("wait for helper");
    assert!(status.success());
    assert!(!child.bomb.is_armed());

    assert_eq!(child.wait().await.expect("repeat wait for helper"), status);
}

#[tokio_crate::test]
async fn stdio_is_available_through_deref_mut() {
    let mut child = command("output")
        .stdout(Stdio::piped())
        .spawn_managed()
        .expect("spawn helper");
    let mut stdout = child.stdout.take().expect("piped stdout");
    let mut output = String::new();
    stdout
        .read_to_string(&mut output)
        .await
        .expect("read stdout");

    assert!(child.wait().await.expect("wait for helper").success());
    assert!(output.contains(STDOUT_TEXT));
}

#[tokio_crate::test]
async fn try_wait_keeps_bomb_armed_until_status_is_available() {
    let mut child = command("sleep").spawn_managed().expect("spawn helper");

    assert_eq!(child.try_wait().expect("poll sleeping helper"), None);
    assert!(child.bomb.is_armed());

    child.start_kill().expect("kill sleeping helper");
    assert!(child.bomb.is_armed());
    assert!(
        !child
            .wait()
            .await
            .expect("wait for killed helper")
            .success()
    );
    assert!(!child.bomb.is_armed());
}

#[tokio_crate::test]
async fn try_wait_disarms_bomb_when_status_is_available() {
    let mut child = command("exit-success")
        .spawn_managed()
        .expect("spawn helper");
    let deadline = Instant::now() + Duration::from_secs(5);

    loop {
        if let Some(status) = child.try_wait().expect("poll helper") {
            assert!(status.success());
            assert!(!child.bomb.is_armed());
            return;
        }
        assert!(Instant::now() < deadline, "helper did not exit");
        sleep(Duration::from_millis(10)).await;
    }
}

#[tokio_crate::test]
async fn wait_with_output_collects_output() {
    let output = command("output")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn_managed()
        .expect("spawn helper")
        .wait_with_output()
        .await
        .expect("collect helper output");

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains(STDOUT_TEXT));
    assert!(String::from_utf8_lossy(&output.stderr).contains(STDERR_TEXT));
}

#[tokio_crate::test]
async fn kill_keeps_bomb_armed_until_explicit_wait() {
    let mut child = command("sleep").spawn_managed().expect("spawn helper");

    child.kill().await.expect("kill sleeping helper");
    assert!(child.bomb.is_armed());

    assert!(
        !child
            .wait()
            .await
            .expect("wait for killed helper")
            .success()
    );
    assert!(!child.bomb.is_armed());
}

#[tokio_crate::test]
async fn cancelled_wait_keeps_bomb_armed() {
    let mut child = command("sleep").spawn_managed().expect("spawn helper");

    assert!(
        timeout(Duration::from_millis(10), child.wait())
            .await
            .is_err()
    );
    assert!(child.bomb.is_armed());

    child.start_kill().expect("kill sleeping helper");
    assert!(
        !child
            .wait()
            .await
            .expect("wait for killed helper")
            .success()
    );
    assert!(!child.bomb.is_armed());
}

#[cfg(debug_assertions)]
#[tokio_crate::test]
async fn cancelled_wait_with_output_panics_when_dropped() {
    let mut command = command("sleep");
    command.kill_on_drop(true);
    let child = command.spawn_managed().expect("spawn helper");
    let mut wait = Box::pin(child.wait_with_output());

    assert!(
        timeout(Duration::from_millis(10), wait.as_mut())
            .await
            .is_err()
    );
    let panic = catch_unwind(AssertUnwindSafe(|| drop(wait)))
        .expect_err("dropping cancelled wait_with_output should panic");

    assert_eq!(panic_message(panic.as_ref()), UNJOINED_CHILD_MESSAGE);
}

#[cfg(not(debug_assertions))]
#[tokio_crate::test]
#[tracing_test::traced_test]
async fn cancelled_wait_with_output_logs_an_error_when_dropped() {
    let mut command = command("sleep");
    command.kill_on_drop(true);
    let child = command.spawn_managed().expect("spawn helper");
    let mut wait = Box::pin(child.wait_with_output());

    assert!(
        timeout(Duration::from_millis(10), wait.as_mut())
            .await
            .is_err()
    );
    drop(wait);

    assert!(logs_contain(UNJOINED_CHILD_MESSAGE));
}

#[cfg(debug_assertions)]
#[tokio_crate::test]
async fn dropping_unjoined_child_panics() {
    let mut child = command("sleep").spawn_managed().expect("spawn helper");
    clean_up_without_disarming(&mut child).await;

    let panic = catch_unwind(AssertUnwindSafe(|| drop(child)))
        .expect_err("dropping unjoined child should panic");

    assert_eq!(panic_message(panic.as_ref()), UNJOINED_CHILD_MESSAGE);
}

#[cfg(not(debug_assertions))]
#[tokio_crate::test]
#[tracing_test::traced_test]
async fn dropping_unjoined_child_logs_an_error() {
    let mut child = command("sleep").spawn_managed().expect("spawn helper");
    clean_up_without_disarming(&mut child).await;

    drop(child);

    assert!(logs_contain(UNJOINED_CHILD_MESSAGE));
}

fn command(mode: &str) -> Command {
    Command::from(test_support::command(mode))
}

async fn clean_up_without_disarming(child: &mut Child) {
    let child = DerefMut::deref_mut(child);
    child.start_kill().expect("kill sleeping helper");
    child.wait().await.expect("reap sleeping helper");
}
