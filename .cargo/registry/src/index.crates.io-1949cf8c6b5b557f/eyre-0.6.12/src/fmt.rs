use crate::{error::ErrorImpl, ptr::RefPtr};
use core::fmt;

impl ErrorImpl<()> {
    pub(crate) fn display(this: RefPtr<'_, Self>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        ErrorImpl::header(this)
            .handler
            .as_ref()
            .map(|handler| handler.display(Self::error(this), f))
            .unwrap_or_else(|| core::fmt::Display::fmt(Self::error(this), f))
    }

    /// Debug formats the error using the captured handler
    pub(crate) fn debug(this: RefPtr<'_, Self>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        ErrorImpl::header(this)
            .handler
            .as_ref()
            .map(|handler| handler.debug(Self::error(this), f))
            .unwrap_or_else(|| core::fmt::Debug::fmt(Self::error(this), f))
    }
}
