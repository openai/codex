#![allow(missing_copy_implementations)]
#![allow(missing_debug_implementations)]
#![cfg_attr(not(feature = "error-context"), allow(dead_code))]
#![cfg_attr(not(feature = "error-context"), allow(unused_imports))]

use std::borrow::Cow;

use crate::builder::Command;
use crate::builder::StyledStr;
use crate::builder::Styles;
#[cfg(feature = "error-context")]
use crate::error::ContextKind;
#[cfg(feature = "error-context")]
use crate::error::ContextValue;
use crate::error::ErrorKind;
use crate::output::TAB;
use crate::ArgAction;

/// Defines how to format an error for displaying to the user
pub trait ErrorFormatter: Sized {
    /// Stylize the error for the terminal
    fn format_error(error: &crate::error::Error<Self>) -> StyledStr;
}

/// Report [`ErrorKind`]
///
/// No context is included.
///
/// <div class="warning">
///
/// **NOTE:** Consider removing the `error-context` default feature if using this to remove all
/// overhead for [`RichFormatter`].
///
/// </div>
#[non_exhaustive]
pub struct KindFormatter;

impl ErrorFormatter for KindFormatter {
    fn format_error(error: &crate::error::Error<Self>) -> StyledStr {
        use std::fmt::Write as _;
        let styles = &error.inner.styles;

        let mut styled = StyledStr::new();
        start_error(&mut styled, styles);
        if let Some(msg) = error.kind().as_str() {
            styled.push_str(msg);
        } else if let Some(source) = error.inner.source.as_ref() {
            let _ = write!(styled, "{source}");
        } else {
            styled.push_str("unknown cause");
        }
        styled.push_str("\n");
        styled
    }
}

/// Richly formatted error context
///
/// This follows the [rustc diagnostic style guide](https://rustc-dev-guide.rust-lang.org/diagnostics.html#suggestion-style-guide).
#[non_exhaustive]
#[cfg(feature = "error-context")]
pub struct RichFormatter;

#[cfg(feature = "error-context")]
impl ErrorFormatter for RichFormatter {
    fn format_error(error: &crate::error::Error<Self>) -> StyledStr {
        use std::fmt::Write as _;
        let styles = &error.inner.styles;
        let valid = &styles.get_valid();

        let mut styled = StyledStr::new();
        start_error(&mut styled, styles);

        if !write_dynamic_context(error, &mut styled, styles) {
            if let Some(msg) = error.kind().as_str() {
                styled.push_str(msg);
            } else if let Some(source) = error.inner.source.as_ref() {
                let _ = write!(styled, "{source}");
            } else {
                styled.push_str("unknown cause");
            }
        }

        let mut suggested = false;
        if let Some(valid) = error.get(ContextKind::SuggestedSubcommand) {
            styled.push_str("\n");
            if !suggested {
                styled.push_str("\n");
                suggested = true;
            }
            did_you_mean(&mut styled, styles, "subcommand", valid);
        }
        if let Some(valid) = error.get(ContextKind::SuggestedArg) {
            styled.push_str("\n");
            if !suggested {
                styled.push_str("\n");
                suggested = true;
            }
            did_you_mean(&mut styled, styles, "argument", valid);
        }
        if let Some(valid) = error.get(ContextKind::SuggestedValue) {
            styled.push_str("\n");
            if !suggested {
                styled.push_str("\n");
                suggested = true;
            }
            did_you_mean(&mut styled, styles, "value", valid);
        }
        let suggestions = error.get(ContextKind::Suggested);
        if let Some(ContextValue::StyledStrs(suggestions)) = suggestions {
            if !suggested {
                styled.push_str("\n");
            }
            for suggestion in suggestions {
                let _ = write!(styled, "\n{TAB}{valid}tip:{valid:#} ",);
                styled.push_styled(suggestion);
            }
        }

        let usage = error.get(ContextKind::Usage);
        if let Some(ContextValue::StyledStr(usage)) = usage {
            put_usage(&mut styled, usage);
        }

        try_help(&mut styled, styles, error.inner.help_flag.as_deref());

        styled
    }
}

