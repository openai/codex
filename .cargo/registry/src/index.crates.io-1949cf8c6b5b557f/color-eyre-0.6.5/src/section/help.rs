//! Provides an extension trait for attaching `Section` to error reports.
use crate::{
    config::Theme,
    eyre::{Report, Result},
    Section,
};
use indenter::indented;
use owo_colors::OwoColorize;
use std::fmt::Write;
use std::fmt::{self, Display};

impl Section for Report {
    type Return = Report;

    fn note<D>(mut self, note: D) -> Self::Return
    where
        D: Display + Send + Sync + 'static,
    {
        if let Some(handler) = self.handler_mut().downcast_mut::<crate::Handler>() {
            handler
                .sections
                .push(HelpInfo::Note(Box::new(note), handler.theme));
        }

        self
    }

    fn with_note<D, F>(mut self, note: F) -> Self::Return
    where
        D: Display + Send + Sync + 'static,
        F: FnOnce() -> D,
    {
        if let Some(handler) = self.handler_mut().downcast_mut::<crate::Handler>() {
            handler
                .sections
                .push(HelpInfo::Note(Box::new(note()), handler.theme));
        }

        self
    }

    fn warning<D>(mut self, warning: D) -> Self::Return
    where
        D: Display + Send + Sync + 'static,
    {
        if let Some(handler) = self.handler_mut().downcast_mut::<crate::Handler>() {
            handler
                .sections
                .push(HelpInfo::Warning(Box::new(warning), handler.theme));
        }

        self
    }

    fn with_warning<D, F>(mut self, warning: F) -> Self::Return
    where
        D: Display + Send + Sync + 'static,
        F: FnOnce() -> D,
    {
        if let Some(handler) = self.handler_mut().downcast_mut::<crate::Handler>() {
            handler
                .sections
                .push(HelpInfo::Warning(Box::new(warning()), handler.theme));
        }

        self
    }

    fn suggestion<D>(mut self, suggestion: D) -> Self::Return
    where
        D: Display + Send + Sync + 'static,
    {
        if let Some(handler) = self.handler_mut().downcast_mut::<crate::Handler>() {
            handler
                .sections
                .push(HelpInfo::Suggestion(Box::new(suggestion), handler.theme));
        }

        self
    }

    fn with_suggestion<D, F>(mut self, suggestion: F) -> Self::Return
    where
        D: Display + Send + Sync + 'static,
        F: FnOnce() -> D,
    {
        if let Some(handler) = self.handler_mut().downcast_mut::<crate::Handler>() {
            handler
                .sections
                .push(HelpInfo::Suggestion(Box::new(suggestion()), handler.theme));
        }

        self
    }

    fn with_section<D, F>(mut self, section: F) -> Self::Return
    where
        D: Display + Send + Sync + 'static,
        F: FnOnce() -> D,
    {
        if let Some(handler) = self.handler_mut().downcast_mut::<crate::Handler>() {
            let section = Box::new(section());
            handler.sections.push(HelpInfo::Custom(section));
        }

        self
    }

    fn section<D>(mut self, section: D) -> Self::Return
    where
        D: Display + Send + Sync + 'static,
    {
        if let Some(handler) = self.handler_mut().downcast_mut::<crate::Handler>() {
            let section = Box::new(section);
            handler.sections.push(HelpInfo::Custom(section));
        }

        self
    }

    fn error<E2>(mut self, error: E2) -> Self::Return
    where
        E2: std::error::Error + Send + Sync + 'static,
    {
        if let Some(handler) = self.handler_mut().downcast_mut::<crate::Handler>() {
            let error = error.into();
            handler.sections.push(HelpInfo::Error(error, handler.theme));
        }

        self
    }

    fn with_error<E2, F>(mut self, error: F) -> Self::Return
    where
        F: FnOnce() -> E2,
        E2: std::error::Error + Send + Sync + 'static,
    {
        if let Some(handler) = self.handler_mut().downcast_mut::<crate::Handler>() {
            let error = error().into();
            handler.sections.push(HelpInfo::Error(error, handler.theme));
        }

        self
    }

    fn suppress_backtrace(mut self, suppress: bool) -> Self::Return {
        if let Some(handler) = self.handler_mut().downcast_mut::<crate::Handler>() {
            handler.suppress_backtrace = suppress;
        }

        self
    }
}

