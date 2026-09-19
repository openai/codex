//! Keep native clipboard ownership after either transcript overlay consumer is closed.

use super::Tui;
use crate::clipboard_copy::CopyFormat;
use crate::clipboard_copy::CopyOutcome;
use crate::clipboard_copy::CopyStatus;

impl Tui {
    pub(crate) fn copy_transcript_selection(&mut self, text: &str) -> Result<CopyStatus, String> {
        self.copy_transcript_selection_with(text, |text| {
            crate::clipboard_copy::copy_to_clipboard(text, CopyFormat::PlainText)
        })
    }

    fn copy_transcript_selection_with(
        &mut self,
        text: &str,
        copy: impl FnOnce(&str) -> Result<CopyOutcome, String>,
    ) -> Result<CopyStatus, String> {
        Ok(copy(text)?.store(&mut self.selection_clipboard_lease))
    }
}

#[cfg(test)]
#[path = "selection_clipboard_tests.rs"]
mod tests;
