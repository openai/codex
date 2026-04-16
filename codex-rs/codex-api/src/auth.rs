use http::HeaderMap;
use http::HeaderValue;

/// Adds authentication headers to API requests.
///
/// Implementations should be cheap and non-blocking; any asynchronous
/// refresh or I/O should be handled by higher layers before requests
/// reach this interface.
pub trait AuthProvider: Send + Sync {
    fn add_auth_headers(&self, headers: &mut HeaderMap);
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
