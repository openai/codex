use tracing::error;

#[derive(Debug)]
pub enum ClipboardError {
    ClipboardUnavailable(String),
    WriteFailed(String),
}

impl std::fmt::Display for ClipboardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClipboardError::ClipboardUnavailable(msg) => {
                write!(f, "clipboard unavailable: {msg}")
            }
            ClipboardError::WriteFailed(msg) => write!(f, "failed to write to clipboard: {msg}"),
        }
    }
}

impl std::error::Error for ClipboardError {}

pub trait ClipboardManager {
    fn set_text(&mut self, text: String) -> Result<(), ClipboardError>;
}

#[cfg(not(target_os = "android"))]
pub struct ArboardClipboardManager {
    inner: Option<arboard::Clipboard>,
}

#[cfg(not(target_os = "android"))]
impl ArboardClipboardManager {
    pub fn new() -> Self {
        match arboard::Clipboard::new() {
            Ok(cb) => Self { inner: Some(cb) },
            Err(err) => {
                error!(error = %err, "failed to initialize clipboard");
                Self { inner: None }
            }
        }
    }
}

#[cfg(not(target_os = "android"))]
impl ClipboardManager for ArboardClipboardManager {
    fn set_text(&mut self, text: String) -> Result<(), ClipboardError> {
        let Some(cb) = &mut self.inner else {
            return Err(ClipboardError::ClipboardUnavailable(
                "clipboard is not available in this environment".to_string(),
            ));
        };
        cb.set_text(text)
            .map_err(|e| ClipboardError::WriteFailed(e.to_string()))
    }
}

#[cfg(target_os = "android")]
pub struct ArboardClipboardManager;

#[cfg(target_os = "android")]
impl ArboardClipboardManager {
    pub fn new() -> Self {
        ArboardClipboardManager
    }
}

#[cfg(target_os = "android")]
impl ClipboardManager for ArboardClipboardManager {
    fn set_text(&mut self, _text: String) -> Result<(), ClipboardError> {
        Err(ClipboardError::ClipboardUnavailable(
            "clipboard text copy is unsupported on Android".to_string(),
        ))
    }
}

pub fn copy_text(text: String) -> Result<(), ClipboardError> {
    let mut manager = ArboardClipboardManager::new();
    manager.set_text(text)
}
