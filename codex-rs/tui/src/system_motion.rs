//! Reads the TUI host's accessibility preference once at launch.
//!
//! Detection never changes persisted configuration or consults the app server. An unavailable
//! preference preserves configured behavior; an explicit request for less motion suppresses all
//! effects governed by `tui.animations`. Restart the TUI after changing the OS preference.

use std::sync::OnceLock;

use crate::motion::MotionMode;

static SYSTEM_MOTION: OnceLock<MotionMode> = OnceLock::new();

pub(crate) async fn initialize() {
    let mode = detect().await.unwrap_or(MotionMode::Animated);
    let _ = SYSTEM_MOTION.set(mode);
}

pub(crate) fn mode() -> MotionMode {
    SYSTEM_MOTION.get().copied().unwrap_or(MotionMode::Animated)
}

#[cfg(target_os = "macos")]
async fn detect() -> Option<MotionMode> {
    let workspace = objc2_app_kit::NSWorkspace::sharedWorkspace();
    Some(if workspace.accessibilityDisplayShouldReduceMotion() {
        MotionMode::Reduced
    } else {
        MotionMode::Animated
    })
}

#[cfg(windows)]
async fn detect() -> Option<MotionMode> {
    use windows_sys::Win32::UI::WindowsAndMessaging::SPI_GETCLIENTAREAANIMATION;
    use windows_sys::Win32::UI::WindowsAndMessaging::SystemParametersInfoW;

    let mut enabled: windows_sys::Win32::Foundation::BOOL = 0;
    // SAFETY: SPI_GETCLIENTAREAANIMATION writes one BOOL to this valid, aligned pointer.
    let success = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            /*uiparam*/ 0,
            (&mut enabled as *mut windows_sys::Win32::Foundation::BOOL).cast(),
            /*fwinini*/ 0,
        )
    };
    (success != 0).then(|| MotionMode::from_animations_enabled(enabled != 0))
}

#[cfg(target_os = "linux")]
async fn detect() -> Option<MotionMode> {
    // A missing or unresponsive session bus must not hold up terminal startup.
    tokio::time::timeout(std::time::Duration::from_millis(/*millis*/ 250), async {
        let connection = zbus::Connection::session().await.ok()?;
        let proxy = zbus::Proxy::new(
            &connection,
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings",
        )
        .await
        .ok()?;
        let value: zbus::zvariant::OwnedValue = proxy
            .call("ReadOne", &("org.freedesktop.appearance", "reduced-motion"))
            .await
            .ok()?;
        let value = u32::try_from(value).ok()?;
        // The portal specifies that unknown values mean no preference.
        Some(if value == 1 {
            MotionMode::Reduced
        } else {
            MotionMode::Animated
        })
    })
    .await
    .ok()
    .flatten()
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
async fn detect() -> Option<MotionMode> {
    None
}