fn start_error(styled: &mut StyledStr, styles: &Styles) {
    use std::fmt::Write as _;
    let error = &styles.get_error();
    let _ = write!(styled, "{error}error:{error:#} ");
}

#[must_use]
#[cfg(feature = "error-context")]
fn write_dynamic_context(
    error: &crate::error::Error,
    styled: &mut StyledStr,
    styles: &Styles,
) -> bool {
    use std::fmt::Write as _;
    let valid = styles.get_valid();
    let invalid = styles.get_invalid();
    let literal = styles.get_literal();

    match error.kind() {
        ErrorKind::ArgumentConflict => {
            let mut prior_arg = error.get(ContextKind::PriorArg);
            if let Some(ContextValue::String(invalid_arg)) = error.get(ContextKind::InvalidArg) {
                if Some(&ContextValue::String(invalid_arg.clone())) == prior_arg {
                    prior_arg = None;
                    let _ = write!(
                        styled,
                        "the argument '{invalid}{invalid_arg}{invalid:#}' cannot be used multiple times",
                    );
                } else {
                    let _ = write!(
                        styled,
                        "the argument '{invalid}{invalid_arg}{invalid:#}' cannot be used with",
                    );
                }
            } else if let Some(ContextValue::String(invalid_arg)) =
                error.get(ContextKind::InvalidSubcommand)
            {
                let _ = write!(
                    styled,
                    "the subcommand '{invalid}{invalid_arg}{invalid:#}' cannot be used with",
                );
            } else {
                styled.push_str(error.kind().as_str().unwrap());
            }

            if let Some(prior_arg) = prior_arg {
                match prior_arg {
                    ContextValue::Strings(values) => {
                        styled.push_str(":");
                        for v in values {
                            let _ = write!(styled, "\n{TAB}{invalid}{v}{invalid:#}",);
                        }
                    }
                    ContextValue::String(value) => {
                        let _ = write!(styled, " '{invalid}{value}{invalid:#}'",);
                    }
                    _ => {
                        styled.push_str(" one or more of the other specified arguments");
                    }
                }
            }

            true
        }
        ErrorKind::NoEquals => {
            let invalid_arg = error.get(ContextKind::InvalidArg);
            if let Some(ContextValue::String(invalid_arg)) = invalid_arg {
                let _ = write!(
                    styled,
                    "equal sign is needed when assigning values to '{invalid}{invalid_arg}{invalid:#}'",
                );
                true
            } else {
                false
            }
        }
        ErrorKind::InvalidValue => {
            let invalid_arg = error.get(ContextKind::InvalidArg);
            let invalid_value = error.get(ContextKind::InvalidValue);
            if let (
                Some(ContextValue::String(invalid_arg)),
                Some(ContextValue::String(invalid_value)),
            ) = (invalid_arg, invalid_value)
            {
                if invalid_value.is_empty() {
                    let _ = write!(
                        styled,
                        "a value is required for '{invalid}{invalid_arg}{invalid:#}' but none was supplied",
                    );
                } else {
                    let _ = write!(
                        styled,
                        "invalid value '{invalid}{invalid_value}{invalid:#}' for '{literal}{invalid_arg}{literal:#}'",
                    );
                }

                let values = error.get(ContextKind::ValidValue);
                write_values_list("possible values", styled, valid, values);

                true
            } else {
                false
            }
        }
        ErrorKind::InvalidSubcommand => {
            let invalid_sub = error.get(ContextKind::InvalidSubcommand);
            if let Some(ContextValue::String(invalid_sub)) = invalid_sub {
                let _ = write!(
                    styled,
                    "unrecognized subcommand '{invalid}{invalid_sub}{invalid:#}'",
                );
                true
            } else {
                false
            }
        }
        ErrorKind::MissingRequiredArgument => {
            let invalid_arg = error.get(ContextKind::InvalidArg);
            if let Some(ContextValue::Strings(invalid_arg)) = invalid_arg {
                styled.push_str("the following required arguments were not provided:");
                for v in invalid_arg {
                    let _ = write!(styled, "\n{TAB}{valid}{v}{valid:#}",);
                }
                true
            } else {
                false
            }
        }
        ErrorKind::MissingSubcommand => {
            let invalid_sub = error.get(ContextKind::InvalidSubcommand);
            if let Some(ContextValue::String(invalid_sub)) = invalid_sub {
                let _ = write!(
                    styled,
                    "'{invalid}{invalid_sub}{invalid:#}' requires a subcommand but one was not provided",
                );
                let values = error.get(ContextKind::ValidSubcommand);
                write_values_list("subcommands", styled, valid, values);

                true
            } else {
                false
            }
        }
        ErrorKind::InvalidUtf8 => false,
        ErrorKind::TooManyValues => {
            let invalid_arg = error.get(ContextKind::InvalidArg);
            let invalid_value = error.get(ContextKind::InvalidValue);
            if let (
                Some(ContextValue::String(invalid_arg)),
                Some(ContextValue::String(invalid_value)),
            ) = (invalid_arg, invalid_value)
            {
                let _ = write!(
                    styled,
                    "unexpected value '{invalid}{invalid_value}{invalid:#}' for '{literal}{invalid_arg}{literal:#}' found; no more were expected",
                );
                true
            } else {
                false
            }
        }
        ErrorKind::TooFewValues => {
            let invalid_arg = error.get(ContextKind::InvalidArg);
            let actual_num_values = error.get(ContextKind::ActualNumValues);
            let min_values = error.get(ContextKind::MinValues);
            if let (
                Some(ContextValue::String(invalid_arg)),
                Some(ContextValue::Number(actual_num_values)),
                Some(ContextValue::Number(min_values)),
            ) = (invalid_arg, actual_num_values, min_values)
            {
                let were_provided = singular_or_plural(*actual_num_values as usize);
                let _ = write!(
                    styled,
                    "{valid}{min_values}{valid:#} values required by '{literal}{invalid_arg}{literal:#}'; only {invalid}{actual_num_values}{invalid:#}{were_provided}",
                );
                true
            } else {
                false
            }
        }
        ErrorKind::ValueValidation => {
            let invalid_arg = error.get(ContextKind::InvalidArg);
            let invalid_value = error.get(ContextKind::InvalidValue);
            if let (
                Some(ContextValue::String(invalid_arg)),
                Some(ContextValue::String(invalid_value)),
            ) = (invalid_arg, invalid_value)
            {
                let _ = write!(
                    styled,
                    "invalid value '{invalid}{invalid_value}{invalid:#}' for '{literal}{invalid_arg}{literal:#}'",
                );
                if let Some(source) = error.inner.source.as_deref() {
                    let _ = write!(styled, ": {source}");
                }
                true
            } else {
                false
            }
        }
        ErrorKind::WrongNumberOfValues => {
            let invalid_arg = error.get(ContextKind::InvalidArg);
            let actual_num_values = error.get(ContextKind::ActualNumValues);
            let num_values = error.get(ContextKind::ExpectedNumValues);
            if let (
                Some(ContextValue::String(invalid_arg)),
                Some(ContextValue::Number(actual_num_values)),
                Some(ContextValue::Number(num_values)),
            ) = (invalid_arg, actual_num_values, num_values)
            {
                let were_provided = singular_or_plural(*actual_num_values as usize);
                let _ = write!(
                    styled,
                    "{valid}{num_values}{valid:#} values required for '{literal}{invalid_arg}{literal:#}' but {invalid}{actual_num_values}{invalid:#}{were_provided}",
                );
                true
            } else {
                false
            }
        }
        ErrorKind::UnknownArgument => {
            let invalid_arg = error.get(ContextKind::InvalidArg);
            if let Some(ContextValue::String(invalid_arg)) = invalid_arg {
                let _ = write!(
                    styled,
                    "unexpected argument '{invalid}{invalid_arg}{invalid:#}' found",
                );
                true
            } else {
                false
            }
        }
        ErrorKind::DisplayHelp
        | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
        | ErrorKind::DisplayVersion
        | ErrorKind::Io
        | ErrorKind::Format => false,
    }
}

