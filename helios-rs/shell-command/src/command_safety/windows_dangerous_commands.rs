use std::path::Path;

use once_cell::sync::Lazy;
use regex::Regex;
use shlex::split as shlex_split;
use url::Url;

pub fn is_dangerous_command_windows(command: &[String]) -> bool {
    // Prefer structured parsing for PowerShell/CMD so we can spot URL-bearing
    // invocations of ShellExecute-style entry points before falling back to
    // simple argv heuristics.
    if is_dangerous_powershell(command) {
        return true;
    }

    if is_dangerous_cmd(command) {
        return true;
    }

    is_direct_gui_launch(command)
}

fn is_dangerous_powershell(command: &[String]) -> bool {
    let Some((exe, rest)) = command.split_first() else {
        return false;
    };
    if !is_powershell_executable(exe) {
        return false;
    }
    // Parse the PowerShell invocation to get a flat token list we can scan for
    // dangerous cmdlets/COM calls plus any URL-looking arguments. This is a
    // best-effort shlex split of the script text, not a full PS parser.
    let Some(parsed) = parse_powershell_invocation(rest) else {
        return false;
    };

    let tokens_lc: Vec<String> = parsed
        .tokens
        .iter()
        .map(|t| t.trim_matches('\'').trim_matches('"').to_ascii_lowercase())
        .collect();
    let has_url = args_have_url(&parsed.tokens);

    if has_url
        && tokens_lc.iter().any(|t| {
            matches!(
                t.as_str(),
                "start-process" | "start" | "saps" | "invoke-item" | "ii"
            ) || t.contains("start-process")
                || t.contains("invoke-item")
        })
    {
        return true;
    }

    if has_url
        && tokens_lc
            .iter()
            .any(|t| t.contains("shellexecute") || t.contains("shell.application"))
    {
        return true;
    }

    if let Some(first) = tokens_lc.first() {
        // Legacy ShellExecute path via url.dll
        if first == "rundll32"
            && tokens_lc
                .iter()
                .any(|t| t.contains("url.dll,fileprotocolhandler"))
            && has_url
        {
            return true;
        }
        if first == "mshta" && has_url {
            return true;
        }
        if is_browser_executable(first) && has_url {
            return true;
        }
        if matches!(first.as_str(), "explorer" | "explorer.exe") && has_url {
            return true;
        }
    }

    // Check for force delete operations (e.g., Remove-Item -Force)
    if has_force_delete_cmdlet(&tokens_lc) {
        return true;
    }

    false
}

fn is_dangerous_cmd(command: &[String]) -> bool {
    let Some((exe, rest)) = command.split_first() else {
        return false;
    };
    let Some(base) = executable_basename(exe) else {
        return false;
    };
    if base != "cmd" && base != "cmd.exe" {
        return false;
    }

    let mut iter = rest.iter();
    for arg in iter.by_ref() {
        let lower = arg.to_ascii_lowercase();
        match lower.as_str() {
            "/c" | "/r" | "-c" => break,
            _ if lower.starts_with('/') => continue,
            // Unknown tokens before the command body => bail.
            _ => return false,
        }
    }

    let remaining: Vec<String> = iter.cloned().collect();
    if remaining.is_empty() {
        return false;
    }

    let cmd_tokens: Vec<String> = match remaining.as_slice() {
        [only] => shlex_split(only).unwrap_or_else(|| vec![only.clone()]),
        _ => remaining,
    };

    // Refine tokens by splitting concatenated CMD operators (e.g. "echo hi&del")
    let tokens: Vec<String> = cmd_tokens
        .into_iter()
        .flat_map(|t| split_embedded_cmd_operators(&t))
        .collect();

    const CMD_SEPARATORS: &[&str] = &["&", "&&", "|", "||"];
    tokens
        .split(|t| CMD_SEPARATORS.contains(&t.as_str()))
        .any(|segment| {
            let Some(cmd) = segment.first() else {
                return false;
            };

            // Classic `cmd /c ... start https://...` ShellExecute path.
            if cmd.eq_ignore_ascii_case("start") && args_have_url(segment) {
                return true;
            }
            // Force delete: del /f, erase /f
            if (cmd.eq_ignore_ascii_case("del") || cmd.eq_ignore_ascii_case("erase"))
                && has_force_flag_cmd(segment)
            {
                return true;
            }
            // Recursive directory removal: rd /s /q, rmdir /s /q
            if (cmd.eq_ignore_ascii_case("rd") || cmd.eq_ignore_ascii_case("rmdir"))
                && has_recursive_flag_cmd(segment)
                && has_quiet_flag_cmd(segment)
            {
                return true;
            }
            false
        })
}

