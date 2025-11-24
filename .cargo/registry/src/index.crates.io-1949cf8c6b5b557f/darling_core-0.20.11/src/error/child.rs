use proc_macro2::Span;

/// Exhaustive mirror of [`proc_macro::Level`].
#[derive(Debug, Clone)]
pub(in crate::error) enum Level {
    Error,
    Warning,
    Note,
    Help,
}

/// Supplemental message for an [`Error`](super::Error) when it's emitted as a `Diagnostic`.
///
/// # Example Output
/// The `note` and `help` lines below come from child diagnostics.
///
/// ```text
/// error: My custom error
///   --> my_project/my_file.rs:3:5
///    |
/// 13 |     FooBar { value: String },
///    |     ^^^^^^
///    |
///    = note: My note on the macro usage
///    = help: Try doing this instead
/// ```
#[derive(Debug, Clone)]
pub(in crate::error) struct ChildDiagnostic {
    level: Level,
    span: Option<Span>,
    message: String,
}

impl ChildDiagnostic {
    pub(in crate::error) fn new(level: Level, span: Option<Span>, message: String) -> Self {
        Self {
            level,
            span,
            message,
        }
    }
}

impl ChildDiagnostic {
    /// Append this child diagnostic to a `Diagnostic`.
    ///
    /// # Panics
    /// This method panics if `self` has a span and is being invoked outside of
    /// a proc-macro due to the behavior of [`Span::unwrap()`](Span).
    pub fn append_to(self, diagnostic: proc_macro::Diagnostic) -> proc_macro::Diagnostic {
        match self.level {
            Level::Error => {
                if let Some(span) = self.span {
                    diagnostic.span_error(span.unwrap(), self.message)
                } else {
                    diagnostic.error(self.message)
                }
            }
            Level::Warning => {
                if let Some(span) = self.span {
                    diagnostic.span_warning(span.unwrap(), self.message)
                } else {
                    diagnostic.warning(self.message)
                }
            }
            Level::Note => {
                if let Some(span) = self.span {
                    diagnostic.span_note(span.unwrap(), self.message)
                } else {
                    diagnostic.note(self.message)
                }
            }
            Level::Help => {
                if let Some(span) = self.span {
                    diagnostic.span_help(span.unwrap(), self.message)
                } else {
                    diagnostic.help(self.message)
                }
            }
        }
    }
}
