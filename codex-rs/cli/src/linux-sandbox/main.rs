#[cfg(not(target_os = "linux"))]
fn main() -> anyhow::Result<()> {
    eprintln!("codex-linux-sandbox is not supported on this platform.");
    std::process::exit(1);
}

#[cfg(target_os = "linux")]
fn main() -> anyhow::Result<()> {
    use clap::Parser;
    use codex_cli::LandlockCommand;
    use codex_cli::create_sandbox_policy;
    use codex_cli::landlock;

    let LandlockCommand {
        full_auto,
        sandbox,
        command,
    } = LandlockCommand::parse();
    let sandbox_policy = create_sandbox_policy(full_auto, sandbox);
    landlock::run_landlock(command, sandbox_policy)?;
    Ok(())
}
