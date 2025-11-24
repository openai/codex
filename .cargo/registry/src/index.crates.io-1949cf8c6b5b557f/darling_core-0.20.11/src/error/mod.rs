//! The `darling::Error` type, the multiple error `Accumulator`, and their internals.
//!
//! Error handling is one of the core values of `darling`; creating great errors is hard and
//! never the reason that a proc-macro author started writing their crate. As a result, the
//! `Error` type in `darling` tries to make adding span information, suggestions, and other
//! help content easy when manually implementing `darling` traits, and automatic when deriving
//! them.

use proc_macro2::{Span, TokenStream};
use std::error::Error as StdError;
use std::fmt;
use std::iter::{self, Iterator};
use std::string::ToString;
use std::vec;
use syn::spanned::Spanned;
use syn::{Expr, Lit, LitStr, Path};

#[cfg(feature = "diagnostics")]
mod child;
mod kind;

use crate::util::path_to_string;

use self::kind::{ErrorKind, ErrorUnknownField};

/// An alias of `Result` specific to attribute parsing.
pub type Result<T> = ::std::result::Result<T, Error>;

/// An error encountered during attribute parsing.
///
/// Given that most errors darling encounters represent code bugs in dependent crates,
/// the internal structure of the error is deliberately opaque.
///
/// # Usage
/// Proc-macro expansion happens very infrequently compared to runtime tasks such as
/// deserialization, and it happens in the context of an expensive compilation taks.
/// For that reason, darling prefers not to fail on the first error it encounters, instead
/// doing as much work as it can, accumulating errors into a single report.
///
/// As a result, `darling::Error` is more of guaranteed-non-empty error collection
/// than a single problem. These errors also have some notion of hierarchy, stemming from
/// the hierarchical nature of darling's input.
///
/// These characteristics make for great experiences when using darling-powered crates,
/// provided crates using darling adhere to some best practices:
///
/// 1. Do not attempt to simplify a `darling::Error` into some other error type, such as
///    `syn::Error`. To surface compile errors, instead use `darling::Error::write_errors`.
///    This preserves all span information, suggestions, etc. Wrapping a `darling::Error` in
///    a custom error enum works as-expected and does not force any loss of fidelity.
/// 2. Do not use early return (e.g. the `?` operator) for custom validations. Instead,
///    create an [`error::Accumulator`](Accumulator) to collect errors as they are encountered.  Then use
///    [`Accumulator::finish`] to return your validated result; it will give `Ok` if and only if
///    no errors were encountered.  This can create very complex custom validation functions;
///    in those cases, split independent "validation chains" out into their own functions to
///    keep the main validator manageable.
/// 3. Use `darling::Error::custom` to create additional errors as-needed, then call `with_span`
///    to ensure those errors appear in the right place. Use `darling::util::SpannedValue` to keep
///    span information around on parsed fields so that custom diagnostics can point to the correct
///    parts of the input AST.
#[derive(Debug, Clone)]
pub struct Error {
    kind: ErrorKind,
    locations: Vec<String>,
    /// The span to highlight in the emitted diagnostic.
    span: Option<Span>,
    /// Additional diagnostic messages to show with the error.
    #[cfg(feature = "diagnostics")]
    children: Vec<child::ChildDiagnostic>,
}

/// Error creation functions
impl Error {
    pub(in crate::error) fn new(kind: ErrorKind) -> Self {
        Error {
            kind,
            locations: Vec::new(),
            span: None,
            #[cfg(feature = "diagnostics")]
            children: vec![],
        }
    }

    /// Creates a new error with a custom message.
    pub fn custom<T: fmt::Display>(msg: T) -> Self {
        Error::new(ErrorKind::Custom(msg.to_string()))
    }

    /// Creates a new error for a field that appears twice in the input.
    pub fn duplicate_field(name: &str) -> Self {
        Error::new(ErrorKind::DuplicateField(name.into()))
    }

    /// Creates a new error for a field that appears twice in the input. Helper to avoid repeating
    /// the syn::Path to String conversion.
    pub fn duplicate_field_path(path: &Path) -> Self {
        Error::duplicate_field(&path_to_string(path))
    }

    /// Creates a new error for a non-optional field that does not appear in the input.
    pub fn missing_field(name: &str) -> Self {
        Error::new(ErrorKind::MissingField(name.into()))
    }

