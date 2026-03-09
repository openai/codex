use std::env;
use std::io::Write;
use std::process::Command;
use std::process::Stdio;
use std::thread;
use std::time::Duration;

const HOLD_STDOUT_OPEN_ARG: &str = "--hold-stdout-open";

fn main() -> anyhow::Result<()> {
    if env::args().nth(1).as_deref() == Some(HOLD_STDOUT_OPEN_ARG) {
        print!("ta");
        std::io::stdout().flush()?;
        thread::sleep(Duration::from_millis(30));
        print!("il");
        std::io::stdout().flush()?;
        thread::sleep(Duration::from_secs(1));
        return Ok(());
    }

    let current_exe = env::current_exe()?;
    Command::new(current_exe)
        .arg(HOLD_STDOUT_OPEN_ARG)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;

    Ok(())
}
