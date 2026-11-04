//! Checks that interactive startup persists detection without replacing a user preference.

use super::focus_palette::PtyCodex;
use super::focus_palette::write_test_config;
use anyhow::Result;
use pretty_assertions::assert_eq;
use std::time::Duration;
use std::time::Instant;

#[test]
fn startup_records_screen_reader_detection() -> Result<()> {
    let repo_root = codex_utils_cargo_bin::repo_root()?;
    let home = tempfile::tempdir()?;
    write_test_config(home.path(), &repo_root)?;
    let path = home.path().join("config.toml");
    let contents = std::fs::read_to_string(&path)?;
    std::fs::write(&path, format!("{contents}\n[tui]\nanimations = true\n"))?;
    let mut terminal = PtyCodex::start(&repo_root, home, &[])?;
    terminal.wait_for_startup()?;
    let deadline = Instant::now() + Duration::from_secs(/*secs*/ 30);
    loop {
        let config: toml::Value = toml::from_str(&std::fs::read_to_string(&path)?)?;
        if config["tui"].get("screen_reader_detection_done").is_some() {
            assert_eq!(
                config["tui"],
                toml::Value::Table(toml::toml! {
                    animations = true
                    screen_reader_detection_done = true
                })
            );
            return Ok(());
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "startup did not record detection"
        );
        terminal.read_output(Duration::from_millis(/*millis*/ 50))?;
    }
}