fn is_direct_gui_launch(command: &[String]) -> bool {
    let Some((exe, rest)) = command.split_first() else {
        return false;
    };
    let Some(base) = executable_basename(exe) else {
        return false;
    };

    // Explorer/rundll32/mshta or direct browser exe with a URL anywhere in args.
    if matches!(base.as_str(), "explorer" | "explorer.exe") && args_have_url(rest) {
        return true;
    }
    if matches!(base.as_str(), "mshta" | "mshta.exe") && args_have_url(rest) {
        return true;
    }
    if (base == "rundll32" || base == "rundll32.exe")
        && rest.iter().any(|t| {
            t.to_ascii_lowercase()
                .contains("url.dll,fileprotocolhandler")
        })
        && args_have_url(rest)
    {
        return true;
    }
    if is_browser_executable(&base) && args_have_url(rest) {
        return true;
    }

    false
}

fn split_embedded_cmd_operators(token: &str) -> Vec<String> {
    // Split concatenated CMD operators so `echo hi&del` becomes `["echo hi", "&", "del"]`.
    // Handles `&`, `&&`, `|`, `||`. Best-effort (CMD escaping is weird by nature).
    let mut parts = Vec::new();
    let mut start = 0;
    let mut it = token.char_indices().peekable();

    while let Some((i, ch)) = it.next() {
        if ch == '&' || ch == '|' {
            if i > start {
                parts.push(token[start..i].to_string());
            }

            // Detect doubled operator: && or ||
            let op_len = match it.peek() {
                Some(&(j, next)) if next == ch => {
                    it.next(); // consume second char
                    (j + next.len_utf8()) - i
                }
                _ => ch.len_utf8(),
            };

            parts.push(token[i..i + op_len].to_string());
            start = i + op_len;
        }
    }

    if start < token.len() {
        parts.push(token[start..].to_string());
    }

    parts.retain(|s| !s.trim().is_empty());
    parts
}

fn has_force_delete_cmdlet(tokens: &[String]) -> bool {
    const DELETE_CMDLETS: &[&str] = &["remove-item", "ri", "rm", "del", "erase", "rd", "rmdir"];

    // Hard separators that end a command segment (so -Force must be in same segment)
    const SEG_SEPS: &[char] = &[';', '|', '&', '\n', '\r', '\t'];

    // Soft separators: punctuation that can stick to tokens (blocks, parens, brackets, commas, etc.)
    const SOFT_SEPS: &[char] = &['{', '}', '(', ')', '[', ']', ',', ';'];

    // Build rough command segments first
    let mut segments: Vec<Vec<String>> = vec![Vec::new()];
    for tok in tokens {
        // If token itself contains segment separators, split it (best-effort)
        let mut cur = String::new();
        for ch in tok.chars() {
            if SEG_SEPS.contains(&ch) {
                let s = cur.trim();
                if let Some(msg) = segments.last_mut()
                    && !s.is_empty()
                {
                    msg.push(s.to_string());
                }
                cur.clear();
                if let Some(last) = segments.last()
                    && !last.is_empty()
                {
                    segments.push(Vec::new());
                }
            } else {
                cur.push(ch);
            }
        }
        let s = cur.trim();
        if let Some(segment) = segments.last_mut()
            && !s.is_empty()
        {
            segment.push(s.to_string());
        }
    }

    // Now, inside each segment, normalize tokens by splitting on soft punctuation
    segments.into_iter().any(|seg| {
        let atoms = seg
            .iter()
            .flat_map(|t| t.split(|c| SOFT_SEPS.contains(&c)))
            .map(str::trim)
            .filter(|s| !s.is_empty());

        let mut has_delete = false;
        let mut has_force = false;

        for a in atoms {
            if DELETE_CMDLETS.iter().any(|cmd| a.eq_ignore_ascii_case(cmd)) {
                has_delete = true;
            }
            if a.eq_ignore_ascii_case("-force")
                || a.get(..7)
                    .is_some_and(|p| p.eq_ignore_ascii_case("-force:"))
            {
                has_force = true;
            }
        }

        has_delete && has_force
    })
}

/// Check for /f or /F flag in CMD del/erase arguments.
fn has_force_flag_cmd(args: &[String]) -> bool {
    args.iter().any(|a| a.eq_ignore_ascii_case("/f"))
}

/// Check for /s or /S flag in CMD rd/rmdir arguments.
fn has_recursive_flag_cmd(args: &[String]) -> bool {
    args.iter().any(|a| a.eq_ignore_ascii_case("/s"))
}

/// Check for /q or /Q flag in CMD rd/rmdir arguments.
fn has_quiet_flag_cmd(args: &[String]) -> bool {
    args.iter().any(|a| a.eq_ignore_ascii_case("/q"))
}

fn args_have_url(args: &[String]) -> bool {
    args.iter().any(|arg| looks_like_url(arg))
}

