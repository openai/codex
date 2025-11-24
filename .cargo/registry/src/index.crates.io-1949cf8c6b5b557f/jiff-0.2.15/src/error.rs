use crate::{shared::util::error::Error as SharedError, util::sync::Arc};

/// Creates a new ad hoc error with no causal chain.
///
/// This accepts the same arguments as the `format!` macro. The error it
/// creates is just a wrapper around the string created by `format!`.
macro_rules! err {
    ($($tt:tt)*) => {{
        crate::error::Error::adhoc_from_args(format_args!($($tt)*))
    }}
}

pub(crate) use err;

/// An error that can occur in this crate.
///
/// The most common type of error is a result of overflow. But other errors
/// exist as well:
///
/// * Time zone database lookup failure.
/// * Configuration problem. (For example, trying to round a span with calendar
/// units without providing a relative datetime.)
/// * An I/O error as a result of trying to open a time zone database from a
/// directory via
/// [`TimeZoneDatabase::from_dir`](crate::tz::TimeZoneDatabase::from_dir).
/// * Parse errors.
///
/// # Introspection is limited
///
/// Other than implementing the [`std::error::Error`] trait when the
/// `std` feature is enabled, the [`core::fmt::Debug`] trait and the
/// [`core::fmt::Display`] trait, this error type currently provides no
/// introspection capabilities.
///
/// # Design
///
/// This crate follows the "One True God Error Type Pattern," where only one
/// error type exists for a variety of different operations. This design was
/// chosen after attempting to provide finer grained error types. But finer
/// grained error types proved difficult in the face of composition.
///
/// More about this design choice can be found in a GitHub issue
/// [about error types].
///
/// [about error types]: https://github.com/BurntSushi/jiff/issues/8
#[derive(Clone)]
pub struct Error {
    /// The internal representation of an error.
    ///
    /// This is in an `Arc` to make an `Error` cloneable. It could otherwise
    /// be automatically cloneable, but it embeds a `std::io::Error` when the
    /// `std` feature is enabled, which isn't cloneable.
    ///
    /// This also makes clones cheap. And it also make the size of error equal
    /// to one word (although a `Box` would achieve that last goal). This is
    /// why we put the `Arc` here instead of on `std::io::Error` directly.
    inner: Option<Arc<ErrorInner>>,
}

#[derive(Debug)]
#[cfg_attr(not(feature = "alloc"), derive(Clone))]
struct ErrorInner {
    kind: ErrorKind,
    #[cfg(feature = "alloc")]
    cause: Option<Error>,
}

/// The underlying kind of a [`Error`].
#[derive(Debug)]
#[cfg_attr(not(feature = "alloc"), derive(Clone))]
enum ErrorKind {
    /// An ad hoc error that is constructed from anything that implements
    /// the `core::fmt::Display` trait.
    ///
    /// In theory we try to avoid these, but they tend to be awfully
    /// convenient. In practice, we use them a lot, and only use a structured
    /// representation when a lot of different error cases fit neatly into a
    /// structure (like range errors).
    Adhoc(AdhocError),
    /// An error that occurs when a number is not within its allowed range.
    ///
    /// This can occur directly as a result of a number provided by the caller
    /// of a public API, or as a result of an operation on a number that
    /// results in it being out of range.
    Range(RangeError),
    /// An error that occurs within `jiff::shared`.
    ///
    /// It has its own error type to avoid bringing in this much bigger error
    /// type.
    Shared(SharedError),
    /// An error associated with a file path.
    ///
    /// This is generally expected to always have a cause attached to it
    /// explaining what went wrong. The error variant is just a path to make
    /// it composable with other error types.
    ///
    /// The cause is typically `Adhoc` or `IO`.
    ///
    /// When `std` is not enabled, this variant can never be constructed.
    #[allow(dead_code)] // not used in some feature configs
    FilePath(FilePathError),
    /// An error that occurs when interacting with the file system.
    ///
    /// This is effectively a wrapper around `std::io::Error` coupled with a
    /// `std::path::PathBuf`.
    ///
    /// When `std` is not enabled, this variant can never be constructed.
    #[allow(dead_code)] // not used in some feature configs
    IO(IOError),
}

