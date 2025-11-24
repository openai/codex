// Unlike `impl Into<Option<T>>` or `Option<impl Into<T>>`, this isn't ambiguous for the `None`
// case.

use crate::builder::ArgAction;
use crate::builder::OsStr;
use crate::builder::Str;
use crate::builder::StyledStr;
use crate::builder::ValueHint;
use crate::builder::ValueParser;
use crate::builder::ValueRange;

/// Clearable builder value
///
/// This allows a builder function to both accept any value that can [`Into::into`] `T` (like
/// `&str` into `OsStr`) as well as `None` to reset it to the default.  This is needed to
/// workaround a limitation where you can't have a function argument that is `impl Into<Option<T>>`
/// where `T` is `impl Into<S>` accept `None` as its type is ambiguous.
///
/// # Example
///
/// ```rust
/// # use clap_builder as clap;
/// # use clap::Command;
/// # use clap::Arg;
/// fn common() -> Command {
///     Command::new("cli")
///         .arg(Arg::new("input").short('i').long("input"))
/// }
/// let mut command = common();
/// command.mut_arg("input", |arg| arg.short(None));
/// ```
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Resettable<T> {
    /// Overwrite builder value
    Value(T),
    /// Reset builder value
    Reset,
}

impl<T> Resettable<T> {
    pub(crate) fn into_option(self) -> Option<T> {
        match self {
            Self::Value(t) => Some(t),
            Self::Reset => None,
        }
    }
}

impl<T> From<T> for Resettable<T> {
    fn from(other: T) -> Self {
        Self::Value(other)
    }
}

impl<T> From<Option<T>> for Resettable<T> {
    fn from(other: Option<T>) -> Self {
        match other {
            Some(inner) => Self::Value(inner),
            None => Self::Reset,
        }
    }
}

/// Convert to the intended resettable type
pub trait IntoResettable<T> {
    /// Convert to the intended resettable type
    fn into_resettable(self) -> Resettable<T>;
}

impl IntoResettable<char> for Option<char> {
    fn into_resettable(self) -> Resettable<char> {
        match self {
            Some(s) => Resettable::Value(s),
            None => Resettable::Reset,
        }
    }
}

impl IntoResettable<usize> for Option<usize> {
    fn into_resettable(self) -> Resettable<usize> {
        match self {
            Some(s) => Resettable::Value(s),
            None => Resettable::Reset,
        }
    }
}

impl IntoResettable<ArgAction> for Option<ArgAction> {
    fn into_resettable(self) -> Resettable<ArgAction> {
        match self {
            Some(s) => Resettable::Value(s),
            None => Resettable::Reset,
        }
    }
}

impl IntoResettable<ValueHint> for Option<ValueHint> {
    fn into_resettable(self) -> Resettable<ValueHint> {
        match self {
            Some(s) => Resettable::Value(s),
            None => Resettable::Reset,
        }
    }
}

impl IntoResettable<ValueParser> for Option<ValueParser> {
    fn into_resettable(self) -> Resettable<ValueParser> {
        match self {
            Some(s) => Resettable::Value(s),
            None => Resettable::Reset,
        }
    }
}

impl IntoResettable<StyledStr> for Option<&'static str> {
    fn into_resettable(self) -> Resettable<StyledStr> {
        match self {
            Some(s) => Resettable::Value(s.into()),
            None => Resettable::Reset,
        }
    }
}

impl IntoResettable<OsStr> for Option<&'static str> {
    fn into_resettable(self) -> Resettable<OsStr> {
        match self {
            Some(s) => Resettable::Value(s.into()),
            None => Resettable::Reset,
        }
    }
}

impl IntoResettable<Str> for Option<&'static str> {
    fn into_resettable(self) -> Resettable<Str> {
        match self {
            Some(s) => Resettable::Value(s.into()),
            None => Resettable::Reset,
        }
    }
}

impl<T> IntoResettable<T> for Resettable<T> {
    fn into_resettable(self) -> Resettable<T> {
        self
    }
}

impl IntoResettable<char> for char {
    fn into_resettable(self) -> Resettable<char> {
        Resettable::Value(self)
    }
}

impl IntoResettable<usize> for usize {
    fn into_resettable(self) -> Resettable<usize> {
        Resettable::Value(self)
    }
}

impl IntoResettable<ArgAction> for ArgAction {
    fn into_resettable(self) -> Resettable<ArgAction> {
        Resettable::Value(self)
    }
}

impl IntoResettable<ValueHint> for ValueHint {
    fn into_resettable(self) -> Resettable<ValueHint> {
        Resettable::Value(self)
    }
}

impl<I: Into<ValueRange>> IntoResettable<ValueRange> for I {
    fn into_resettable(self) -> Resettable<ValueRange> {
        Resettable::Value(self.into())
    }
}

impl<I: Into<ValueParser>> IntoResettable<ValueParser> for I {
    fn into_resettable(self) -> Resettable<ValueParser> {
        Resettable::Value(self.into())
    }
}

impl<I: Into<String>> IntoResettable<String> for I {
    fn into_resettable(self) -> Resettable<String> {
        Resettable::Value(self.into())
    }
}

impl<I: Into<StyledStr>> IntoResettable<StyledStr> for I {
    fn into_resettable(self) -> Resettable<StyledStr> {
        Resettable::Value(self.into())
    }
}

impl<I: Into<OsStr>> IntoResettable<OsStr> for I {
    fn into_resettable(self) -> Resettable<OsStr> {
        Resettable::Value(self.into())
    }
}

impl<I: Into<Str>> IntoResettable<Str> for I {
    fn into_resettable(self) -> Resettable<Str> {
        Resettable::Value(self.into())
    }
}

impl<I: Into<crate::Id>> IntoResettable<crate::Id> for I {
    fn into_resettable(self) -> Resettable<crate::Id> {
        Resettable::Value(self.into())
    }
}
