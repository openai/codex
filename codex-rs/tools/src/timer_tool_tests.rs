use crate::ResponsesApiTool;
use crate::ToolSpec;
use pretty_assertions::assert_eq;

use super::create_delete_timer_tool;
use super::create_list_timers_tool;
use super::create_timer_tool;

#[test]
fn timer_create_tool_uses_expected_name() {
    let ToolSpec::Function(ResponsesApiTool { name, .. }) = create_timer_tool() else {
        panic!("expected function tool");
    };
    assert_eq!(name, "create_timer");
}

#[test]
fn timer_create_tool_exposes_only_core_timer_payload_fields() {
    let ToolSpec::Function(ResponsesApiTool { parameters, .. }) = create_timer_tool() else {
        panic!("expected function tool");
    };
    let properties = parameters.properties.expect("create_timer properties");

    assert!(properties.contains_key("content"));
    assert!(!properties.contains_key("prompt"));
    assert!(!properties.contains_key("instructions"));
}

#[test]
fn timer_delete_tool_uses_expected_name() {
    let ToolSpec::Function(ResponsesApiTool { name, .. }) = create_delete_timer_tool() else {
        panic!("expected function tool");
    };
    assert_eq!(name, "delete_timer");
}

#[test]
fn timer_list_tool_uses_expected_name() {
    let ToolSpec::Function(ResponsesApiTool { name, .. }) = create_list_timers_tool() else {
        panic!("expected function tool");
    };
    assert_eq!(name, "list_timers");
}
