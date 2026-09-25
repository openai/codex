use super::*;
use pretty_assertions::assert_eq;
use std::time::Duration;

#[test]
fn text_preserves_whitespace_and_rejects_late_or_oversized_reads() {
    let deadline = Instant::now() + Duration::from_secs(/*secs*/ 5);
    assert_eq!(
        validate(" café\r\n\t".into(), deadline),
        Ok(" café\r\n\t".into())
    );
    assert!(validate("late".into(), Instant::now()).is_err());
    assert!(validate("x".repeat(MAX_USER_INPUT_TEXT_CHARS + 1), deadline).is_err());
}

#[cfg(unix)]
#[tokio::test]
async fn command_drains_large_output_and_bounds_waiting() {
    let mut command = tokio::process::Command::new("sh");
    command.args([
        "-c",
        "head -c 131072 /dev/zero | tr '\\000' x; printf '\\n'",
    ]);
    assert_eq!(
        read_command(command, Instant::now() + Duration::from_secs(/*secs*/ 5)).await,
        Ok(format!("{}\n", "x".repeat(/*n*/ 131072)))
    );
    let mut command = tokio::process::Command::new("sh");
    command.args(["-c", "exec sleep 10"]);
    assert_eq!(
        read_command(
            command,
            Instant::now() + Duration::from_millis(/*millis*/ 20)
        )
        .await,
        Err("clipboard read timed out".into())
    );
}
