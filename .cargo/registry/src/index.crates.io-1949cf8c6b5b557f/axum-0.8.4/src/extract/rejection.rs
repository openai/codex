//! Rejection response types.

use axum_core::__composite_rejection as composite_rejection;
use axum_core::__define_rejection as define_rejection;

pub use crate::extract::path::{FailedToDeserializePathParams, InvalidUtf8InPathParam};
pub use axum_core::extract::rejection::*;

#[cfg(feature = "json")]
define_rejection! {
    #[status = UNPROCESSABLE_ENTITY]
    #[body = "Failed to deserialize the JSON body into the target type"]
    #[cfg_attr(docsrs, doc(cfg(feature = "json")))]
    /// Rejection type for [`Json`](super::Json).
    ///
    /// This rejection is used if the request body is syntactically valid JSON but couldn't be
    /// deserialized into the target type.
    pub struct JsonDataError(Error);
}

#[cfg(feature = "json")]
define_rejection! {
    #[status = BAD_REQUEST]
    #[body = "Failed to parse the request body as JSON"]
    #[cfg_attr(docsrs, doc(cfg(feature = "json")))]
    /// Rejection type for [`Json`](super::Json).
    ///
    /// This rejection is used if the request body didn't contain syntactically valid JSON.
    pub struct JsonSyntaxError(Error);
}

#[cfg(feature = "json")]
define_rejection! {
    #[status = UNSUPPORTED_MEDIA_TYPE]
    #[body = "Expected request with `Content-Type: application/json`"]
    #[cfg_attr(docsrs, doc(cfg(feature = "json")))]
    /// Rejection type for [`Json`](super::Json) used if the `Content-Type`
    /// header is missing.
    pub struct MissingJsonContentType;
}

define_rejection! {
    #[status = INTERNAL_SERVER_ERROR]
    #[body = "Missing request extension"]
    /// Rejection type for [`Extension`](super::Extension) if an expected
    /// request extension was not found.
    pub struct MissingExtension(Error);
}

define_rejection! {
    #[status = INTERNAL_SERVER_ERROR]
    #[body = "No paths parameters found for matched route"]
    /// Rejection type used if axum's internal representation of path parameters
    /// is missing. This is commonly caused by extracting `Request<_>`. `Path`
    /// must be extracted first.
    pub struct MissingPathParams;
}

define_rejection! {
    #[status = UNSUPPORTED_MEDIA_TYPE]
    #[body = "Form requests must have `Content-Type: application/x-www-form-urlencoded`"]
    /// Rejection type for [`Form`](super::Form) or [`RawForm`](super::RawForm)
    /// used if the `Content-Type` header is missing
    /// or its value is not `application/x-www-form-urlencoded`.
    pub struct InvalidFormContentType;
}

define_rejection! {
    #[status = BAD_REQUEST]
    #[body = "Failed to deserialize form"]
    /// Rejection type used if the [`Form`](super::Form) extractor is unable to
    /// deserialize the form into the target type.
    pub struct FailedToDeserializeForm(Error);
}

define_rejection! {
    #[status = UNPROCESSABLE_ENTITY]
    #[body = "Failed to deserialize form body"]
    /// Rejection type used if the [`Form`](super::Form) extractor is unable to
    /// deserialize the form body into the target type.
    pub struct FailedToDeserializeFormBody(Error);
}

define_rejection! {
    #[status = BAD_REQUEST]
    #[body = "Failed to deserialize query string"]
    /// Rejection type used if the [`Query`](super::Query) extractor is unable to
    /// deserialize the query string into the target type.
    pub struct FailedToDeserializeQueryString(Error);
}

composite_rejection! {
    /// Rejection used for [`Query`](super::Query).
    ///
    /// Contains one variant for each way the [`Query`](super::Query) extractor
    /// can fail.
    pub enum QueryRejection {
        FailedToDeserializeQueryString,
    }
}

composite_rejection! {
    /// Rejection used for [`Form`](super::Form).
    ///
    /// Contains one variant for each way the [`Form`](super::Form) extractor
    /// can fail.
    pub enum FormRejection {
        InvalidFormContentType,
        FailedToDeserializeForm,
        FailedToDeserializeFormBody,
        BytesRejection,
    }
}

composite_rejection! {
    /// Rejection used for [`RawForm`](super::RawForm).
    ///
    /// Contains one variant for each way the [`RawForm`](super::RawForm) extractor
    /// can fail.
    pub enum RawFormRejection {
        InvalidFormContentType,
        BytesRejection,
    }
}

#[cfg(feature = "json")]
composite_rejection! {
    /// Rejection used for [`Json`](super::Json).
    ///
    /// Contains one variant for each way the [`Json`](super::Json) extractor
    /// can fail.
    #[cfg_attr(docsrs, doc(cfg(feature = "json")))]
    pub enum JsonRejection {
        JsonDataError,
        JsonSyntaxError,
        MissingJsonContentType,
        BytesRejection,
    }
}

composite_rejection! {
    /// Rejection used for [`Extension`](super::Extension).
    ///
    /// Contains one variant for each way the [`Extension`](super::Extension) extractor
    /// can fail.
    pub enum ExtensionRejection {
        MissingExtension,
    }
}

composite_rejection! {
    /// Rejection used for [`Path`](super::Path).
    ///
    /// Contains one variant for each way the [`Path`](super::Path) extractor
    /// can fail.
    pub enum PathRejection {
        FailedToDeserializePathParams,
        MissingPathParams,
    }
}

composite_rejection! {
    /// Rejection used for [`RawPathParams`](super::RawPathParams).
    ///
    /// Contains one variant for each way the [`RawPathParams`](super::RawPathParams) extractor
    /// can fail.
    pub enum RawPathParamsRejection {
        InvalidUtf8InPathParam,
        MissingPathParams,
    }
}

#[cfg(feature = "matched-path")]
define_rejection! {
    #[status = INTERNAL_SERVER_ERROR]
    #[body = "No matched path found"]
    /// Rejection if no matched path could be found.
    ///
    /// See [`MatchedPath`](super::MatchedPath) for more details.
    #[cfg_attr(docsrs, doc(cfg(feature = "matched-path")))]
    pub struct MatchedPathMissing;
}

#[cfg(feature = "matched-path")]
composite_rejection! {
    /// Rejection used for [`MatchedPath`](super::MatchedPath).
    #[cfg_attr(docsrs, doc(cfg(feature = "matched-path")))]
    pub enum MatchedPathRejection {
        MatchedPathMissing,
    }
}

define_rejection! {
    #[status = INTERNAL_SERVER_ERROR]
    #[body = "The matched route is not nested"]
    /// Rejection type for [`NestedPath`](super::NestedPath).
    ///
    /// This rejection is used if the matched route wasn't nested.
    pub struct NestedPathRejection;
}
