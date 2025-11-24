use crate::util::{
    captures, look,
    primitives::{PatternID, StateID},
};

/// An error that can occurred during the construction of a thompson NFA.
///
/// This error does not provide many introspection capabilities. There are
/// generally only two things you can do with it:
///
/// * Obtain a human readable message via its `std::fmt::Display` impl.
/// * Access an underlying [`regex_syntax::Error`] type from its `source`
/// method via the `std::error::Error` trait. This error only occurs when using
/// convenience routines for building an NFA directly from a pattern string.
///
/// Otherwise, errors typically occur when a limit has been breeched. For
/// example, if the total heap usage of the compiled NFA exceeds the limit
/// set by [`Config::nfa_size_limit`](crate::nfa::thompson::Config), then
/// building the NFA will fail.
#[derive(Clone, Debug)]
pub struct BuildError {
    kind: BuildErrorKind,
}

/// The kind of error that occurred during the construction of a thompson NFA.
#[derive(Clone, Debug)]
enum BuildErrorKind {
    /// An error that occurred while parsing a regular expression. Note that
    /// this error may be printed over multiple lines, and is generally
    /// intended to be end user readable on its own.
    #[cfg(feature = "syntax")]
    Syntax(regex_syntax::Error),
    /// An error that occurs if the capturing groups provided to an NFA builder
    /// do not satisfy the documented invariants. For example, things like
    /// too many groups, missing groups, having the first (zeroth) group be
    /// named or duplicate group names within the same pattern.
    Captures(captures::GroupInfoError),
    /// An error that occurs when an NFA contains a Unicode word boundary, but
    /// where the crate was compiled without the necessary data for dealing
    /// with Unicode word boundaries.
    Word(look::UnicodeWordBoundaryError),
    /// An error that occurs if too many patterns were given to the NFA
    /// compiler.
    TooManyPatterns {
        /// The number of patterns given, which exceeds the limit.
        given: usize,
        /// The limit on the number of patterns.
        limit: usize,
    },
    /// An error that occurs if too states are produced while building an NFA.
    TooManyStates {
        /// The minimum number of states that are desired, which exceeds the
        /// limit.
        given: usize,
        /// The limit on the number of states.
        limit: usize,
    },
    /// An error that occurs when NFA compilation exceeds a configured heap
    /// limit.
    ExceededSizeLimit {
        /// The configured limit, in bytes.
        limit: usize,
    },
    /// An error that occurs when an invalid capture group index is added to
    /// the NFA. An "invalid" index can be one that would otherwise overflow
    /// a `usize` on the current target.
    InvalidCaptureIndex {
        /// The invalid index that was given.
        index: u32,
    },
    /// An error that occurs when one tries to build a reverse NFA with
    /// captures enabled. Currently, this isn't supported, but we probably
    /// should support it at some point.
    #[cfg(feature = "syntax")]
    UnsupportedCaptures,
}

impl BuildError {
    /// If this error occurred because the NFA exceeded the configured size
    /// limit before being built, then this returns the configured size limit.
    ///
    /// The limit returned is what was configured, and corresponds to the
    /// maximum amount of heap usage in bytes.
    pub fn size_limit(&self) -> Option<usize> {
        match self.kind {
            BuildErrorKind::ExceededSizeLimit { limit } => Some(limit),
            _ => None,
        }
    }

    fn kind(&self) -> &BuildErrorKind {
        &self.kind
    }

    #[cfg(feature = "syntax")]
    pub(crate) fn syntax(err: regex_syntax::Error) -> BuildError {
        BuildError { kind: BuildErrorKind::Syntax(err) }
    }

    pub(crate) fn captures(err: captures::GroupInfoError) -> BuildError {
        BuildError { kind: BuildErrorKind::Captures(err) }
    }

    pub(crate) fn word(err: look::UnicodeWordBoundaryError) -> BuildError {
        BuildError { kind: BuildErrorKind::Word(err) }
    }

    pub(crate) fn too_many_patterns(given: usize) -> BuildError {
        let limit = PatternID::LIMIT;
        BuildError { kind: BuildErrorKind::TooManyPatterns { given, limit } }
    }

    pub(crate) fn too_many_states(given: usize) -> BuildError {
        let limit = StateID::LIMIT;
        BuildError { kind: BuildErrorKind::TooManyStates { given, limit } }
    }

    pub(crate) fn exceeded_size_limit(limit: usize) -> BuildError {
        BuildError { kind: BuildErrorKind::ExceededSizeLimit { limit } }
    }

    pub(crate) fn invalid_capture_index(index: u32) -> BuildError {
        BuildError { kind: BuildErrorKind::InvalidCaptureIndex { index } }
    }

    #[cfg(feature = "syntax")]
    pub(crate) fn unsupported_captures() -> BuildError {
        BuildError { kind: BuildErrorKind::UnsupportedCaptures }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for BuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self.kind() {
            #[cfg(feature = "syntax")]
            BuildErrorKind::Syntax(ref err) => Some(err),
            BuildErrorKind::Captures(ref err) => Some(err),
            _ => None,
        }
    }
}

impl core::fmt::Display for BuildError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.kind() {
            #[cfg(feature = "syntax")]
            BuildErrorKind::Syntax(_) => write!(f, "error parsing regex"),
            BuildErrorKind::Captures(_) => {
                write!(f, "error with capture groups")
            }
            BuildErrorKind::Word(_) => {
                write!(f, "NFA contains Unicode word boundary")
            }
            BuildErrorKind::TooManyPatterns { given, limit } => write!(
                f,
                "attempted to compile {} patterns, \
                 which exceeds the limit of {}",
                given, limit,
            ),
            BuildErrorKind::TooManyStates { given, limit } => write!(
                f,
                "attempted to compile {} NFA states, \
                 which exceeds the limit of {}",
                given, limit,
            ),
            BuildErrorKind::ExceededSizeLimit { limit } => write!(
                f,
                "heap usage during NFA compilation exceeded limit of {}",
                limit,
            ),
            BuildErrorKind::InvalidCaptureIndex { index } => write!(
                f,
                "capture group index {} is invalid (too big or discontinuous)",
                index,
            ),
            #[cfg(feature = "syntax")]
            BuildErrorKind::UnsupportedCaptures => write!(
                f,
                "currently captures must be disabled when compiling \
                 a reverse NFA",
            ),
        }
    }
}
