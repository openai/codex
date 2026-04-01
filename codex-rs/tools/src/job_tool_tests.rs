use crate::ResponsesApiTool;
use crate::ToolSpec;
use pretty_assertions::assert_eq;

use super::create_job_create_tool;
use super::create_job_delete_tool;
use super::create_job_list_tool;

#[test]
fn job_create_tool_uses_expected_name() {
    let ToolSpec::Function(ResponsesApiTool { name, .. }) = create_job_create_tool() else {
        panic!("expected function tool");
    };
    assert_eq!(name, "JobCreate");
}

#[test]
fn job_delete_tool_uses_expected_name() {
    let ToolSpec::Function(ResponsesApiTool { name, .. }) = create_job_delete_tool() else {
        panic!("expected function tool");
    };
    assert_eq!(name, "JobDelete");
}

#[test]
fn job_list_tool_uses_expected_name() {
    let ToolSpec::Function(ResponsesApiTool { name, .. }) = create_job_list_tool() else {
        panic!("expected function tool");
    };
    assert_eq!(name, "JobList");
}