    /// Creates a new error for a field name that appears in the input but does not correspond
    /// to a known field.
    pub fn unknown_field(name: &str) -> Self {
        Error::new(ErrorKind::UnknownField(name.into()))
    }

    /// Creates a new error for a field name that appears in the input but does not correspond
    /// to a known field. Helper to avoid repeating the syn::Path to String conversion.
    pub fn unknown_field_path(path: &Path) -> Self {
        Error::unknown_field(&path_to_string(path))
    }

    /// Creates a new error for a field name that appears in the input but does not correspond to
    /// a known attribute. The second argument is the list of known attributes; if a similar name
    /// is found that will be shown in the emitted error message.
    pub fn unknown_field_with_alts<'a, T, I>(field: &str, alternates: I) -> Self
    where
        T: AsRef<str> + 'a,
        I: IntoIterator<Item = &'a T>,
    {
        Error::new(ErrorUnknownField::with_alts(field, alternates).into())
    }

    /// Creates a new error for a field name that appears in the input but does not correspond to
    /// a known attribute. The second argument is the list of known attributes; if a similar name
    /// is found that will be shown in the emitted error message.
    pub fn unknown_field_path_with_alts<'a, T, I>(field: &Path, alternates: I) -> Self
    where
        T: AsRef<str> + 'a,
        I: IntoIterator<Item = &'a T>,
    {
        Error::new(ErrorUnknownField::with_alts(&path_to_string(field), alternates).into())
    }

    /// Creates a new error for a struct or variant that does not adhere to the supported shape.
    pub fn unsupported_shape(shape: &str) -> Self {
        Error::new(ErrorKind::UnsupportedShape {
            observed: shape.into(),
            expected: None,
        })
    }

    pub fn unsupported_shape_with_expected<T: fmt::Display>(shape: &str, expected: &T) -> Self {
        Error::new(ErrorKind::UnsupportedShape {
            observed: shape.into(),
            expected: Some(expected.to_string()),
        })
    }

    pub fn unsupported_format(format: &str) -> Self {
        Error::new(ErrorKind::UnexpectedFormat(format.into()))
    }

    /// Creates a new error for a field which has an unexpected literal type.
    pub fn unexpected_type(ty: &str) -> Self {
        Error::new(ErrorKind::UnexpectedType(ty.into()))
    }

    pub fn unexpected_expr_type(expr: &Expr) -> Self {
        Error::unexpected_type(match *expr {
            Expr::Array(_) => "array",
            Expr::Assign(_) => "assign",
            Expr::Async(_) => "async",
            Expr::Await(_) => "await",
            Expr::Binary(_) => "binary",
            Expr::Block(_) => "block",
            Expr::Break(_) => "break",
            Expr::Call(_) => "call",
            Expr::Cast(_) => "cast",
            Expr::Closure(_) => "closure",
            Expr::Const(_) => "const",
            Expr::Continue(_) => "continue",
            Expr::Field(_) => "field",
            Expr::ForLoop(_) => "for_loop",
            Expr::Group(_) => "group",
            Expr::If(_) => "if",
            Expr::Index(_) => "index",
            Expr::Infer(_) => "infer",
            Expr::Let(_) => "let",
            Expr::Lit(_) => "lit",
            Expr::Loop(_) => "loop",
            Expr::Macro(_) => "macro",
            Expr::Match(_) => "match",
            Expr::MethodCall(_) => "method_call",
            Expr::Paren(_) => "paren",
            Expr::Path(_) => "path",
            Expr::Range(_) => "range",
            Expr::Reference(_) => "reference",
            Expr::Repeat(_) => "repeat",
            Expr::Return(_) => "return",
            Expr::Struct(_) => "struct",
            Expr::Try(_) => "try",
            Expr::TryBlock(_) => "try_block",
            Expr::Tuple(_) => "tuple",
            Expr::Unary(_) => "unary",
            Expr::Unsafe(_) => "unsafe",
            Expr::Verbatim(_) => "verbatim",
            Expr::While(_) => "while",
            Expr::Yield(_) => "yield",
            // non-exhaustive enum
            _ => "unknown",
        })
        .with_span(expr)
    }