impl Error {
    /// Creates a new error value from `core::fmt::Arguments`.
    ///
    /// It is expected to use [`format_args!`](format_args) from
    /// Rust's standard library (available in `core`) to create a
    /// `core::fmt::Arguments`.
    ///
    /// Callers should generally use their own error types. But in some
    /// circumstances, it can be convenient to manufacture a Jiff error value
    /// specifically.
    ///
    /// # Example
    ///
    /// ```
    /// use jiff::Error;
    ///
    /// let err = Error::from_args(format_args!("something failed"));
    /// assert_eq!(err.to_string(), "something failed");
    /// ```
    pub fn from_args<'a>(message: core::fmt::Arguments<'a>) -> Error {
        Error::from(ErrorKind::Adhoc(AdhocError::from_args(message)))
    }

    #[inline(never)]
    #[cold]
    fn context_impl(self, consequent: Error) -> Error {
        #[cfg(feature = "alloc")]
        {
            let mut err = consequent;
            if err.inner.is_none() {
                err = err!("unknown jiff error");
            }
            let inner = err.inner.as_mut().unwrap();
            assert!(
                inner.cause.is_none(),
                "cause of consequence must be `None`"
            );
            // OK because we just created this error so the Arc
            // has one reference.
            Arc::get_mut(inner).unwrap().cause = Some(self);
            err
        }
        #[cfg(not(feature = "alloc"))]
        {
            // We just completely drop `self`. :-(
            consequent
        }
    }
}

impl Error {
    /// Creates a new "ad hoc" error value.
    ///
    /// An ad hoc error value is just an opaque string.
    #[cfg(feature = "alloc")]
    #[inline(never)]
    #[cold]
    pub(crate) fn adhoc<'a>(message: impl core::fmt::Display + 'a) -> Error {
        Error::from(ErrorKind::Adhoc(AdhocError::from_display(message)))
    }

    /// Like `Error::adhoc`, but accepts a `core::fmt::Arguments`.
    ///
    /// This is used with the `err!` macro so that we can thread a
    /// `core::fmt::Arguments` down. This lets us extract a `&'static str`
    /// from some messages in core-only mode and provide somewhat decent error
    /// messages in some cases.
    #[inline(never)]
    #[cold]
    pub(crate) fn adhoc_from_args<'a>(
        message: core::fmt::Arguments<'a>,
    ) -> Error {
        Error::from(ErrorKind::Adhoc(AdhocError::from_args(message)))
    }

    /// Like `Error::adhoc`, but creates an error from a `String` directly.
    ///
    /// This exists to explicitly monomorphize a very common case.
    #[cfg(feature = "alloc")]
    #[inline(never)]
    #[cold]
    fn adhoc_from_string(message: alloc::string::String) -> Error {
        Error::adhoc(message)
    }

    /// Like `Error::adhoc`, but creates an error from a `&'static str`
    /// directly.
    ///
    /// This is useful in contexts where you know you have a `&'static str`,
    /// and avoids relying on `alloc`-only routines like `Error::adhoc`.
    #[inline(never)]
    #[cold]
    pub(crate) fn adhoc_from_static_str(message: &'static str) -> Error {
        Error::from(ErrorKind::Adhoc(AdhocError::from_static_str(message)))
    }

    /// Creates a new error indicating that a `given` value is out of the
    /// specified `min..=max` range. The given `what` label is used in the
    /// error message as a human readable description of what exactly is out
    /// of range. (e.g., "seconds")
    #[inline(never)]
    #[cold]
    pub(crate) fn range(
        what: &'static str,
        given: impl Into<i128>,
        min: impl Into<i128>,
        max: impl Into<i128>,
    ) -> Error {
        Error::from(ErrorKind::Range(RangeError::new(what, given, min, max)))
    }

    /// Creates a new error from the special "shared" error type.
    pub(crate) fn shared(err: SharedError) -> Error {
        Error::from(ErrorKind::Shared(err))
    }

    /// A convenience constructor for building an I/O error.
    ///
    /// This returns an error that is just a simple wrapper around the
    /// `std::io::Error` type. In general, callers should alwasys attach some
    /// kind of context to this error (like a file path).
    ///
    /// This is only available when the `std` feature is enabled.
    #[cfg(feature = "std")]
    #[inline(never)]
    #[cold]
    pub(crate) fn io(err: std::io::Error) -> Error {
        Error::from(ErrorKind::IO(IOError { err }))
    }

    /// Contextualizes this error by associating the given file path with it.
    ///
    /// This is a convenience routine for calling `Error::context` with a
    /// `FilePathError`.
    #[cfg(any(feature = "tzdb-zoneinfo", feature = "tzdb-concatenated"))]
    #[inline(never)]
    #[cold]
    pub(crate) fn path(self, path: impl Into<std::path::PathBuf>) -> Error {
        let err = Error::from(ErrorKind::FilePath(FilePathError {
            path: path.into(),
        }));
        self.context(err)
    }

    /*
    /// Creates a new "unknown" Jiff error.
    ///
    /// The benefit of this API is that it permits creating an `Error` in a
    /// `const` context. But the error message quality is currently pretty
    /// bad: it's just a generic "unknown jiff error" message.
    ///
    /// This could be improved to take a `&'static str`, but I believe this
    /// will require pointer tagging in order to avoid increasing the size of
    /// `Error`. (Which is important, because of how many perf sensitive
    /// APIs return a `Result<T, Error>` in Jiff.
    pub(crate) const fn unknown() -> Error {
        Error { inner: None }
    }
    */
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        #[cfg(feature = "alloc")]
        {
            let mut err = self;
            loop {
                let Some(ref inner) = err.inner else {
                    write!(f, "unknown jiff error")?;
                    break;
                };
                write!(f, "{}", inner.kind)?;
                err = match inner.cause.as_ref() {
                    None => break,
                    Some(err) => err,
                };
                write!(f, ": ")?;
            }
            Ok(())
        }
        #[cfg(not(feature = "alloc"))]
        {
            match self.inner {
                None => write!(f, "unknown jiff error"),
                Some(ref inner) => write!(f, "{}", inner.kind),
            }
        }
    }
}

