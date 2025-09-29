use shlex::split as shlex_split;

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

fn try_parse_powershell_command_sequence(command: &[String]) -> Option<Vec<Vec<String>>> {
    let (exe, rest) = command.split_first()?;
    if !is_powershell_executable(exe) {
        return None;
    }
    parse_powershell_invocation(rest)
}

fn parse_powershell_invocation(args: &[String]) -> Option<Vec<Vec<String>>> {
    if args.is_empty() {
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
                    return None;
                }
                return parse_powershell_script(script);
            }
            _ if lower.starts_with("-command:") || lower.starts_with("/command:") => {
                if idx + 1 != args.len() {
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
                return None;
            }

            // Unknown switch → bail conservatively.
            _ if lower.starts_with('-') => {
                return None;
            }

            // If we hit non-flag tokens, treat the remainder as a command sequence.
            _ => {
                return split_into_commands(args[idx..].to_vec());
            }
        }
    }

    None
}

fn parse_powershell_script(script: &str) -> Option<Vec<Vec<String>>> {
    let tokens = shlex_split(script)?;
    split_into_commands(tokens)
}

fn split_into_commands(tokens: Vec<String>) -> Option<Vec<Vec<String>>> {
    if tokens.is_empty() {
        return None;
    }

    let mut commands = Vec::new();
    let mut current = Vec::new();
    for token in tokens.into_iter() {
        match token.as_str() {
            "|" | "||" | "&&" | ";" => {
                if current.is_empty() {
                    return None;
                }
                commands.push(current);
                current = Vec::new();
            }
            // Reject if any token embeds separators, redirection, or call operator characters.
            _ if token.contains(['|', ';', '>', '<', '&']) || token.contains("$(") => {
                return None;
            }
            _ => current.push(token),
        }
    }

    if current.is_empty() {
        return None;
    }
    commands.push(current);
    Some(commands)
}

fn is_powershell_executable(exe: &str) -> bool {
    matches!(
        exe.to_ascii_lowercase().as_str(),
        "powershell" | "powershell.exe" | "pwsh" | "pwsh.exe"
    )
}

fn is_safe_powershell_command(words: &[String]) -> bool {
    if words.is_empty() {
        return false;
    }

    // Block PowerShell call operator or any redirection explicitly.
    if words.iter().any(|w| {
        matches!(
            w.as_str(),
            "&" | ">" | ">>" | "1>" | "2>" | "2>&1" | "*>" | "<" | "<<"
        )
    }) {
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
        | "copy-item" | "rename-item" | "start-process" | "stop-process" => false,

        _ => false,
    }
}

fn is_safe_ripgrep(words: &[String]) -> bool {
    const UNSAFE_RIPGREP_OPTIONS_WITH_ARGS: &[&str] = &["--pre", "--hostname-bin"];
    const UNSAFE_RIPGREP_OPTIONS_WITHOUT_ARGS: &[&str] = &["--search-zip", "-z"];

    !words.iter().skip(1).any(|arg| {
        let arg_lc = arg.to_ascii_lowercase();
        UNSAFE_RIPGREP_OPTIONS_WITHOUT_ARGS.contains(&arg_lc.as_str())
            || UNSAFE_RIPGREP_OPTIONS_WITH_ARGS
                .iter()
                .any(|opt| arg_lc == *opt || arg_lc.starts_with(&format!("{opt}=")))
    })
}

fn is_safe_git_command(words: &[String]) -> bool {
    const SAFE_SUBCOMMANDS: &[&str] = &["status", "log", "show", "diff", "cat-file"];

    let mut iter = words.iter().skip(1);
    while let Some(arg) = iter.next() {
        let arg_lc = arg.to_ascii_lowercase();

        if arg.starts_with('-') {
            if arg.eq_ignore_ascii_case("-c") || arg.eq_ignore_ascii_case("--config") {
                if iter.next().is_none() {
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
                    return false;
                }
                continue;
            }

            continue;
        }

        return SAFE_SUBCOMMANDS.contains(&arg_lc.as_str());
    }

    false
}

#[cfg(test)]
mod tests {
    use super::is_safe_command_windows;
    use std::string::ToString;

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
    }
}
