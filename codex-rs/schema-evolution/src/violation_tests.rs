use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn breakage_projection_renders_typed_scope_paths_and_payloads() {
    let at = Location::method(
        "test/method",
        SchemaPath::default()
            .property("params")
            .property("items")
            .tuple_item(/*index*/ 1)
            .additional_properties(),
    );
    let violation = Violation::ConstraintChanged {
        at,
        before: SchemaSnapshot(json!({ "minimum": 0 })),
        after: SchemaSnapshot(json!({ "minimum": 1 })),
    };

    assert_eq!(
        violation.breakage(),
        SchemaBreakage {
            kind: ViolationKind::ConstraintChanged,
            method: "test/method".to_string(),
            path: "params.items[1].*".to_string(),
            before: json!({ "minimum": 0 }),
            after: json!({ "minimum": 1 }),
        }
    );
}