impl core::fmt::Debug for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        if !f.alternate() {
            core::fmt::Display::fmt(self, f)
        } else {
            let Some(ref inner) = self.inner else {
                return f
                    .debug_struct("Error")
                    .field("kind", &"None")
                    .finish();
            };
            #[cfg(feature = "alloc")]
            {
                f.debug_struct("Error")
                    .field("kind", &inner.kind)
                    .field("cause", &inner.cause)
                    .finish()
            }
            #[cfg(not(feature = "alloc"))]
            {
                f.debug_struct("Error").field("kind", &inner.kind).finish()
            }
        }
    }
}

impl core::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match *self {
            ErrorKind::Adhoc(ref msg) => msg.fmt(f),
            ErrorKind::Range(ref err) => err.fmt(f),
            ErrorKind::Shared(ref err) => err.fmt(f),
            ErrorKind::FilePath(ref err) => err.fmt(f),
            ErrorKind::IO(ref err) => err.fmt(f),
        }
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Error {
        #[cfg(feature = "alloc")]
        {
            Error { inner: Some(Arc::new(ErrorInner { kind, cause: None })) }
        }
        #[cfg(not(feature = "alloc"))]
        {
            Error { inner: Some(Arc::new(ErrorInner { kind })) }
        }
    }
}

/// A generic error message.
///
/// This somewhat unfortunately represents most of the errors in Jiff. When I
/// first started building Jiff, I had a goal of making every error structured.
/// But this ended up being a ton of work, and I find it much easier and nicer
/// for error messages to be embedded where they occur.
#[cfg_attr(not(feature = "alloc"), derive(Clone))]
struct AdhocError {
    #[cfg(feature = "alloc")]
    message: alloc::boxed::Box<str>,
    #[cfg(not(feature = "alloc"))]
    message: &'static str,
}

