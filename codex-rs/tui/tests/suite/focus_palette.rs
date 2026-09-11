use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::path::Path;
use std::process::Child;
use std::process::Command;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use anyhow::ensure;
#[cfg(target_os = "macos")]
use pretty_assertions::assert_eq;
use tempfile::TempDir;

// Full startup continues after the composer first appears and can be slower under Rosetta in CI.
const STARTUP_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 30);
const FOCUS_INPUT_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 5);
const FOCUS_PROBE_INPUT: &str = "focus-palette-24527";

#[test]
fn focus_gained_with_unanswered_palette_queries_preserves_immediate_input() -> Result<()> {
    let repo_root = codex_utils_cargo_bin::repo_root()?;
    let codex_home = tempfile::tempdir()?;
    write_test_config(codex_home.path(), &repo_root)?;

    let mut terminal = PtyCodex::start(&repo_root, codex_home, &[])?;
    terminal.wait_for_startup()?;

    let startup_output_len = terminal.output.len();
    let focus_started = Instant::now();
    terminal.write_input(format!("\u{1b}[I{FOCUS_PROBE_INPUT}").as_bytes())?;
    terminal.wait_for_focus_input(FOCUS_PROBE_INPUT, focus_started, startup_output_len)?;

    let delayed_input = format!("{FOCUS_PROBE_INPUT}-delayed");
    let delayed_focus_started = Instant::now();
    terminal.write_input(b"\x1b[I")?;
    terminal.read_output(Duration::from_millis(/*millis*/ 20))?;
    terminal.write_input(delayed_input.as_bytes())?;
    terminal.wait_for_focus_input(&delayed_input, delayed_focus_started, startup_output_len)?;

    Ok(())
}

