//! Detect local Linux PRIMARY without blocking input or consuming an inherited Wayland socket.

use super::PasteOwner;
use nix::sys::socket::getsockopt;
use nix::sys::socket::sockopt::PeerCredentials;
use std::os::linux::net::SocketAddrExt;
use std::os::unix::net::SocketAddr;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use wl_clipboard_rs::utils::PrimarySelectionCheckError;
use wl_clipboard_rs::utils::is_primary_selection_supported;

pub(crate) fn detect_primary_selection() -> Arc<AtomicBool> {
    if super::is_ssh_session()
        || super::is_tmux_session()
        || super::is_wsl_session()
        || matches!(super::paste_owner(), PasteOwner::Terminal(_))
        || std::env::var_os("WAYLAND_SOCKET").is_some()
    {
        return Default::default();
    }
    let display = std::env::var_os("DISPLAY");
    let Some(wayland) = std::env::var_os("WAYLAND_DISPLAY") else {
        return Arc::new(AtomicBool::new(display.is_some()));
    };

    let available = Arc::new(AtomicBool::new(false));
    let result = Arc::clone(&available);
    let _ = std::thread::Builder::new().spawn(move || {
        let supported = wayland_primary_available(
            is_primary_selection_supported(),
            display.as_deref(),
            |number| same_listener(PathBuf::from(wayland), number).unwrap_or(false),
        );
        result.store(supported, Ordering::Relaxed);
    });
    available
}

fn wayland_primary_available(
    probe: Result<bool, PrimarySelectionCheckError>,
    display: Option<&std::ffi::OsStr>,
    same_session: impl FnOnce(u16) -> bool,
) -> bool {
    // Arboard may retry X11 on a later connection failure. Reject remote addresses.
    let number = display.and_then(|display| {
        let (host, suffix) = display.to_str()?.rsplit_once(':')?;
        if !matches!(host, "" | "unix/") {
            return None;
        }
        suffix.split('.').next()?.parse().ok()
    });
    match probe {
        Ok(supported) => supported && (display.is_none() || number.is_some()),
        Err(PrimarySelectionCheckError::MissingProtocol) => number.is_some_and(same_session),
        Err(_) => false,
    }
}

// Mutter opens both listeners and passes the X11 sockets to XWayland. Linux
// reports the listener's credentials even after the file descriptor is passed.
fn same_listener(mut wayland: PathBuf, number: u16) -> Option<bool> {
    if !wayland.is_absolute() {
        wayland = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR")?).join(wayland);
    }
    let compositor = UnixStream::connect(wayland).ok()?;
    let path = format!("/tmp/.X11-unix/X{number}");
    // Match x11rb's preference for the abstract socket over the filesystem socket.
    let address = SocketAddr::from_abstract_name(path.as_bytes()).ok()?;
    let x11 = UnixStream::connect_addr(&address)
        .or_else(|_| UnixStream::connect(path))
        .ok()?;
    let owner = getsockopt(&compositor, PeerCredentials).ok()?;
    // A peer outside our PID namespace can have an invisible (zero) PID.
    Some(owner.pid() > 0 && owner == getsockopt(&x11, PeerCredentials).ok()?)
}

#[cfg(test)]
#[path = "primary_tests.rs"]
mod tests;