    /// Creates a new error for a field which has an unexpected literal type. This will automatically
    /// extract the literal type name from the passed-in `Lit` and set the span to encompass only the
    /// literal value.
    ///
    /// # Usage
    /// This is most frequently used in overrides of the `FromMeta::from_value` method.
    ///
    /// ```rust
    /// # // pretend darling_core is darling so the doc example looks correct.
    /// # extern crate darling_core as darling;
    /// # extern crate syn;
    ///
    /// use darling::{FromMeta, Error, Result};
    /// use syn::{Lit, LitStr};
    ///
    /// pub struct Foo(String);
    ///
    /// impl FromMeta for Foo {
    ///     fn from_value(value: &Lit) -> Result<Self> {
    ///         if let Lit::Str(ref lit_str) = *value {
    ///             Ok(Foo(lit_str.value()))
    ///         } else {
    ///             Err(Error::unexpected_lit_type(value))
    ///         }
    ///     }
    /// }
    ///
    /// # fn main() {}
    /// ```
    pub fn unexpected_lit_type(lit: &Lit) -> Self {
        Error::unexpected_type(match *lit {
            Lit::Str(_) => "string",
            Lit::ByteStr(_) => "byte string",
            Lit::Byte(_) => "byte",
            Lit::Char(_) => "char",
            Lit::Int(_) => "int",
            Lit::Float(_) => "float",
            Lit::Bool(_) => "bool",
            Lit::Verbatim(_) => "verbatim",
            // non-exhaustive enum
            _ => "unknown",
        })
        .with_span(lit)
    }

    /// Creates a new error for a value which doesn't match a set of expected literals.
    pub fn unknown_value(value: &str) -> Self {
        Error::new(ErrorKind::UnknownValue(value.into()))
    }

    /// Creates a new error for a list which did not get enough items to proceed.
    pub fn too_few_items(min: usize) -> Self {
        Error::new(ErrorKind::TooFewItems(min))
    }

    /// Creates a new error when a list got more items than it supports. The `max` argument
    /// is the largest number of items the receiver could accept.
    pub fn too_many_items(max: usize) -> Self {
        Error::new(ErrorKind::TooManyItems(max))
    }

    /// Bundle a set of multiple errors into a single `Error` instance.
    ///
    /// Usually it will be more convenient to use an [`error::Accumulator`](Accumulator).
    ///
    /// # Panics
    /// This function will panic if `errors.is_empty() == true`.
    pub fn multiple(mut errors: Vec<Error>) -> Self {
        match errors.len() {
            1 => errors
                .pop()
                .expect("Error array of length 1 has a first item"),
            0 => panic!("Can't deal with 0 errors"),
            _ => Error::new(ErrorKind::Multiple(errors)),
        }
    }

    /// Creates an error collector, for aggregating multiple errors
    ///
    /// See [`Accumulator`] for details.
    pub fn accumulator() -> Accumulator {
        Default::default()
    }
}

impl Error {
    /// Create a new error about a literal string that doesn't match a set of known
    /// or permissible values. This function can be made public if the API proves useful
    /// beyond impls for `syn` types.
    pub(crate) fn unknown_lit_str_value(value: &LitStr) -> Self {
        Error::unknown_value(&value.value()).with_span(value)
    }
}

/// Error instance methods
#[allow(clippy::len_without_is_empty)] // Error can never be empty
impl Error {
    /// Check if this error is associated with a span in the token stream.
    pub fn has_span(&self) -> bool {
        self.span.is_some()
    }

    /// Tie a span to the error if none is already present. This is used in `darling::FromMeta`
    /// and other traits to attach errors to the most specific possible location in the input
    /// source code.
    ///
    /// All `darling`-built impls, either from the crate or from the proc macro, will call this
    /// when appropriate during parsing, so it should not be necessary to call this unless you have
    /// overridden:
    ///
    /// * `FromMeta::from_meta`
    /// * `FromMeta::from_nested_meta`
    /// * `FromMeta::from_value`
    pub fn with_span<T: Spanned>(mut self, node: &T) -> Self {
        if !self.has_span() {
            self.span = Some(node.span());
        }

        self
    }

