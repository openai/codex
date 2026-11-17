//! Exercise editor handoff through a terminal, including editors that switch screens.

use super::PtyCodex;
use super::write_test_config_with_bundled_models;
use anyhow::Result;
use anyhow::ensure;
use std::os::fd::AsRawFd;
use std::time::Duration;
use std::time::Instant;

#[test]
fn external_editors_keep_the_screen_and_return_the_draft() -> Result<()> {
    let repo_root = codex_utils_cargo_bin::repo_root()?;
    for (editor_output, fullscreen) in [
        ("", true),
        ("printf '\\033[?1049h\\033[2J\\033[Hterminal editor'", true),
        ("printf '\\033[2J\\033[Hterminal editor'", true),
        ("printf '\\033[?1049l\\033[2J\\033[Hterminal editor'", true),
        ("", false),
    ] {
        let home = tempfile::tempdir()?;
        write_test_config_with_bundled_models(home.path(), &repo_root)?;
        let editor = home.path().join("editor.sh");
        codex_utils_cargo_bin::write_executable(
            &editor,
            &format!(
                "#!/bin/sh\n{editor_output}\n: > \"$0.ready\"\nIFS= read -r ignored\n\
                 {reset_mode}{exit_screen}\nprintf 'edited draft' > \"$1\"\n",
                // Simulate an editor restoring canonical mode after suspend/resume.
                reset_mode = if fullscreen {
                    ""
                } else {
                    "stty sane || exit 1\n"
                },
                exit_screen = if editor_output.contains("1049h") {
                    "printf '\\033[?1049l'"
                } else {
                    ""
                },
            ),
        )?;
        // Give each editor its own app server and wait for the thread before sending Ctrl+G.
        let args = if fullscreen {
            vec!["--no-daemon", "-c", "sandbox_mode=\"read-only\""]
        } else {
            vec![
                "--no-daemon",
                "-c",
                "sandbox_mode=\"read-only\"",
                "-c",
                "tui.fullscreen_transcript=false",
            ]
        };
        let codex = codex_utils_cargo_bin::cargo_bin("codex-tui")
            .or_else(|_| codex_utils_cargo_bin::cargo_bin("codex"))?;
        let mut terminal = PtyCodex::start_binary(&codex, &repo_root, home, &args, Some(&editor))?;
        terminal.wait_for_startup()?;
        terminal.wait_for_screen("GPT-5.6-Terra")?;
        terminal.write_input(b"initial draft")?;
        terminal.wait_for_screen("initial draft")?;
        terminal.write_input(b"\x07")?;

        let deadline = Instant::now() + Duration::from_secs(/*secs*/ 10);
        while !editor.with_extension("sh.ready").exists() && Instant::now() < deadline {
            terminal.read_output(Duration::from_millis(/*millis*/ 50))?;
            terminal.ensure_running()?;
        }
        ensure!(
            editor.with_extension("sh.ready").exists(),
            "editor did not start (fullscreen={fullscreen}, output={editor_output:?}); screen:\n{}",
            terminal.screen_contents()
        );

        let mut visible = None;
        if editor_output.is_empty() {
            terminal.wait_for_screen("Save and close external editor to continue.")?;
            ensure!(terminal.screen_contains("initial draft"));
            if fullscreen {
                ensure!(terminal.screen_contains("OpenAI Codex"));
                ensure!(terminal.screen_contains("GPT-5.6-Terra"));
            }
            ensure!(
                terminal.parser.screen().mouse_protocol_mode() == vt100::MouseProtocolMode::None
            );
            if fullscreen {
                visible = Some(
                    terminal
                        .screen_contents()
                        .lines()
                        .filter(|line| {
                            line.contains("initial draft")
                                || line.contains("Save and close external editor to continue.")
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                );
            }
        } else {
            terminal.wait_for_screen("terminal editor")?;
        }

        terminal.write_input(b"\n")?;
        terminal.wait_for_screen("edited draft")?;
        let mut mode = std::mem::MaybeUninit::<libc::termios>::uninit();
        // SAFETY: the PTY master is open and mode points to writable termios storage.
        ensure!(unsafe { libc::tcgetattr(terminal.master.as_raw_fd(), mode.as_mut_ptr()) } == 0);
        // SAFETY: tcgetattr succeeded and initialized mode.
        let mode = unsafe { mode.assume_init() };
        ensure!(mode.c_lflag & (libc::ICANON | libc::ECHO) == 0);
        ensure!(terminal.parser.screen().alternate_screen() == fullscreen);
        terminal.write_input(b"!")?;
        terminal.wait_for_screen("edited draft!")?;
        if let Some(visible) = visible {
            insta::assert_snapshot!("external_editor_visible_handoff", visible);
        }
    }
    Ok(())
}
