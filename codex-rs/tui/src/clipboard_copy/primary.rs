//! Local X11 PRIMARY selection. Never forward PRIMARY over SSH or tmux, or
//! accidentally target a Wayland compositor or WSL host clipboard.

#[cfg(target_os = "linux")]
use super::ClipboardLease;
use super::CopyOutcome;
use std::time::Instant;

pub(crate) fn available() -> bool {
    cfg!(target_os = "linux")
        && std::env::var_os("DISPLAY").is_some()
        && std::env::var_os("WAYLAND_DISPLAY").is_none()
        && !super::is_ssh_session()
        && !super::is_tmux_session()
        && !super::is_wsl_session()
        && crate::tui::detect_vscode_terminal() != crate::tui::VscodeDetection::VsCode
}

#[cfg(target_os = "linux")]
pub(super) fn copy(
    text: &str,
    begin_delivery: impl FnOnce() -> Result<(), String>,
) -> Result<CopyOutcome, String> {
    use arboard::SetExtLinux;
    let mut clipboard = arboard::Clipboard::new().map_err(|err| err.to_string())?;
    begin_delivery()?;
    clipboard
        .set()
        .clipboard(arboard::LinuxClipboardKind::Primary)
        .text(text)
        .map_err(|err| format!("X11 primary selection: {err}"))?;
    Ok(CopyOutcome::Copied(Some(ClipboardLease::native_linux(
        clipboard,
    ))))
}

#[cfg(not(target_os = "linux"))]
pub(super) fn copy(
    _text: &str,
    _begin_delivery: impl FnOnce() -> Result<(), String>,
) -> Result<CopyOutcome, String> {
    Err("X11 primary selection unavailable".into())
}

#[cfg(target_os = "linux")]
pub(super) fn read(deadline: Instant) -> Result<String, String> {
    use arboard::GetExtLinux;
    crate::clipboard_paste::text::native_result(
        arboard::Clipboard::new().and_then(|mut clipboard| {
            clipboard
                .get()
                .clipboard(arboard::LinuxClipboardKind::Primary)
                .text()
        }),
        deadline,
    )
}

#[cfg(not(target_os = "linux"))]
pub(super) fn read(_deadline: Instant) -> Result<String, String> {
    Err("X11 primary selection unavailable".into())
}
