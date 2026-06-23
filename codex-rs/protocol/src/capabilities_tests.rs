use super::CapabilityRootLocation;
use super::SelectedCapabilityRoot;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;

#[test]
fn environment_capability_root_requires_path_uri_on_the_wire() {
    let err = serde_json::from_str::<SelectedCapabilityRoot>(
        r#"{
            "id": "selected-demo",
            "location": {
                "type": "environment",
                "environmentId": "executor-test",
                "path": "/plugins/demo"
            }
        }"#,
    )
    .expect_err("native paths should be rejected");

    assert!(
        err.to_string().contains("relative URL without a base"),
        "unexpected error: {err}"
    );
}

#[test]
fn environment_capability_root_accepts_foreign_file_uri() {
    let selected_root = serde_json::from_str::<SelectedCapabilityRoot>(
        r#"{
            "id": "selected-demo",
            "location": {
                "type": "environment",
                "environmentId": "executor-test",
                "path": "file:///C:/plugins/demo"
            }
        }"#,
    )
    .expect("file URI should deserialize");

    assert_eq!(
        selected_root,
        SelectedCapabilityRoot {
            id: "selected-demo".to_string(),
            location: CapabilityRootLocation::Environment {
                environment_id: "executor-test".to_string(),
                path: PathUri::parse("file:///C:/plugins/demo").expect("path URI"),
            },
        }
    );
}
