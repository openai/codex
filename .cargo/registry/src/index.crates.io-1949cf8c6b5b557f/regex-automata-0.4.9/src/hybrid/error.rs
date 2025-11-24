use crate::{hybrid::id::LazyStateIDError, nfa, util::search::Anchored};

/// An error that occurs when initial construction of a lazy DFA fails.
///
/// A build error can occur when insufficient cache capacity is configured or
/// if something about the NFA is unsupported. (For example, if one attempts
/// to build a lazy DFA without heuristic Unicode support but with an NFA that
/// contains a Unicode word boundary.)
///
/// This error does not provide many introspection capabilities. There are
/// generally only two things you can do with it:
///
/// * Obtain a human readable message via its `std::fmt::Display` impl.
/// * Access an underlying
/// [`nfa::thompson::BuildError`](crate::nfa::thompson::BuildError)
/// type from its `source` method via the `std::error::Error` trait. This error
/// only occurs when using convenience routines for building a lazy DFA
/// directly from a pattern string.
///
/// When the `std` feature is enabled, this implements the `std::error::Error`
/// trait.
#[derive(Clone, Debug)]
pub struct BuildError {
    kind: BuildErrorKind,
}

#[derive(Clone, Debug)]
enum BuildErrorKind {
    NFA(nfa::thompson::BuildError),
    InsufficientCacheCapacity { minimum: usize, given: usize },
    InsufficientStateIDCapacity { err: LazyStateIDError },
    Unsupported(&'static str),
}

impl BuildError {
    pub(crate) fn nfa(err: nfa::thompson::BuildError) -> BuildError {
        BuildError { kind: BuildErrorKind::NFA(err) }
    }

    pub(crate) fn insufficient_cache_capacity(
        minimum: usize,
        given: usize,
    ) -> BuildError {
        BuildError {
            kind: BuildErrorKind::InsufficientCacheCapacity { minimum, given },
        }
    }

    pub(crate) fn insufficient_state_id_capacity(
        err: LazyStateIDError,
    ) -> BuildError {
        BuildError {
            kind: BuildErrorKind::InsufficientStateIDCapacity { err },
        }
    }

    pub(crate) fn unsupported_dfa_word_boundary_unicode() -> BuildError {
        let msg = "cannot build lazy DFAs for regexes with Unicode word \
                   boundaries; switch to ASCII word boundaries, or \
                   heuristically enable Unicode word boundaries or use a \
                   different regex engine";
        BuildError { kind: BuildErrorKind::Unsupported(msg) }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for BuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self.kind {
            BuildErrorKind::NFA(ref err) => Some(err),
            _ => None,
        }
    }
}

impl core::fmt::Display for BuildError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.kind {
            BuildErrorKind::NFA(_) => write!(f, "error building NFA"),
            BuildErrorKind::InsufficientCacheCapacity { minimum, given } => {
                write!(
                    f,
                    "given cache capacity ({}) is smaller than \
                     minimum required ({})",
                    given, minimum,
                )
            }
            BuildErrorKind::InsufficientStateIDCapacity { ref err } => {
                err.fmt(f)
            }
            BuildErrorKind::Unsupported(ref msg) => {
                write!(f, "unsupported regex feature for DFAs: {}", msg)
            }
        }
    }
}

/// An error that can occur when computing the start state for a search.
///
/// Computing a start state can fail for a few reasons, either
/// based on incorrect configuration or even based on whether
/// the look-behind byte triggers a quit state. Typically
/// one does not need to handle this error if you're using
/// [`DFA::start_state_forward`](crate::hybrid::dfa::DFA::start_state_forward)
/// (or its reverse counterpart), as that routine automatically converts
/// `StartError` to a [`MatchError`](crate::MatchError) for you.
///
/// This error may be returned by the
/// [`DFA::start_state`](crate::hybrid::dfa::DFA::start_state) routine.
///
/// This error implements the `std::error::Error` trait when the `std` feature
/// is enabled.
///
/// This error is marked as non-exhaustive. New variants may be added in a
/// semver compatible release.
#[non_exhaustive]
#[derive(Clone, Debug)]
pub enum StartError {
    /// An error that occurs when cache inefficiency has dropped below the
    /// configured heuristic thresholds.
    Cache {
        /// The underlying cache error that occurred.
        err: CacheError,
    },
    /// An error that occurs when a starting configuration's look-behind byte
    /// is in this DFA's quit set.
    Quit {
        /// The quit byte that was found.
        byte: u8,
    },
    /// An error that occurs when the caller requests an anchored mode that
    /// isn't supported by the DFA.
    UnsupportedAnchored {
        /// The anchored mode given that is unsupported.
        mode: Anchored,
    },
}

impl StartError {
    pub(crate) fn cache(err: CacheError) -> StartError {
        StartError::Cache { err }
    }

    pub(crate) fn quit(byte: u8) -> StartError {
        StartError::Quit { byte }
    }

    pub(crate) fn unsupported_anchored(mode: Anchored) -> StartError {
        StartError::UnsupportedAnchored { mode }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for StartError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match *self {
            StartError::Cache { ref err } => Some(err),
            _ => None,
        }
    }
}

impl core::fmt::Display for StartError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {
            StartError::Cache { .. } => write!(
                f,
                "error computing start state because of cache inefficiency"
            ),
            StartError::Quit { byte } => write!(
                f,
                "error computing start state because the look-behind byte \
                 {:?} triggered a quit state",
                crate::util::escape::DebugByte(byte),
            ),
            StartError::UnsupportedAnchored { mode: Anchored::Yes } => {
                write!(
                    f,
                    "error computing start state because \
                     anchored searches are not supported or enabled"
                )
            }
            StartError::UnsupportedAnchored { mode: Anchored::No } => {
                write!(
                    f,
                    "error computing start state because \
                     unanchored searches are not supported or enabled"
                )
            }
            StartError::UnsupportedAnchored {
                mode: Anchored::Pattern(pid),
            } => {
                write!(
                    f,
                    "error computing start state because \
                     anchored searches for a specific pattern ({}) \
                     are not supported or enabled",
                    pid.as_usize(),
                )
            }
        }
    }
}

/// An error that occurs when cache usage has become inefficient.
///
/// One of the weaknesses of a lazy DFA is that it may need to clear its
/// cache repeatedly if it's not big enough. If this happens too much, then it
/// can slow searching down significantly. A mitigation to this is to use
/// heuristics to detect whether the cache is being used efficiently or not.
/// If not, then a lazy DFA can return a `CacheError`.
///
/// The default configuration of a lazy DFA in this crate is
/// set such that a `CacheError` will never occur. Instead,
/// callers must opt into this behavior with settings like
/// [`dfa::Config::minimum_cache_clear_count`](crate::hybrid::dfa::Config::minimum_cache_clear_count)
/// and
/// [`dfa::Config::minimum_bytes_per_state`](crate::hybrid::dfa::Config::minimum_bytes_per_state).
///
/// When the `std` feature is enabled, this implements the `std::error::Error`
/// trait.
#[derive(Clone, Debug)]
pub struct CacheError(());

impl CacheError {
    pub(crate) fn too_many_cache_clears() -> CacheError {
        CacheError(())
    }

    pub(crate) fn bad_efficiency() -> CacheError {
        CacheError(())
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CacheError {}

impl core::fmt::Display for CacheError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "lazy DFA cache has been cleared too many times")
    }
}
