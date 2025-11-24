// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::log;
use crate::{marker::DataMarkerId, prelude::*};
use core::fmt;
use displaydoc::Display;

/// A list specifying general categories of data provider error.
///
/// Errors may be caused either by a malformed request or by the data provider
/// not being able to fulfill a well-formed request.
#[derive(Clone, Copy, Eq, PartialEq, Display, Debug)]
#[non_exhaustive]
pub enum DataErrorKind {
    /// No data for the requested data marker. This is only returned by [`DynamicDataProvider`].
    #[displaydoc("Missing data for marker")]
    MarkerNotFound,

    /// There is data for the data marker, but not for this particular data identifier.
    #[displaydoc("Missing data for identifier")]
    IdentifierNotFound,

    /// The request is invalid, such as a request for a singleton marker containing a data identifier.
    #[displaydoc("Invalid request")]
    InvalidRequest,

    /// The data for two [`DataMarker`]s is not consistent.
    #[displaydoc("The data for two markers is not consistent: {0:?} (were they generated in different datagen invocations?)")]
    InconsistentData(DataMarkerInfo),

    /// An error occured during [`Any`](core::any::Any) downcasting.
    #[displaydoc("Downcast: expected {0}, found")]
    Downcast(&'static str),

    /// An error occured during [`serde`] deserialization.
    ///
    /// Check debug logs for potentially more information.
    #[displaydoc("Deserialize")]
    Deserialize,

    /// An unspecified error occurred.
    ///
    /// Check debug logs for potentially more information.
    #[displaydoc("Custom")]
    Custom,

    /// An error occurred while accessing a system resource.
    #[displaydoc("I/O: {0:?}")]
    #[cfg(feature = "std")]
    Io(std::io::ErrorKind),
}

/// The error type for ICU4X data provider operations.
///
/// To create one of these, either start with a [`DataErrorKind`] or use [`DataError::custom()`].
///
/// # Example
///
/// Create a IdentifierNotFound error and attach a data request for context:
///
/// ```no_run
/// # use icu_provider::prelude::*;
/// let marker: DataMarkerInfo = unimplemented!();
/// let req: DataRequest = unimplemented!();
/// DataErrorKind::IdentifierNotFound.with_req(marker, req);
/// ```
///
/// Create a named custom error:
///
/// ```
/// # use icu_provider::prelude::*;
/// DataError::custom("This is an example error");
/// ```
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
#[non_exhaustive]
pub struct DataError {
    /// Broad category of the error.
    pub kind: DataErrorKind,

    /// The data marker of the request, if available.
    pub marker: Option<DataMarkerId>,

    /// Additional context, if available.
    pub str_context: Option<&'static str>,

    /// Whether this error was created in silent mode to not log.
    pub silent: bool,
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ICU4X data error")?;
        if self.kind != DataErrorKind::Custom {
            write!(f, ": {}", self.kind)?;
        }
        if let Some(marker) = self.marker {
            write!(f, " (marker: {marker:?})")?;
        }
        if let Some(str_context) = self.str_context {
            write!(f, ": {str_context}")?;
        }
        Ok(())
    }
}

impl DataErrorKind {
    /// Converts this DataErrorKind into a DataError.
    ///
    /// If possible, you should attach context using a `with_` function.
    #[inline]
    pub const fn into_error(self) -> DataError {
        DataError {
            kind: self,
            marker: None,
            str_context: None,
            silent: false,
        }
    }

    /// Creates a DataError with a data marker context.
    #[inline]
    pub const fn with_marker(self, marker: DataMarkerInfo) -> DataError {
        self.into_error().with_marker(marker)
    }

    /// Creates a DataError with a string context.
    #[inline]
    pub const fn with_str_context(self, context: &'static str) -> DataError {
        self.into_error().with_str_context(context)
    }

    /// Creates a DataError with a type name context.
    #[inline]
    pub fn with_type_context<T>(self) -> DataError {
        self.into_error().with_type_context::<T>()
    }

    /// Creates a DataError with a request context.
    #[inline]
    pub fn with_req(self, marker: DataMarkerInfo, req: DataRequest) -> DataError {
        self.into_error().with_req(marker, req)
    }
}

