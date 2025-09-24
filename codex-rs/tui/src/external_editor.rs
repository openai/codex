use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};
use tempfile::Builder;

use crate::exec_command::escape_command;

#[cfg(test)]
use std::sync::{Mutex, OnceLock};
#[cfg(test)]
type LaunchMock = Box<
    dyn Fn(&EditorCandidate, &Path, &Path) -> Result<std::process::ExitStatus> + Send + 'static,
>;
#[cfg(test)]
static MOCK_LAUNCH: OnceLock<Mutex<Option<LaunchMock>>> = OnceLock::new();

const MAX_EDIT_BYTES: u64 = 256 * 1024;

#[derive(Clone, Debug)]
struct EditorCandidate {
    program: OsString,
    args: Vec<OsString>,
    label: String,
}

impl EditorCandidate {
    fn command_vector(&self, file: &Path) -> Vec<OsString> {
        let mut out = Vec::with_capacity(self.args.len() + 2);
        out.push(self.program.clone());
        out.extend(self.args.clone());
        out.push(file.as_os_str().to_owned());
        out
    }

    fn display_command(&self, file: &Path) -> String {
        let parts: Vec<String> = self
            .command_vector(file)
            .into_iter()
            .map(|part| part.to_string_lossy().into_owned())
            .collect();
        escape_command(&parts)
    }
}

#[derive(Debug)]
pub(crate) struct ExternalEditorRequest<'a> {
    pub initial_text: &'a str,
    pub working_directory: &'a Path,
    pub keep_file: bool,
}

#[derive(Debug)]
pub(crate) struct ExternalEditorResponse {
    pub edited_text: Option<String>,
    pub editor_label: String,
    pub command_display: String,
    pub kept_path: Option<PathBuf>,
}

pub(crate) fn external_editor_is_enabled() -> bool {
    std::env::var_os("CODEX_EDIT_DISABLE").is_none()
}

pub(crate) fn run_external_editor_with_hook<F>(
    request: ExternalEditorRequest<'_>,
    mut on_launch: F,
) -> Result<ExternalEditorResponse>
where
    F: FnMut(&str, &str),
{
    let temp_file = Builder::new()
        .prefix("codex-edit-")
        .suffix(".txt")
        .tempfile()
        .context("failed to create temporary file for /edit")?;

    if !request.initial_text.is_empty() {
        fs::write(temp_file.path(), request.initial_text).with_context(|| {
            format!(
                "failed to write initial contents to {}",
                temp_file.path().display()
            )
        })?;
    }

    let candidates = editor_candidates();
    if candidates.is_empty() {
        bail!("No editor candidates available. Set $VISUAL or $EDITOR.");
    }

    let mut last_spawn_error: Option<anyhow::Error> = None;
    for candidate in candidates {
        let display_command = candidate.display_command(temp_file.path());
        on_launch(&candidate.label, &display_command);
        match launch_editor(&candidate, temp_file.path(), request.working_directory) {
            Ok(status) => {
                if !status.success() {
                    if let Some(code) = status.code() {
                        bail!("Editor exited with status {code}.");
                    } else {
                        bail!("Editor terminated by signal.");
                    }
                }

                let metadata = fs::metadata(temp_file.path()).with_context(|| {
                    format!("failed to stat edited file {}", temp_file.path().display())
                })?;
                if metadata.len() > MAX_EDIT_BYTES {
                    bail!(
                        "Edited file is too large ({} bytes; limit is {}).",
                        metadata.len(),
                        MAX_EDIT_BYTES
                    );
                }

                let contents = fs::read_to_string(temp_file.path()).with_context(|| {
                    format!("failed to read edited file {}", temp_file.path().display())
                })?;

                let normalized = normalize_newlines(&contents);
                let edited_text = Some(normalized);

                let kept_path = if request.keep_file {
                    let persisted_path = temp_file.path().to_path_buf();
                    let (_file, path) = temp_file.keep().with_context(|| {
                        format!("failed to keep temp file {}", persisted_path.display())
                    })?;
                    Some(path)
                } else {
                    None
                };

                return Ok(ExternalEditorResponse {
                    edited_text,
                    editor_label: candidate.label,
                    command_display: display_command,
                    kept_path,
                });
            }
            Err(err) => {
                last_spawn_error = Some(err);
                continue;
            }
        }
    }

    Err(last_spawn_error.unwrap_or_else(|| anyhow!("failed to launch external editor")))
}

fn launch_editor(
    candidate: &EditorCandidate,
    file: &Path,
    cwd: &Path,
) -> Result<std::process::ExitStatus> {
    #[cfg(test)]
    if let Some(mock) = MOCK_LAUNCH
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap()
        .as_mut()
    {
        return mock(candidate, file, cwd);
    }

    crate::tui::restore().context("failed to restore terminal before launching editor")?;

    let status_result = Command::new(&candidate.program)
        .args(&candidate.args)
        .arg(file)
        .current_dir(cwd)
        .status()
        .with_context(|| {
            format!(
                "failed to spawn external editor '{}'",
                candidate.program.to_string_lossy()
            )
        });

    let modes_result =
        crate::tui::set_modes().context("failed to re-enable terminal after closing editor");

    let status = status_result?;
    modes_result?;
    Ok(status)
}

fn editor_candidates() -> Vec<EditorCandidate> {
    let mut out = Vec::new();

    if let Some(candidate) = env_candidate("VISUAL") {
        out.push(candidate);
    }

    if let Some(candidate) = env_candidate("EDITOR") {
        out.push(candidate);
    }

    out.extend(default_candidates());
    out
}

fn env_candidate(var: &str) -> Option<EditorCandidate> {
    let value = std::env::var_os(var)?;
    let label = format!("${var}={}", value.to_string_lossy());
    Some(parse_command_value(value, label))
}

