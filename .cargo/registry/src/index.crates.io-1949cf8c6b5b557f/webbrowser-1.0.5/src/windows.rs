use crate::common::{for_each_token, run_command};
use crate::{Browser, BrowserOptions, Error, ErrorKind, Result, TargetType};
use log::trace;
use std::path::Path;
use std::process::Command;

const ASSOCF_IS_PROTOCOL: u32 = 0x00001000;
const ASSOCSTR_COMMAND: i32 = 1;

#[link(name = "shlwapi")]
extern "system" {
    fn AssocQueryStringW(
        flags: u32,
        string: i32,
        association: *const u16,
        extra: *const u16,
        out: *mut u16,
        out_len: *mut u32,
    ) -> i32;
}

/// Deal with opening of browsers on Windows.
///
/// We first use [`AssocQueryStringW`](https://learn.microsoft.com/en-us/windows/win32/api/shlwapi/nf-shlwapi-assocquerystringw)
/// function to determine the default browser, and then invoke it with appropriate parameters.
///
/// We ignore BrowserOptions on Windows, except for honouring [BrowserOptions::dry_run]
pub(super) fn open_browser_internal(
    browser: Browser,
    target: &TargetType,
    options: &BrowserOptions,
) -> Result<()> {
    match browser {
        Browser::Default => {
            // always return true for a dry run for default browser
            if options.dry_run {
                return Ok(());
            }

            trace!("trying to figure out default browser command");
            let cmdline = unsafe {
                const BUF_SIZE: usize = 512;
                let mut cmdline_u16 = [0_u16; BUF_SIZE];
                let mut line_len = BUF_SIZE as u32;
                if AssocQueryStringW(
                    ASSOCF_IS_PROTOCOL,
                    ASSOCSTR_COMMAND,
                    [0x68, 0x74, 0x74, 0x70, 0x0].as_ptr(), // http\0
                    std::ptr::null(),
                    cmdline_u16.as_mut_ptr(),
                    &mut line_len,
                ) != 0
                {
                    return Err(Error::new(
                        ErrorKind::Other,
                        "failed to get default browser",
                    ));
                }

                use std::os::windows::ffi::OsStringExt;
                std::ffi::OsString::from_wide(&cmdline_u16[..(line_len - 1) as usize])
                    .into_string()
                    .map_err(|_err| {
                        Error::new(
                            ErrorKind::Other,
                            "The default web browser command contains invalid unicode characters",
                        )
                    })?
            };
            trace!("default browser command: {}", &cmdline);
            let cmdline = ensure_cmd_quotes(&cmdline);
            let mut cmd = get_browser_cmd(&cmdline, target)?;
            run_command(&mut cmd, true, options)
        }
        _ => Err(Error::new(
            ErrorKind::NotFound,
            "Only the default browser is supported on this platform right now",
        )),
    }
}

/// It seems that sometimes browser exe paths which have spaces are not quoted, so we keep going over
/// each token, until we encounter what looks like a valid exe.
///
/// See https://github.com/amodm/webbrowser-rs/issues/68
fn ensure_cmd_quotes(cmdline: &str) -> String {
    if !cmdline.starts_with('"') {
        let mut end = 0;
        for (idx, ch) in cmdline.char_indices() {
            if ch == ' ' {
                // does the path till now look like a valid exe?
                let potential_exe = Path::new(&cmdline[..idx]);
                if potential_exe.exists() {
                    end = idx;
                    break;
                }
            }
        }
        if end > 0 {
            return format!("\"{}\"{}", &cmdline[..end], &cmdline[end..]);
        }
    }

    // else we default to returning the original cmdline
    cmdline.to_string()
}

/// Given the configured command line `cmdline` in registry, and the given `url`,
/// return the appropriate `Command` to invoke
fn get_browser_cmd(cmdline: &str, url: &TargetType) -> Result<Command> {
    let mut tokens: Vec<String> = Vec::new();
    for_each_token(cmdline, |token: &str| {
        if matches!(token, "%0" | "%1") {
            tokens.push(url.to_string());
        } else {
            tokens.push(token.to_string());
        }
    });
    if tokens.is_empty() {
        Err(Error::new(ErrorKind::NotFound, "invalid command"))
    } else {
        let mut cmd = Command::new(&tokens[0]);
        if tokens.len() > 1 {
            cmd.args(&tokens[1..]);
        }
        Ok(cmd)
    }
}
