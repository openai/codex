use codex_utils_absolute_path::AbsolutePathBuf;

use crate::command_safety::PowershellParseOutcome;
use crate::command_safety::TrustedPowerShellFlavor;
use crate::command_safety::is_trusted_powershell_parser_executable;
use crate::command_safety::parse_powershell_ast_commands_with_trusted_flavor;
use crate::command_safety::try_parse_powershell_ast_commands;

const POWERSHELL_FLAGS: &[&str] = &["-nologo", "-noprofile", "-command", "-c"];

/// Prefixed command for powershell shell calls to request UTF-8 console output.
pub const UTF8_OUTPUT_PREFIX: &str =
    "try { [Console]::OutputEncoding=[System.Text.Encoding]::UTF8 } catch {}\n";

pub fn prefix_powershell_script_with_utf8(command: &[String]) -> Vec<String> {
    let Some((_, script)) = extract_powershell_command(command) else {
        return command.to_vec();
    };

    let trimmed = script.trim_start();
    let script = if trimmed.starts_with(UTF8_OUTPUT_PREFIX) {
        script.to_string()
    } else {
        format!("{UTF8_OUTPUT_PREFIX}{script}")
    };

    let mut command: Vec<String> = command[..(command.len() - 1)]
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
    command.push(script);
    command
}

/// Extract the PowerShell script body from an invocation such as:
///
/// - ["pwsh", "-NoProfile", "-Command", "Get-ChildItem -Recurse | Select-String foo"]
/// - ["powershell.exe", "-Command", "Write-Host hi"]
/// - ["powershell", "-NoLogo", "-NoProfile", "-Command", "...script..."]
///
/// Returns (`shell`, `script`) when the first arg is a PowerShell executable and a
/// `-Command` (or `-c`) flag is present followed by a script string.
pub fn extract_powershell_command(command: &[String]) -> Option<(&str, &str)> {
    if command.len() < 3 {
        return None;
    }

    let shell = &command[0];
    powershell_flavor_for_executable(shell)?;

    // Find the first occurrence of -Command (accept common short alias -c as well)
    let mut i = 1usize;
    while i + 1 < command.len() {
        let flag = &command[i];
        // Reject unknown flags
        if !POWERSHELL_FLAGS.contains(&flag.to_ascii_lowercase().as_str()) {
            return None;
        }
        if flag.eq_ignore_ascii_case("-Command") || flag.eq_ignore_ascii_case("-c") {
            if i + 2 != command.len() {
                return None;
            }
            let script = &command[i + 1];
            return Some((shell, script));
        }
        i += 1;
    }
    None
}

/// Parse the script body from a top-level PowerShell wrapper into argv-like commands.
///
/// This is intentionally narrower than the Windows safe-command parser: it only unwraps the
/// `-Command`/`-c` body from a PowerShell invocation we already recognize, then delegates the
/// script itself to the PowerShell AST parser.
pub fn parse_powershell_command_into_plain_commands(
    command: &[String],
) -> Option<Vec<Vec<String>>> {
    let (executable, script) = extract_powershell_command(command)?;
    try_parse_powershell_ast_commands(executable, script)
}

/// Selects which protected machine-wide PowerShell installation parses a script.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PowerShellFlavor {
    WindowsPowerShell,
    PowerShell7,
}

