use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
#[cfg(windows)]
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
#[cfg(windows)]
use std::ffi::OsString;
use std::io::BufRead;
use std::io::BufReader;
use std::io::ErrorKind;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Child;
use std::process::ChildStdin;
use std::process::ChildStdout;
use std::process::Command;
use std::process::Stdio;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::PoisonError;

const POWERSHELL_PARSER_SCRIPT: &str = include_str!("powershell_parser.ps1");
#[cfg(any(test, windows))]
const WINDOWS_POWERSHELL_SUFFIX: &str = r"WindowsPowerShell\v1.0\powershell.exe";
#[cfg(any(test, windows))]
const WINDOWS_PWSH_SUFFIX: &str = r"PowerShell\7\pwsh.exe";

/// Cache one long-lived parser process per executable path so repeated safety checks reuse
/// PowerShell startup work while still consulting the real parser every time.
///
/// We keep the cache behind one mutex because each child process speaks a simple
/// request/response protocol over a single stdin/stdout pair, so callers targeting the same
/// executable must serialize access anyway.
fn parse_with_powershell_ast(executable: &Path, script: &str) -> PowershellParseOutcome {
    static PARSER_PROCESSES: LazyLock<Mutex<HashMap<PathBuf, PowershellParserProcess>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));

    let mut parser_processes = PARSER_PROCESSES
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    parse_with_cached_process(&mut parser_processes, executable, script)
}

pub(crate) fn try_parse_powershell_ast_commands(
    executable: &str,
    script: &str,
) -> Option<Vec<Vec<String>>> {
    let parser_executable = trusted_powershell_parser_executable(executable)?;
    match parse_with_powershell_ast(&parser_executable, script) {
        PowershellParseOutcome::Commands(commands) => Some(commands),
        PowershellParseOutcome::Unsupported | PowershellParseOutcome::Failed => None,
    }
}

/// Selects the host-side parser only when the command itself names that same trusted binary.
///
/// The parser runs before the command approval and sandbox boundaries. Bare executable names and
/// arbitrary paths are therefore not eligible: their eventual resolution can depend on the
/// workspace, current directory, or `PATH`. Returning a canonical executable derived from an
/// authoritative Windows known folder also ensures the parser spawn never receives the command's
/// attacker-controlled `argv[0]`.
fn trusted_powershell_parser_executable(executable: &str) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        resolve_trusted_powershell_parser_executable_with(
            executable,
            trusted_windows_root,
            canonicalize_trusted_windows_path,
            Path::is_file,
        )
    }

    #[cfg(not(windows))]
    {
        let _ = executable;
        None
    }
}

/// Returns the authoritative Windows PowerShell invocation path only when the production trust
/// resolver accepts that exact known-folder-derived path.
#[cfg(all(test, windows))]
pub(crate) fn trusted_windows_powershell_invocation_path() -> Option<PathBuf> {
    trusted_powershell_invocation_path(TrustedPowerShellRoot::System, WINDOWS_POWERSHELL_SUFFIX)
}

/// Returns the standard machine-wide PowerShell 7 invocation path when it is installed and the
/// production trust resolver accepts it.
#[cfg(all(test, windows))]
pub(crate) fn trusted_standard_pwsh_invocation_path() -> Option<PathBuf> {
    trusted_powershell_invocation_path(TrustedPowerShellRoot::ProgramFiles, WINDOWS_PWSH_SUFFIX)
}

#[cfg(all(test, windows))]
fn trusted_powershell_invocation_path(
    root_kind: TrustedPowerShellRoot,
    suffix: &str,
) -> Option<PathBuf> {
    let root = trusted_windows_root(root_kind).ok()?;
    let invocation_path = join_windows_path(&root, suffix);
    let invocation_path_str = invocation_path.to_str()?;
    trusted_powershell_parser_executable(invocation_path_str)?;
    Some(invocation_path)
}

#[cfg(any(test, windows))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TrustedPowerShellRoot {
    System,
    ProgramFiles,
}

#[cfg(any(test, windows))]
fn resolve_trusted_powershell_parser_executable_with(
    executable: &str,
    resolve_root: impl Fn(TrustedPowerShellRoot) -> std::io::Result<PathBuf>,
    canonicalize: impl Fn(&Path) -> std::io::Result<PathBuf>,
    is_file: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    let normalized_executable = normalized_windows_path(executable);

    [
        (TrustedPowerShellRoot::System, WINDOWS_POWERSHELL_SUFFIX),
        (TrustedPowerShellRoot::ProgramFiles, WINDOWS_PWSH_SUFFIX),
    ]
    .into_iter()
    .find_map(|(root_kind, suffix)| {
        let root = resolve_root(root_kind).ok()?;
        let candidate = join_windows_path(&root, suffix);
        let candidate_str = candidate.to_str()?;
        if !normalized_executable.eq_ignore_ascii_case(&normalized_windows_path(candidate_str)) {
            return None;
        }

        let canonical_root = canonicalize(&root).ok()?;
        let canonical_candidate = canonicalize(&candidate).ok()?;
        let expected_candidate = join_windows_path(&canonical_root, suffix);
        if !normalized_windows_path(canonical_candidate.to_str()?)
            .eq_ignore_ascii_case(&normalized_windows_path(expected_candidate.to_str()?))
            || !is_file(&canonical_candidate)
        {
            return None;
        }

        Some(canonical_candidate)
    })
}