    /// Get a span for the error.
    ///
    /// # Return Value
    /// This function will return [`Span::call_site()`](proc_macro2::Span) if [`Self::has_span`] is `false`.
    /// To get the span only if one has been explicitly set for `self`, instead use [`Error::explicit_span`].
    pub fn span(&self) -> Span {
        self.span.unwrap_or_else(Span::call_site)
    }

    /// Get the span for `self`, if one has been set.
    pub fn explicit_span(&self) -> Option<Span> {
        self.span
    }

    /// Recursively converts a tree of errors to a flattened list.
    ///
    /// # Child Diagnostics
    /// If the `diagnostics` feature is enabled, any child diagnostics on `self`
    /// will be cloned down to all the errors within `self`.
    pub fn flatten(self) -> Self {
        Error::multiple(self.into_vec())
    }

    fn into_vec(self) -> Vec<Self> {
        if let ErrorKind::Multiple(errors) = self.kind {
            let locations = self.locations;

            #[cfg(feature = "diagnostics")]
            let children = self.children;

            errors
                .into_iter()
                .flat_map(|error| {
                    // This is mutated if the diagnostics feature is enabled
                    #[allow(unused_mut)]
                    let mut error = error.prepend_at(locations.clone());

                    // Any child diagnostics in `self` are cloned down to all the distinct
                    // errors contained in `self`.
                    #[cfg(feature = "diagnostics")]
                    error.children.extend(children.iter().cloned());

                    error.into_vec()
                })
                .collect()
        } else {
            vec![self]
        }
    }

    /// Adds a location to the error, such as a field or variant.
    /// Locations must be added in reverse order of specificity.
    pub fn at<T: fmt::Display>(mut self, location: T) -> Self {
        self.locations.insert(0, location.to_string());
        self
    }

    /// Adds a location to the error, such as a field or variant.
    /// Locations must be added in reverse order of specificity. This is a helper function to avoid
    /// repeating path to string logic.
    pub fn at_path(self, path: &Path) -> Self {
        self.at(path_to_string(path))
    }

    /// Gets the number of individual errors in this error.
    ///
    /// This function never returns `0`, as it's impossible to construct
    /// a multi-error from an empty `Vec`.
    pub fn len(&self) -> usize {
        self.kind.len()
    }

    /// Consider additional field names as "did you mean" suggestions for
    /// unknown field errors **if and only if** the caller appears to be operating
    /// at error's origin (meaning no calls to [`Self::at`] have yet taken place).
    ///
    /// # Usage
    /// `flatten` fields in derived trait implementations rely on this method to offer correct
    /// "did you mean" suggestions in errors.
    ///
    /// Because the `flatten` field receives _all_ unknown fields, if a user mistypes a field name
    /// that is present on the outer struct but not the flattened struct, they would get an incomplete
    /// or inferior suggestion unless this method was invoked.
    pub fn add_sibling_alts_for_unknown_field<'a, T, I>(mut self, alternates: I) -> Self
    where
        T: AsRef<str> + 'a,
        I: IntoIterator<Item = &'a T>,
    {
        // The error may have bubbled up before this method was called,
        // and in those cases adding alternates would be incorrect.
        if !self.locations.is_empty() {
            return self;
        }

        if let ErrorKind::UnknownField(unknown_field) = &mut self.kind {
            unknown_field.add_alts(alternates);
        } else if let ErrorKind::Multiple(errors) = self.kind {
            let alternates = alternates.into_iter().collect::<Vec<_>>();
            self.kind = ErrorKind::Multiple(
                errors
                    .into_iter()
                    .map(|err| {
                        err.add_sibling_alts_for_unknown_field(
                            // This clone seems like it shouldn't be necessary.
                            // Attempting to borrow alternates here leads to the following compiler error:
                            //
                            // error: reached the recursion limit while instantiating `darling::Error::add_sibling_alts_for_unknown_field::<'_, &&&&..., ...>`
                            alternates.clone(),
                        )
                    })
                    .collect(),
            )
        }

        self
    }

    /// Adds a location chain to the head of the error's existing locations.
    fn prepend_at(mut self, mut locations: Vec<String>) -> Self {
        if !locations.is_empty() {
            locations.extend(self.locations);
            self.locations = locations;
        }

        self
    }

    /// Gets the location slice.
    #[cfg(test)]
    pub(crate) fn location(&self) -> Vec<&str> {
        self.locations.iter().map(|i| i.as_str()).collect()
    }

