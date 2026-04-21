use crate::FreeformTool;
use crate::FreeformToolFormat;
use crate::JsonSchema;
use crate::ResponsesApiTool;
use crate::ToolSpec;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;

const APPLY_PATCH_LARK_GRAMMAR: &str = include_str!("tool_apply_patch.lark");
const APPLY_PATCH_ENVIRONMENT_LARK_GRAMMAR: &str =
    include_str!("tool_apply_patch_environment.lark");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApplyPatchToolOptions {
    pub multi_environment_tools: bool,
}

const APPLY_PATCH_JSON_TOOL_DESCRIPTION: &str = r#"Use the `apply_patch` tool to edit files.
Your patch language is a stripped‑down, file‑oriented diff format designed to be easy to parse and safe to apply. You can think of it as a high‑level envelope:

*** Begin Patch
[ one or more file sections ]
*** End Patch

Within that envelope, you get a sequence of file operations.
You MUST include a header to specify the action you are taking.
Each operation starts with one of three headers:

*** Add File: <path> - create a new file. Every following line is a + line (the initial contents).
*** Delete File: <path> - remove an existing file. Nothing follows.
*** Update File: <path> - patch an existing file in place (optionally with a rename).

May be immediately followed by *** Move to: <new path> if you want to rename the file.
Then one or more “hunks”, each introduced by @@ (optionally followed by a hunk header).
Within a hunk each line starts with:

For instructions on [context_before] and [context_after]:
- By default, show 3 lines of code immediately above and 3 lines immediately below each change. If a change is within 3 lines of a previous change, do NOT duplicate the first change’s [context_after] lines in the second change’s [context_before] lines.
- If 3 lines of context is insufficient to uniquely identify the snippet of code within the file, use the @@ operator to indicate the class or function to which the snippet belongs. For instance, we might have:
@@ class BaseClass
[3 lines of pre-context]
- [old_code]
+ [new_code]
[3 lines of post-context]

- If a code block is repeated so many times in a class or function such that even a single `@@` statement and 3 lines of context cannot uniquely identify the snippet of code, you can use multiple `@@` statements to jump to the right context. For instance:

@@ class BaseClass
@@ 	 def method():
[3 lines of pre-context]
- [old_code]
+ [new_code]
[3 lines of post-context]

The full grammar definition is below:
Patch := Begin { FileOp } End
Begin := "*** Begin Patch" NEWLINE
End := "*** End Patch" NEWLINE
FileOp := AddFile | DeleteFile | UpdateFile
AddFile := "*** Add File: " path NEWLINE { "+" line NEWLINE }
DeleteFile := "*** Delete File: " path NEWLINE
UpdateFile := "*** Update File: " path NEWLINE [ MoveTo ] { Hunk }
MoveTo := "*** Move to: " newPath NEWLINE
Hunk := "@@" [ header ] NEWLINE { HunkLine } [ "*** End of File" NEWLINE ]
HunkLine := (" " | "-" | "+") text NEWLINE

A full patch can combine several operations:

*** Begin Patch
*** Add File: hello.txt
+Hello world
*** Update File: src/app.py
*** Move to: src/main.py
@@ def greet():
-print("Hi")
+print("Hello, world!")
*** Delete File: obsolete.txt
*** End Patch

It is important to remember:

- You must include a header with your intended action (Add/Delete/Update)
- You must prefix new lines with `+` even when creating a new file
- File references can only be relative, NEVER ABSOLUTE.
"#;

/// TODO(dylan): deprecate once we get rid of json tool
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyPatchToolArgs {
    pub input: String,
    #[serde(default, alias = "environmentId")]
    pub environment_id: Option<String>,
}

/// Returns a custom tool that can be used to edit files. Well-suited for GPT-5 models
/// https://platform.openai.com/docs/guides/function-calling#custom-tools
pub fn create_apply_patch_freeform_tool(options: ApplyPatchToolOptions) -> ToolSpec {
    let description = if options.multi_environment_tools {
        "Use the `apply_patch` tool to edit files. This is a FREEFORM tool, so do not wrap the patch in JSON. To target a non-primary selected environment, add a first line `*** Environment: <environment_id>` before `*** Begin Patch`."
    } else {
        "Use the `apply_patch` tool to edit files. This is a FREEFORM tool, so do not wrap the patch in JSON."
    };
    let definition = if options.multi_environment_tools {
        APPLY_PATCH_ENVIRONMENT_LARK_GRAMMAR
    } else {
        APPLY_PATCH_LARK_GRAMMAR
    };
    ToolSpec::Freeform(FreeformTool {
        name: "apply_patch".to_string(),
        description: description.to_string(),
        format: FreeformToolFormat {
            r#type: "grammar".to_string(),
            syntax: "lark".to_string(),
            definition: definition.to_string(),
        },
    })
}

/// Returns a json tool that can be used to edit files. Should only be used with gpt-oss models
pub fn create_apply_patch_json_tool(options: ApplyPatchToolOptions) -> ToolSpec {
    let mut properties = BTreeMap::from([(
        "input".to_string(),
        JsonSchema::string(Some(
            "The entire contents of the apply_patch command".to_string(),
        )),
    )]);
    if options.multi_environment_tools {
        properties.insert(
            "environment_id".to_string(),
            JsonSchema::string(Some(
                "Optional selected environment id to apply the patch in; omit to use the primary environment.".to_string(),
            )),
        );
    }

    ToolSpec::Function(ResponsesApiTool {
        name: "apply_patch".to_string(),
        description: APPLY_PATCH_JSON_TOOL_DESCRIPTION.to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(
            properties,
            Some(vec!["input".to_string()]),
            Some(false.into()),
        ),
        output_schema: None,
    })
}

#[cfg(test)]
#[path = "apply_patch_tool_tests.rs"]
mod tests;
