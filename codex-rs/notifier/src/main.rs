#![cfg_attr(not(target_os = "macos"), allow(unused))]

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use serde::Deserialize;
use std::env;
use std::ffi::CString;
use std::fs;
use std::io::Read;
use std::io::{self};
use std::os::raw::c_char;
use std::path::Path;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum NotificationPayload {
    #[serde(rename_all = "kebab-case")]
    AgentTurnComplete {
        thread_id: String,
        turn_id: String,
        cwd: String,
        #[serde(default)]
        input_messages: Vec<String>,
        #[serde(default)]
        last_assistant_message: Option<String>,
    },
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn codex_post_user_notification(
        title: *const c_char,
        subtitle: *const c_char,
        body: *const c_char,
        icon_path: *const c_char,
    ) -> i32;
}

fn main() -> Result<()> {
    real_main()
}

#[cfg(target_os = "macos")]
fn real_main() -> Result<()> {
    let payload = read_payload()?;
    let (title, subtitle, body) = render_notification(&payload)?;
    dispatch_notification(&title, subtitle.as_deref(), &body)?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn real_main() -> Result<()> {
    bail!("codex-notifier is only supported on macOS");
}

#[cfg(target_os = "macos")]
fn read_payload() -> Result<NotificationPayload> {
    let mut args = env::args().skip(1);
    let mut payload_json: Option<String> = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--payload" => {
                let Some(value) = args.next() else {
                    bail!("missing value for --payload");
                };
                payload_json = Some(value);
                break;
            }
            "--payload-file" => {
                let Some(path) = args.next() else {
                    bail!("missing value for --payload-file");
                };
                payload_json = Some(
                    fs::read_to_string(Path::new(&path))
                        .with_context(|| format!("failed to read payload file at {path}"))?,
                );
                // Best-effort cleanup to avoid cluttering temp directories.
                let _ = fs::remove_file(path);
                break;
            }
            // Ignore arguments injected by `open -a`.
            arg if arg.starts_with("-Apple") || arg == "-NSDocumentRevisionsDebugMode" => {
                let _ = args.next(); // consume companion value if present
            }
            _ => {
                // Unrecognised argument – continue scanning in case the payload flag appears later.
            }
        }
    }

    let json = if let Some(payload) = payload_json {
        payload
    } else {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .context("failed to read payload from stdin")?;
        buf
    };

    serde_json::from_str(&json).context("failed to parse notification payload JSON")
}

#[cfg(target_os = "macos")]
fn render_notification(payload: &NotificationPayload) -> Result<(String, Option<String>, String)> {
    match payload {
        NotificationPayload::AgentTurnComplete {
            input_messages,
            last_assistant_message,
            cwd,
            ..
        } => {
            let title = "Codex CLI".to_string();
            let subtitle = last_assistant_message
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .or_else(|| Some("Turn complete".to_string()));

            let mut body = String::new();
            if !input_messages.is_empty() {
                body.push_str(&input_messages.join(" "));
            }
            if body.is_empty() {
                body.push_str("Agent turn finished");
            }
            body.push_str("\n");
            body.push_str(cwd);

            Ok((title, subtitle, body))
        }
    }
}

#[cfg(target_os = "macos")]
fn dispatch_notification(title: &str, subtitle: Option<&str>, body: &str) -> Result<()> {
    let title_c = CString::new(title)?;
    let subtitle_c = subtitle
        .map(|s| CString::new(s))
        .transpose()
        .context("invalid subtitle string")?;
    let body_c = CString::new(body)?;

    let icon_ptr = std::ptr::null();

    let code = unsafe {
        codex_post_user_notification(
            title_c.as_ptr(),
            subtitle_c.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()),
            body_c.as_ptr(),
            icon_ptr,
        )
    };

    if code != 0 {
        bail!("codex_post_user_notification returned error code {code}");
    }
    Ok(())
}