    /// Write this error and any children as compile errors into a `TokenStream` to
    /// be returned by the proc-macro.
    ///
    /// The behavior of this method will be slightly different if the `diagnostics` feature
    /// is enabled: In that case, the diagnostics will be emitted immediately by this call,
    /// and an empty `TokenStream` will be returned.
    ///
    /// Return these tokens unmodified to avoid disturbing the attached span information.
    ///
    /// # Usage
    /// ```rust,ignore
    /// // in your proc-macro function
    /// let opts = match MyOptions::from_derive_input(&ast) {
    ///     Ok(val) => val,
    ///     Err(err) => {
    ///         return err.write_errors();
    ///     }
    /// }
    /// ```
    pub fn write_errors(self) -> TokenStream {
        #[cfg(feature = "diagnostics")]
        {
            self.emit();
            TokenStream::default()
        }

        #[cfg(not(feature = "diagnostics"))]
        {
            syn::Error::from(self).into_compile_error()
        }
    }

    #[cfg(feature = "diagnostics")]
    fn single_to_diagnostic(self) -> ::proc_macro::Diagnostic {
        use proc_macro::{Diagnostic, Level};

        // Delegate to dedicated error formatters when applicable.
        //
        // If span information is available, don't include the error property path
        // since it's redundant and not consistent with native compiler diagnostics.
        let diagnostic = match self.kind {
            ErrorKind::UnknownField(euf) => euf.into_diagnostic(self.span),
            _ => match self.span {
                Some(span) => span.unwrap().error(self.kind.to_string()),
                None => Diagnostic::new(Level::Error, self.to_string()),
            },
        };

        self.children
            .into_iter()
            .fold(diagnostic, |out, child| child.append_to(out))
    }

    /// Transform this error and its children into a list of compiler diagnostics
    /// and emit them. If the `Error` has associated span information, the diagnostics
    /// will identify the correct location in source code automatically.
    ///
    /// # Stability
    /// This is only available on `nightly` until the compiler `proc_macro_diagnostic`
    /// feature stabilizes. Until then, it may break at any time.
    #[cfg(feature = "diagnostics")]
    pub fn emit(self) {
        for error in self.flatten() {
            error.single_to_diagnostic().emit()
        }
    }

    /// Transform the error into a compiler diagnostic and - if the diagnostic points to
    /// a specific code location - add a spanned help child diagnostic that points to the
    /// parent derived trait.
    ///
    /// This is experimental and therefore not exposed outside the crate.
    #[cfg(feature = "diagnostics")]
    #[allow(dead_code)]
    fn emit_with_macro_help_span(self) {
        use proc_macro::Diagnostic;

        for error in self.flatten() {
            let needs_help = error.has_span();
            let diagnostic = error.single_to_diagnostic();
            Diagnostic::emit(if needs_help {
                diagnostic.span_help(
                    Span::call_site().unwrap(),
                    "Encountered as part of this derive-mode-macro",
                )
            } else {
                diagnostic
            })
        }
    }
}

#[cfg(feature = "diagnostics")]
macro_rules! add_child {
    ($unspanned:ident, $spanned:ident, $level:ident) => {
        #[doc = concat!("Add a child ", stringify!($unspanned), " message to this error.")]
        #[doc = "# Example"]
        #[doc = "```rust"]
        #[doc = "# use darling_core::Error;"]
        #[doc = concat!(r#"Error::custom("Example")."#, stringify!($unspanned), r#"("message content");"#)]
        #[doc = "```"]
        pub fn $unspanned<T: fmt::Display>(mut self, message: T) -> Self {
            self.children.push(child::ChildDiagnostic::new(
                child::Level::$level,
                None,
                message.to_string(),
            ));
            self
        }

        #[doc = concat!("Add a child ", stringify!($unspanned), " message to this error with its own span.")]
        #[doc = "# Example"]
        #[doc = "```rust"]
        #[doc = "# use darling_core::Error;"]
        #[doc = "# let item_to_span = proc_macro2::Span::call_site();"]
        #[doc = concat!(r#"Error::custom("Example")."#, stringify!($spanned), r#"(&item_to_span, "message content");"#)]
        #[doc = "```"]
        pub fn $spanned<S: Spanned, T: fmt::Display>(mut self, span: &S, message: T) -> Self {
            self.children.push(child::ChildDiagnostic::new(
                child::Level::$level,
                Some(span.span()),
                message.to_string(),
            ));
            self
        }
    };
}