#[cfg(windows)]
fn join_windows_path(root: &Path, suffix: &str) -> PathBuf {
    root.join(suffix)
}

#[cfg(all(test, not(windows)))]
fn join_windows_path(root: &Path, suffix: &str) -> PathBuf {
    let mut path = root
        .to_string_lossy()
        .trim_end_matches(['/', '\\'])
        .to_string();
    path.push('\\');
    path.push_str(suffix);
    PathBuf::from(path)
}

#[cfg(any(test, windows))]
fn normalized_windows_path(path: &str) -> String {
    path.strip_prefix(r"\\?\")
        .unwrap_or(path)
        .replace('/', "\\")
}

#[cfg(windows)]
fn canonicalize_trusted_windows_path(path: &Path) -> std::io::Result<PathBuf> {
    AbsolutePathBuf::from_absolute_path_checked(path)?
        .canonicalize()
        .map(AbsolutePathBuf::into_path_buf)
}

#[cfg(windows)]
fn trusted_windows_root(root: TrustedPowerShellRoot) -> std::io::Result<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::System::Com::CoTaskMemFree;
    use windows_sys::Win32::UI::Shell::FOLDERID_ProgramFiles;
    use windows_sys::Win32::UI::Shell::FOLDERID_System;
    use windows_sys::Win32::UI::Shell::KF_FLAG_DEFAULT;
    use windows_sys::Win32::UI::Shell::SHGetKnownFolderPath;

    let folder_id = match root {
        TrustedPowerShellRoot::System => &FOLDERID_System,
        TrustedPowerShellRoot::ProgramFiles => &FOLDERID_ProgramFiles,
    };
    let mut path_ptr = std::ptr::null_mut::<u16>();
    let flags = u32::try_from(KF_FLAG_DEFAULT)
        .map_err(|_| std::io::Error::other("KF_FLAG_DEFAULT did not fit in u32"))?;
    // SAFETY: SHGetKnownFolderPath initializes path_ptr with a CoTaskMem-allocated,
    // null-terminated UTF-16 string on success.
    let hr = unsafe { SHGetKnownFolderPath(folder_id, flags, 0, &mut path_ptr) };
    if hr != 0 {
        if !path_ptr.is_null() {
            // SAFETY: Any non-null path returned by SHGetKnownFolderPath uses CoTaskMem.
            unsafe { CoTaskMemFree(path_ptr.cast()) };
        }
        return Err(std::io::Error::other(format!(
            "SHGetKnownFolderPath failed with HRESULT {hr:#010x}"
        )));
    }
    if path_ptr.is_null() {
        return Err(std::io::Error::other(
            "SHGetKnownFolderPath returned a null pointer",
        ));
    }

    // SAFETY: path_ptr is a valid null-terminated UTF-16 string allocated by
    // SHGetKnownFolderPath and is freed after copying.
    let path = unsafe {
        let mut len = 0usize;
        while *path_ptr.add(len) != 0 {
            len += 1;
        }
        let wide = std::slice::from_raw_parts(path_ptr, len);
        let path = PathBuf::from(OsString::from_wide(wide));
        CoTaskMemFree(path_ptr.cast());
        path
    };

    Ok(path)
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum PowershellParseOutcome {
    Commands(Vec<Vec<String>>),
    Unsupported,
    Failed,
}

fn parse_with_cached_process(
    parser_processes: &mut HashMap<PathBuf, PowershellParserProcess>,
    executable: &Path,
    script: &str,
) -> PowershellParseOutcome {
    // `powershell.exe` and `pwsh.exe` do not accept the same language surface, so each
    // executable keeps its own parser process and request stream.
    let parser_key = executable.to_path_buf();
    for attempt in 0..=1 {
        if !parser_processes.contains_key(&parser_key) {
            match PowershellParserProcess::spawn(executable) {
                Ok(process) => {
                    parser_processes.insert(parser_key.clone(), process);
                }
                Err(_) => return PowershellParseOutcome::Failed,
            }
        }

        let Some(parser_process) = parser_processes.get_mut(&parser_key) else {
            return PowershellParseOutcome::Failed;
        };
        let parse_result = parser_process.parse(script);
        match parse_result {
            Ok(outcome) => return outcome,
            Err(_) if attempt == 0 => {
                // The common failure mode here is that a previously cached child exited or its
                // stdio stream became unusable between requests. Drop that process and retry once
                // with a fresh child before giving up.
                parser_processes.remove(&parser_key);
            }
            Err(_) => return PowershellParseOutcome::Failed,
        }
    }

    PowershellParseOutcome::Failed
}

fn encode_powershell_base64(script: &str) -> String {
    let mut utf16 = Vec::with_capacity(script.len() * 2);
    for unit in script.encode_utf16() {
        utf16.extend_from_slice(&unit.to_le_bytes());
    }
    BASE64_STANDARD.encode(utf16)
}

fn encoded_parser_script() -> &'static str {
    static ENCODED: LazyLock<String> =
        LazyLock::new(|| encode_powershell_base64(POWERSHELL_PARSER_SCRIPT));
    &ENCODED
}

