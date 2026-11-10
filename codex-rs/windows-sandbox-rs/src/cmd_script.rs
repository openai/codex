//! Serialize the single script argument used by the Windows command interpreter.

/// Build the native command line for Codex's `cmd.exe /c SCRIPT` invocation.
/// The script is shell syntax. CRT argument escaping would introduce literal
/// backslashes before its quotes. With /s, cmd strips exactly the outer pair.
pub fn cmd_script_command_line(argv: &[String]) -> Option<String> {
    let [program, switch, script] = argv else {
        return None;
    };
    let name = program.rsplit(['/', '\\']).next()?;
    if !(name.eq_ignore_ascii_case("cmd.exe") || name.eq_ignore_ascii_case("cmd"))
        || !switch.eq_ignore_ascii_case("/c")
        || program.contains(['"', '\0'])
        || script.contains('\0')
    {
        return None;
    }
    Some(format!(r#""{program}" /s /c "{script}""#))
}

#[cfg(test)]
#[path = "cmd_script_tests.rs"]
mod tests;
