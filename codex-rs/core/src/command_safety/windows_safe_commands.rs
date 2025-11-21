use shlex::split as shlex_split;
use std::path::Path;

/// On Windows, we conservatively allow only clearly read-only PowerShell invocations
/// that match a small safelist. Anything else (including direct CMD commands) is unsafe.
pub fn is_safe_command_windows(command: &[String]) -> bool {
    if let Some(commands) = try_parse_powershell_command_sequence(command) {
        return commands
            .iter()
            .all(|cmd| is_safe_powershell_command(cmd.as_slice()));
    }
    // Only PowerShell invocations are allowed on Windows for now; anything else is unsafe.
    false
}

/// Returns each command sequence if the invocation starts with a PowerShell binary.
/// For example, the tokens from `pwsh Get-ChildItem | Measure-Object` become two sequences.
fn try_parse_powershell_command_sequence(command: &[String]) -> Option<Vec<Vec<String>>> {
    let (exe, rest) = command.split_first()?;
    if !is_powershell_executable(exe) {
        return None;
    }
    parse_powershell_invocation(rest)
}

/// Parses a PowerShell invocation into discrete command vectors, rejecting unsafe patterns.
fn parse_powershell_invocation(args: &[String]) -> Option<Vec<Vec<String>>> {
    if args.is_empty() {
        // Examples rejected here: "pwsh" and "powershell.exe" with no additional arguments.
        return None;
    }

    let mut idx = 0;
    while idx < args.len() {
        let arg = &args[idx];
        let lower = arg.to_ascii_lowercase();
        match lower.as_str() {
            "-command" | "/command" | "-c" => {
                let script = args.get(idx + 1)?;
                if idx + 2 != args.len() {
                    // Reject if there is more than one token representing the actual command.
                    // Examples rejected here: "pwsh -Command foo bar" and "powershell -c ls extra".
                    return None;
                }
                return parse_powershell_script(script);
            }
            _ if lower.starts_with("-command:") || lower.starts_with("/command:") => {
                if idx + 1 != args.len() {
                    // Reject if there are more tokens after the command itself.
                    // Examples rejected here: "pwsh -Command:dir C:\\" and "powershell /Command:dir C:\\" with trailing args.
                    return None;
                }
                let script = arg.split_once(':')?.1;
                return parse_powershell_script(script);
            }

            // Benign, no-arg flags we tolerate.
            "-nologo" | "-noprofile" | "-noninteractive" | "-mta" | "-sta" => {
                idx += 1;
                continue;
            }

            // Explicitly forbidden/opaque or unnecessary for read-only operations.
            "-encodedcommand" | "-ec" | "-file" | "/file" | "-windowstyle" | "-executionpolicy"
            | "-workingdirectory" => {
                // Examples rejected here: "pwsh -EncodedCommand ..." and "powershell -File script.ps1".
                return None;
            }

            // Unknown switch → bail conservatively.
            _ if lower.starts_with('-') => {
                // Examples rejected here: "pwsh -UnknownFlag" and "powershell -foo bar".
                return None;
            }

            // If we hit non-flag tokens, treat the remainder as a command sequence.
            // This happens if powershell is invoked without -Command, e.g.
            // ["pwsh", "-NoLogo", "git", "-c", "core.pager=cat", "status"]
            _ => {
                return split_into_commands(args[idx..].to_vec());
            }
        }
    }

    // Examples rejected here: "pwsh" and "powershell.exe -NoLogo" without a script.
    None
}

/// Tokenizes an inline PowerShell script and delegates to the command splitter.
/// Examples of when this is called: pwsh.exe -Command '<script>' or pwsh.exe -Command:<script>
fn parse_powershell_script(script: &str) -> Option<Vec<Vec<String>>> {
    let tokens = shlex_split(script)?;
    split_into_commands(tokens)
}

