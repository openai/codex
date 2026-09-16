//! Seeds a persistent animation default from a one-time host screen-reader probe.
//!
//! Presence of the completion marker skips detection, including when its value is false.
//! Detection is bounded; explicit animation preferences and unrelated config are preserved.
//! A detected reader also supplies a session default if persistence fails.

use crate::motion::MotionMode;
use codex_config::ConfigLayerStack;
use codex_utils_path::resolve_symlink_write_paths;
use codex_utils_path::write_atomically;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;
use toml_edit::DocumentMut;
use toml_edit::value;

const DETECTION_TIMEOUT: Duration = Duration::from_millis(/*millis*/ 450);
static ANIMATION_DEFAULT: OnceLock<MotionMode> = OnceLock::new();

pub(crate) async fn initialize(stack: &ConfigLayerStack) -> anyhow::Result<()> {
    let (animation_default, result) = initialize_with_probe(stack, detect()).await;
    let _ = ANIMATION_DEFAULT.set(animation_default);
    result
}

pub(crate) fn animation_default() -> MotionMode {
    ANIMATION_DEFAULT
        .get()
        .copied()
        .unwrap_or(MotionMode::Animated)
}

async fn initialize_with_probe(
    stack: &ConfigLayerStack,
    probe: impl std::future::Future<Output = bool>,
) -> (MotionMode, anyhow::Result<()>) {
    let Some(path) = stack.get_user_config_file() else {
        return (MotionMode::Animated, Ok(()));
    };
    let user_config = stack.effective_user_config();
    let tui = user_config.as_ref().and_then(|config| config.get("tui"));
    if tui
        .and_then(|tui| tui.get("screen_reader_detection_done"))
        .is_some()
    {
        return (MotionMode::Animated, Ok(()));
    }
    let animations_configured = tui.and_then(|tui| tui.get("animations")).is_some();
    let mut animation_default = MotionMode::Animated;
    let result = initialize_file(path.as_path(), animations_configured, async {
        let detected = probe.await;
        animation_default = MotionMode::from_animations_enabled(!detected);
        detected
    })
    .await;
    (animation_default, result)
}

async fn initialize_file(
    path: &Path,
    animations_configured: bool,
    probe: impl std::future::Future<Output = bool>,
) -> anyhow::Result<()> {
    let mut doc = read_config(path)?;
    if doc
        .get("tui")
        .and_then(|tui| tui.get("screen_reader_detection_done"))
        .is_some()
    {
        return Ok(());
    }
    let detected = tokio::time::timeout(DETECTION_TIMEOUT, probe)
        .await
        .unwrap_or(false);
    // Re-read after the probe so preferences saved during detection are preserved.
    doc = read_config(path)?;
    if doc
        .get("tui")
        .and_then(|tui| tui.get("screen_reader_detection_done"))
        .is_some()
    {
        return Ok(());
    }
    doc.entry("tui")
        .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()));
    if detected
        && !animations_configured
        && doc
            .get("tui")
            .and_then(|tui| tui.get("animations"))
            .is_none()
    {
        doc["tui"]["animations"] = value(false);
    }
    doc["tui"]["screen_reader_detection_done"] = value(true);
    let paths = resolve_symlink_write_paths(path)?;
    write_atomically(&paths.write_path, &doc.to_string())?;
    Ok(())
}

fn read_config(path: &Path) -> anyhow::Result<DocumentMut> {
    match std::fs::read_to_string(path) {
        Ok(contents) => contents
            .parse()
            .map_err(|_| anyhow::anyhow!("invalid TOML in user config")),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(DocumentMut::new()),
        Err(err) => Err(err.into()),
    }
}

#[cfg(any(target_os = "macos", windows))]
async fn detect() -> bool {
    let (tx, rx) = tokio::sync::oneshot::channel();
    // Native calls cannot be cancelled. A detached thread bounds startup and avoids waiting
    // for a blocked OS probe when the Tokio runtime shuts down.
    let _ = std::thread::Builder::new()
        .name("screen-reader".into())
        .spawn(move || {
            #[cfg(target_os = "macos")]
            let detected = objc2_app_kit::NSWorkspace::sharedWorkspace().isVoiceOverEnabled();
            #[cfg(windows)]
            let detected = {
                use windows_sys::Win32::UI::WindowsAndMessaging::SPI_GETSCREENREADER;
                use windows_sys::Win32::UI::WindowsAndMessaging::SystemParametersInfoW;
                let mut enabled: windows_sys::Win32::Foundation::BOOL = 0;
                // SAFETY: SPI_GETSCREENREADER writes one BOOL to this valid, aligned pointer.
                let success = unsafe {
                    SystemParametersInfoW(
                        SPI_GETSCREENREADER,
                        /*uiparam*/ 0,
                        (&mut enabled as *mut windows_sys::Win32::Foundation::BOOL).cast(),
                        /*fwinini*/ 0,
                    )
                };
                (success != 0 && enabled != 0)
                    || windows_screen_reader::narrator_running() == Some(true)
            };
            let _ = tx.send(detected);
        });
    rx.await.unwrap_or(false)
}

#[cfg(target_os = "linux")]
async fn detect() -> bool {
    let Ok(connection) = zbus::Connection::session().await else {
        return false;
    };
    let atspi = async {
        let proxy = zbus::Proxy::new(
            &connection,
            "org.a11y.Bus",
            "/org/a11y/bus",
            "org.a11y.Status",
        )
        .await
        .ok()?;
        // Prefer the reader-specific signal. Newer AT-SPI only exposes IsEnabled, which can
        // also indicate other assistive tools, so err on the side of reducing motion.
        match proxy.get_property::<bool>("ScreenReaderEnabled").await {
            Ok(enabled) => Some(enabled),
            Err(_) => proxy.get_property::<bool>("IsEnabled").await.ok(),
        }
    };
    let orca = async {
        let proxy = zbus::fdo::DBusProxy::new(&connection).await.ok()?;
        let names = proxy.list_names().await.ok()?;
        Some(names.iter().any(|name| {
            matches!(
                name.as_str(),
                "org.gnome.Orca.Service" | "org.gnome.Orca1.Service"
            )
        }))
    };
    tokio::pin!(atspi, orca);
    // A positive probe wins immediately, even if the other probe is unresponsive.
    tokio::select! {
        result = &mut atspi => result == Some(true) || orca.await == Some(true),
        result = &mut orca => result == Some(true) || atspi.await == Some(true),
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
async fn detect() -> bool {
    false
}

#[cfg(windows)]
#[path = "screen_reader_windows.rs"]
mod windows_screen_reader;

#[cfg(test)]
#[path = "screen_reader_tests.rs"]
mod tests;
