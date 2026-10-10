//! Local clipboard text reads, executed only by the shared clipboard worker.
//! WSL reads the Windows clipboard; it never falls back to a separate Linux clipboard.

use codex_protocol::user_input::MAX_USER_INPUT_TEXT_CHARS;
use std::time::Instant;

/// Constructed on the clipboard worker, never on the UI thread.
pub(crate) fn reader() -> impl FnMut(Instant) -> Result<String, String> {
    #[cfg(target_os = "linux")]
    let windows =
        (super::is_probably_wsl() && !crate::clipboard_copy::is_ssh_session()).then(|| {
            use std::sync::mpsc;

            let (requests, reads) = mpsc::sync_channel::<(
                Instant,
                mpsc::Sender<Result<String, String>>,
            )>(/*bound*/ 1);
            // WSL process creation itself can hang. Never do it on the shared copy worker,
            // including on retries; at most one Windows reader can be starting or running.
            std::thread::Builder::new()
                .name("wsl-clipboard-reader".into())
                .spawn(move || {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build();
                    let mut child = runtime
                        .as_ref()
                        .ok()
                        .and_then(|runtime| start_windows_reader(runtime).ok());
                    while let Ok((deadline, response)) = reads.recv() {
                        if Instant::now() >= deadline {
                            continue;
                        }
                        let result = match &runtime {
                            Ok(runtime) => {
                                if child.is_none() {
                                    child = start_windows_reader(runtime).ok();
                                }
                                match child.as_mut() {
                                    Some(child) => runtime.block_on(read_command(child, deadline)),
                                    None => Err("Windows clipboard reader is unavailable".into()),
                                }
                            }
                            Err(_) => Err("could not start clipboard reader".into()),
                        };
                        // Never mistake a partial response for the next paste.
                        if result.is_err() {
                            child.take();
                        }
                        let _ = response.send(result);
                    }
                })
                .map(|_| requests)
                .map_err(|_| "could not start clipboard reader".to_string())
        });
    move |deadline| {
        if crate::clipboard_copy::is_ssh_session() {
            return Err("clipboard text is unavailable over SSH".into());
        }
        #[cfg(target_os = "linux")]
        if let Some(windows) = &windows {
            let (response, result) = std::sync::mpsc::channel();
            windows
                .as_ref()
                .map_err(Clone::clone)?
                .try_send((deadline, response))
                .map_err(|_| "Windows clipboard reader is busy or unavailable")?;
            return result
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .map_err(|_| "clipboard read timed out")?;
        }
        #[cfg(not(target_os = "android"))]
        {
            native_result(
                arboard::Clipboard::new().and_then(|mut clipboard| clipboard.get_text()),
                deadline,
            )
        }
        #[cfg(target_os = "android")]
        {
            validate(String::new(), deadline)
        }
    }
}

#[cfg(not(target_os = "android"))]
pub(crate) fn native_result(
    result: Result<String, arboard::Error>,
    deadline: Instant,
) -> Result<String, String> {
    let text = match result {
        Ok(text) => text,
        Err(arboard::Error::ContentNotAvailable) => String::new(),
        Err(_) => return Err("clipboard text is unavailable".into()),
    };
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

#[cfg(target_os = "linux")]
fn start_windows_reader(
    runtime: &tokio::runtime::Runtime,
) -> Result<tokio::process::Child, String> {
    use std::process::Stdio;

    let _guard = runtime.enter();
    let executable = codex_utils_path::system_executable("powershell.exe")
        .ok_or("Windows clipboard reader is unavailable")?;
    let mut command = tokio::process::Command::new(executable);
    // Length-framed UTF-8 preserves every newline and whitespace, including empty text.
    // Neither the prompt nor clipboard contents are ever evaluated as PowerShell.
    command.args([
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        r#"
$ErrorActionPreference = 'Stop'
$out = [Console]::OpenStandardOutput()
$utf8 = [System.Text.UTF8Encoding]::new($false)
while ($null -ne [Console]::ReadLine()) {
    $bytes = $utf8.GetBytes([string](Get-Clipboard -Raw))
    $header = [BitConverter]::GetBytes([uint32]$bytes.Length)
    $out.Write($header, 0, $header.Length)
    $out.Write($bytes, 0, $bytes.Length)
    $out.Flush()
}
"#,
    ]);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(/*kill_on_drop*/ true)
        .spawn()
        .map_err(|_| "could not start Windows clipboard reader".into())
}

#[cfg(any(target_os = "linux", all(test, unix)))]
async fn read_command(
    child: &mut tokio::process::Child,
    deadline: Instant,
) -> Result<String, String> {
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;

    let result = tokio::time::timeout_at(deadline.into(), async {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or("clipboard reader has no input")?;
        stdin
            .write_all(b"\n")
            .await
            .map_err(|_| "could not start clipboard read")?;
        stdin
            .flush()
            .await
            .map_err(|_| "could not start clipboard read")?;
        let stdout = child
            .stdout
            .as_mut()
            .ok_or("clipboard reader has no output")?;
        let size = stdout
            .read_u32_le()
            .await
            .map_err(|_| "could not read clipboard text")? as usize;
        if size > MAX_USER_INPUT_TEXT_CHARS * 4 {
            return Err("clipboard text exceeds the message size limit");
        }
        let mut bytes = vec![0; size];
        stdout
            .read_exact(&mut bytes)
            .await
            .map_err(|_| "could not read clipboard text")?;
        String::from_utf8(bytes).map_err(|_| "clipboard text is not UTF-8")
    })
    .await
    .unwrap_or(Err("clipboard read timed out"));
    validate(result.map_err(str::to_owned)?, deadline)
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
