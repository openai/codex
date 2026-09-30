use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn security_setup_rejects_unsafe_actions_and_terminal_controls() {
    let value = json!({"title":"Keep using Daybreak mode","description":"Set up security.",
        "action":{"label":"Set up security","url":"https://chatgpt.com/cyber"}});
    let notice: Notice = serde_json::from_value(value.clone()).unwrap();
    assert!(notice.valid());
    for url in [
        "http://chatgpt.com/cyber",
        "https://chatgpt.com.evil.test/cyber",
        "https://evil.test/",
        "https://user@chatgpt.com/cyber",
        "https://chatgpt.com:8080/cyber",
        "javascript:alert(1)",
    ] {
        let mut invalid = value.clone();
        invalid["action"]["url"] = json!(url);
        assert!(
            !serde_json::from_value::<Notice>(invalid).unwrap().valid(),
            "{url}"
        );
    }
    let mut invalid = notice;
    invalid.title = "untrusted\u{1b}[31m".into();
    assert_eq!(invalid.valid(), false);
}
