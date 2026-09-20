//! The picker persists a preference, but only a new process adopts the renderer.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn tui_mode_picker_applies_only_after_restart() -> Result<()> {
    let root = codex_utils_cargo_bin::repo_root()?;
    let mut home = tempfile::tempdir()?;
    write_test_config(home.path(), &root)?;
    let path = home.path().join("config.toml");
    let config = std::fs::read_to_string(&path)?;
    std::fs::write(
        &path,
        format!(
            "tui.fullscreen_transcript = false\ntui.alternate_screen = \"always\"\ntui.disable_paste_burst = true\ntui.status_line = [\"thread-id\"]\n{config}"
        ),
    )?;
    for (launch, current) in [false, true, false].into_iter().enumerate() {
        let mut terminal = PtyCodex::start(&root, home, &[])?;
        terminal.wait_for_startup()?;
        // The header precedes session setup; wait for the thread ID before sending commands.
        let deadline = Instant::now() + STARTUP_TIMEOUT;
        loop {
            terminal.read_output(Duration::from_millis(/*millis*/ 50))?;
            terminal.answer_startup_queries()?;
            if terminal
                .screen_contents()
                .split_whitespace()
                .any(|word| uuid::Uuid::parse_str(word).is_ok())
            {
                break;
            }
            ensure!(
                Instant::now() < deadline,
                "thread did not become ready: {}",
                terminal.screen_contents()
            );
            terminal.ensure_running()?;
        }
        assert_eq!(terminal.parser.screen().alternate_screen(), current);
        if launch == 2 {
            break;
        }
        terminal.write_input(b"/tui")?;
        terminal.wait_for_screen("/tui")?;
        terminal.write_input(b"\r")?;
        terminal.wait_for_screen("TUI mode for next launch")?;
        terminal.write_input(if current { b"\x1b[A" } else { b"\x1b[B" })?;
        terminal.write_input(b"\r")?;
        terminal.wait_for_screen(if current {
            "Saved TUI mode: Scrollback"
        } else {
            "Saved TUI mode: Fullscreen"
        })?;
        assert_eq!(terminal.parser.screen().alternate_screen(), current);
        let saved: toml::Value = toml::from_str(&std::fs::read_to_string(&path)?)?;
        assert_eq!(
            saved["tui"]["fullscreen_transcript"].as_bool(),
            Some(!current)
        );
        home = std::mem::replace(&mut terminal._codex_home, tempfile::tempdir()?);
        drop(terminal);
    }
    Ok(())
}