/// Add child diagnostics to the error.
///
/// # Example
///
/// ## Code
///
/// ```rust
/// # use darling_core::Error;
/// # let struct_ident = proc_macro2::Span::call_site();
/// Error::custom("this is a demo")
///     .with_span(&struct_ident)
///     .note("we wrote this")
///     .help("try doing this instead");
/// ```
/// ## Output
///
/// ```text
/// error: this is a demo
///   --> my_project/my_file.rs:3:5
///    |
/// 13 |     FooBar { value: String },
///    |     ^^^^^^
///    |
///    = note: we wrote this
///    = help: try doing this instead
/// ```
#[cfg(feature = "diagnostics")]
impl Error {
    add_child!(error, span_error, Error);
    add_child!(warning, span_warning, Warning);
    add_child!(note, span_note, Note);
    add_child!(help, span_help, Help);
}

impl StdError for Error {
    fn description(&self) -> &str {
        self.kind.description()
    }

    fn cause(&self) -> Option<&dyn StdError> {
        None
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.kind)?;
        if !self.locations.is_empty() {
            write!(f, " at {}", self.locations.join("/"))?;
        }

        Ok(())
    }
}

impl From<syn::Error> for Error {
    fn from(e: syn::Error) -> Self {
        // This impl assumes there is nothing but the message and span that needs to be preserved
        // from the passed-in error. If this changes at some point, a new ErrorKind should be made
        // to hold the syn::Error, and this impl should preserve it unmodified while setting its own
        // span to be a copy of the passed-in error.
        Self {
            span: Some(e.span()),
            ..Self::custom(e)
        }
    }
}

impl From<Error> for syn::Error {
    fn from(e: Error) -> Self {
        if e.len() == 1 {
            if let Some(span) = e.explicit_span() {
                // Don't include the location path if the error has an explicit span,
                // since it will be redundant and isn't consistent with how rustc
                // exposes errors.
                syn::Error::new(span, e.kind)
            } else {
                // If the error's span is going to be the macro call site, include
                // the location information to try and help the user pinpoint the issue.
                syn::Error::new(e.span(), e)
            }
        } else {
            let mut syn_errors = e.flatten().into_iter().map(syn::Error::from);
            let mut error = syn_errors
                .next()
                .expect("darling::Error can never be empty");

            for next_error in syn_errors {
                error.combine(next_error);
            }

            error
        }
    }
}

// Don't want to publicly commit to Error supporting equality yet, but
// not having it makes testing very difficult. Note that spans are not
// considered for equality since that would break testing in most cases.
#[cfg(test)]
impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.locations == other.locations
    }
}

#[cfg(test)]
impl Eq for Error {}

impl IntoIterator for Error {
    type Item = Error;
    type IntoIter = IntoIter;

    fn into_iter(self) -> IntoIter {
        if let ErrorKind::Multiple(errors) = self.kind {
            IntoIter {
                inner: IntoIterEnum::Multiple(errors.into_iter()),
            }
        } else {
            IntoIter {
                inner: IntoIterEnum::Single(iter::once(self)),
            }
        }
    }
}

enum IntoIterEnum {
    Single(iter::Once<Error>),
    Multiple(vec::IntoIter<Error>),
}

impl Iterator for IntoIterEnum {
    type Item = Error;

    fn next(&mut self) -> Option<Self::Item> {
        match *self {
            IntoIterEnum::Single(ref mut content) => content.next(),
            IntoIterEnum::Multiple(ref mut content) => content.next(),
        }
    }
}

/// An iterator that moves out of an `Error`.
pub struct IntoIter {
    inner: IntoIterEnum,
}

impl Iterator for IntoIter {
    type Item = Error;

    fn next(&mut self) -> Option<Error> {
        self.inner.next()
    }
}