fn looks_like_url(token: &str) -> bool {
    // Strip common PowerShell punctuation around inline URLs (quotes, parens, trailing semicolons).
    // Capture the middle token after trimming leading quotes/parens/whitespace and trailing semicolons/closing parens.
    static RE: Lazy<Option<Regex>> =
        Lazy::new(|| Regex::new(r#"^[ "'\(\s]*([^\s"'\);]+)[\s;\)]*$"#).ok());
    // If the token embeds a URL alongside other text (e.g., Start-Process('https://...'))
    // as a single shlex token, grab the substring starting at the first URL prefix.
    let urlish = token
        .find("https://")
        .or_else(|| token.find("http://"))
        .map(|idx| &token[idx..])
        .unwrap_or(token);

    let candidate = RE
        .as_ref()
        .and_then(|re| re.captures(urlish))
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str())
        .unwrap_or(urlish);
    let Ok(url) = Url::parse(candidate) else {
        return false;
    };
    matches!(url.scheme(), "http" | "https")
}

fn executable_basename(exe: &str) -> Option<String> {
    Path::new(exe)
        .file_name()
        .and_then(|osstr| osstr.to_str())
        .map(str::to_ascii_lowercase)
}

fn is_powershell_executable(exe: &str) -> bool {
    matches!(
        executable_basename(exe).as_deref(),
        Some("powershell") | Some("powershell.exe") | Some("pwsh") | Some("pwsh.exe")
    )
}

fn is_browser_executable(name: &str) -> bool {
    matches!(
        name,
        "chrome"
            | "chrome.exe"
            | "msedge"
            | "msedge.exe"
            | "firefox"
            | "firefox.exe"
            | "iexplore"
            | "iexplore.exe"
    )
}

struct ParsedPowershell {
    tokens: Vec<String>,
}

fn parse_powershell_invocation(args: &[String]) -> Option<ParsedPowershell> {
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
                let tokens = shlex_split(script)?;
                return Some(ParsedPowershell { tokens });
            }
            _ if lower.starts_with("-command:") || lower.starts_with("/command:") => {
                if idx + 1 != args.len() {
                    return None;
                }
                let (_, script) = arg.split_once(':')?;
                let tokens = shlex_split(script)?;
                return Some(ParsedPowershell { tokens });
            }
            "-nologo" | "-noprofile" | "-noninteractive" | "-mta" | "-sta" => {
                idx += 1;
            }
            _ if lower.starts_with('-') => {
                idx += 1;
            }
            _ => {
                let rest = args[idx..].to_vec();
                return Some(ParsedPowershell { tokens: rest });
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::is_dangerous_command_windows;

    fn vec_str(items: &[&str]) -> Vec<String> {
        items.iter().map(std::string::ToString::to_string).collect()
    }

    #[test]
    fn powershell_start_process_url_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "powershell",
            "-NoLogo",
            "-Command",
            "Start-Process 'https://example.com'"
        ])));
    }

    #[test]
    fn powershell_start_process_url_with_trailing_semicolon_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "powershell",
            "-Command",
            "Start-Process('https://example.com');"
        ])));
    }

    #[test]
    fn powershell_start_process_local_is_not_flagged() {
        assert!(!is_dangerous_command_windows(&vec_str(&[
            "powershell",
            "-Command",
            "Start-Process notepad.exe"
        ])));
    }

    #[test]
    fn cmd_start_with_url_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "cmd",
            "/c",
            "start",
            "https://example.com"
        ])));
    }

    #[test]
    fn msedge_with_url_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "msedge.exe",
            "https://example.com"
        ])));
    }

    #[test]
    fn explorer_with_directory_is_not_flagged() {
        assert!(!is_dangerous_command_windows(&vec_str(&[
            "explorer.exe",
            "."
        ])));
    }

    // Force delete tests for PowerShell

    #[test]
    fn powershell_remove_item_force_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "powershell",
            "-Command",
            "Remove-Item test -Force"
        ])));
    }

    #[test]
    fn powershell_remove_item_recurse_force_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "powershell",
            "-Command",
            "Remove-Item test -Recurse -Force"
        ])));
    }

    #[test]
    fn powershell_ri_alias_force_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "pwsh",
            "-Command",
            "ri test -Force"
        ])));
    }

    #[test]
    fn powershell_remove_item_without_force_is_not_flagged() {
        assert!(!is_dangerous_command_windows(&vec_str(&[
            "powershell",
            "-Command",
            "Remove-Item test"
        ])));
    }

    // Force delete tests for CMD
    #[test]
    fn cmd_del_force_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "cmd", "/c", "del", "/f", "test.txt"
        ])));
    }

    #[test]
    fn cmd_erase_force_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "cmd", "/c", "erase", "/f", "test.txt"
        ])));
    }

    #[test]
    fn cmd_del_without_force_is_not_flagged() {
        assert!(!is_dangerous_command_windows(&vec_str(&[
            "cmd", "/c", "del", "test.txt"
        ])));
    }

    #[test]
    fn cmd_rd_recursive_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "cmd", "/c", "rd", "/s", "/q", "test"
        ])));
    }

    #[test]
    fn cmd_rd_without_quiet_is_not_flagged() {
        assert!(!is_dangerous_command_windows(&vec_str(&[
            "cmd", "/c", "rd", "/s", "test"
        ])));
    }

    #[test]
    fn cmd_rmdir_recursive_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "cmd", "/c", "rmdir", "/s", "/q", "test"
        ])));
    }

    // Test exact scenario from issue #8567
    #[test]
    fn powershell_remove_item_path_recurse_force_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "powershell",
            "-Command",
            "Remove-Item -Path 'test' -Recurse -Force"
        ])));
    }

    #[test]
    fn powershell_remove_item_force_with_semicolon_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "powershell",
            "-Command",
            "Remove-Item test -Force; Write-Host done"
        ])));
    }

    #[test]
    fn powershell_remove_item_force_inside_block_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "powershell",
            "-Command",
            "if ($true) { Remove-Item test -Force}"
        ])));
    }

    #[test]
    fn powershell_remove_item_force_inside_brackets_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "powershell",
            "-Command",
            "[void]( Remove-Item test -Force)]"
        ])));
    }

    #[test]
    fn cmd_del_path_containing_f_is_not_flagged() {
        assert!(!is_dangerous_command_windows(&vec_str(&[
            "cmd",
            "/c",
            "del",
            "C:/foo/bar.txt"
        ])));
    }

    #[test]
    fn cmd_rd_path_containing_s_is_not_flagged() {
        assert!(!is_dangerous_command_windows(&vec_str(&[
            "cmd",
            "/c",
            "rd",
            "C:/source"
        ])));
    }

    #[test]
    fn cmd_bypass_chained_del_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "cmd", "/c", "echo", "hello", "&", "del", "/f", "file.txt"
        ])));
    }

    #[test]
    fn powershell_chained_no_space_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "powershell",
            "-Command",
            "Write-Host hi;Remove-Item -Force C:\\tmp"
        ])));
    }

    #[test]
    fn powershell_comma_separated_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "powershell",
            "-Command",
            "del,-Force,C:\\foo"
        ])));
    }

    #[test]
    fn cmd_echo_del_is_not_dangerous() {
        assert!(!is_dangerous_command_windows(&vec_str(&[
            "cmd", "/c", "echo", "del", "/f"
        ])));
    }

    #[test]
    fn cmd_del_single_string_argument_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "cmd",
            "/c",
            "del /f file.txt"
        ])));
    }

    #[test]
    fn cmd_del_chained_single_string_argument_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "cmd",
            "/c",
            "echo hello & del /f file.txt"
        ])));
    }

    #[test]
    fn cmd_chained_no_space_del_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "cmd",
            "/c",
            "echo hi&del /f file.txt"
        ])));
    }

    #[test]
    fn cmd_chained_andand_no_space_del_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "cmd",
            "/c",
            "echo hi&&del /f file.txt"
        ])));
    }

    #[test]
    fn cmd_chained_oror_no_space_del_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "cmd",
            "/c",
            "echo hi||del /f file.txt"
        ])));
    }

    #[test]
    fn cmd_start_url_single_string_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "cmd",
            "/c",
            "start https://example.com"
        ])));
    }

    #[test]
    fn cmd_chained_no_space_rmdir_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "cmd",
            "/c",
            "echo hi&rmdir /s /q testdir"
        ])));
    }

    #[test]
    fn cmd_del_force_uppercase_flag_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "cmd", "/c", "DEL", "/F", "file.txt"
        ])));
    }

    #[test]
    fn cmdexe_r_del_force_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "cmd.exe", "/r", "del", "/f", "file.txt"
        ])));
    }

    #[test]
    fn cmd_start_quoted_url_single_string_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "cmd",
            "/c",
            r#"start "https://example.com""#
        ])));
    }

    #[test]
    fn cmd_start_title_then_url_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "cmd",
            "/c",
            r#"start "" https://example.com"#
        ])));
    }

    #[test]
    fn powershell_rm_alias_force_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "powershell",
            "-Command",
            "rm test -Force"
        ])));
    }

    #[test]
    fn powershell_benign_force_separate_command_is_not_dangerous() {
        assert!(!is_dangerous_command_windows(&vec_str(&[
            "powershell",
            "-Command",
            "Get-ChildItem -Force; Remove-Item test"
        ])));
    }
}
