use super::BrowserUseRequirements;
use super::RequestHeader;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn request_header_debug_redacts_values_without_changing_serialization() {
    let wire = json!({ "name": "x-company-token", "value": "private-header-token" });
    let header: RequestHeader = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(serde_json::to_value(&header).unwrap(), wire);
    assert_eq!(
        format!("{header:?}"),
        r#"RequestHeader { name: "x-company-token", value: "[REDACTED]" }"#
    );
    let requirements: BrowserUseRequirements = serde_json::from_value(json!({
        "extension": { "requestHeaders": [wire] }
    }))
    .unwrap();
    for debug in [format!("{requirements:?}"), format!("{requirements:#?}")] {
        assert!(!debug.contains("private-header-token"));
        assert!(debug.contains("x-company-token"));
        assert!(debug.contains("[REDACTED]"));
    }
    assert_eq!(
        serde_json::to_value(&requirements).unwrap()["extension"]["requestHeaders"],
        json!([wire])
    );
}
