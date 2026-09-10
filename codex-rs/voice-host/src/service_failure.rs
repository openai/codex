//! Fixed local media service failures for diagnostics without native error text.

#[derive(Debug)]
#[cfg_attr(
    not(any(
        target_os = "macos",
        all(target_os = "linux", target_env = "gnu"),
        all(windows, target_env = "msvc")
    )),
    allow(dead_code)
)]
pub(crate) enum ServiceFailure {
    Playout,
    Render,
    Capture,
    Device,
    Send,
}

impl std::fmt::Display for ServiceFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Playout => "audio playout failed",
            Self::Render => "audio rendering failed",
            Self::Capture => "audio capture processing failed",
            Self::Device => "audio device failed",
            Self::Send => "voice audio send failed",
        })
    }
}

impl std::error::Error for ServiceFailure {}
