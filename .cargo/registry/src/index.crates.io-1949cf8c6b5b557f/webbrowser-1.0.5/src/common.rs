use super::{BrowserOptions, Error, ErrorKind, Result};
use log::debug;
use std::process::{Command, Stdio};

/// Parses `line` to find tokens (including quoted strings), and invokes `op`
/// on each token
pub(crate) fn for_each_token<F>(line: &str, mut op: F)
where
    F: FnMut(&str),
{
    let mut start: Option<usize> = None;
    let mut in_quotes = false;
    let mut idx = 0;
    for ch in line.chars() {
        idx += 1;
        match ch {
            '"' => {
                if let Some(start_idx) = start {
                    op(&line[start_idx..idx - 1]);
                    start = None;
                    in_quotes = false;
                } else {
                    start = Some(idx);
                    in_quotes = true;
                }
            }
            ' ' => {
                if !in_quotes {
                    if let Some(start_idx) = start {
                        op(&line[start_idx..idx - 1]);
                        start = None;
                    }
                }
            }
            _ => {
                if start.is_none() {
                    start = Some(idx - 1);
                }
            }
        }
    }
    if let Some(start_idx) = start {
        op(&line[start_idx..idx]);
    }
}

/// Run the specified command in foreground/background
pub(crate) fn run_command(
    cmd: &mut Command,
    background: bool,
    options: &BrowserOptions,
) -> Result<()> {
    // if dry_run, we return a true, as executable existence check has
    // already been done
    if options.dry_run {
        debug!("dry-run enabled, so not running: {:?}", &cmd);
        return Ok(());
    }

    if background {
        debug!("background spawn: {:?}", &cmd);
        // if we're in background, set stdin/stdout to null and spawn a child, as we're
        // not supposed to have any interaction.
        if options.suppress_output {
            cmd.stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
        } else {
            cmd
        }
        .spawn()
        .map(|_| ())
    } else {
        debug!("foreground exec: {:?}", &cmd);
        // if we're in foreground, use status() instead of spawn(), as we'd like to wait
        // till completion.
        // We also specifically don't suppress anything here, because we're running here
        // most likely because of a text browser
        cmd.status().and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                Err(Error::new(
                    ErrorKind::Other,
                    "command present but exited unsuccessfully",
                ))
            }
        })
    }
}
