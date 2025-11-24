use regex_syntax::{ast, hir};

use crate::{nfa, util::search::MatchError, PatternID};

/// An error that occurs when construction of a `Regex` fails.
///
/// A build error is generally a result of one of two possible failure
/// modes. First is a parse or syntax error in the concrete syntax of a
/// pattern. Second is that the construction of the underlying regex matcher
/// fails, usually because it gets too big with respect to limits like
/// [`Config::nfa_size_limit`](crate::meta::Config::nfa_size_limit).
///
/// This error provides very little introspection capabilities. You can:
///
/// * Ask for the [`PatternID`] of the pattern that caused an error, if one
/// is available. This is available for things like syntax errors, but not for
/// cases where build limits are exceeded.
/// * Ask for the underlying syntax error, but only if the error is a syntax
/// error.
/// * Ask for a human readable message corresponding to the underlying error.
/// * The `BuildError::source` method (from the `std::error::Error`
/// trait implementation) may be used to query for an underlying error if one
/// exists. There are no API guarantees about which error is returned.
///
/// When the `std` feature is enabled, this implements `std::error::Error`.
#[derive(Clone, Debug)]
pub struct BuildError {
    kind: BuildErrorKind,
}

#[derive(Clone, Debug)]
enum BuildErrorKind {
    Syntax { pid: PatternID, err: regex_syntax::Error },
    NFA(nfa::thompson::BuildError),
}

impl BuildError {
    /// If it is known which pattern ID caused this build error to occur, then
    /// this method returns it.
    ///
    /// Some errors are not associated with a particular pattern. However, any
    /// errors that occur as part of parsing a pattern are guaranteed to be
    /// associated with a pattern ID.
    ///
    /// # Example
    ///
    /// ```
    /// use regex_automata::{meta::Regex, PatternID};
    ///
    /// let err = Regex::new_many(&["a", "b", r"\p{Foo}", "c"]).unwrap_err();
    /// assert_eq!(Some(PatternID::must(2)), err.pattern());
    /// ```
    pub fn pattern(&self) -> Option<PatternID> {
        match self.kind {
            BuildErrorKind::Syntax { pid, .. } => Some(pid),
            _ => None,
        }
    }

    /// If this error occurred because the regex exceeded the configured size
    /// limit before being built, then this returns the configured size limit.
    ///
    /// The limit returned is what was configured, and corresponds to the
    /// maximum amount of heap usage in bytes.
    pub fn size_limit(&self) -> Option<usize> {
        match self.kind {
            BuildErrorKind::NFA(ref err) => err.size_limit(),
            _ => None,
        }
    }

    /// If this error corresponds to a syntax error, then a reference to it is
    /// returned by this method.
    pub fn syntax_error(&self) -> Option<&regex_syntax::Error> {
        match self.kind {
            BuildErrorKind::Syntax { ref err, .. } => Some(err),
            _ => None,
        }
    }

    pub(crate) fn ast(pid: PatternID, err: ast::Error) -> BuildError {
        let err = regex_syntax::Error::from(err);
        BuildError { kind: BuildErrorKind::Syntax { pid, err } }
    }

    pub(crate) fn hir(pid: PatternID, err: hir::Error) -> BuildError {
        let err = regex_syntax::Error::from(err);
        BuildError { kind: BuildErrorKind::Syntax { pid, err } }
    }

    pub(crate) fn nfa(err: nfa::thompson::BuildError) -> BuildError {
        BuildError { kind: BuildErrorKind::NFA(err) }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for BuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self.kind {
            BuildErrorKind::Syntax { ref err, .. } => Some(err),
            BuildErrorKind::NFA(ref err) => Some(err),
        }
    }
}

impl core::fmt::Display for BuildError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.kind {
            BuildErrorKind::Syntax { pid, .. } => {
                write!(f, "error parsing pattern {}", pid.as_usize())
            }
            BuildErrorKind::NFA(_) => write!(f, "error building NFA"),
        }
    }
}

