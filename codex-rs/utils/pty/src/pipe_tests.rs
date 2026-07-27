use super::*;

#[test]
fn process_fallback_interrupt_terminates_root() -> anyhow::Result<()> {
    let mut child = std::process::Command::new("ping.exe")
        .args(["-n", "60", "127.0.0.1"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let mut terminator = PipeChildTerminator {
        windows: WindowsChildTerminator::Process(child.id()),
    };

    terminator.signal(ProcessSignal::Interrupt)?;

    assert!(!child.wait()?.success());
    Ok(())
}
