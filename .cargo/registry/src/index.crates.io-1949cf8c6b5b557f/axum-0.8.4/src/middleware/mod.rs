//! Utilities for writing middleware
//!
#![doc = include_str!("../docs/middleware.md")]

mod from_extractor;
mod from_fn;
mod map_request;
mod map_response;

pub use self::from_extractor::{
    from_extractor, from_extractor_with_state, FromExtractor, FromExtractorLayer,
};
pub use self::from_fn::{from_fn, from_fn_with_state, FromFn, FromFnLayer, Next};
pub use self::map_request::{
    map_request, map_request_with_state, IntoMapRequestResult, MapRequest, MapRequestLayer,
};
pub use self::map_response::{
    map_response, map_response_with_state, MapResponse, MapResponseLayer,
};
pub use crate::extension::AddExtension;

pub mod future {
    //! Future types.

    pub use super::from_extractor::ResponseFuture as FromExtractorResponseFuture;
    pub use super::from_fn::ResponseFuture as FromFnResponseFuture;
    pub use super::map_request::ResponseFuture as MapRequestResponseFuture;
    pub use super::map_response::ResponseFuture as MapResponseResponseFuture;
}
