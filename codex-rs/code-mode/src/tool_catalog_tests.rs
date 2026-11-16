#![cfg(test)]

use super::*;
use codex_code_mode_protocol::CodeModeToolKind;
use codex_protocol::ToolName;
use pretty_assertions::assert_eq;

#[test]
fn references_require_an_acknowledged_matching_admission() {
    let cache = Arc::new(Mutex::new(Catalog::default()));
    let mut tools = [ToolDefinition {
        name: "echo".to_string(),
        tool_name: ToolName::plain("echo"),
        description: String::new(),
        kind: CodeModeToolKind::Function,
        input_schema: None,
        input_schema_max_bytes: None,
        output_schema: None,
    }];
    let cancelled = CatalogAdmission::prepare(&cache, &tools).unwrap();
    assert!(!cancelled.reuses_catalog());
    assert!(CatalogAdmission::prepare(&cache, &tools).is_none());
    let abandoned_revision = cancelled.revision();
    drop(cancelled);
    let registration = CatalogAdmission::prepare(&cache, &tools).unwrap();
    assert!(!registration.reuses_catalog());
    assert!(registration.revision() > abandoned_revision);
    let revision = registration.revision();
    registration.accepted();
    let hit = CatalogAdmission::prepare(&cache, &tools).unwrap();
    assert!(hit.reuses_catalog());
    assert_eq!(hit.revision(), revision);
    hit.accepted();
    let other = Arc::new(Mutex::new(Catalog::default()));
    let registration = CatalogAdmission::prepare(&other, &tools).unwrap();
    assert!(!registration.reuses_catalog());
    registration.accepted();
    {
        let first = cache.try_lock().unwrap();
        let second = other.try_lock().unwrap();
        assert!(Arc::ptr_eq(
            &first.tools.as_ref().unwrap()[0],
            &second.tools.as_ref().unwrap()[0],
        ));
    }
    tools[0].description = "changed".to_string();
    assert!(
        !CatalogAdmission::prepare(&cache, &tools)
            .unwrap()
            .reuses_catalog()
    );
}
