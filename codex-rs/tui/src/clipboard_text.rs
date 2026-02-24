#[derive(Debug, Clone)]
pub enum ClipboardTextError {
    ClipboardUnavailable(String),
}

impl std::fmt::Display for ClipboardTextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClipboardTextError::ClipboardUnavailable(msg) => {
                write!(f, "clipboard unavailable: {msg}")
            }
        }
    }
}

impl std::error::Error for ClipboardTextError {}

pub fn copy_text_to_clipboard(text: &str) -> Result<(), ClipboardTextError> {
    let mut cb = arboard::Clipboard::new()
        .map_err(|e| ClipboardTextError::ClipboardUnavailable(e.to_string()))?;
    cb.set_text(text.to_string())
        .map_err(|e| ClipboardTextError::ClipboardUnavailable(e.to_string()))
}
