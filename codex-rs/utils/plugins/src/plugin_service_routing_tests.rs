use super::*;
use pretty_assertions::assert_eq;

#[test]
fn preview_signal_requires_exact_enabled_value() {
    for value in [
        None,
        Some(""),
        Some("0"),
        Some("true"),
        Some(" 1"),
        Some("1 "),
    ] {
        assert!(!plugin_service_preview_enabled_from_value(
            value.map(OsStr::new)
        ));
    }
    assert!(plugin_service_preview_enabled_from_value(Some(OsStr::new(
        "1"
    ))));
}

#[test]
fn routing_cookie_is_disabled_by_default_and_cannot_be_enabled_by_caller() {
    assert_eq!(
        plugin_service_routing_cookie(&[], /*preview_enabled*/ false),
        None
    );
    assert_eq!(
        plugin_service_routing_cookie(
            &[
                b"session=abc; oai-chat-plugin-service-preview=true".as_slice(),
                b"theme=dark".as_slice(),
            ],
            /*preview_enabled*/ false,
        ),
        Some(b"session=abc; theme=dark".to_vec()),
    );
}

#[test]
fn routing_cookie_preserves_unrelated_cookies_and_replaces_caller_value() {
    assert_eq!(
        plugin_service_routing_cookie(
            &[
                b"session=abc; oai-chat-plugin-service-preview=false".as_slice(),
                b"theme=dark; oai-chat-plugin-service-preview=true".as_slice(),
            ],
            /*preview_enabled*/ true,
        ),
        Some(b"session=abc; theme=dark; oai-chat-plugin-service-preview=true".to_vec()),
    );
}
