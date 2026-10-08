//! Verify startup acknowledgment, quit without saving, and restart suppression.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn gov_cloud_startup_persists_only_acknowledgment() -> Result<()> {
    let cwd = codex_utils_cargo_bin::repo_root()?;
    let args = [
        "--no-daemon",
        "-c",
        "features.api_key_model_discovery=false",
    ];
    for action in ["quit", "acknowledge"] {
        let home = tempfile::tempdir()?;
        write_test_config(home.path(), &cwd)?;
        let path = home.path().join("config.toml");
        let config = std::fs::read_to_string(&path)?.replace(
            "model_provider = \"openai\"",
            "model_provider = \"amazon-bedrock\"",
        );
        let config = format!(
            "{config}\n[model_providers.amazon-bedrock]\nbase_url = 'https://bedrock-mantle.us-gov-west-1.api.aws/openai/v1'\n"
        );
        std::fs::write(&path, &config)?;
        let mut terminal = PtyCodex::start(&cwd, home, &args)?;
        terminal.wait_for_screen("Using Codex with AWS GovCloud")?;
        let original = std::fs::read_to_string(&path)?;
        let deadline = Instant::now() + STARTUP_TIMEOUT;
        loop {
            // Startup drains pending keys after drawing the guidance; retry until it handles one.
            terminal.write_input(if action == "quit" { b"q" } else { b"\r" })?;
            terminal.read_output(Duration::from_millis(/*millis*/ 50))?;
            if action == "quit" {
                if let Some(status) = terminal.child.try_wait()? {
                    ensure!(status.success(), "quit exited with {status}");
                    break;
                }
            } else if terminal.screen_contains("OpenAI Codex") {
                break;
            }
            ensure!(Instant::now() < deadline, "{action} did not complete");
        }
        let saved = std::fs::read_to_string(&path)?;
        if action == "acknowledge" {
            let config: toml::Value = toml::from_str(&saved)?;
            assert_eq!(
                config["notice"]["hide_gov_cloud_guidance"].as_bool(),
                Some(true)
            );
            let home = std::mem::replace(&mut terminal._codex_home, tempfile::tempdir()?);
            drop(terminal);
            let mut restarted = PtyCodex::start(&cwd, home, &args)?;
            // The startup draft says "loading"; the actual model appears after guidance.
            restarted.wait_for_screen("gpt-5.6-terra")?;
            assert!(!restarted.screen_contains("Using Codex with AWS GovCloud"));
        } else {
            assert_eq!(saved, original);
        }
    }
    Ok(())
}
