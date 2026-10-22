//! Validate original HTTP framing and the declared length for AWS signing.

use rama_http::HeaderMap;

pub(crate) fn is_amazon_event_stream(headers: &HeaderMap) -> bool {
    headers.get_all("content-type").iter().any(|value| {
        value.to_str().is_ok_and(|value| {
            value.split(';').next().is_some_and(|mime| {
                mime.trim()
                    .eq_ignore_ascii_case("application/vnd.amazon.eventstream")
            })
        })
    })
}

pub(crate) fn supported_transfer_framing(headers: &HeaderMap) -> bool {
    let codings = headers.get_all("transfer-encoding");
    let mut codings = codings.iter();
    match codings.next() {
        None => true,
        Some(coding) => {
            !headers.contains_key("content-length")
                && codings.next().is_none()
                && coding
                    .to_str()
                    .is_ok_and(|value| value.trim().eq_ignore_ascii_case("chunked"))
        }
    }
}

pub(crate) fn content_length_matches(headers: &HeaderMap, size: usize) -> bool {
    let lengths = headers.get_all("content-length");
    let mut lengths = lengths.iter();
    match lengths.next() {
        None => true,
        Some(length) => {
            lengths.next().is_none()
                && length.to_str().is_ok_and(|value| {
                    !value.is_empty()
                        && value.bytes().all(|byte| byte.is_ascii_digit())
                        && value.parse::<usize>() == Ok(size)
                })
        }
    }
}
