#[path = "../setup_main_win.rs"]
mod win;

#[cfg(target_os = "windows")]
fn main() -> anyhow::Result<()> {
    win::main()
}

#[cfg(not(target_os = "windows"))]
fn main() {
    panic!("codex-windows-sandbox-setup is Windows-only");
}
