//! Input validation API (Multi-line editing)

use crate::keymap::Invoke;
use crate::Result;

/// Input validation result
#[non_exhaustive]
pub enum ValidationResult {
    /// Incomplete input
    Incomplete,
    /// Validation fails with an optional error message. User must fix the
    /// input.
    Invalid(Option<String>),
    /// Validation succeeds with an optional message
    Valid(Option<String>),
}

impl ValidationResult {
    pub(crate) fn is_valid(&self) -> bool {
        matches!(self, ValidationResult::Valid(_))
    }

    pub(crate) fn has_message(&self) -> bool {
        matches!(
            self,
            ValidationResult::Valid(Some(_)) | ValidationResult::Invalid(Some(_))
        )
    }
}

/// Give access to user input.
pub struct ValidationContext<'i> {
    i: &'i mut dyn Invoke,
}

impl<'i> ValidationContext<'i> {
    pub(crate) fn new(i: &'i mut dyn Invoke) -> Self {
        ValidationContext { i }
    }

    /// Returns user input.
    #[must_use]
    pub fn input(&self) -> &str {
        self.i.input()
    }

    // TODO
    //fn invoke(&mut self, cmd: Cmd) -> Result<?> {
    //    self.i.invoke(cmd)
    //}
}

/// This trait provides an extension interface for determining whether
/// the current input buffer is valid. Rustyline uses the method
/// provided by this trait to decide whether hitting the enter key
/// will end the current editing session and return the current line
/// buffer to the caller of `Editor::readline` or variants.
pub trait Validator {
    /// Takes the currently edited `input` and returns a
    /// `ValidationResult` indicating whether it is valid or not along
    /// with an option message to display about the result. The most
    /// common validity check to implement is probably whether the
    /// input is complete or not, for instance ensuring that all
    /// delimiters are fully balanced.
    ///
    /// If you implement more complex validation checks it's probably
    /// a good idea to also implement a `Hinter` to provide feedback
    /// about what is invalid.
    ///
    /// For auto-correction like a missing closing quote or to reject invalid
    /// char while typing, the input will be mutable (TODO).
    fn validate(&self, ctx: &mut ValidationContext) -> Result<ValidationResult> {
        let _ = ctx;
        Ok(ValidationResult::Valid(None))
    }

    /// Configure whether validation is performed while typing or only
    /// when user presses the Enter key.
    ///
    /// Default is `false`.
    ///
    /// This feature is not yet implemented, so this function is currently a
    /// no-op
    fn validate_while_typing(&self) -> bool {
        false
    }
}

impl Validator for () {}

impl<'v, V: ?Sized + Validator> Validator for &'v V {
    fn validate(&self, ctx: &mut ValidationContext) -> Result<ValidationResult> {
        (**self).validate(ctx)
    }

    fn validate_while_typing(&self) -> bool {
        (**self).validate_while_typing()
    }
}

/// Simple matching bracket validator.
#[derive(Default)]
pub struct MatchingBracketValidator {
    _priv: (),
}

impl MatchingBracketValidator {
    /// Constructor
    #[must_use]
    pub fn new() -> Self {
        Self { _priv: () }
    }
}

impl Validator for MatchingBracketValidator {
    fn validate(&self, ctx: &mut ValidationContext) -> Result<ValidationResult> {
        Ok(validate_brackets(ctx.input()))
    }
}

fn validate_brackets(input: &str) -> ValidationResult {
    let mut stack = vec![];
    for c in input.chars() {
        match c {
            '(' | '[' | '{' => stack.push(c),
            ')' | ']' | '}' => match (stack.pop(), c) {
                (Some('('), ')') | (Some('['), ']') | (Some('{'), '}') => {}
                (Some(wanted), _) => {
                    return ValidationResult::Invalid(Some(format!(
                        "Mismatched brackets: {wanted:?} is not properly closed"
                    )))
                }
                (None, c) => {
                    return ValidationResult::Invalid(Some(format!(
                        "Mismatched brackets: {c:?} is unpaired"
                    )))
                }
            },
            _ => {}
        }
    }
    if stack.is_empty() {
        ValidationResult::Valid(None)
    } else {
        ValidationResult::Incomplete
    }
}
