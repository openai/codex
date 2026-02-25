#[path = "../command_runner_win.rs"]
mod win;

#[cfg(target_os = "windows")]
fn main() -> anyhow::Result<()> {
    win::main()
}

#[cfg(not(target_os = "windows"))]
fn main() {
    panic!("helios-command-runner is Windows-only");
}
