use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::io::BufRead;
use std::io::BufReader;
use std::io::ErrorKind;
use std::io::Write;
use std::process::Child;
use std::process::ChildStdin;
use std::process::ChildStdout;
use std::process::Command;
use std::process::Stdio;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::PoisonError;

const POWERSHELL_PARSER_SCRIPT: &str = include_str!("powershell_parser.ps1");

/// Cache one long-lived parser process per executable path so repeated safety checks reuse
/// PowerShell startup work while still consulting the real parser every time.
///
/// We keep the cache behind one mutex because each child process speaks a simple
/// request/response protocol over a single stdin/stdout pair, so callers targeting the same
/// executable must serialize access anyway.
pub(super) fn parse_with_powershell_ast(executable: &str, script: &str) -> PowershellParseOutcome {
    static PARSER_PROCESSES: LazyLock<Mutex<HashMap<String, PowershellParserProcess>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));

    let mut parser_processes = PARSER_PROCESSES
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    parse_with_cached_process(&mut parser_processes, executable, script)
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum PowershellParseOutcome {
    Commands(Vec<Vec<String>>),
    Unsupported,
    Failed,
}

fn parse_with_cached_process(
    parser_processes: &mut HashMap<String, PowershellParserProcess>,
    executable: &str,
    script: &str,
) -> PowershellParseOutcome {
    // `powershell.exe` and `pwsh.exe` do not accept the same language surface, so each
    // executable keeps its own parser process and request stream.
    let parser_key = executable.to_string();
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
    fn spawn(executable: &str) -> std::io::Result<Self> {
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

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::powershell::try_find_powershell_executable_blocking;
    use pretty_assertions::assert_eq;

    #[test]
    fn parser_process_handles_multiple_requests() {
        let Some(powershell) = try_find_powershell_executable_blocking() else {
            return;
        };
        let powershell = powershell.as_path().to_str().unwrap();
        let mut parser = PowershellParserProcess::spawn(powershell).unwrap();

        let first = parser.parse("Get-Content 'foo bar'").unwrap();
        assert_eq!(
            first,
            PowershellParseOutcome::Commands(vec![vec![
                "Get-Content".to_string(),
                "foo bar".to_string(),
            ]]),
        );

        let second = parser.parse("Write-Output foo | Measure-Object").unwrap();
        assert_eq!(
            second,
            PowershellParseOutcome::Commands(vec![
                vec!["Write-Output".to_string(), "foo".to_string()],
                vec!["Measure-Object".to_string()],
            ]),
        );
    }
}
