#[path = "../setup_main_win.rs"]
mod win;

#[cfg(target_os = "windows")]
fn main() -> anyhow::Result<()> {
    win::main()
// windows-sandbox-rs/src/bin/setup_main.rs
}

#[cfg(not(target_os = "windows"))]
fn main() {
    panic!("codex-windows-sandbox-setup is Windows-only");
}
