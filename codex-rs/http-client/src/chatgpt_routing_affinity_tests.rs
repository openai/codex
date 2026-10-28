use super::*;
use pretty_assertions::assert_eq;

#[test]
fn affinity_retains_only_the_response_pin_with_its_cookie_scope_and_expiry() {
    let uri: Uri = "wss://affinity.chatgpt.com/backend-api/codex/responses"
        .parse()
        .unwrap();
    let mut headers = HeaderMap::new();
    headers.append(
        SET_COOKIE,
        HeaderValue::from_static("__oailb=parent; Path=/backend-api; Max-Age=3600; Secure"),
    );
    headers.append(
        SET_COOKIE,
        HeaderValue::from_static("__cflb=parent-native; Path=/backend-api; Max-Age=3600; Secure"),
    );
    headers.append(
        SET_COOKIE,
        HeaderValue::from_static("chatgpt_session=secret; Path=/; Secure"),
    );
    let affinity = ChatGptRoutingAffinity::from_response(&uri, &headers).unwrap();
    let pin = affinity.cookie_header(&uri).unwrap();
    let mut cookies = pin.to_str().unwrap().split("; ").collect::<Vec<_>>();
    cookies.sort_unstable();
    assert_eq!(cookies, ["__cflb=parent-native", "__oailb=parent"]);
    assert!(pin.is_sensitive());
    let existing = HeaderValue::from_static(
        "__oailb=old; cf_clearance=keep; __cflb=old-native; configured=keep",
    );
    let merged = affinity.merge_cookie_header(&uri, Some(&existing)).unwrap();
    let mut cookies = merged.to_str().unwrap().split("; ").collect::<Vec<_>>();
    cookies.sort_unstable();
    assert_eq!(
        cookies,
        [
            "__cflb=parent-native",
            "__oailb=parent",
            "cf_clearance=keep",
            "configured=keep",
        ]
    );
    assert!(merged.is_sensitive());
    assert_eq!(
        affinity.merge_cookie_header(&uri, /*existing*/ None),
        Some(pin.clone())
    );
    assert_eq!(format!("{affinity:?}"), "ChatGptRoutingAffinity { .. }");
    for outside_scope in [
        "ws://affinity.chatgpt.com/backend-api/codex/responses",
        "wss://other.chatgpt.com/backend-api/codex/responses",
        "wss://affinity.chatgpt.com:444/backend-api/codex/responses",
        "wss://affinity.chatgpt.com/backend-api/codex/other",
        "wss://api.openai.com/backend-api/codex/responses",
    ] {
        assert_eq!(
            affinity.cookie_header(&outside_scope.parse().unwrap()),
            None
        );
        assert_eq!(
            affinity.merge_cookie_header(&outside_scope.parse().unwrap(), Some(&existing)),
            None
        );
    }
    for name in ["__oailb", "__cflb"] {
        headers.insert(
            SET_COOKIE,
            HeaderValue::from_str(&format!("{name}=only; Path=/; Secure")).unwrap(),
        );
        assert_eq!(
            ChatGptRoutingAffinity::from_response(&uri, &headers)
                .unwrap()
                .cookie_header(&uri),
            Some(HeaderValue::from_str(&format!("{name}=only")).unwrap())
        );
        for invalid in [
            "__oailb=deleted; Path=/; Max-Age=0; Secure",
            "__oailb=expired; Path=/; Expires=Thu, 01 Jan 1970 00:00:00 GMT; Secure",
            "__oailb=wrong-path; Path=/elsewhere; Secure",
            "__oailb=wrong-domain; Domain=other.chatgpt.com; Path=/; Secure",
            "chatgpt_session=secret; Path=/; Secure",
        ] {
            headers.insert(
                SET_COOKIE,
                HeaderValue::from_str(&invalid.replace("__oailb", name)).unwrap(),
            );
            assert!(ChatGptRoutingAffinity::from_response(&uri, &headers).is_none());
        }
        headers.insert(
            SET_COOKIE,
            HeaderValue::from_str(&format!("{name}=one; Path=/; Secure")).unwrap(),
        );
        headers.append(
            SET_COOKIE,
            HeaderValue::from_str(&format!("{name}=two; Path=/; Secure")).unwrap(),
        );
        assert!(ChatGptRoutingAffinity::from_response(&uri, &headers).is_none());
    }
    assert_eq!(affinity.cookie_header(&uri), Some(pin));
}