impl AdhocError {
    #[cfg(feature = "alloc")]
    fn from_display<'a>(message: impl core::fmt::Display + 'a) -> AdhocError {
        use alloc::string::ToString;

        let message = message.to_string().into_boxed_str();
        AdhocError { message }
    }

    fn from_args<'a>(message: core::fmt::Arguments<'a>) -> AdhocError {
        #[cfg(feature = "alloc")]
        {
            AdhocError::from_display(message)
        }
        #[cfg(not(feature = "alloc"))]
        {
            let message = message.as_str().unwrap_or(
                "unknown Jiff error (better error messages require \
                 enabling the `alloc` feature for the `jiff` crate)",
            );
            AdhocError::from_static_str(message)
        }
    }

    fn from_static_str(message: &'static str) -> AdhocError {
        #[cfg(feature = "alloc")]
        {
            AdhocError::from_display(message)
        }
        #[cfg(not(feature = "alloc"))]
        {
            AdhocError { message }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for AdhocError {}

impl core::fmt::Display for AdhocError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        core::fmt::Display::fmt(&self.message, f)
    }
}

impl core::fmt::Debug for AdhocError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        core::fmt::Debug::fmt(&self.message, f)
    }
}

/// An error that occurs when an input value is out of bounds.
///
/// The error message produced by this type will include a name describing
/// which input was out of bounds, the value given and its minimum and maximum
/// allowed values.
#[derive(Debug)]
#[cfg_attr(not(feature = "alloc"), derive(Clone))]
struct RangeError {
    what: &'static str,
    #[cfg(feature = "alloc")]
    given: i128,
    #[cfg(feature = "alloc")]
    min: i128,
    #[cfg(feature = "alloc")]
    max: i128,
}

impl RangeError {
    fn new(
        what: &'static str,
        _given: impl Into<i128>,
        _min: impl Into<i128>,
        _max: impl Into<i128>,
    ) -> RangeError {
        RangeError {
            what,
            #[cfg(feature = "alloc")]
            given: _given.into(),
            #[cfg(feature = "alloc")]
            min: _min.into(),
            #[cfg(feature = "alloc")]
            max: _max.into(),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RangeError {}

impl core::fmt::Display for RangeError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        #[cfg(feature = "alloc")]
        {
            let RangeError { what, given, min, max } = *self;
            write!(
                f,
                "parameter '{what}' with value {given} \
                 is not in the required range of {min}..={max}",
            )
        }
        #[cfg(not(feature = "alloc"))]
        {
            let RangeError { what } = *self;
            write!(f, "parameter '{what}' is not in the required range")
        }
    }
}

/// A `std::io::Error`.
///
/// This type is itself always available, even when the `std` feature is not
/// enabled. When `std` is not enabled, a value of this type can never be
/// constructed.
///
/// Otherwise, this type is a simple wrapper around `std::io::Error`. Its
/// purpose is to encapsulate the conditional compilation based on the `std`
/// feature.
#[cfg_attr(not(feature = "alloc"), derive(Clone))]
struct IOError {
    #[cfg(feature = "std")]
    err: std::io::Error,
}

#[cfg(feature = "std")]
impl std::error::Error for IOError {}

impl core::fmt::Display for IOError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        #[cfg(feature = "std")]
        {
            write!(f, "{}", self.err)
        }
        #[cfg(not(feature = "std"))]
        {
            write!(f, "<BUG: SHOULD NOT EXIST>")
        }
    }
}

impl core::fmt::Debug for IOError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        #[cfg(feature = "std")]
        {
            f.debug_struct("IOError").field("err", &self.err).finish()
        }
        #[cfg(not(feature = "std"))]
        {
            write!(f, "<BUG: SHOULD NOT EXIST>")
        }
    }
}

#[cfg(feature = "std")]
impl From<std::io::Error> for IOError {
    fn from(err: std::io::Error) -> IOError {
        IOError { err }
    }
}

#[cfg_attr(not(feature = "alloc"), derive(Clone))]
struct FilePathError {
    #[cfg(feature = "std")]
    path: std::path::PathBuf,
}

#[cfg(feature = "std")]
impl std::error::Error for FilePathError {}

impl core::fmt::Display for FilePathError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        #[cfg(feature = "std")]
        {
            write!(f, "{}", self.path.display())
        }
        #[cfg(not(feature = "std"))]
        {
            write!(f, "<BUG: SHOULD NOT EXIST>")
        }
    }
}

impl core::fmt::Debug for FilePathError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        #[cfg(feature = "std")]
        {
            f.debug_struct("FilePathError").field("path", &self.path).finish()
        }
        #[cfg(not(feature = "std"))]
        {
            write!(f, "<BUG: SHOULD NOT EXIST>")
        }
    }
}

