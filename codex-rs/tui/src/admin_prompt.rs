use codex_core::config::Config;
use std::io::IsTerminal;
use std::io::Write;
use std::io::{self};

pub fn prompt_for_admin_danger_reason(config: &Config) -> std::io::Result<Option<String>> {
    let Some(prompt) = config.admin_danger_prompt.as_ref() else {
        return Ok(None);
    };

    if !prompt.needs_prompt() {
        return Ok(None);
    }

    if !io::stdin().is_terminal() {
        return Err(io::Error::other(
            "Administrator requires a justification for dangerous sandbox usage, but stdin is not interactive.",
        ));
    }

    let red = "\x1b[31m";
    let reset = "\x1b[0m";
    let blue = "\x1b[34m";
    let mut stderr = io::stderr();
    writeln!(stderr)?;
    writeln!(
        stderr,
        "{red}╔══════════════════════════════════════════════════════════════╗{reset}"
    )?;
    writeln!(
        stderr,
        "{red}║  DANGEROUS USAGE WARNING                                     ║{reset}"
    )?;
    writeln!(
        stderr,
        "{red}╚══════════════════════════════════════════════════════════════╝{reset}"
    )?;
    for line in codex_core::admin_controls::ADMIN_DANGEROUS_SANDBOX_DISABLED_PROMPT_LINES {
        writeln!(stderr, "{red}{line}{reset}")?;
    }
    writeln!(stderr)?;
    write!(
        stderr,
        "{red}?{reset} {blue}Or provide a justification to continue anyway:{reset} "
    )?;
    stderr.flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let justification = input.trim();
    if justification.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Justification is required by your administrator to proceed with dangerous usage.",
        ));
    }
    if justification.len() < 4 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Justification must be at least 4 characters long and is required by your administrator to proceed with dangerous usage.",
        ));
    }

    Ok(Some(justification.to_string()))
}