struct PowershellParserProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    // Request ids are monotonic within one child process so the caller can detect protocol
    // desynchronization if stdout is contaminated or the child is unexpectedly replaced.
    next_request_id: u64,
}

impl PowershellParserProcess {
    fn spawn(executable: &Path) -> std::io::Result<Self> {
        let mut child = Command::new(executable)
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-EncodedCommand",
                encoded_parser_script(),
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let stdin = match take_child_stdin(&mut child) {
            Ok(stdin) => stdin,
            Err(error) => {
                kill_child(&mut child);
                return Err(error);
            }
        };
        let stdout = match take_child_stdout(&mut child) {
            Ok(stdout) => stdout,
            Err(error) => {
                kill_child(&mut child);
                return Err(error);
            }
        };
        Ok(Self {
            child,
            stdin,
            stdout,
            next_request_id: 0,
        })
    }

    fn parse(&mut self, script: &str) -> std::io::Result<PowershellParseOutcome> {
        let request = PowershellParserRequest {
            id: self.next_request_id,
            payload: encode_powershell_base64(script),
        };
        self.next_request_id = self.next_request_id.wrapping_add(1);
        let mut request_json = serialize_request(&request)?;
        request_json.push('\n');
        self.stdin.write_all(request_json.as_bytes())?;
        self.stdin.flush()?;

        let mut response_line = String::new();
        if self.stdout.read_line(&mut response_line)? == 0 {
            return Err(std::io::Error::new(
                ErrorKind::UnexpectedEof,
                "PowerShell parser closed stdout",
            ));
        }

        let response = deserialize_response(&response_line)?;
        // Requests are serialized today; the id still catches protocol desyncs if stdout is
        // contaminated or the child process is unexpectedly replaced mid-request. That turns an
        // ambiguous parser result into a hard failure so the caller can discard the cached child.
        if response.id != request.id {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                format!(
                    "PowerShell parser returned response id {} for request {}",
                    response.id, request.id
                ),
            ));
        }

        Ok(response.into_outcome())
    }
}

impl Drop for PowershellParserProcess {
    fn drop(&mut self) {
        kill_child(&mut self.child);
    }
}

fn take_child_stdin(child: &mut Child) -> std::io::Result<ChildStdin> {
    child.stdin.take().ok_or_else(|| {
        std::io::Error::new(
            ErrorKind::BrokenPipe,
            "PowerShell parser child did not expose stdin",
        )
    })
}

fn take_child_stdout(child: &mut Child) -> std::io::Result<BufReader<ChildStdout>> {
    child.stdout.take().map(BufReader::new).ok_or_else(|| {
        std::io::Error::new(
            ErrorKind::BrokenPipe,
            "PowerShell parser child did not expose stdout",
        )
    })
}

fn serialize_request(request: &PowershellParserRequest) -> std::io::Result<String> {
    serde_json::to_string(request).map_err(|error| {
        std::io::Error::new(
            ErrorKind::InvalidData,
            format!("failed to serialize PowerShell parser request: {error}"),
        )
    })
}

fn deserialize_response(response_line: &str) -> std::io::Result<PowershellParserResponse> {
    serde_json::from_str(response_line).map_err(|error| {
        std::io::Error::new(
            ErrorKind::InvalidData,
            format!("failed to parse PowerShell parser response: {error}"),
        )
    })
}

