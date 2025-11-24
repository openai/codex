use http::header::HeaderValue;
use httpdate::HttpDate;
use std::time::SystemTime;

pub(super) struct LastModified(pub(super) HttpDate);

impl From<SystemTime> for LastModified {
    fn from(time: SystemTime) -> Self {
        LastModified(time.into())
    }
}

pub(super) struct IfModifiedSince(HttpDate);

impl IfModifiedSince {
    /// Check if the supplied time means the resource has been modified.
    pub(super) fn is_modified(&self, last_modified: &LastModified) -> bool {
        self.0 < last_modified.0
    }

    /// convert a header value into a IfModifiedSince, invalid values are silentely ignored
    pub(super) fn from_header_value(value: &HeaderValue) -> Option<IfModifiedSince> {
        std::str::from_utf8(value.as_bytes())
            .ok()
            .and_then(|value| httpdate::parse_http_date(value).ok())
            .map(|time| IfModifiedSince(time.into()))
    }
}

pub(super) struct IfUnmodifiedSince(HttpDate);

impl IfUnmodifiedSince {
    /// Check if the supplied time passes the precondtion.
    pub(super) fn precondition_passes(&self, last_modified: &LastModified) -> bool {
        self.0 >= last_modified.0
    }

    /// Convert a header value into a IfModifiedSince, invalid values are silentely ignored
    pub(super) fn from_header_value(value: &HeaderValue) -> Option<IfUnmodifiedSince> {
        std::str::from_utf8(value.as_bytes())
            .ok()
            .and_then(|value| httpdate::parse_http_date(value).ok())
            .map(|time| IfUnmodifiedSince(time.into()))
    }
}
