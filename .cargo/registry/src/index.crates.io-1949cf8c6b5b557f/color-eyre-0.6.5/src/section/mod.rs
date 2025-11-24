//! Helpers for adding custom sections to error reports
use crate::writers::WriterExt;
use std::fmt::{self, Display};

#[cfg(feature = "issue-url")]
pub(crate) mod github;
pub(crate) mod help;

/// An indented section with a header for an error report
///
/// # Details
///
/// This helper provides two functions to help with constructing nicely formatted
/// error reports. First, it handles indentation of every line of the body for
/// you, and makes sure it is consistent with the rest of color-eyre's output.
/// Second, it omits outputting the header if the body itself is empty,
/// preventing unnecessary pollution of the report for sections with dynamic
/// content.
///
/// # Examples
///
/// ```rust
/// use color_eyre::{eyre::eyre, SectionExt, Section, eyre::Report};
/// use std::process::Command;
/// use tracing::instrument;
///
/// trait Output {
///     fn output2(&mut self) -> Result<String, Report>;
/// }
///
/// impl Output for Command {
///     #[instrument]
///     fn output2(&mut self) -> Result<String, Report> {
///         let output = self.output()?;
///
///         let stdout = String::from_utf8_lossy(&output.stdout);
///
///         if !output.status.success() {
///             let stderr = String::from_utf8_lossy(&output.stderr);
///             Err(eyre!("cmd exited with non-zero status code"))
///                 .with_section(move || stdout.trim().to_string().header("Stdout:"))
///                 .with_section(move || stderr.trim().to_string().header("Stderr:"))
///         } else {
///             Ok(stdout.into())
///         }
///     }
/// }
/// ```
#[allow(missing_debug_implementations)]
pub struct IndentedSection<H, B> {
    header: H,
    body: B,
}

impl<H, B> fmt::Display for IndentedSection<H, B>
where
    H: Display + Send + Sync + 'static,
    B: Display + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use std::fmt::Write;
        let mut headered = f.header(&self.header);
        let headered = headered.ready();
        let mut headered = headered.header("\n");

        let mut headered = headered.ready();

        let mut indented = indenter::indented(&mut headered)
            .with_format(indenter::Format::Uniform { indentation: "   " });

        write!(&mut indented, "{}", self.body)?;

        Ok(())
    }
}

/// Extension trait for constructing sections with commonly used formats
pub trait SectionExt: Sized {
    /// Add a header to a `Section` and indent the body
    ///
    /// # Details
    ///
    /// Bodies are always indented to the same level as error messages and spans.
    /// The header is not printed if the display impl of the body produces no
    /// output.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use color_eyre::{eyre::eyre, Section, SectionExt, eyre::Report};
    ///
    /// let all_in_header = "header\n   body\n   body";
    /// let report = Err::<(), Report>(eyre!("an error occurred"))
    ///     .section(all_in_header)
    ///     .unwrap_err();
    ///
    /// let just_header = "header";
    /// let just_body = "body\nbody";
    /// let report2 = Err::<(), Report>(eyre!("an error occurred"))
    ///     .section(just_body.header(just_header))
    ///     .unwrap_err();
    ///
    /// assert_eq!(format!("{:?}", report), format!("{:?}", report2))
    /// ```
    fn header<C>(self, header: C) -> IndentedSection<C, Self>
    where
        C: Display + Send + Sync + 'static;
}

impl<T> SectionExt for T
where
    T: Display + Send + Sync + 'static,
{
    fn header<C>(self, header: C) -> IndentedSection<C, Self>
    where
        C: Display + Send + Sync + 'static,
    {
        IndentedSection { body: self, header }
    }
}

/// A helper trait for attaching informational sections to error reports to be
/// displayed after the chain of errors
///
/// # Details
///
/// `color_eyre` provides two types of help text that can be attached to error reports: custom
/// sections and pre-configured sections. Custom sections are added via the `section` and
/// `with_section` methods, and give maximum control over formatting.
///
/// The pre-configured sections are provided via `suggestion`, `warning`, and `note`. These
/// sections are displayed after all other sections with no extra newlines between subsequent Help
/// sections. They consist only of a header portion and are prepended with a colored string
/// indicating the kind of section, e.g. `Note: This might have failed due to ..."
pub trait Section: crate::private::Sealed {
    /// The return type of each method after adding context
    type Return;

    /// Add a section to an error report, to be displayed after the chain of errors.
    ///
    /// # Details
    ///
    /// Sections are displayed in the order they are added to the error report. They are displayed
    /// immediately after the `Error:` section and before the `SpanTrace` and `Backtrace` sections.
    /// They consist of a header and an optional body. The body of the section is indented by
    /// default.
    ///
    /// # Examples
    ///
    /// ```rust,should_panic
    /// use color_eyre::{eyre::eyre, eyre::Report, Section};
    ///
    /// Err(eyre!("command failed"))
    ///     .section("Please report bugs to https://real.url/bugs")?;
    /// # Ok::<_, Report>(())
    /// ```
    fn section<D>(self, section: D) -> Self::Return
    where
        D: Display + Send + Sync + 'static;