impl DataError {
    /// Returns a new, empty DataError with kind Custom and a string error message.
    #[inline]
    pub const fn custom(str_context: &'static str) -> Self {
        Self {
            kind: DataErrorKind::Custom,
            marker: None,
            str_context: Some(str_context),
            silent: false,
        }
    }

    /// Sets the data marker of a DataError, returning a modified error.
    #[inline]
    pub const fn with_marker(self, marker: DataMarkerInfo) -> Self {
        Self {
            kind: self.kind,
            marker: Some(marker.id),
            str_context: self.str_context,
            silent: self.silent,
        }
    }

    /// Sets the string context of a DataError, returning a modified error.
    #[inline]
    pub const fn with_str_context(self, context: &'static str) -> Self {
        Self {
            kind: self.kind,
            marker: self.marker,
            str_context: Some(context),
            silent: self.silent,
        }
    }

    /// Sets the string context of a DataError to the given type name, returning a modified error.
    #[inline]
    pub fn with_type_context<T>(self) -> Self {
        if !self.silent {
            log::warn!("{self}: Type context: {}", core::any::type_name::<T>());
        }
        self.with_str_context(core::any::type_name::<T>())
    }

    /// Logs the data error with the given request, returning an error containing the data marker.
    ///
    /// If the "logging" Cargo feature is enabled, this logs the whole request. Either way,
    /// it returns an error with the data marker portion of the request as context.
    pub fn with_req(mut self, marker: DataMarkerInfo, req: DataRequest) -> Self {
        if req.metadata.silent {
            self.silent = true;
        }
        // Don't write out a log for MissingDataMarker since there is no context to add
        if !self.silent && self.kind != DataErrorKind::MarkerNotFound {
            log::warn!("{self} (marker: {marker:?}, request: {})", req.id);
        }
        self.with_marker(marker)
    }

    /// Logs the data error with the given context, then return self.
    ///
    /// This does not modify the error, but if the "logging" Cargo feature is enabled,
    /// it will print out the context.
    #[cfg(feature = "std")]
    pub fn with_path_context(self, _path: &std::path::Path) -> Self {
        if !self.silent {
            log::warn!("{self} (path: {_path:?})");
        }
        self
    }

    /// Logs the data error with the given context, then return self.
    ///
    /// This does not modify the error, but if the "logging" Cargo feature is enabled,
    /// it will print out the context.
    #[cfg_attr(not(feature = "logging"), allow(unused_variables))]
    #[inline]
    pub fn with_display_context<D: fmt::Display + ?Sized>(self, context: &D) -> Self {
        if !self.silent {
            log::warn!("{self}: {context}");
        }
        self
    }

    /// Logs the data error with the given context, then return self.
    ///
    /// This does not modify the error, but if the "logging" Cargo feature is enabled,
    /// it will print out the context.
    #[cfg_attr(not(feature = "logging"), allow(unused_variables))]
    #[inline]
    pub fn with_debug_context<D: fmt::Debug + ?Sized>(self, context: &D) -> Self {
        if !self.silent {
            log::warn!("{self}: {context:?}");
        }
        self
    }

    #[inline]
    pub(crate) fn for_type<T>() -> DataError {
        DataError {
            kind: DataErrorKind::Downcast(core::any::type_name::<T>()),
            marker: None,
            str_context: None,
            silent: false,
        }
    }
}

impl core::error::Error for DataError {}

#[cfg(feature = "std")]
impl From<std::io::Error> for DataError {
    fn from(e: std::io::Error) -> Self {
        log::warn!("I/O error: {e}");
        DataErrorKind::Io(e.kind()).into_error()
    }
}

/// Extension trait for `Result<T, DataError>`.
pub trait ResultDataError<T>: Sized {
    /// Propagates all errors other than [`DataErrorKind::IdentifierNotFound`], and returns `None` in that case.
    fn allow_identifier_not_found(self) -> Result<Option<T>, DataError>;
}

impl<T> ResultDataError<T> for Result<T, DataError> {
    fn allow_identifier_not_found(self) -> Result<Option<T>, DataError> {
        match self {
            Ok(t) => Ok(Some(t)),
            Err(DataError {
                kind: DataErrorKind::IdentifierNotFound,
                ..
            }) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
