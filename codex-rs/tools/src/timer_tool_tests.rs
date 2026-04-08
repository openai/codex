use crate::ResponsesApiTool;
use crate::ToolSpec;
use pretty_assertions::assert_eq;

use super::create_timer_create_tool;
use super::create_timer_delete_tool;
use super::create_timer_list_tool;

#[test]
fn timer_create_tool_uses_expected_name() {
    let ToolSpec::Function(ResponsesApiTool { name, .. }) = create_timer_create_tool() else {
        panic!("expected function tool");
    };
    assert_eq!(name, "TimerCreate");
}

#[test]
fn timer_delete_tool_uses_expected_name() {
    let ToolSpec::Function(ResponsesApiTool { name, .. }) = create_timer_delete_tool() else {
        panic!("expected function tool");
    };
    assert_eq!(name, "TimerDelete");
}

#[test]
fn timer_list_tool_uses_expected_name() {
    let ToolSpec::Function(ResponsesApiTool { name, .. }) = create_timer_list_tool() else {
        panic!("expected function tool");
    };
    assert_eq!(name, "TimerList");
}
