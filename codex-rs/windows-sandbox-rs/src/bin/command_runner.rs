#[path = "../command_runner_win.rs"]
mod win;

#[cfg(target_os = "windows")]
fn main() -> anyhow::Result<()> {
    win::main()
// windows-sandbox-rs/src/bin/command_runner.rs
}

#[cfg(not(target_os = "windows"))]
fn main() {
    panic!("codex-command-runner is Windows-only");
}