#[derive(Serialize)]
struct PowershellParserRequest {
    id: u64,
    payload: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PowershellParserResponse {
    id: u64,
    status: String,
    commands: Option<Vec<Vec<String>>>,
}

impl PowershellParserResponse {
    fn into_outcome(self) -> PowershellParseOutcome {
        match self.status.as_str() {
            "ok" => self
                .commands
                .filter(|commands| {
                    !commands.is_empty()
                        && commands
                            .iter()
                            .all(|cmd| !cmd.is_empty() && cmd.iter().all(|word| !word.is_empty()))
                })
                .map(PowershellParseOutcome::Commands)
                .unwrap_or(PowershellParseOutcome::Unsupported),
            "unsupported" => PowershellParseOutcome::Unsupported,
            _ => PowershellParseOutcome::Failed,
        }
    }
}

fn kill_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
#[path = "powershell_parser_trust_tests.rs"]
mod trust_tests;

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn production_resolver_handles_multiple_windows_powershell_requests() {
        let Some(powershell) = trusted_windows_powershell_invocation_path() else {
            panic!("Windows PowerShell must exist at the authoritative System known folder");
        };
        let Some(powershell) = powershell.to_str() else {
            panic!("the Windows System known folder must be valid UTF-8");
        };

        let first = try_parse_powershell_ast_commands(powershell, "Get-Content 'foo bar'")
            .map(PowershellParseOutcome::Commands)
            .unwrap();
        assert_eq!(
            first,
            PowershellParseOutcome::Commands(vec![vec![
                "Get-Content".to_string(),
                "foo bar".to_string(),
            ]]),
        );

        let second =
            try_parse_powershell_ast_commands(powershell, "Write-Output foo | Measure-Object")
                .map(PowershellParseOutcome::Commands)
                .unwrap();
        assert_eq!(
            second,
            PowershellParseOutcome::Commands(vec![
                vec!["Write-Output".to_string(), "foo".to_string()],
                vec!["Measure-Object".to_string()],
            ]),
        );
    }

    #[test]
    fn production_resolver_uses_standard_machine_wide_pwsh_when_installed() {
        let Some(pwsh) = trusted_standard_pwsh_invocation_path() else {
            eprintln!(
                "skipping standard pwsh trust test because Program Files\\PowerShell\\7\\pwsh.exe is not installed"
            );
            return;
        };
        let Some(pwsh) = pwsh.to_str() else {
            panic!("the Program Files known folder must be valid UTF-8");
        };

        assert_eq!(
            try_parse_powershell_ast_commands(pwsh, "pwd && ls"),
            Some(vec![vec!["pwd".to_string()], vec!["ls".to_string()]])
        );
    }

    fn trusted_windows_powershell_parser() -> PathBuf {
        let Some(invocation_path) = trusted_windows_powershell_invocation_path() else {
            panic!("Windows PowerShell must exist at the authoritative System known folder");
        };
        let Some(invocation_path_str) = invocation_path.to_str() else {
            panic!("the Windows System known folder must be valid UTF-8");
        };
        let Some(parser) = trusted_powershell_parser_executable(invocation_path_str) else {
            panic!("the production trust resolver must accept Windows PowerShell");
        };
        parser
    }

    #[test]
    fn parser_process_rejects_stop_parsing_forms() {
        let powershell = trusted_windows_powershell_parser();
        let mut parser = PowershellParserProcess::spawn(&powershell).unwrap();

        let parsed = parser
            .parse("git log --% HEAD --output=codex_poc.txt")
            .unwrap();
        assert_eq!(parsed, PowershellParseOutcome::Unsupported);
    }

    #[test]
    fn parser_process_rejects_param_blocks() {
        let powershell = trusted_windows_powershell_parser();
        let mut parser = PowershellParserProcess::spawn(&powershell).unwrap();

        let parsed = parser
            .parse("param([string]$path = (Get-Location)) Write-Output test")
            .unwrap();
        assert_eq!(parsed, PowershellParseOutcome::Unsupported);
    }

    #[test]
    fn parser_process_rejects_named_blocks() {
        let powershell = trusted_windows_powershell_parser();
        let mut parser = PowershellParserProcess::spawn(&powershell).unwrap();

        let parsed = parser
            .parse("begin { Set-Content codex_poc.txt pwned } end { Get-Content Cargo.toml }")
            .unwrap();
        assert_eq!(parsed, PowershellParseOutcome::Unsupported);
    }

    #[test]
    fn parser_process_rejects_using_statements() {
        let powershell = trusted_windows_powershell_parser();
        let mut parser = PowershellParserProcess::spawn(&powershell).unwrap();

        let parsed = parser
            .parse("using module ./codex_poc.psm1\nGet-Content Cargo.toml")
            .unwrap();
        assert_eq!(parsed, PowershellParseOutcome::Unsupported);
    }

    #[test]
    fn parser_process_rejects_trap_blocks() {
        let powershell = trusted_windows_powershell_parser();
        let mut parser = PowershellParserProcess::spawn(&powershell).unwrap();

        let parsed = parser
            .parse(
                "trap { Set-Content codex_poc.txt pwned; continue } Get-Content missing -ErrorAction Stop",
            )
            .unwrap();
        assert_eq!(parsed, PowershellParseOutcome::Unsupported);
    }
}