/// Splits tokens into pipeline segments while ensuring no unsafe separators slip through.
/// e.g. Get-ChildItem | Measure-Object -> [['Get-ChildItem'], ['Measure-Object']]
fn split_into_commands(tokens: Vec<String>) -> Option<Vec<Vec<String>>> {
    if tokens.is_empty() {
        // Examples rejected here: "pwsh -Command ''" and "powershell -Command \"\"".
        return None;
    }

    let mut commands = Vec::new();
    let mut current = Vec::new();
    for token in tokens.into_iter() {
        match token.as_str() {
            "|" | "||" | "&&" | ";" => {
                if current.is_empty() {
                    // Examples rejected here: "pwsh -Command '| Get-ChildItem'" and "pwsh -Command '; dir'".
                    return None;
                }
                commands.push(current);
                current = Vec::new();
            }
            // Reject if any token embeds separators, redirection, or call operator characters.
            _ if token.contains(['|', ';', '>', '<', '&']) || token.contains("$(") => {
                // Examples rejected here: "pwsh -Command 'dir|select'" and "pwsh -Command 'echo hi > out.txt'".
                return None;
            }
            _ => current.push(token),
        }
    }

    if current.is_empty() {
        // Examples rejected here: "pwsh -Command 'dir |'" and "pwsh -Command 'Get-ChildItem ;'".
        return None;
    }
    commands.push(current);
    Some(commands)
}

/// Returns true when the executable name is one of the supported PowerShell binaries.
fn is_powershell_executable(exe: &str) -> bool {
    let executable_name = Path::new(exe)
        .file_name()
        .and_then(|osstr| osstr.to_str())
        .unwrap_or(exe)
        .to_ascii_lowercase();

    matches!(
        executable_name.as_str(),
        "powershell" | "powershell.exe" | "pwsh" | "pwsh.exe"
    )
}

/// Validates that a parsed PowerShell command stays within our read-only safelist.
/// Everything before this is parsing, and rejecting things that make us feel uncomfortable.
fn is_safe_powershell_command(words: &[String]) -> bool {
    if words.is_empty() {
        // Examples rejected here: "pwsh -Command ''" and "pwsh -Command \"\"".
        return false;
    }

    // Reject nested unsafe cmdlets inside parentheses or arguments
    for w in words.iter() {
        let inner = w
            .trim_matches(|c| c == '(' || c == ')')
            .trim_start_matches('-')
            .to_ascii_lowercase();
        if matches!(
            inner.as_str(),
            "set-content"
                | "add-content"
                | "out-file"
                | "new-item"
                | "remove-item"
                | "move-item"
                | "copy-item"
                | "rename-item"
                | "start-process"
                | "stop-process"
        ) {
            // Examples rejected here: "Write-Output (Set-Content foo6.txt 'abc')" and "Get-Content (New-Item bar.txt)".
            return false;
        }
    }

    // Block PowerShell call operator or any redirection explicitly.
    if words.iter().any(|w| {
        matches!(
            w.as_str(),
            "&" | ">" | ">>" | "1>" | "2>" | "2>&1" | "*>" | "<" | "<<"
        )
    }) {
        // Examples rejected here: "pwsh -Command '& Remove-Item foo'" and "pwsh -Command 'Get-Content foo > bar'".
        return false;
    }

    let command = words[0]
        .trim_matches(|c| c == '(' || c == ')')
        .trim_start_matches('-')
        .to_ascii_lowercase();
    match command.as_str() {
        "echo" | "write-output" | "write-host" => true, // (no redirection allowed)
        "dir" | "ls" | "get-childitem" | "gci" => true,
        "cat" | "type" | "gc" | "get-content" => true,
        "select-string" | "sls" | "findstr" => true,
        "measure-object" | "measure" => true,
        "get-location" | "gl" | "pwd" => true,
        "test-path" | "tp" => true,
        "resolve-path" | "rvpa" => true,
        "select-object" | "select" => true,
        "get-item" => true,

        "git" => is_safe_git_command(words),

        "rg" => is_safe_ripgrep(words),

        // Extra safety: explicitly prohibit common side-effecting cmdlets regardless of args.
        "set-content" | "add-content" | "out-file" | "new-item" | "remove-item" | "move-item"
        | "copy-item" | "rename-item" | "start-process" | "stop-process" => {
            // Examples rejected here: "pwsh -Command 'Set-Content notes.txt data'" and "pwsh -Command 'Remove-Item temp.log'".
            false
        }

        _ => {
            // Examples rejected here: "pwsh -Command 'Invoke-WebRequest https://example.com'" and "pwsh -Command 'Start-Service Spooler'".
            false
        }
    }
}