#[cfg(feature = "error-context")]
fn write_values_list(
    list_name: &'static str,
    styled: &mut StyledStr,
    valid: &anstyle::Style,
    possible_values: Option<&ContextValue>,
) {
    use std::fmt::Write as _;
    if let Some(ContextValue::Strings(possible_values)) = possible_values {
        if !possible_values.is_empty() {
            let _ = write!(styled, "\n{TAB}[{list_name}: ");

            for (idx, val) in possible_values.iter().enumerate() {
                if idx > 0 {
                    styled.push_str(", ");
                }
                let _ = write!(styled, "{valid}{}{valid:#}", Escape(val));
            }

            styled.push_str("]");
        }
    }
}

pub(crate) fn format_error_message(
    message: &str,
    styles: &Styles,
    cmd: Option<&Command>,
    usage: Option<&StyledStr>,
) -> StyledStr {
    let mut styled = StyledStr::new();
    start_error(&mut styled, styles);
    styled.push_str(message);
    if let Some(usage) = usage {
        put_usage(&mut styled, usage);
    }
    if let Some(cmd) = cmd {
        try_help(&mut styled, styles, get_help_flag(cmd).as_deref());
    }
    styled
}

/// Returns the singular or plural form on the verb to be based on the argument's value.
fn singular_or_plural(n: usize) -> &'static str {
    if n > 1 {
        " were provided"
    } else {
        " was provided"
    }
}

