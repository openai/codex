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
    use codex_core::config::Config;
    use codex_core::config::ConfigOverrides;

    let LandlockCommand {
        full_auto,
        sandbox,
        command,
    } = LandlockCommand::parse();
    let sandbox_policy = create_sandbox_policy(full_auto, sandbox);
    let config = Config::load_with_overrides(ConfigOverrides {
        sandbox_policy: Some(sandbox_policy),
        ..Default::default()
    })?;
    landlock::run_landlock(command, &config)?;
    Ok(())
}