fn default_candidates() -> Vec<EditorCandidate> {
    #[cfg(target_os = "macos")]
    {
        vec![EditorCandidate {
            program: OsString::from("open"),
            args: vec![OsString::from("-W"), OsString::from("-t")],
            label: "macOS open -t".to_string(),
        }]
    }

    #[cfg(target_os = "windows")]
    {
        vec![EditorCandidate {
            program: OsString::from("notepad"),
            args: Vec::new(),
            label: "notepad".to_string(),
        }]
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        vec![
            EditorCandidate {
                program: OsString::from("nano"),
                args: Vec::new(),
                label: "nano".to_string(),
            },
            EditorCandidate {
                program: OsString::from("vim"),
                args: Vec::new(),
                label: "vim".to_string(),
            },
        ]
    }
}

fn parse_command_value(value: OsString, label: String) -> EditorCandidate {
    let value_str = value.to_string_lossy().to_string();
    let mut parts: Vec<OsString> = match shlex::split(&value_str) {
        Some(split) if !split.is_empty() => split.into_iter().map(OsString::from).collect(),
        _ => vec![value],
    };

    let program = parts.remove(0);
    EditorCandidate {
        program,
        args: parts,
        label,
    }
}

fn normalize_newlines(input: &str) -> String {
    let mut out = input.replace("\r\n", "\n");
    if out.contains('\r') {
        out = out.replace('\r', "\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::env;
    use std::fs;
    use tempfile::tempdir;

    static TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

    fn test_lock_guard() -> std::sync::MutexGuard<'static, ()> {
        TEST_MUTEX
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    fn set_mock_launch(mock: Option<LaunchMock>) {
        *MOCK_LAUNCH
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap()
            = mock;
    }

    struct EnvVarGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = env::var(key).ok();
            unsafe { env::set_var(key, value) };
            Self { key, previous }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => unsafe { env::set_var(self.key, value) },
                None => unsafe { env::remove_var(self.key) },
            }
        }
    }

    struct MockLaunchGuard;

    impl MockLaunchGuard {
        fn set(mock: LaunchMock) -> Self {
            set_mock_launch(Some(mock));
            Self
        }
    }

    impl Drop for MockLaunchGuard {
        fn drop(&mut self) {
            set_mock_launch(None);
        }
    }

    #[cfg(unix)]
    fn exit_status(code: i32) -> std::process::ExitStatus {
        use std::os::unix::process::ExitStatusExt;
        std::process::ExitStatus::from_raw((code & 0xff) << 8)
    }

    #[cfg(windows)]
    fn exit_status(code: i32) -> std::process::ExitStatus {
        use std::os::windows::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(code as u32)
    }

    #[test]
    fn normalize_newlines_replaces_crlf_and_cr() {
        let mixed = "line1\r\nline2\rline3\n";
        let normalized = normalize_newlines(mixed);
        assert_eq!(normalized, "line1\nline2\nline3\n");
    }

    #[test]
    fn run_external_editor_happy_path_applies_edits() {
        let _lock = test_lock_guard();
        let dir = tempdir().unwrap();
        let _env_guard = EnvVarGuard::set("VISUAL", "mock-editor");
        let _mock_guard = MockLaunchGuard::set(Box::new(|_, file, _| {
            fs::write(file, "edited from mock\n")?;
            Ok(exit_status(0))
        }));

        let request = ExternalEditorRequest {
            initial_text: "original",
            working_directory: dir.path(),
            keep_file: false,
        };

        let result = run_external_editor_with_hook(request, |_, _| {}).expect("run external editor");

        assert_eq!(result.edited_text.as_deref(), Some("edited from mock\n"));
        assert!(result.editor_label.contains("mock-editor"));
    }

    #[test]
    fn run_external_editor_propagates_nonzero_exit() {
        let _lock = test_lock_guard();
        let dir = tempdir().unwrap();
        let _env_guard = EnvVarGuard::set("VISUAL", "mock-editor");
        let _mock_guard = MockLaunchGuard::set(Box::new(|_, _file, _| Ok(exit_status(2))));

        let request = ExternalEditorRequest {
            initial_text: "unchanged",
            working_directory: dir.path(),
            keep_file: false,
        };

        let err = run_external_editor_with_hook(request, |_, _| {}).expect_err("should fail");
        assert!(format!("{err:#}").contains("status 2"));
    }

    #[test]
    fn run_external_editor_rejects_large_output() {
        let _lock = test_lock_guard();
        let dir = tempdir().unwrap();
        let _env_guard = EnvVarGuard::set("VISUAL", "mock-editor");
        let _mock_guard = MockLaunchGuard::set(Box::new(|_, file, _| {
            let big = "x".repeat((MAX_EDIT_BYTES as usize) + 1);
            fs::write(file, big)?;
            Ok(exit_status(0))
        }));

        let request = ExternalEditorRequest {
            initial_text: "small",
            working_directory: dir.path(),
            keep_file: false,
        };

        let err = run_external_editor_with_hook(request, |_, _| {}).expect_err("should fail");
        assert!(format!("{err:#}").contains("too large"));
    }

    #[test]
    fn run_external_editor_returns_empty_string_for_empty_file() {
        let _lock = test_lock_guard();
        let dir = tempdir().unwrap();
        let _env_guard = EnvVarGuard::set("VISUAL", "mock-editor");
        let _mock_guard = MockLaunchGuard::set(Box::new(|_, _file, _| Ok(exit_status(0))));

        let request = ExternalEditorRequest {
            initial_text: "",
            working_directory: dir.path(),
            keep_file: false,
        };

        let result = run_external_editor_with_hook(request, |_, _| {}).expect("run external editor");
        assert_eq!(result.edited_text.as_deref(), Some(""));
    }
}