impl<T, E> Section for Result<T, E>
where
    E: Into<Report>,
{
    type Return = Result<T, Report>;

    fn note<D>(self, note: D) -> Self::Return
    where
        D: Display + Send + Sync + 'static,
    {
        self.map_err(|error| error.into())
            .map_err(|report| report.note(note))
    }

    fn with_note<D, F>(self, note: F) -> Self::Return
    where
        D: Display + Send + Sync + 'static,
        F: FnOnce() -> D,
    {
        self.map_err(|error| error.into())
            .map_err(|report| report.note(note()))
    }

    fn warning<D>(self, warning: D) -> Self::Return
    where
        D: Display + Send + Sync + 'static,
    {
        self.map_err(|error| error.into())
            .map_err(|report| report.warning(warning))
    }

    fn with_warning<D, F>(self, warning: F) -> Self::Return
    where
        D: Display + Send + Sync + 'static,
        F: FnOnce() -> D,
    {
        self.map_err(|error| error.into())
            .map_err(|report| report.warning(warning()))
    }

    fn suggestion<D>(self, suggestion: D) -> Self::Return
    where
        D: Display + Send + Sync + 'static,
    {
        self.map_err(|error| error.into())
            .map_err(|report| report.suggestion(suggestion))
    }

    fn with_suggestion<D, F>(self, suggestion: F) -> Self::Return
    where
        D: Display + Send + Sync + 'static,
        F: FnOnce() -> D,
    {
        self.map_err(|error| error.into())
            .map_err(|report| report.suggestion(suggestion()))
    }

    fn with_section<D, F>(self, section: F) -> Self::Return
    where
        D: Display + Send + Sync + 'static,
        F: FnOnce() -> D,
    {
        self.map_err(|error| error.into())
            .map_err(|report| report.section(section()))
    }

    fn section<D>(self, section: D) -> Self::Return
    where
        D: Display + Send + Sync + 'static,
    {
        self.map_err(|error| error.into())
            .map_err(|report| report.section(section))
    }

    fn error<E2>(self, error: E2) -> Self::Return
    where
        E2: std::error::Error + Send + Sync + 'static,
    {
        self.map_err(|error| error.into())
            .map_err(|report| report.error(error))
    }

    fn with_error<E2, F>(self, error: F) -> Self::Return
    where
        F: FnOnce() -> E2,
        E2: std::error::Error + Send + Sync + 'static,
    {
        self.map_err(|error| error.into())
            .map_err(|report| report.error(error()))
    }

    fn suppress_backtrace(self, suppress: bool) -> Self::Return {
        self.map_err(|error| error.into())
            .map_err(|report| report.suppress_backtrace(suppress))
    }
}

pub(crate) enum HelpInfo {
    Error(Box<dyn std::error::Error + Send + Sync + 'static>, Theme),
    Custom(Box<dyn Display + Send + Sync + 'static>),
    Note(Box<dyn Display + Send + Sync + 'static>, Theme),
    Warning(Box<dyn Display + Send + Sync + 'static>, Theme),
    Suggestion(Box<dyn Display + Send + Sync + 'static>, Theme),
}

impl Display for HelpInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HelpInfo::Note(note, theme) => {
                write!(f, "{}: {}", "Note".style(theme.help_info_note), note)
            }
            HelpInfo::Warning(warning, theme) => write!(
                f,
                "{}: {}",
                "Warning".style(theme.help_info_warning),
                warning
            ),
            HelpInfo::Suggestion(suggestion, theme) => write!(
                f,
                "{}: {}",
                "Suggestion".style(theme.help_info_suggestion),
                suggestion
            ),
            HelpInfo::Custom(section) => write!(f, "{}", section),
            HelpInfo::Error(error, theme) => {
                // a lot here
                let errors = std::iter::successors(
                    Some(error.as_ref() as &(dyn std::error::Error + 'static)),
                    |e| e.source(),
                );

                write!(f, "Error:")?;
                for (n, error) in errors.enumerate() {
                    writeln!(f)?;
                    write!(indented(f).ind(n), "{}", error.style(theme.help_info_error))?;
                }

                Ok(())
            }
        }
    }
}

impl fmt::Debug for HelpInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HelpInfo::Note(note, ..) => f
                .debug_tuple("Note")
                .field(&format_args!("{}", note))
                .finish(),
            HelpInfo::Warning(warning, ..) => f
                .debug_tuple("Warning")
                .field(&format_args!("{}", warning))
                .finish(),
            HelpInfo::Suggestion(suggestion, ..) => f
                .debug_tuple("Suggestion")
                .field(&format_args!("{}", suggestion))
                .finish(),
            HelpInfo::Custom(custom, ..) => f
                .debug_tuple("CustomSection")
                .field(&format_args!("{}", custom))
                .finish(),
            HelpInfo::Error(error, ..) => f.debug_tuple("Error").field(error).finish(),
        }
    }
}
