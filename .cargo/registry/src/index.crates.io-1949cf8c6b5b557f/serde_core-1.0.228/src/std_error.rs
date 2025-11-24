use crate::lib::{Debug, Display};

/// Either a re-export of std::error::Error or a new identical trait, depending
/// on whether Serde's "std" feature is enabled.
///
/// Serde's error traits [`serde::ser::Error`] and [`serde::de::Error`] require
/// [`std::error::Error`] as a supertrait, but only when Serde is built with
/// "std" enabled. Data formats that don't care about no\_std support should
/// generally provide their error types with a `std::error::Error` impl
/// directly:
///
/// ```edition2021
/// #[derive(Debug)]
/// struct MySerError {...}
///
/// impl serde::ser::Error for MySerError {...}
///
/// impl std::fmt::Display for MySerError {...}
///
/// // We don't support no_std!
/// impl std::error::Error for MySerError {}
/// ```
///
/// Data formats that *do* support no\_std may either have a "std" feature of
/// their own:
///
/// ```toml
/// [features]
/// std = ["serde/std"]
/// ```
///
/// ```edition2021
/// #[cfg(feature = "std")]
/// impl std::error::Error for MySerError {}
/// ```
///
/// ... or else provide the std Error impl unconditionally via Serde's
/// re-export:
///
/// ```edition2021
/// impl serde::ser::StdError for MySerError {}
/// ```
pub trait Error: Debug + Display {
    /// The underlying cause of this error, if any.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