fn put_usage(styled: &mut StyledStr, usage: &StyledStr) {
    styled.push_str("\n\n");
    styled.push_styled(usage);
}

pub(crate) fn get_help_flag(cmd: &Command) -> Option<Cow<'static, str>> {
    if !cmd.is_disable_help_flag_set() {
        Some(Cow::Borrowed("--help"))
    } else if let Some(flag) = get_user_help_flag(cmd) {
        Some(Cow::Owned(flag))
    } else if cmd.has_subcommands() && !cmd.is_disable_help_subcommand_set() {
        Some(Cow::Borrowed("help"))
    } else {
        None
    }
}

fn get_user_help_flag(cmd: &Command) -> Option<String> {
    let arg = cmd.get_arguments().find(|arg| match arg.get_action() {
        ArgAction::Help | ArgAction::HelpShort | ArgAction::HelpLong => true,
        ArgAction::Append
        | ArgAction::Count
        | ArgAction::SetTrue
        | ArgAction::SetFalse
        | ArgAction::Set
        | ArgAction::Version => false,
    })?;

    arg.get_long()
        .map(|long| format!("--{long}"))
        .or_else(|| arg.get_short().map(|short| format!("-{short}")))
}

fn try_help(styled: &mut StyledStr, styles: &Styles, help: Option<&str>) {
    if let Some(help) = help {
        use std::fmt::Write as _;
        let literal = &styles.get_literal();
        let _ = write!(
            styled,
            "\n\nFor more information, try '{literal}{help}{literal:#}'.\n",
        );
    } else {
        styled.push_str("\n");
    }
}

#[cfg(feature = "error-context")]
fn did_you_mean(styled: &mut StyledStr, styles: &Styles, context: &str, possibles: &ContextValue) {
    use std::fmt::Write as _;

    let valid = &styles.get_valid();
    let _ = write!(styled, "{TAB}{valid}tip:{valid:#}",);
    if let ContextValue::String(possible) = possibles {
        let _ = write!(
            styled,
            " a similar {context} exists: '{valid}{possible}{valid:#}'",
        );
    } else if let ContextValue::Strings(possibles) = possibles {
        if possibles.len() == 1 {
            let _ = write!(styled, " a similar {context} exists: ",);
        } else {
            let _ = write!(styled, " some similar {context}s exist: ",);
        }
        for (i, possible) in possibles.iter().enumerate() {
            if i != 0 {
                styled.push_str(", ");
            }
            let _ = write!(styled, "'{valid}{possible}{valid:#}'",);
        }
    }
}

struct Escape<'s>(&'s str);

impl std::fmt::Display for Escape<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0.contains(char::is_whitespace) {
            std::fmt::Debug::fmt(self.0, f)
        } else {
            self.0.fmt(f)
        }
    }
}