    /// Add a Section to an error report, to be displayed after the chain of errors. The closure to
    /// create the Section is lazily evaluated only in the case of an error.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use color_eyre::{eyre::eyre, eyre::Report, Section, SectionExt};
    ///
    /// # #[cfg(not(miri))]
    /// # {
    /// let output = std::process::Command::new("ls")
    ///     .output()?;
    ///
    /// let output = if !output.status.success() {
    ///     let stderr = String::from_utf8_lossy(&output.stderr);
    ///     Err(eyre!("cmd exited with non-zero status code"))
    ///         .with_section(move || stderr.trim().to_string().header("Stderr:"))?
    /// } else {
    ///     String::from_utf8_lossy(&output.stdout)
    /// };
    ///
    /// println!("{}", output);
    /// # }
    /// # Ok::<_, Report>(())
    /// ```
    fn with_section<D, F>(self, section: F) -> Self::Return
    where
        D: Display + Send + Sync + 'static,
        F: FnOnce() -> D;

    /// Add an error section to an error report, to be displayed after the primary error message
    /// section.
    ///
    /// # Examples
    ///
    /// ```rust,should_panic
    /// use color_eyre::{eyre::eyre, eyre::Report, Section};
    /// use thiserror::Error;
    ///
    /// #[derive(Debug, Error)]
    /// #[error("{0}")]
    /// struct StrError(&'static str);
    ///
    /// Err(eyre!("command failed"))
    ///     .error(StrError("got one error"))
    ///     .error(StrError("got a second error"))?;
    /// # Ok::<_, Report>(())
    /// ```
    fn error<E>(self, error: E) -> Self::Return
    where
        E: std::error::Error + Send + Sync + 'static;

    /// Add an error section to an error report, to be displayed after the primary error message
    /// section. The closure to create the Section is lazily evaluated only in the case of an error.
    ///
    /// # Examples
    ///
    /// ```rust,should_panic
    /// use color_eyre::{eyre::eyre, eyre::Report, Section};
    /// use thiserror::Error;
    ///
    /// #[derive(Debug, Error)]
    /// #[error("{0}")]
    /// struct StringError(String);
    ///
    /// Err(eyre!("command failed"))
    ///     .with_error(|| StringError("got one error".into()))
    ///     .with_error(|| StringError("got a second error".into()))?;
    /// # Ok::<_, Report>(())
    /// ```
    fn with_error<E, F>(self, error: F) -> Self::Return
    where
        F: FnOnce() -> E,
        E: std::error::Error + Send + Sync + 'static;

    /// Add a Note to an error report, to be displayed after the chain of errors.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use std::{error::Error, fmt::{self, Display}};
    /// # use color_eyre::eyre::Result;
    /// # #[derive(Debug)]
    /// # struct FakeErr;
    /// # impl Display for FakeErr {
    /// #     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    /// #         write!(f, "FakeErr")
    /// #     }
    /// # }
    /// # impl std::error::Error for FakeErr {}
    /// # fn main() -> Result<()> {
    /// # fn fallible_fn() -> Result<(), FakeErr> {
    /// #       Ok(())
    /// # }
    /// use color_eyre::Section as _;
    ///
    /// fallible_fn().note("This might have failed due to ...")?;
    /// # Ok(())
    /// # }
    /// ```
    fn note<D>(self, note: D) -> Self::Return
    where
        D: Display + Send + Sync + 'static;

    /// Add a Note to an error report, to be displayed after the chain of errors. The closure to
    /// create the Note is lazily evaluated only in the case of an error.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use std::{error::Error, fmt::{self, Display}};
    /// # use color_eyre::eyre::Result;
    /// # #[derive(Debug)]
    /// # struct FakeErr;
    /// # impl Display for FakeErr {
    /// #     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    /// #         write!(f, "FakeErr")
    /// #     }
    /// # }
    /// # impl std::error::Error for FakeErr {}
    /// # fn main() -> Result<()> {
    /// # fn fallible_fn() -> Result<(), FakeErr> {
    /// #       Ok(())
    /// # }
    /// use color_eyre::Section as _;
    ///
    /// fallible_fn().with_note(|| {
    ///         format!("This might have failed due to ... It has failed {} times", 100)
    ///     })?;
    /// # Ok(())
    /// # }
    /// ```
    fn with_note<D, F>(self, f: F) -> Self::Return
    where
        D: Display + Send + Sync + 'static,
        F: FnOnce() -> D;

    /// Add a Warning to an error report, to be displayed after the chain of errors.
    fn warning<D>(self, warning: D) -> Self::Return
    where
        D: Display + Send + Sync + 'static;

    /// Add a Warning to an error report, to be displayed after the chain of errors. The closure to
    /// create the Warning is lazily evaluated only in the case of an error.
    fn with_warning<D, F>(self, f: F) -> Self::Return
    where
        D: Display + Send + Sync + 'static,
        F: FnOnce() -> D;

    /// Add a Suggestion to an error report, to be displayed after the chain of errors.
    fn suggestion<D>(self, suggestion: D) -> Self::Return
    where
        D: Display + Send + Sync + 'static;

    /// Add a Suggestion to an error report, to be displayed after the chain of errors. The closure
    /// to create the Suggestion is lazily evaluated only in the case of an error.
    fn with_suggestion<D, F>(self, f: F) -> Self::Return
    where
        D: Display + Send + Sync + 'static,
        F: FnOnce() -> D;

    /// Whether to suppress printing of collected backtrace (if any).
    ///
    /// Useful for reporting "unexceptional" errors for which a backtrace
    /// isn't really necessary.
    fn suppress_backtrace(self, suppress: bool) -> Self::Return;
}

/// Trait for printing a panic error message for the given PanicInfo
pub trait PanicMessage: Send + Sync + 'static {
    /// Display trait equivalent for implementing the display logic
    fn display(&self, pi: &std::panic::PanicInfo<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result;
}
