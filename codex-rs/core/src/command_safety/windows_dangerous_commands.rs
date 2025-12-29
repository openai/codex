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

    // Keep parity with Unix-style `rm -f` checks: in PowerShell, `Remove-Item` (and
    // common aliases like `rm`) becomes meaningfully more dangerous when `-Force`
    // (or `-f`) is present.
    if is_powershell_force_delete(&tokens_lc) {
        return true;
    }

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

    false
}

fn is_powershell_force_delete(tokens_lc: &[String]) -> bool {
    let Some(first) = tokens_lc.first() else {
        return false;
    };

    // `rm`, `ri`, `del`, `erase` are common aliases for `Remove-Item`.
    if !matches!(
        first.as_str(),
        "remove-item" | "rm" | "ri" | "del" | "erase"
    ) {
        return false;
    }

    tokens_lc.iter().any(|t| {
        // Common truthy forms.
        if matches!(t.as_str(), "-force" | "-f" | "-rf" | "-fr") {
            return true;
        }

        // Only treat explicit truthy assignments as force. Avoid flagging `-Force:$false`.
        let value = if let Some(v) = t.strip_prefix("-force:") {
            v
        } else if let Some(v) = t.strip_prefix("-f:") {
            v
        } else {
            return false;
        };

        let value = value.trim();
        value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("$true") || value == "1"
    })
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

    let Some(first_cmd) = iter.next() else {
        return false;
    };
    let remaining: Vec<String> = iter.cloned().collect();

    if is_cmd_force_delete(first_cmd, &remaining) {
        return true;
    }

    // Classic `cmd /c start https://...` ShellExecute path.
    if !first_cmd.eq_ignore_ascii_case("start") {
        return false;
    }

    args_have_url(&remaining)
}

fn is_cmd_force_delete(first_cmd_lc: &str, cmd_args: &[String]) -> bool {
    // Helper: detect cmd-style switches, including grouped forms like `/q/f`.
    let has_switch = |needle: &str| {
        let needle = needle.to_ascii_lowercase();
        cmd_args.iter().any(|a| {
            let lower = a.to_ascii_lowercase();
            if !(lower.starts_with('/') || lower.starts_with('-')) {
                return false;
            }

            // Strip the prefix and split on additional `/` in grouped forms.
            let rest = lower.trim_start_matches(['/', '-']);
            rest.split('/').any(|seg| {
                let seg = seg.trim();
                seg == needle || seg.starts_with(&format!("{needle}:"))
            })
        })
    };

    // File deletes: `del/erase /f` forces deletion of read-only files.
    if matches!(first_cmd_lc, "del" | "erase") {
        return has_switch("f");
    }

    // Directory deletes: `rd/rmdir /s` removes directory trees.
    if matches!(first_cmd_lc, "rd" | "rmdir") {
        return has_switch("s");
    }

    false
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
    fn powershell_remove_item_force_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "powershell",
            "-Command",
            "Remove-Item -Force foo.txt"
        ])));
    }

    #[test]
    fn powershell_rm_f_alias_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "powershell",
            "-Command",
            "rm -f foo.txt"
        ])));
    }

    #[test]
    fn powershell_remove_item_without_force_is_not_flagged() {
        assert!(!is_dangerous_command_windows(&vec_str(&[
            "powershell",
            "-Command",
            "Remove-Item foo.txt"
        ])));
    }

    #[test]
    fn powershell_remove_item_force_false_is_not_flagged() {
        assert!(!is_dangerous_command_windows(&vec_str(&[
            "powershell",
            "-Command",
            "Remove-Item -Force:$false foo.txt"
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
    fn cmd_del_force_is_dangerous() {
        assert!(is_dangerous_command_windows(&vec_str(&[
            "cmd", "/c", "del", "/f", "foo.txt"
        ])));
    }

    #[test]
    fn cmd_del_without_force_is_not_flagged() {
        assert!(!is_dangerous_command_windows(&vec_str(&[
            "cmd", "/c", "del", "foo.txt"
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
}
