use super::*;
use http::HeaderValue;
use pretty_assertions::assert_eq;

/// Delta seconds start at receipt and remain present once their deadline expires.
#[tokio::test(start_paused = true)]
async fn delta_seconds_count_down_to_zero() {
    let mut headers = HeaderMap::new();
    headers.insert(RETRY_AFTER, HeaderValue::from_static("10"));
    let advice = RetryAfter::from_headers(&headers).expect("valid advice");
    let deadline = Instant::now() + Duration::from_secs(10);
    assert_eq!(advice.deadline(), deadline);
    assert_eq!(advice.remaining_delay(), Duration::from_secs(10));
    tokio::time::advance(Duration::from_secs(4)).await;
    assert_eq!(advice.remaining_delay(), Duration::from_secs(6));
    tokio::time::advance(Duration::from_secs(7)).await;
    assert_eq!(advice.remaining_delay(), Duration::ZERO);
}

/// HTTP dates share the monotonic clock; expired dates and explicit zero are still advice.
#[tokio::test(start_paused = true)]
async fn http_dates_and_invalid_advice() {
    let value = httpdate::fmt_http_date(SystemTime::now() + Duration::from_secs(60));
    let date = httpdate::parse_http_date(&value).expect("formatted date");
    let before = SystemTime::now();
    let advice = RetryAfter::from_header(&value).expect("valid date");
    let after = SystemTime::now();
    let remaining = advice.remaining_delay();
    assert!(
        (date.duration_since(after).unwrap()..=date.duration_since(before).unwrap())
            .contains(&remaining)
    );
    tokio::time::advance(Duration::from_secs(3)).await;
    assert_eq!(advice.remaining_delay(), remaining - Duration::from_secs(3));

    for value in ["0", "Thu, 01 Jan 1970 00:00:00 GMT"] {
        assert_eq!(
            RetryAfter::from_header(value).map(RetryAfter::remaining_delay),
            Some(Duration::ZERO)
        );
    }
    for value in ["", "-1", "+1", "1.5", "invalid", "18446744073709551615"] {
        assert_eq!(RetryAfter::from_header(value), None, "{value}");
    }
}
