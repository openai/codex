//! Submit selection copies without waiting for native clipboard or helper I/O.

use super::Tui;
use crate::clipboard_copy::CopyFormat;
use crate::clipboard_copy::CopyStatus;
use crate::clipboard_copy::worker::CopyDestination;
use crate::clipboard_copy::worker::CopyResult;
use std::sync::Arc;

/// Clipboard destinations and completion behavior shared by selectable surfaces.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SelectionCopy {
    clipboard: bool,
    primary: bool,
    clear_selection: bool,
}

impl SelectionCopy {
    pub(crate) const EXPLICIT: Self = Self {
        clipboard: true,
        primary: false,
        clear_selection: true,
    };

    pub(crate) fn on_select(clipboard: bool, primary: bool, clear_selection: bool) -> Self {
        Self {
            clipboard,
            primary,
            clear_selection,
        }
    }

    pub(crate) fn copies_clipboard(self) -> bool {
        self.clipboard
    }

    pub(crate) fn publishes_primary(self) -> bool {
        self.primary
    }

    pub(crate) fn clear_selection(self) -> bool {
        self.clear_selection
    }
}

impl Tui {
    /// Copy to CLIPBOARD and/or publish PRIMARY under one selection policy.
    pub(crate) fn copy_selection(
        &mut self,
        text: &str,
        format: CopyFormat,
        selection_copy: SelectionCopy,
        primary_text: &str,
    ) -> (CopyResult, Option<Arc<()>>) {
        let text: Arc<str> = text.into();
        match (
            selection_copy.copies_clipboard(),
            selection_copy.publishes_primary(),
        ) {
            (true, true) => {
                let (result, owner) = self.clipboard.select(
                    Arc::clone(&text),
                    format,
                    CopyDestination::ClipboardAndPrimary(primary_text.into()),
                    self.frame_requester(),
                );
                (result, Some(owner))
            }
            (true, false) => (
                self.clipboard.copy(text, format, self.frame_requester()),
                None,
            ),
            (false, true) => {
                let (result, owner) = self.clipboard.select(
                    text,
                    format,
                    CopyDestination::Primary,
                    self.frame_requester(),
                );
                (result, Some(owner))
            }
            (false, false) => (Err("selection copy has no destination".into()), None),
        }
    }

    pub(crate) fn copy_transcript_selection(
        &mut self,
        text: &str,
        format: CopyFormat,
    ) -> Result<CopyStatus, String> {
        self.copy_selection(text, format, SelectionCopy::EXPLICIT, text)
            .0
    }
}