/// A simple trait to encapsulate automatic conversion to `Error`.
///
/// This trait basically exists to make `Error::context` work without needing
/// to rely on public `From` impls. For example, without this trait, we might
/// otherwise write `impl From<String> for Error`. But this would make it part
/// of the public API. Which... maybe we should do, but at time of writing,
/// I'm starting very conservative so that we can evolve errors in semver
/// compatible ways.
pub(crate) trait IntoError {
    fn into_error(self) -> Error;
}

impl IntoError for Error {
    #[inline(always)]
    fn into_error(self) -> Error {
        self
    }
}

impl IntoError for &'static str {
    #[inline(always)]
    fn into_error(self) -> Error {
        Error::adhoc_from_static_str(self)
    }
}

#[cfg(feature = "alloc")]
impl IntoError for alloc::string::String {
    #[inline(always)]
    fn into_error(self) -> Error {
        Error::adhoc_from_string(self)
    }
}

/// A trait for contextualizing error values.
///
/// This makes it easy to contextualize either `Error` or `Result<T, Error>`.
/// Specifically, in the latter case, it absolves one of the need to call
/// `map_err` everywhere one wants to add context to an error.
///
/// This trick was borrowed from `anyhow`.
pub(crate) trait ErrorContext {
    /// Contextualize the given consequent error with this (`self`) error as
    /// the cause.
    ///
    /// This is equivalent to saying that "consequent is caused by self."
    ///
    /// Note that if an `Error` is given for `kind`, then this panics if it has
    /// a cause. (Because the cause would otherwise be dropped. An error causal
    /// chain is just a linked list, not a tree.)
    fn context(self, consequent: impl IntoError) -> Self;

    /// Like `context`, but hides error construction within a closure.
    ///
    /// This is useful if the creation of the consequent error is not otherwise
    /// guarded and when error construction is potentially "costly" (i.e., it
    /// allocates). The closure avoids paying the cost of contextual error
    /// creation in the happy path.
    ///
    /// Usually this only makes sense to use on a `Result<T, Error>`, otherwise
    /// the closure is just executed immediately anyway.
    fn with_context<E: IntoError>(
        self,
        consequent: impl FnOnce() -> E,
    ) -> Self;
}

impl ErrorContext for Error {
    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn context(self, consequent: impl IntoError) -> Error {
        self.context_impl(consequent.into_error())
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn with_context<E: IntoError>(
        self,
        consequent: impl FnOnce() -> E,
    ) -> Error {
        self.context_impl(consequent().into_error())
    }
}

impl<T> ErrorContext for Result<T, Error> {
    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn context(self, consequent: impl IntoError) -> Result<T, Error> {
        self.map_err(|err| err.context_impl(consequent.into_error()))
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn with_context<E: IntoError>(
        self,
        consequent: impl FnOnce() -> E,
    ) -> Result<T, Error> {
        self.map_err(|err| err.context_impl(consequent().into_error()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // We test that our 'Error' type is the size we expect. This isn't an API
    // guarantee, but if the size increases, we really want to make sure we
    // decide to do that intentionally. So this should be a speed bump. And in
    // general, we should not increase the size without a very good reason.
    #[test]
    fn error_size() {
        let mut expected_size = core::mem::size_of::<usize>();
        if !cfg!(feature = "alloc") {
            // oooowwwwwwwwwwwch.
            //
            // Like, this is horrible, right? core-only environments are
            // precisely the place where one want to keep things slim. But
            // in core-only, I don't know of a way to introduce any sort of
            // indirection in the library level without using a completely
            // different API.
            //
            // This is what makes me doubt that core-only Jiff is actually
            // useful. In what context are people using a huge library like
            // Jiff but can't define a small little heap allocator?
            //
            // OK, this used to be `expected_size *= 10`, but I slimmed it down
            // to x3. Still kinda sucks right? If we tried harder, I think we
            // could probably slim this down more. And if we were willing to
            // sacrifice error message quality even more (like, all the way),
            // then we could make `Error` a zero sized type. Which might
            // actually be the right trade-off for core-only, but I'll hold off
            // until we have some real world use cases.
            expected_size *= 3;
        }
        assert_eq!(expected_size, core::mem::size_of::<Error>());
    }
}