/// Result of inspecting a PowerShell wrapper for exec-policy evaluation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PowerShellExecPolicyParse {
    /// The runtime wrapper is the same protected executable used for parsing.
    TrustedRuntime {
        outcome: PowerShellExecPolicyParseOutcome,
    },
    /// A protected parser inspected the body, but the runtime wrapper remains untrusted.
    UntrustedRuntime {
        outcome: PowerShellExecPolicyParseOutcome,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PowerShellExecPolicyParseOutcome {
    /// The protected parser safely lowered the script into argv-like commands.
    Commands(Vec<Vec<String>>),
    /// The script or wrapper is valid but outside the safe lowering subset.
    Unsupported,
    /// The protected parser could not return an authoritative result.
    Failed,
}

/// Inspects a PowerShell wrapper without spawning its model-selected runtime.
pub fn parse_powershell_command_for_exec_policy(
    command: &[String],
) -> Option<PowerShellExecPolicyParse> {
    let executable = command.first()?;
    let flavor = powershell_flavor_for_executable(executable)?;
    let outcome = extract_powershell_command(command)
        .map(|(_, script)| parse_powershell_script_with_trusted_parser_outcome(flavor, script))
        .unwrap_or(PowerShellExecPolicyParseOutcome::Unsupported);
    if is_trusted_powershell_parser_executable(executable) {
        Some(PowerShellExecPolicyParse::TrustedRuntime { outcome })
    } else {
        Some(PowerShellExecPolicyParse::UntrustedRuntime { outcome })
    }
}

fn powershell_flavor_for_executable(executable: &str) -> Option<PowerShellFlavor> {
    let mut executable_name = std::path::Path::new(executable).file_name()?.to_str()?;
    #[cfg(windows)]
    {
        executable_name = executable_name.trim_end_matches([' ', '.']);
    }
    loop {
        let executable_stem = std::path::Path::new(executable_name)
            .file_stem()?
            .to_str()?;
        if executable_stem.eq_ignore_ascii_case("powershell") {
            return Some(PowerShellFlavor::WindowsPowerShell);
        } else if executable_stem.eq_ignore_ascii_case("pwsh") {
            return Some(PowerShellFlavor::PowerShell7);
        } else if executable_stem == executable_name {
            return None;
        }
        executable_name = executable_stem;
    }
}

/// Parses a PowerShell script without using the runtime wrapper as the parser executable.
///
/// The selected parser comes from the authoritative Windows System or Program Files known folder.
/// On unsupported hosts, or when the protected parser is unavailable, this fails closed.
pub fn parse_powershell_script_with_trusted_parser(
    flavor: PowerShellFlavor,
    script: &str,
) -> Option<Vec<Vec<String>>> {
    match parse_powershell_script_with_trusted_parser_outcome(flavor, script) {
        PowerShellExecPolicyParseOutcome::Commands(commands) => Some(commands),
        PowerShellExecPolicyParseOutcome::Unsupported
        | PowerShellExecPolicyParseOutcome::Failed => None,
    }
}

fn parse_powershell_script_with_trusted_parser_outcome(
    flavor: PowerShellFlavor,
    script: &str,
) -> PowerShellExecPolicyParseOutcome {
    let trusted_flavor = match flavor {
        PowerShellFlavor::WindowsPowerShell => TrustedPowerShellFlavor::WindowsPowerShell,
        PowerShellFlavor::PowerShell7 => TrustedPowerShellFlavor::PowerShell7,
    };
    match parse_powershell_ast_commands_with_trusted_flavor(trusted_flavor, script) {
        PowershellParseOutcome::Commands(commands) => {
            PowerShellExecPolicyParseOutcome::Commands(commands)
        }
        PowershellParseOutcome::Unsupported => PowerShellExecPolicyParseOutcome::Unsupported,
        PowershellParseOutcome::Failed => PowerShellExecPolicyParseOutcome::Failed,
    }
}

/// This function attempts to find a powershell.exe executable on the system.
pub fn try_find_powershell_executable_blocking() -> Option<AbsolutePathBuf> {
    try_find_powershellish_executable_in_path(&["powershell.exe"])
}

/// This function attempts to find a pwsh.exe executable on the system.
/// Note that pwsh.exe and powershell.exe are different executables:
///
/// - pwsh.exe is the cross-platform PowerShell Core (v6+) executable
/// - powershell.exe is the Windows PowerShell (v5.1 and earlier) executable
///
/// Further, while powershell.exe is included by default on Windows systems,
/// pwsh.exe must be installed separately by the user. And even when the user
/// has installed pwsh.exe, it may not be available in the system PATH, in which
/// case we attempt to locate it via other means.
pub fn try_find_pwsh_executable_blocking() -> Option<AbsolutePathBuf> {
    if let Some(ps_home) = std::process::Command::new("cmd")
        .args(["/C", "pwsh", "-NoProfile", "-Command", "$PSHOME"])
        .output()
        .ok()
        .and_then(|out| {
            if !out.status.success() {
                return None;
            }
            let stdout = String::from_utf8_lossy(&out.stdout);
            let trimmed = stdout.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        })
    {
        let candidate = AbsolutePathBuf::resolve_path_against_base("pwsh.exe", &ps_home);

        if is_powershellish_executable_available(candidate.as_path()) {
            return Some(candidate);
        }
    }

    try_find_powershellish_executable_in_path(&["pwsh.exe"])
}

fn try_find_powershellish_executable_in_path(candidates: &[&str]) -> Option<AbsolutePathBuf> {
    for candidate in candidates {
        let Ok(resolved_path) = which::which(candidate) else {
            continue;
        };

        if !is_powershellish_executable_available(&resolved_path) {
            continue;
        }

        let Ok(abs_path) = AbsolutePathBuf::from_absolute_path(resolved_path) else {
            continue;
        };

        return Some(abs_path);
    }

    None
}

fn is_powershellish_executable_available(powershell_or_pwsh_exe: &std::path::Path) -> bool {
    // This test works for both powershell.exe and pwsh.exe.
    std::process::Command::new(powershell_or_pwsh_exe)
        .args(["-NoLogo", "-NoProfile", "-Command", "Write-Output ok"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use super::PowerShellExecPolicyParse;
    #[cfg(windows)]
    use super::PowerShellExecPolicyParseOutcome;
    #[cfg(windows)]
    use super::PowerShellFlavor;
    use super::UTF8_OUTPUT_PREFIX;
    use super::extract_powershell_command;
    #[cfg(windows)]
    use super::parse_powershell_command_for_exec_policy;
    #[cfg(windows)]
    use super::parse_powershell_command_into_plain_commands;
    #[cfg(windows)]
    use super::parse_powershell_script_with_trusted_parser;
    #[cfg(windows)]
    use super::powershell_flavor_for_executable;
    use super::prefix_powershell_script_with_utf8;

    #[cfg(windows)]
    fn trusted_windows_powershell_executable() -> String {
        crate::command_safety::trusted_windows_powershell_invocation_path()
            .expect("Windows PowerShell must exist at the authoritative System known folder")
            .to_str()
            .expect("the Windows System known folder must be valid UTF-8")
            .to_string()
    }

    #[test]
    fn extracts_basic_powershell_command() {
        let cmd = vec![
            "powershell".to_string(),
            "-Command".to_string(),
            "Write-Host hi".to_string(),
        ];
        let (_shell, script) = extract_powershell_command(&cmd).expect("extract");
        assert_eq!(script, "Write-Host hi");
    }

    #[test]
    fn extracts_lowercase_flags() {
        let cmd = vec![
            "powershell".to_string(),
            "-nologo".to_string(),
            "-command".to_string(),
            "Write-Host hi".to_string(),
        ];
        let (_shell, script) = extract_powershell_command(&cmd).expect("extract");
        assert_eq!(script, "Write-Host hi");
    }

    #[test]
    fn extracts_full_path_powershell_command() {
        #[cfg(windows)]
        let command = trusted_windows_powershell_executable();
        #[cfg(not(windows))]
        let command = "/usr/local/bin/powershell.exe".to_string();
        let cmd = vec![command, "-Command".to_string(), "Write-Host hi".to_string()];
        let (_shell, script) = extract_powershell_command(&cmd).expect("extract");
        assert_eq!(script, "Write-Host hi");
    }

    #[test]
    fn extracts_with_noprofile_and_alias() {
        let cmd = vec![
            "pwsh".to_string(),
            "-NoProfile".to_string(),
            "-c".to_string(),
            "Get-ChildItem | Select-String foo".to_string(),
        ];
        let (_shell, script) = extract_powershell_command(&cmd).expect("extract");
        assert_eq!(script, "Get-ChildItem | Select-String foo");
    }

    #[test]
    fn rejects_arguments_after_powershell_script() {
        let command = vec![
            "powershell.exe".to_string(),
            "-Command".to_string(),
            "Write-Host hi".to_string(),
            "unexpected".to_string(),
        ];

        assert_eq!(extract_powershell_command(&command), None);
    }

    #[test]
    fn prefixes_powershell_command_with_best_effort_utf8() {
        let cmd = vec![
            "powershell".to_string(),
            "-Command".to_string(),
            "Write-Host hi".to_string(),
        ];

        let prefixed = prefix_powershell_script_with_utf8(&cmd);

        assert_eq!(
            prefixed,
            vec![
                "powershell".to_string(),
                "-Command".to_string(),
                format!("{UTF8_OUTPUT_PREFIX}Write-Host hi"),
            ]
        );
    }

    #[test]
    fn does_not_duplicate_utf8_prefix() {
        let cmd = vec![
            "powershell".to_string(),
            "-Command".to_string(),
            format!("{UTF8_OUTPUT_PREFIX}Write-Host hi"),
        ];

        assert_eq!(prefix_powershell_script_with_utf8(&cmd), cmd);
    }

    #[cfg(windows)]
    #[test]
    fn parses_plain_powershell_commands() {
        let commands = parse_powershell_command_into_plain_commands(&[
            trusted_windows_powershell_executable(),
            "-NoProfile".to_string(),
            "-Command".to_string(),
            "echo hi".to_string(),
        ])
        .expect("parse");

        assert_eq!(commands, vec![vec!["echo".to_string(), "hi".to_string()]]);
    }

    #[cfg(windows)]
    #[test]
    fn parses_multiple_plain_powershell_commands() {
        let commands = parse_powershell_command_into_plain_commands(&[
            trusted_windows_powershell_executable(),
            "-NoProfile".to_string(),
            "-Command".to_string(),
            "Write-Output foo | Measure-Object".to_string(),
        ])
        .expect("parse");

        assert_eq!(
            commands,
            vec![
                vec!["Write-Output".to_string(), "foo".to_string()],
                vec!["Measure-Object".to_string()],
            ]
        );
    }

    #[cfg(windows)]
    #[test]
    fn parses_with_authoritative_parser_selected_independently_of_runtime_wrapper() {
        assert_eq!(
            parse_powershell_script_with_trusted_parser(
                PowerShellFlavor::WindowsPowerShell,
                "Write-Output windows",
            ),
            Some(vec![vec![
                "Write-Output".to_string(),
                "windows".to_string(),
            ]]),
        );

        if crate::command_safety::trusted_standard_pwsh_invocation_path().is_some() {
            assert_eq!(
                parse_powershell_script_with_trusted_parser(
                    PowerShellFlavor::PowerShell7,
                    "Write-Output core",
                ),
                Some(vec![vec!["Write-Output".to_string(), "core".to_string(),]]),
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn exec_policy_parse_preserves_runtime_trust_and_aliases() {
        assert_eq!(
            powershell_flavor_for_executable(r"\\.\UNC\server\share\pwsh.EXE.bAt "),
            Some(PowerShellFlavor::PowerShell7)
        );
        let trusted = trusted_windows_powershell_executable();
        let parsed = PowerShellExecPolicyParseOutcome::Commands(vec![vec![
            "echo".to_string(),
            "classified".to_string(),
        ]]);
        let cases = [
            (trusted, true),
            ("powershell.exe".to_string(), false),
            ("powershell.EXE.CmD".to_string(), false),
            (r".\tools\powershell.exe".to_string(), false),
            (r"C:\workspace\powershell.exe".to_string(), false),
            ("powershell.exe.".to_string(), false),
            ("powershell.exe ".to_string(), false),
            (r"\\?\C:\workspace\powershell.exe.".to_string(), false),
            (r"\\.\C:\workspace\powershell.exe ".to_string(), false),
            (r"\\.\UNC\server\share\powershell.exe ".to_string(), false),
        ];

        for (executable, trusted) in cases {
            let result = parse_powershell_command_for_exec_policy(&[
                executable.clone(),
                "-NoProfile".to_string(),
                "-Command".to_string(),
                "echo classified".to_string(),
            ]);
            let expected = if trusted {
                PowerShellExecPolicyParse::TrustedRuntime {
                    outcome: parsed.clone(),
                }
            } else {
                PowerShellExecPolicyParse::UntrustedRuntime {
                    outcome: parsed.clone(),
                }
            };
            assert_eq!(result, Some(expected), "runtime {executable:?}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn ordinary_trailing_aliases_launch_but_remain_untrusted() {
        let trusted = trusted_windows_powershell_executable();
        for executable in [format!("{trusted}."), format!("{trusted} ")] {
            assert!(super::is_powershellish_executable_available(
                std::path::Path::new(&executable)
            ));
            assert!(matches!(
                parse_powershell_command_for_exec_policy(&[
                    executable,
                    "-Command".to_string(),
                    "echo launchable".to_string(),
                ]),
                Some(PowerShellExecPolicyParse::UntrustedRuntime {
                    outcome: PowerShellExecPolicyParseOutcome::Commands(_)
                })
            ));
        }
    }

    #[cfg(windows)]
    #[test]
    fn exec_policy_parse_marks_unsupported_bodies_opaque() {
        let command = |executable: &str, args: &[&str]| {
            std::iter::once(executable.to_string())
                .chain(args.iter().map(|arg| (*arg).to_string()))
                .collect::<Vec<_>>()
        };
        let cases = [
            (
                command(
                    &trusted_windows_powershell_executable(),
                    &["-Command", "param([string]$path) echo blocked"],
                ),
                PowerShellExecPolicyParse::TrustedRuntime {
                    outcome: PowerShellExecPolicyParseOutcome::Unsupported,
                },
            ),
            (
                command(
                    "powershell.exe",
                    &["-NonInteractive", "-Command", "echo blocked"],
                ),
                PowerShellExecPolicyParse::UntrustedRuntime {
                    outcome: PowerShellExecPolicyParseOutcome::Unsupported,
                },
            ),
            (
                command("powershell.exe", &["-Command", "echo blocked", "trailing"]),
                PowerShellExecPolicyParse::UntrustedRuntime {
                    outcome: PowerShellExecPolicyParseOutcome::Unsupported,
                },
            ),
        ];

        for (command, expected) in cases {
            assert_eq!(
                parse_powershell_command_for_exec_policy(&command),
                Some(expected),
                "command {command:?}"
            );
        }
    }
}
