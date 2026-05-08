#[cfg(target_os = "windows")]
fn main() -> anyhow::Result<()> {
    codex_windows_sandbox::setup_main()
}

#[cfg(not(target_os = "windows"))]
fn main() {
    panic!("codex-windows-sandbox-setup is Windows-only");
}
