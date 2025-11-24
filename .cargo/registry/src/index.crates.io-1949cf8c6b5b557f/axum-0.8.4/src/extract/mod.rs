#![doc = include_str!("../docs/extract.md")]

use http::header::{self, HeaderMap};

#[cfg(feature = "tokio")]
pub mod connect_info;
pub mod path;
pub mod rejection;

#[cfg(feature = "ws")]
pub mod ws;

pub(crate) mod nested_path;
#[cfg(feature = "original-uri")]
mod original_uri;
mod raw_form;
mod raw_query;
mod state;

#[doc(inline)]
pub use axum_core::extract::{
    DefaultBodyLimit, FromRef, FromRequest, FromRequestParts, OptionalFromRequest,
    OptionalFromRequestParts, Request,
};

#[cfg(feature = "macros")]
pub use axum_macros::{FromRef, FromRequest, FromRequestParts};

#[doc(inline)]
pub use self::{
    nested_path::NestedPath,
    path::{Path, RawPathParams},
    raw_form::RawForm,
    raw_query::RawQuery,
    state::State,
};

#[doc(inline)]
#[cfg(feature = "tokio")]
pub use self::connect_info::ConnectInfo;

#[doc(no_inline)]
#[cfg(feature = "json")]
pub use crate::Json;

#[doc(no_inline)]
pub use crate::Extension;

#[cfg(feature = "form")]
#[doc(no_inline)]
pub use crate::form::Form;

#[cfg(feature = "matched-path")]
pub(crate) mod matched_path;

#[cfg(feature = "matched-path")]
#[doc(inline)]
pub use self::matched_path::MatchedPath;

#[cfg(feature = "multipart")]
pub mod multipart;

#[cfg(feature = "multipart")]
#[doc(inline)]
pub use self::multipart::Multipart;

#[cfg(feature = "query")]
mod query;

#[cfg(feature = "query")]
#[doc(inline)]
pub use self::query::Query;

#[cfg(feature = "original-uri")]
#[doc(inline)]
pub use self::original_uri::OriginalUri;

#[cfg(feature = "ws")]
#[doc(inline)]
pub use self::ws::WebSocketUpgrade;

// this is duplicated in `axum-extra/src/extract/form.rs`
pub(super) fn has_content_type(headers: &HeaderMap, expected_content_type: &mime::Mime) -> bool {
    let content_type = if let Some(content_type) = headers.get(header::CONTENT_TYPE) {
        content_type
    } else {
        return false;
    };

    let content_type = if let Ok(content_type) = content_type.to_str() {
        content_type
    } else {
        return false;
    };

    content_type.starts_with(expected_content_type.as_ref())
}

#[cfg(test)]
mod tests {
    use crate::{routing::get, test_helpers::*, Router};

    #[crate::test]
    async fn consume_body() {
        let app = Router::new().route("/", get(|body: String| async { body }));

        let client = TestClient::new(app);
        let res = client.get("/").body("foo").await;
        let body = res.text().await;

        assert_eq!(body, "foo");
    }
}
