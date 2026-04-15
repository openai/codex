use http::HeaderMap;
use http::HeaderValue;
use std::sync::Arc;

/// Adds authentication headers to API requests.
///
/// Implementations should be cheap and non-blocking; any asynchronous
/// refresh or I/O should be handled by higher layers before requests
/// reach this interface.
pub trait AuthProvider: Send + Sync {
    fn add_auth_headers(&self, headers: &mut HeaderMap);

    fn auth_header_attached(&self) -> bool {
        false
    }

    fn auth_header_name(&self) -> Option<&'static str> {
        None
    }
}

impl<T: AuthProvider + ?Sized> AuthProvider for Arc<T> {
    fn add_auth_headers(&self, headers: &mut HeaderMap) {
        self.as_ref().add_auth_headers(headers);
    }

    fn auth_header_attached(&self) -> bool {
        self.as_ref().auth_header_attached()
    }

    fn auth_header_name(&self) -> Option<&'static str> {
        self.as_ref().auth_header_name()
    }
}

pub(crate) fn add_fedramp_routing_header(headers: &mut HeaderMap) {
    headers.insert("X-OpenAI-Fedramp", HeaderValue::from_static("true"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_fedramp_routing_header_sets_header() {
        let mut headers = HeaderMap::new();

        add_fedramp_routing_header(&mut headers);

        assert_eq!(
            headers
                .get("X-OpenAI-Fedramp")
                .and_then(|v| v.to_str().ok()),
            Some("true")
        );
    }
}