/// Checks that an `rg` invocation avoids options that can spawn arbitrary executables.
fn is_safe_ripgrep(words: &[String]) -> bool {
    const UNSAFE_RIPGREP_OPTIONS_WITH_ARGS: &[&str] = &["--pre", "--hostname-bin"];
    const UNSAFE_RIPGREP_OPTIONS_WITHOUT_ARGS: &[&str] = &["--search-zip", "-z"];

    !words.iter().skip(1).any(|arg| {
        let arg_lc = arg.to_ascii_lowercase();
        // Examples rejected here: "pwsh -Command 'rg --pre cat pattern'" and "pwsh -Command 'rg --search-zip pattern'".
        UNSAFE_RIPGREP_OPTIONS_WITHOUT_ARGS.contains(&arg_lc.as_str())
            || UNSAFE_RIPGREP_OPTIONS_WITH_ARGS
                .iter()
                .any(|opt| arg_lc == *opt || arg_lc.starts_with(&format!("{opt}=")))
    })
}

/// Ensures a Git command sticks to whitelisted read-only subcommands and flags.
fn is_safe_git_command(words: &[String]) -> bool {
    const SAFE_SUBCOMMANDS: &[&str] = &["status", "log", "show", "diff", "cat-file"];

    let mut iter = words.iter().skip(1);
    while let Some(arg) = iter.next() {
        let arg_lc = arg.to_ascii_lowercase();

        if arg.starts_with('-') {
            if arg.eq_ignore_ascii_case("-c") || arg.eq_ignore_ascii_case("--config") {
                if iter.next().is_none() {
                    // Examples rejected here: "pwsh -Command 'git -c'" and "pwsh -Command 'git --config'".
                    return false;
                }
                continue;
            }

            if arg_lc.starts_with("-c=")
                || arg_lc.starts_with("--config=")
                || arg_lc.starts_with("--git-dir=")
                || arg_lc.starts_with("--work-tree=")
            {
                continue;
            }

            if arg.eq_ignore_ascii_case("--git-dir") || arg.eq_ignore_ascii_case("--work-tree") {
                if iter.next().is_none() {
                    // Examples rejected here: "pwsh -Command 'git --git-dir'" and "pwsh -Command 'git --work-tree'".
                    return false;
                }
                continue;
            }

            continue;
        }

        return SAFE_SUBCOMMANDS.contains(&arg_lc.as_str());
    }

    // Examples rejected here: "pwsh -Command 'git'" and "pwsh -Command 'git status --short | Remove-Item foo'".
    false
}

#[cfg(test)]
mod tests {
    use super::is_safe_command_windows;
    use std::string::ToString;

