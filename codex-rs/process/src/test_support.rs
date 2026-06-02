use std::any::Any;
use std::process::Command;
use std::time::Duration;

const SUBPROCESS_MODE_ENV: &str = "CODEX_PROCESS_TEST_SUBPROCESS_MODE";

pub(crate) const STDOUT_TEXT: &str = "managed stdout";
pub(crate) const STDERR_TEXT: &str = "managed stderr";

pub(crate) fn command(mode: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().expect("current test binary"));
    command
        .arg("--exact")
        .arg("test_support::subprocess_helper")
        .arg("--ignored")
        .arg("--nocapture")
        .env(SUBPROCESS_MODE_ENV, mode);
    command
}

pub(crate) fn panic_message(payload: &(dyn Any + Send)) -> &str {
    if let Some(message) = payload.downcast_ref::<&str>() {
        message
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message
    } else {
        "non-string panic payload"
    }
}

#[test]
#[ignore]
fn subprocess_helper() {
    match std::env::var(SUBPROCESS_MODE_ENV).as_deref() {
        Ok("exit-success") => {}
        Ok("output") => {
            println!("{STDOUT_TEXT}");
            eprintln!("{STDERR_TEXT}");
        }
        Ok("sleep") => std::thread::sleep(Duration::from_secs(60)),
        Ok(mode) => panic!("unsupported subprocess mode: {mode}"),
        Err(error) => panic!("missing subprocess mode: {error}"),
    }
}
