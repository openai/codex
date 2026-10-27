#[test]
fn plugin_arguments_are_parsed_before_startup() -> Result<(), Box<dyn std::error::Error>> {
    let program = codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")?;
    for arguments in [
        ["--help", "--codex-ui-plugin-process"],
        ["--codex-ui-plugin-process", "--help"],
    ] {
        let output = codex_utils_process::background_command(&program)
            .args(arguments)
            .output()?;
        assert!(output.status.success());
        assert!(String::from_utf8(output.stdout)?.contains("Usage:"));
    }
    for arguments in [
        ["--unknown", "--codex-ui-plugin-process"],
        ["--codex-ui-plugin-process", "--unknown"],
        ["--listen=stdio", "--codex-ui-plugin-process"],
        ["--codex-ui-plugin-process", "--listen=stdio"],
    ] {
        let output = codex_utils_process::background_command(&program)
            .args(arguments)
            .output()?;
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8(output.stderr)?.contains("error:"));
    }
    Ok(())
}