    /// Converts a slice of string literals into owned `String`s for the tests.
    fn vec_str(args: &[&str]) -> Vec<String> {
        args.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn recognizes_safe_powershell_wrappers() {
        assert!(is_safe_command_windows(&vec_str(&[
            "powershell.exe",
            "-NoLogo",
            "-Command",
            "Get-ChildItem -Path .",
        ])));

        assert!(is_safe_command_windows(&vec_str(&[
            "powershell.exe",
            "-NoProfile",
            "-Command",
            "git status",
        ])));

        assert!(is_safe_command_windows(&vec_str(&[
            "powershell.exe",
            "Get-Content",
            "Cargo.toml",
        ])));

        // pwsh parity
        assert!(is_safe_command_windows(&vec_str(&[
            "pwsh.exe",
            "-NoProfile",
            "-Command",
            "Get-ChildItem",
        ])));
    }

    #[test]
    fn accepts_full_path_powershell_invocations() {
        if !cfg!(windows) {
            // Windows only because on Linux path splitting doesn't handle `/` separators properly
            return;
        }

        assert!(is_safe_command_windows(&vec_str(&[
            r"C:\Program Files\PowerShell\7\pwsh.exe",
            "-NoProfile",
            "-Command",
            "Get-ChildItem -Path .",
        ])));

        assert!(is_safe_command_windows(&vec_str(&[
            r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe",
            "-Command",
            "Get-Content Cargo.toml",
        ])));
    }

    #[test]
    fn allows_read_only_pipelines_and_git_usage() {
        assert!(is_safe_command_windows(&vec_str(&[
            "pwsh",
            "-NoLogo",
            "-NoProfile",
            "-Command",
            "rg --files-with-matches foo | Measure-Object | Select-Object -ExpandProperty Count",
        ])));

        assert!(is_safe_command_windows(&vec_str(&[
            "pwsh",
            "-NoLogo",
            "-NoProfile",
            "-Command",
            "Get-Content foo.rs | Select-Object -Skip 200",
        ])));

        assert!(is_safe_command_windows(&vec_str(&[
            "pwsh",
            "-NoLogo",
            "-NoProfile",
            "-Command",
            "git -c core.pager=cat show HEAD:foo.rs",
        ])));

        assert!(is_safe_command_windows(&vec_str(&[
            "pwsh",
            "-Command",
            "-git cat-file -p HEAD:foo.rs",
        ])));

        assert!(is_safe_command_windows(&vec_str(&[
            "pwsh",
            "-Command",
            "(Get-Content foo.rs -Raw)",
        ])));

        assert!(is_safe_command_windows(&vec_str(&[
            "pwsh",
            "-Command",
            "Get-Item foo.rs | Select-Object Length",
        ])));
    }

    #[test]
    fn rejects_powershell_commands_with_side_effects() {
        assert!(!is_safe_command_windows(&vec_str(&[
            "powershell.exe",
            "-NoLogo",
            "-Command",
            "Remove-Item foo.txt",
        ])));

        assert!(!is_safe_command_windows(&vec_str(&[
            "powershell.exe",
            "-NoProfile",
            "-Command",
            "rg --pre cat",
        ])));

        assert!(!is_safe_command_windows(&vec_str(&[
            "powershell.exe",
            "-Command",
            "Set-Content foo.txt 'hello'",
        ])));

        // Redirections are blocked
        assert!(!is_safe_command_windows(&vec_str(&[
            "powershell.exe",
            "-Command",
            "echo hi > out.txt",
        ])));
        assert!(!is_safe_command_windows(&vec_str(&[
            "powershell.exe",
            "-Command",
            "Get-Content x | Out-File y",
        ])));
        assert!(!is_safe_command_windows(&vec_str(&[
            "powershell.exe",
            "-Command",
            "Write-Output foo 2> err.txt",
        ])));

        // Call operator is blocked
        assert!(!is_safe_command_windows(&vec_str(&[
            "powershell.exe",
            "-Command",
            "& Remove-Item foo",
        ])));

        // Chained safe + unsafe must fail
        assert!(!is_safe_command_windows(&vec_str(&[
            "powershell.exe",
            "-Command",
            "Get-ChildItem; Remove-Item foo",
        ])));
        // Nested unsafe cmdlet inside safe command must fail
        assert!(!is_safe_command_windows(&vec_str(&[
            "powershell.exe",
            "-Command",
            "Write-Output (Set-Content foo6.txt 'abc')",
        ])));
        // Additional nested unsafe cmdlet examples must fail
        assert!(!is_safe_command_windows(&vec_str(&[
            "powershell.exe",
            "-Command",
            "Write-Host (Remove-Item foo.txt)",
        ])));
        assert!(!is_safe_command_windows(&vec_str(&[
            "powershell.exe",
            "-Command",
            "Get-Content (New-Item bar.txt)",
        ])));
    }
}