/// An error that occurs when a search should be retried.
///
/// This retry error distinguishes between two different failure modes.
///
/// The first is one where potential quadratic behavior has been detected.
/// In this case, whatever optimization that led to this behavior should be
/// stopped, and the next best strategy should be used.
///
/// The second indicates that the underlying regex engine has failed for some
/// reason. This usually occurs because either a lazy DFA's cache has become
/// ineffective or because a non-ASCII byte has been seen *and* a Unicode word
/// boundary was used in one of the patterns. In this failure case, a different
/// regex engine that won't fail in these ways (PikeVM, backtracker or the
/// one-pass DFA) should be used.
///
/// This is an internal error only and should never bleed into the public
/// API.
#[derive(Debug)]
pub(crate) enum RetryError {
    Quadratic(RetryQuadraticError),
    Fail(RetryFailError),
}

#[cfg(feature = "std")]
impl std::error::Error for RetryError {}

impl core::fmt::Display for RetryError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {
            RetryError::Quadratic(ref err) => err.fmt(f),
            RetryError::Fail(ref err) => err.fmt(f),
        }
    }
}

impl From<MatchError> for RetryError {
    fn from(merr: MatchError) -> RetryError {
        RetryError::Fail(RetryFailError::from(merr))
    }
}

/// An error that occurs when potential quadratic behavior has been detected
/// when applying either the "reverse suffix" or "reverse inner" optimizations.
///
/// When this error occurs, callers should abandon the "reverse" optimization
/// and use a normal forward search.
#[derive(Debug)]
pub(crate) struct RetryQuadraticError(());

impl RetryQuadraticError {
    pub(crate) fn new() -> RetryQuadraticError {
        RetryQuadraticError(())
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RetryQuadraticError {}

impl core::fmt::Display for RetryQuadraticError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "regex engine gave up to avoid quadratic behavior")
    }
}

impl From<RetryQuadraticError> for RetryError {
    fn from(err: RetryQuadraticError) -> RetryError {
        RetryError::Quadratic(err)
    }
}

/// An error that occurs when a regex engine "gives up" for some reason before
/// finishing a search. Usually this occurs because of heuristic Unicode word
/// boundary support or because of ineffective cache usage in the lazy DFA.
///
/// When this error occurs, callers should retry the regex search with a
/// different regex engine.
///
/// Note that this has convenient `From` impls that will automatically
/// convert a `MatchError` into this error. This works because the meta
/// regex engine internals guarantee that errors like `HaystackTooLong` and
/// `UnsupportedAnchored` will never occur. The only errors left are `Quit` and
/// `GaveUp`, which both correspond to this "failure" error.
#[derive(Debug)]
pub(crate) struct RetryFailError {
    offset: usize,
}

impl RetryFailError {
    pub(crate) fn from_offset(offset: usize) -> RetryFailError {
        RetryFailError { offset }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RetryFailError {}

impl core::fmt::Display for RetryFailError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "regex engine failed at offset {:?}", self.offset)
    }
}

impl From<RetryFailError> for RetryError {
    fn from(err: RetryFailError) -> RetryError {
        RetryError::Fail(err)
    }
}

impl From<MatchError> for RetryFailError {
    fn from(merr: MatchError) -> RetryFailError {
        use crate::util::search::MatchErrorKind::*;

        match *merr.kind() {
            Quit { offset, .. } => RetryFailError::from_offset(offset),
            GaveUp { offset } => RetryFailError::from_offset(offset),
            // These can never occur because we avoid them by construction
            // or with higher level control flow logic. For example, the
            // backtracker's wrapper will never hand out a backtracker engine
            // when the haystack would be too long.
            HaystackTooLong { .. } | UnsupportedAnchored { .. } => {
                unreachable!("found impossible error in meta engine: {}", merr)
            }
        }
    }
}
