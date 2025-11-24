#[cfg(all(feature = "client", feature = "http1"))]
use bytes::BytesMut;
use http::header::HeaderValue;
#[cfg(all(feature = "http2", feature = "client"))]
use http::Method;
#[cfg(any(feature = "client", all(feature = "server", feature = "http2")))]
use http::{
    header::{ValueIter, CONTENT_LENGTH},
    HeaderMap,
};

#[cfg(feature = "http1")]
pub(super) fn connection_keep_alive(value: &HeaderValue) -> bool {
    connection_has(value, "keep-alive")
}

#[cfg(feature = "http1")]
pub(super) fn connection_close(value: &HeaderValue) -> bool {
    connection_has(value, "close")
}

#[cfg(feature = "http1")]
fn connection_has(value: &HeaderValue, needle: &str) -> bool {
    if let Ok(s) = value.to_str() {
        for val in s.split(',') {
            if val.trim().eq_ignore_ascii_case(needle) {
                return true;
            }
        }
    }
    false
}

#[cfg(all(feature = "http1", feature = "server"))]
pub(super) fn content_length_parse(value: &HeaderValue) -> Option<u64> {
    from_digits(value.as_bytes())
}

#[cfg(any(feature = "client", all(feature = "server", feature = "http2")))]
pub(super) fn content_length_parse_all(headers: &HeaderMap) -> Option<u64> {
    content_length_parse_all_values(headers.get_all(CONTENT_LENGTH).into_iter())
}

#[cfg(any(feature = "client", all(feature = "server", feature = "http2")))]
pub(super) fn content_length_parse_all_values(values: ValueIter<'_, HeaderValue>) -> Option<u64> {
    // If multiple Content-Length headers were sent, everything can still
    // be alright if they all contain the same value, and all parse
    // correctly. If not, then it's an error.

    let mut content_length: Option<u64> = None;
    for h in values {
        if let Ok(line) = h.to_str() {
            for v in line.split(',') {
                if let Some(n) = from_digits(v.trim().as_bytes()) {
                    if content_length.is_none() {
                        content_length = Some(n)
                    } else if content_length != Some(n) {
                        return None;
                    }
                } else {
                    return None;
                }
            }
        } else {
            return None;
        }
    }

    content_length
}

fn from_digits(bytes: &[u8]) -> Option<u64> {
    // cannot use FromStr for u64, since it allows a signed prefix
    let mut result = 0u64;
    const RADIX: u64 = 10;

    if bytes.is_empty() {
        return None;
    }

    for &b in bytes {
        // can't use char::to_digit, since we haven't verified these bytes
        // are utf-8.
        match b {
            b'0'..=b'9' => {
                result = result.checked_mul(RADIX)?;
                result = result.checked_add((b - b'0') as u64)?;
            }
            _ => {
                // not a DIGIT, get outta here!
                return None;
            }
        }
    }

    Some(result)
}

#[cfg(all(feature = "http2", feature = "client"))]
pub(super) fn method_has_defined_payload_semantics(method: &Method) -> bool {
    !matches!(
        *method,
        Method::GET | Method::HEAD | Method::DELETE | Method::CONNECT
    )
}

#[cfg(feature = "http2")]
pub(super) fn set_content_length_if_missing(headers: &mut HeaderMap, len: u64) {
    headers
        .entry(CONTENT_LENGTH)
        .or_insert_with(|| HeaderValue::from(len));
}

#[cfg(all(feature = "client", feature = "http1"))]
pub(super) fn transfer_encoding_is_chunked(headers: &HeaderMap) -> bool {
    is_chunked(headers.get_all(http::header::TRANSFER_ENCODING).into_iter())
}

#[cfg(all(feature = "client", feature = "http1"))]
pub(super) fn is_chunked(mut encodings: ValueIter<'_, HeaderValue>) -> bool {
    // chunked must always be the last encoding, according to spec
    if let Some(line) = encodings.next_back() {
        return is_chunked_(line);
    }

    false
}

#[cfg(feature = "http1")]
pub(super) fn is_chunked_(value: &HeaderValue) -> bool {
    // chunked must always be the last encoding, according to spec
    if let Ok(s) = value.to_str() {
        if let Some(encoding) = s.rsplit(',').next() {
            return encoding.trim().eq_ignore_ascii_case("chunked");
        }
    }

    false
}

#[cfg(all(feature = "client", feature = "http1"))]
pub(super) fn add_chunked(mut entry: http::header::OccupiedEntry<'_, HeaderValue>) {
    const CHUNKED: &str = "chunked";

    if let Some(line) = entry.iter_mut().next_back() {
        // + 2 for ", "
        let new_cap = line.as_bytes().len() + CHUNKED.len() + 2;
        let mut buf = BytesMut::with_capacity(new_cap);
        buf.extend_from_slice(line.as_bytes());
        buf.extend_from_slice(b", ");
        buf.extend_from_slice(CHUNKED.as_bytes());

        *line = HeaderValue::from_maybe_shared(buf.freeze())
            .expect("original header value plus ascii is valid");
        return;
    }

    entry.insert(HeaderValue::from_static(CHUNKED));
}
