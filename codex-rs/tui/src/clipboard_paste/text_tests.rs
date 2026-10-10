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
    use std::process::Stdio;

    let mut child = tokio::process::Command::new("sh")
        .args([
            "-c",
            // Two distinct framed clipboard results, only in response to separate requests.
            r#"read -r request; printf '\001\000\002\000'; head -c 131072 /dev/zero | tr '\000' x; printf '\n'; read -r request; printf '\010\000\000\000 café\r\n'"#,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .kill_on_drop(/*kill_on_drop*/ true)
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(/*secs*/ 5);
    assert_eq!(
        read_command(&mut child, deadline).await,
        Ok(format!("{}\n", "x".repeat(/*n*/ 131072)))
    );
    assert_eq!(
        read_command(&mut child, deadline).await,
        Ok(" café\r\n".into())
    );
    let mut child = tokio::process::Command::new("sh")
        .args(["-c", "exec sleep 10"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .kill_on_drop(/*kill_on_drop*/ true)
        .spawn()
        .unwrap();
    assert_eq!(
        read_command(
            &mut child,
            Instant::now() + Duration::from_millis(/*millis*/ 20)
        )
        .await,
        Err("clipboard read timed out".into())
    );
}
