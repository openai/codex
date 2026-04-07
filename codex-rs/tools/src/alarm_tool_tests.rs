use crate::ResponsesApiTool;
use crate::ToolSpec;
use pretty_assertions::assert_eq;

use super::create_alarm_create_tool;
use super::create_alarm_delete_tool;
use super::create_alarm_list_tool;

#[test]
fn alarm_create_tool_uses_expected_name() {
    let ToolSpec::Function(ResponsesApiTool { name, .. }) = create_alarm_create_tool() else {
        panic!("expected function tool");
    };
    assert_eq!(name, "AlarmCreate");
}

#[test]
fn alarm_delete_tool_uses_expected_name() {
    let ToolSpec::Function(ResponsesApiTool { name, .. }) = create_alarm_delete_tool() else {
        panic!("expected function tool");
    };
    assert_eq!(name, "AlarmDelete");
}

#[test]
fn alarm_list_tool_uses_expected_name() {
    let ToolSpec::Function(ResponsesApiTool { name, .. }) = create_alarm_list_tool() else {
        panic!("expected function tool");
    };
    assert_eq!(name, "AlarmList");
}