/// Accumulator for errors, for helping call [`Error::multiple`].
///
/// See the docs for [`darling::Error`](Error) for more discussion of error handling with darling.
///
/// # Panics
///
/// `Accumulator` panics on drop unless [`finish`](Self::finish), [`finish_with`](Self::finish_with),
/// or [`into_inner`](Self::into_inner) has been called, **even if it contains no errors**.
/// If you want to discard an `Accumulator` that you know to be empty, use `accumulator.finish().unwrap()`.
///
/// # Example
///
/// ```
/// # extern crate darling_core as darling;
/// # struct Thing;
/// # struct Output;
/// # impl Thing { fn validate(self) -> darling::Result<Output> { Ok(Output) } }
/// fn validate_things(inputs: Vec<Thing>) -> darling::Result<Vec<Output>> {
///     let mut errors = darling::Error::accumulator();
///
///     let outputs = inputs
///         .into_iter()
///         .filter_map(|thing| errors.handle_in(|| thing.validate()))
///         .collect::<Vec<_>>();
///
///     errors.finish()?;
///     Ok(outputs)
/// }
/// ```
#[derive(Debug)]
#[must_use = "Accumulator will panic on drop if not defused."]
pub struct Accumulator(Option<Vec<Error>>);

impl Accumulator {
    /// Runs a closure, returning the successful value as `Some`, or collecting the error
    ///
    /// The closure's return type is `darling::Result`, so inside it one can use `?`.
    pub fn handle_in<T, F: FnOnce() -> Result<T>>(&mut self, f: F) -> Option<T> {
        self.handle(f())
    }

    /// Handles a possible error.
    ///
    /// Returns a successful value as `Some`, or collects the error and returns `None`.
    pub fn handle<T>(&mut self, result: Result<T>) -> Option<T> {
        match result {
            Ok(y) => Some(y),
            Err(e) => {
                self.push(e);
                None
            }
        }
    }

    /// Stop accumulating errors, producing `Ok` if there are no errors or producing
    /// an error with all those encountered by the accumulator.
    pub fn finish(self) -> Result<()> {
        self.finish_with(())
    }

    /// Bundles the collected errors if there were any, or returns the success value
    ///
    /// Call this at the end of your input processing.
    ///
    /// If there were no errors recorded, returns `Ok(success)`.
    /// Otherwise calls [`Error::multiple`] and returns the result as an `Err`.
    pub fn finish_with<T>(self, success: T) -> Result<T> {
        let errors = self.into_inner();
        if errors.is_empty() {
            Ok(success)
        } else {
            Err(Error::multiple(errors))
        }
    }

    fn errors(&mut self) -> &mut Vec<Error> {
        match &mut self.0 {
            Some(errors) => errors,
            None => panic!("darling internal error: Accumulator accessed after defuse"),
        }
    }

    /// Returns the accumulated errors as a `Vec`.
    ///
    /// This function defuses the drop bomb.
    #[must_use = "Accumulated errors should be handled or propagated to the caller"]
    pub fn into_inner(mut self) -> Vec<Error> {
        match self.0.take() {
            Some(errors) => errors,
            None => panic!("darling internal error: Accumulator accessed after defuse"),
        }
    }

    /// Add one error to the collection.
    pub fn push(&mut self, error: Error) {
        self.errors().push(error)
    }

    /// Finish the current accumulation, and if there are no errors create a new `Self` so processing may continue.
    ///
    /// This is shorthand for:
    ///
    /// ```rust,ignore
    /// errors.finish()?;
    /// errors = Error::accumulator();
    /// ```
    ///
    /// # Drop Behavior
    /// This function returns a new [`Accumulator`] in the success case.
    /// This new accumulator is "armed" and will detonate if dropped without being finished.
    ///
    /// # Example
    ///
    /// ```
    /// # extern crate darling_core as darling;
    /// # struct Thing;
    /// # struct Output;
    /// # impl Thing { fn validate(&self) -> darling::Result<Output> { Ok(Output) } }
    /// fn validate(lorem_inputs: &[Thing], ipsum_inputs: &[Thing])
    ///             -> darling::Result<(Vec<Output>, Vec<Output>)> {
    ///     let mut errors = darling::Error::accumulator();
    ///
    ///     let lorems = lorem_inputs.iter().filter_map(|l| {
    ///         errors.handle(l.validate())
    ///     }).collect();
    ///
    ///     errors = errors.checkpoint()?;
    ///
    ///     let ipsums = ipsum_inputs.iter().filter_map(|l| {
    ///         errors.handle(l.validate())
    ///     }).collect();
    ///
    ///     errors.finish_with((lorems, ipsums))
    /// }
    /// # validate(&[], &[]).unwrap();
    /// ```
    pub fn checkpoint(self) -> Result<Accumulator> {
        // The doc comment says on success we "return the Accumulator for future use".
        // Actually, we have consumed it by feeding it to finish so we make a fresh one.
        // This is OK since by definition of the success path, it was empty on entry.
        self.finish()?;
        Ok(Self::default())
    }
}

