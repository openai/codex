use super::*;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ToolSpec;
use pretty_assertions::assert_eq;

#[test]
fn files_namespace_contains_poc_actions() {
    let ToolSpec::Namespace(namespace) = create_files_namespace_tool() else {
        panic!("files tool should be a namespace");
    };

    assert_eq!(namespace.name, FILES_NAMESPACE);
    let tool_names = namespace
        .tools
        .iter()
        .map(|tool| match tool {
            ResponsesApiNamespaceTool::Function(tool) => tool.name.as_str(),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        tool_names,
        vec![
            FILES_MATERIALIZE_TOOL_NAME,
            FILES_COPY_TOOL_NAME,
            FILES_EXPORT_FOR_TOOL_NAME,
        ]
    );
}
