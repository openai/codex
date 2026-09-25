//! Local clipboard text reads, executed only by the shared clipboard worker.
//! WSL reads the Windows clipboard; it never falls back to a separate Linux clipboard.

use codex_protocol::user_input::MAX_USER_INPUT_TEXT_CHARS;
use std::time::Instant;

pub(crate) fn read(deadline: Instant) -> Result<String, String> {
    if crate::clipboard_copy::is_ssh_session() {
        return Err("clipboard text is unavailable over SSH".into());
    }
    #[cfg(target_os = "linux")]
    if super::is_probably_wsl() {
        let executable = codex_utils_path::system_executable("powershell.exe")
            .ok_or("Windows clipboard reader is unavailable")?;
        let mut command = tokio::process::Command::new(executable);
        command.args(["-NoProfile", "-NonInteractive", "-Command",
            "$ErrorActionPreference = 'Stop'; [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false); [Console]::Out.Write((Get-Clipboard -Raw))"]);
        return tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| "could not start clipboard reader")?
            .block_on(read_command(command, deadline));
    }
    #[cfg(not(target_os = "android"))]
    let text = match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.get_text()) {
        Ok(text) => text,
        Err(arboard::Error::ContentNotAvailable) => String::new(),
        Err(_) => return Err("clipboard text is unavailable".into()),
    };
    #[cfg(target_os = "android")]
    let text = String::new();
    validate(text, deadline)
}

fn validate(text: String, deadline: Instant) -> Result<String, String> {
    if Instant::now() >= deadline {
        Err("clipboard read timed out".into())
    } else if text.chars().count() > MAX_USER_INPUT_TEXT_CHARS {
        Err("clipboard text exceeds the message size limit".into())
    } else {
        Ok(text)
    }
}

#[cfg(any(target_os = "linux", all(test, unix)))]
async fn read_command(
    mut command: tokio::process::Command,
    deadline: Instant,
) -> Result<String, String> {
    use std::process::Stdio;
    use tokio::io::AsyncReadExt;

    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(/*kill_on_drop*/ true)
        .spawn()
        .map_err(|_| "could not start Windows clipboard reader")?;
    // Spawn can itself stall under WSL. Both the UI and this read retain the original deadline.
    let result = tokio::time::timeout_at(deadline.into(), async {
        let stdout = child
            .stdout
            .take()
            .ok_or("clipboard reader has no output")?;
        let limit = MAX_USER_INPUT_TEXT_CHARS * 4;
        let mut bytes = Vec::new();
        stdout
            .take((limit + 1) as u64)
            .read_to_end(&mut bytes)
            .await
            .map_err(|_| "could not read clipboard text")?;
        if bytes.len() > limit {
            return Err("clipboard text exceeds the message size limit");
        }
        if !child
            .wait()
            .await
            .map_err(|_| "clipboard reader failed")?
            .success()
        {
            return Err("clipboard text is unavailable");
        }
        String::from_utf8(bytes).map_err(|_| "clipboard text is not UTF-8")
    })
    .await
    .unwrap_or(Err("clipboard read timed out"));
    if result.is_err() {
        let _ = child.kill().await;
    }
    validate(result.map_err(str::to_owned)?, deadline)
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