#[cfg(target_os = "macos")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interactive_startup_honors_codex_home_symlink_opt_out() -> Result<()> {
    use core_test_support::responses;
    use wiremock::matchers::body_string_contains;

    core_test_support::skip_if_sandbox!(Ok(()));
    let workspace = tempfile::tempdir()?;
    let workspace_path = workspace.path().canonicalize()?;
    let codex_home = tempfile::tempdir()?;
    let target = tempfile::tempdir()?;
    let visualizations = codex_home.path().join("visualizations");
    std::os::unix::fs::symlink(target.path(), &visualizations)?;
    write_test_config(codex_home.path(), &workspace_path)?;

    let server = responses::start_mock_server().await;
    // Automatic thread-title requests must not consume the tool responses.
    let _title_mock = responses::mount_sse_once_match(
        &server,
        body_string_contains(r#"\"thread_source\":\"system\""#),
        responses::sse(vec![
            responses::ev_assistant_message("title", r#"{"title":"Symlink probe"}"#),
            responses::ev_completed("title"),
        ]),
    )
    .await;
    let _write_mock = responses::mount_sse_once_match(
        &server,
        body_string_contains(r#"\"thread_source\":\"user\""#),
        responses::sse(vec![
            responses::ev_exec_command_call_with_args(
                "write",
                &serde_json::json!({
                    "cmd": "printf ready > ready.txt",
                    "workdir": visualizations,
                    "shell": "/bin/sh",
                    "login": false,
                }),
            ),
            responses::ev_completed("startup"),
        ]),
    )
    .await;
    let completion_mock = responses::mount_sse_once_match(
        &server,
        body_string_contains(r#"\"thread_source\":\"user\""#),
        responses::sse(vec![
            responses::ev_assistant_message("ready", "symlink-opt-out-ready"),
            responses::ev_completed("complete"),
        ]),
    )
    .await;
    let base_url = server.uri();
    let visualizations = toml::Value::String(visualizations.display().to_string());
    let config_path = codex_home.path().join("config.toml");
    let config = std::fs::read_to_string(&config_path)?;
    std::fs::write(
        config_path,
        format!(
            "allow_symlinked_codex_home = true\n\
             sandbox_mode = \"workspace-write\"\n\
             approval_policy = \"never\"\n\
             {config}\n\
             [sandbox_workspace_write]\nwritable_roots = [{visualizations}]\n\
             [model_providers.test]\nname = \"Mock\"\n\
             base_url = \"{base_url}/v1\"\nwire_api = \"responses\"\n\
             requires_openai_auth = false\nsupports_websockets = false\n"
        ),
    )?;

    // A fresh home has no daemon. Writing through the link exercises the embedded server's
    // local executor and its sandbox policy.
    let mut terminal = PtyCodex::start(
        &workspace_path,
        codex_home,
        &["-c", "model_provider=\"test\"", "Write ready.txt"],
    )?;
    terminal.wait_for_startup()?;
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    while Instant::now() < deadline {
        terminal.read_output(Duration::from_millis(/*millis*/ 50))?;
        if terminal.screen_contains("symlink-opt-out-ready") {
            let tool_output = completion_mock
                .function_call_output_text("write")
                .context("missing symlink write tool output")?;
            ensure!(
                tool_output.contains("Process exited with code 0"),
                "{tool_output}"
            );
            assert_eq!(std::fs::read(target.path().join("ready.txt"))?, b"ready");
            return Ok(());
        }
        if terminal.child.try_wait()?.is_some() {
            break;
        }
    }
    bail!(
        "interactive startup did not honor the symlink opt-out; screen:\n{}",
        terminal.screen_contents(),
    );
}

pub(super) struct PtyCodex {
    master: File,
    child: Child,
    parser: vt100::Parser,
    output: Vec<u8>,
    cursor_answered: bool,
    palette_answered: bool,
    keyboard_answered: bool,
    _codex_home: TempDir,
}

impl PtyCodex {
    pub(super) fn start(
        repo_root: &Path,
        codex_home: TempDir,
        extra_args: &[&str],
    ) -> Result<Self> {
        let mut master_fd = -1;
        let mut slave_fd = -1;
        let mut window_size = libc::winsize {
            ws_row: 32,
            ws_col: 120,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        // SAFETY: `openpty` initializes both file descriptors on success, and the supplied window
        // size remains valid for the duration of the call.
        let result = unsafe {
            libc::openpty(
                &mut master_fd,
                &mut slave_fd,
                /*name*/ std::ptr::null_mut(),
                /*termp*/ std::ptr::null_mut(),
                &raw mut window_size,
            )
        };
        if result == -1 {
            return Err(std::io::Error::last_os_error()).context("open focus-test pseudo-terminal");
        }

        // SAFETY: a successful `openpty` transfers ownership of both unique file descriptors.
        let master = File::from(unsafe { OwnedFd::from_raw_fd(master_fd) });
        // SAFETY: `slave_fd` is the second unique descriptor initialized by `openpty`.
        let slave = File::from(unsafe { OwnedFd::from_raw_fd(slave_fd) });
        let stdin = slave.try_clone().context("clone pseudo-terminal stdin")?;
        let stdout = slave.try_clone().context("clone pseudo-terminal stdout")?;

        let codex = codex_utils_cargo_bin::cargo_bin("codex-tui")
            .or_else(|_| codex_utils_cargo_bin::cargo_bin("codex"))?;
        let child = Command::new(codex)
            .args(extra_args)
            .arg("--no-alt-screen")
            .arg("-C")
            .arg(repo_root)
            .env("TERM", "xterm-256color")
            .env("OPENAI_API_KEY", "focus-palette-test")
            .env("CODEX_HOME", codex_home.path())
            .stdin(stdin)
            .stdout(stdout)
            .stderr(slave)
            .spawn()
            .context("start Codex in focus-test pseudo-terminal")?;

        Ok(Self {
            master,
            child,
            parser: vt100::Parser::new(
                /*rows*/ 32, /*cols*/ 120, /*scrollback_len*/ 0,
            ),
            output: Vec::new(),
            cursor_answered: false,
            palette_answered: false,
            keyboard_answered: false,
            _codex_home: codex_home,
        })
    }

    pub(super) fn wait_for_startup(&mut self) -> Result<()> {
        let deadline = Instant::now() + STARTUP_TIMEOUT;
        while Instant::now() < deadline {
            self.read_output(Duration::from_millis(/*millis*/ 50))?;
            self.answer_startup_queries()?;

            if self.palette_answered && self.screen_contains("OpenAI Codex") {
                return Ok(());
            }

            if let Some(status) = self.child.try_wait()? {
                bail!(
                    "Codex exited before the focus test started ({status}); screen:\n{}",
                    self.screen_contents(),
                );
            }
        }

        bail!(
            "Codex did not initialize within {:?}; screen:\n{}",
            STARTUP_TIMEOUT,
            self.screen_contents(),
        );
    }

    fn wait_for_focus_input(
        &mut self,
        input: &str,
        focus_started: Instant,
        startup_output_len: usize,
    ) -> Result<()> {
        while focus_started.elapsed() < FOCUS_INPUT_TIMEOUT {
            self.read_output(Duration::from_millis(/*millis*/ 20))?;
            let focus_output = &self.output[startup_output_len..];
            ensure!(
                !contains_bytes(focus_output, b"\x1b]10;?")
                    && !contains_bytes(focus_output, b"\x1b]11;?"),
                "focus regain queried terminal colors after the startup palette was cached",
            );
            if self.screen_contains(input) {
                return Ok(());
            }
        }

        bail!(
            "focus-time palette refresh blocked or discarded {input:?} for more than {:?}; \
             screen:\n{}",
            FOCUS_INPUT_TIMEOUT,
            self.screen_contents(),
        );
    }

    fn answer_startup_queries(&mut self) -> Result<()> {
        if !self.cursor_answered && contains_bytes(&self.output, b"\x1b[6n") {
            self.write_input(b"\x1b[1;1R")?;
            self.cursor_answered = true;
        }

        if !self.keyboard_answered && contains_bytes(&self.output, b"\x1b[?u") {
            self.write_input(b"\x1b[?0u\x1b[?1;2c")?;
            self.keyboard_answered = true;
        }

        if !self.palette_answered
            && contains_bytes(&self.output, b"\x1b]10;?")
            && contains_bytes(&self.output, b"\x1b]11;?")
        {
            self.write_input(b"\x1b]10;rgb:ffff/ffff/ffff\x1b\\\x1b]11;rgb:0000/0000/0000\x1b\\")?;
            self.palette_answered = true;
        }

        Ok(())
    }

    pub(super) fn read_output(&mut self, timeout: Duration) -> Result<()> {
        let timeout_ms = timeout.as_millis().min(libc::c_int::MAX as u128) as libc::c_int;
        let mut descriptor = libc::pollfd {
            fd: self.master.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };

        // SAFETY: `descriptor` points to one initialized poll descriptor.
        let ready = unsafe {
            libc::poll(&mut descriptor, /*nfds*/ 1, timeout_ms)
        };
        if ready == -1 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                return Ok(());
            }
            return Err(error).context("poll focus-test pseudo-terminal");
        }
        if ready == 0 || descriptor.revents & libc::POLLIN == 0 {
            return Ok(());
        }

        let mut chunk = [0_u8; 8192];
        let count = self.master.read(&mut chunk)?;
        self.output.extend_from_slice(&chunk[..count]);
        self.parser.process(&chunk[..count]);
        Ok(())
    }

    pub(super) fn write_input(&mut self, bytes: &[u8]) -> Result<()> {
        self.master.write_all(bytes)?;
        self.master.flush()?;
        Ok(())
    }

    pub(super) fn screen_contains(&self, text: &str) -> bool {
        self.parser.screen().contents().contains(text)
    }

    pub(super) fn screen_contents(&self) -> String {
        self.parser.screen().contents()
    }

    pub(super) fn wait_for_screen(&mut self, text: &str) -> Result<()> {
        let deadline = Instant::now() + STARTUP_TIMEOUT;
        while Instant::now() < deadline {
            self.read_output(Duration::from_millis(/*millis*/ 50))?;
            self.answer_startup_queries()?;
            if self.screen_contains(text) {
                return Ok(());
            }
            if let Some(status) = self.child.try_wait()? {
                bail!("Codex exited while waiting for {text:?} ({status})");
            }
        }
        bail!("missing {text:?}; screen:\n{}", self.screen_contents())
    }

    pub(super) fn ensure_running(&mut self) -> Result<()> {
        ensure!(
            self.child.try_wait()?.is_none(),
            "Codex exited unexpectedly; screen:\n{}",
            self.screen_contents()
        );
        Ok(())
    }
}

impl Drop for PtyCodex {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn contains_bytes(buffer: &[u8], needle: &[u8]) -> bool {
    buffer.windows(needle.len()).any(|window| window == needle)
}

pub(super) fn write_test_config(codex_home: &Path, repo_root: &Path) -> Result<()> {
    let repo_root = repo_root.display();
    let config = format!(
        "model = \"gpt-5.6-terra\"\nmodel_provider = \"openai\"\n\
         suppress_unstable_features_warning = true\nanalytics.enabled = false\n\n\
         [projects.\"{repo_root}\"]\ntrust_level = \"trusted\"\n"
    );
    std::fs::write(codex_home.join("config.toml"), config)
        .context("write focus-test Codex configuration")?;
    std::fs::write(
        codex_home.join("auth.json"),
        r#"{"OPENAI_API_KEY":"focus-palette-test","tokens":null,"last_refresh":null}"#,
    )
    .context("write focus-test API-key authentication")
}