impl Default for Accumulator {
    fn default() -> Self {
        Accumulator(Some(vec![]))
    }
}

impl Extend<Error> for Accumulator {
    fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = Error>,
    {
        self.errors().extend(iter)
    }
}

impl Drop for Accumulator {
    fn drop(&mut self) {
        // don't try to panic if we are currently unwinding a panic
        // otherwise we end up with an unhelful "thread panicked while panicking. aborting." message
        if !std::thread::panicking() {
            if let Some(errors) = &mut self.0 {
                match errors.len() {
                    0 => panic!("darling::error::Accumulator dropped without being finished"),
                    error_count => panic!("darling::error::Accumulator dropped without being finished. {} errors were lost.", error_count)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Error;

    #[test]
    fn flatten_noop() {
        let err = Error::duplicate_field("hello").at("world");
        assert_eq!(err.clone().flatten(), err);
    }

    #[test]
    fn flatten_simple() {
        let err = Error::multiple(vec![
            Error::unknown_field("hello").at("world"),
            Error::missing_field("hell_no").at("world"),
        ])
        .at("foo")
        .flatten();

        assert!(err.location().is_empty());

        let mut err_iter = err.into_iter();

        let first = err_iter.next();
        assert!(first.is_some());
        assert_eq!(first.unwrap().location(), vec!["foo", "world"]);

        let second = err_iter.next();
        assert!(second.is_some());

        assert_eq!(second.unwrap().location(), vec!["foo", "world"]);

        assert!(err_iter.next().is_none());
    }

    #[test]
    fn len_single() {
        let err = Error::duplicate_field("hello");
        assert_eq!(1, err.len());
    }

    #[test]
    fn len_multiple() {
        let err = Error::multiple(vec![
            Error::duplicate_field("hello"),
            Error::missing_field("hell_no"),
        ]);
        assert_eq!(2, err.len());
    }

    #[test]
    fn len_nested() {
        let err = Error::multiple(vec![
            Error::duplicate_field("hello"),
            Error::multiple(vec![
                Error::duplicate_field("hi"),
                Error::missing_field("bye"),
                Error::multiple(vec![Error::duplicate_field("whatsup")]),
            ]),
        ]);

        assert_eq!(4, err.len());
    }

    #[test]
    fn accum_ok() {
        let errs = Error::accumulator();
        assert_eq!("test", errs.finish_with("test").unwrap());
    }

    #[test]
    fn accum_errr() {
        let mut errs = Error::accumulator();
        errs.push(Error::custom("foo!"));
        errs.finish().unwrap_err();
    }

    #[test]
    fn accum_into_inner() {
        let mut errs = Error::accumulator();
        errs.push(Error::custom("foo!"));
        let errs: Vec<_> = errs.into_inner();
        assert_eq!(errs.len(), 1);
    }

    #[test]
    #[should_panic(expected = "Accumulator dropped")]
    fn accum_drop_panic() {
        let _errs = Error::accumulator();
    }

    #[test]
    #[should_panic(expected = "2 errors")]
    fn accum_drop_panic_with_error_count() {
        let mut errors = Error::accumulator();
        errors.push(Error::custom("first"));
        errors.push(Error::custom("second"));
    }

    #[test]
    fn accum_checkpoint_error() {
        let mut errs = Error::accumulator();
        errs.push(Error::custom("foo!"));
        errs.checkpoint().unwrap_err();
    }

    #[test]
    #[should_panic(expected = "Accumulator dropped")]
    fn accum_checkpoint_drop_panic() {
        let mut errs = Error::accumulator();
        errs = errs.checkpoint().unwrap();
        let _ = errs;
    }
}
