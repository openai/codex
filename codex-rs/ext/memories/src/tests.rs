use std::path::Path;
use std::sync::Arc;

use codex_extension_api::ContextContributor;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionToolExecutor;
use codex_extension_api::PromptSlot;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolContributor;
use codex_extension_api::ToolName;
use codex_extension_api::ToolPayload;
use codex_tools::ToolOutput;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_absolute_path::test_support::PathExt;
use codex_utils_absolute_path::test_support::test_path_buf;
use pretty_assertions::assert_eq;
use serde_json::json;

use crate::extension::MemoriesExtension;
use crate::extension::MemoriesExtensionConfig;
use crate::local::LocalMemoriesBackend;

#[test]
fn tools_are_not_contributed_without_thread_config() {
    let extension = MemoriesExtension;

    assert!(
        extension
            .tools(
                &ExtensionData::new("session"),
                &ExtensionData::new("thread")
            )
            .is_empty()
    );
}

#[test]
fn tools_are_not_contributed_when_disabled() {
    let extension = MemoriesExtension;
    let thread_store = ExtensionData::new("thread");
    thread_store.insert(MemoriesExtensionConfig {
        enabled: false,
        codex_home: test_path_buf("/tmp/codex-home").abs(),
    });

    assert!(
        extension
            .tools(&ExtensionData::new("session"), &thread_store)
            .is_empty()
    );
}

#[test]
fn tools_are_contributed_when_enabled() {
    let extension = MemoriesExtension;
    let thread_store = ExtensionData::new("thread");
    thread_store.insert(MemoriesExtensionConfig {
        enabled: true,
        codex_home: test_path_buf("/tmp/codex-home").abs(),
    });

    let tool_names = extension
        .tools(&ExtensionData::new("session"), &thread_store)
        .into_iter()
        .map(|tool| tool.tool_name())
        .collect::<Vec<_>>();

    assert_eq!(
        tool_names,
        vec![
            memory_tool_name(crate::LIST_TOOL_NAME),
            memory_tool_name(crate::READ_TOOL_NAME),
            memory_tool_name(crate::SEARCH_TOOL_NAME),
        ]
    );
}

#[tokio::test]
async fn prompt_contribution_uses_memory_summary_when_enabled() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let memories_dir = tempdir.path().join("memories");
    tokio::fs::create_dir_all(&memories_dir)
        .await
        .expect("create memories dir");
    tokio::fs::write(
        memories_dir.join("memory_summary.md"),
        "Remember repository-specific implementation preferences.",
    )
    .await
    .expect("write memory summary");

    let extension = MemoriesExtension;
    let thread_store = ExtensionData::new("thread");
    thread_store.insert(MemoriesExtensionConfig {
        enabled: true,
        codex_home: tempdir.path().abs(),
    });

    let fragments = extension
        .contribute(&ExtensionData::new("session"), &thread_store)
        .await;

    assert_eq!(fragments.len(), 1);
    assert_eq!(fragments[0].slot(), PromptSlot::DeveloperPolicy);
    assert!(
        fragments[0]
            .text()
            .contains("Remember repository-specific implementation preferences.")
    );
}

#[tokio::test]
async fn read_tool_reads_memory_file() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let memory_root = tempdir.path().join("memories");
    tokio::fs::create_dir_all(&memory_root)
        .await
        .expect("create memories dir");
    tokio::fs::write(
        memory_root.join("MEMORY.md"),
        "first line\nsecond needle line\nthird line\n",
    )
    .await
    .expect("write memory");
    let tool = memory_tool(&memory_root, crate::READ_TOOL_NAME);
    let payload = ToolPayload::Function {
        arguments: json!({
            "path": "MEMORY.md",
            "line_offset": 2,
            "max_lines": 1
        })
        .to_string(),
    };

    let output = tool
        .handle(ToolCall {
            call_id: "call-1".to_string(),
            tool_name: memory_tool_name(crate::READ_TOOL_NAME),
            payload: payload.clone(),
        })
        .await
        .expect("read should succeed");

    assert_eq!(
        output.post_tool_use_response("call-1", &payload),
        Some(json!({
            "path": "MEMORY.md",
            "content": "second needle line\n",
            "start_line_number": 2,
            "truncated": true
        }))
    );
}

#[tokio::test]
async fn search_tool_accepts_multiple_queries() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let memory_root = tempdir.path().join("memories");
    tokio::fs::create_dir_all(&memory_root)
        .await
        .expect("create memories dir");
    tokio::fs::write(
        memory_root.join("MEMORY.md"),
        "alpha only\nneedle only\nalpha needle\n",
    )
    .await
    .expect("write memory");
    let tool = memory_tool(&memory_root, crate::SEARCH_TOOL_NAME);
    let payload = ToolPayload::Function {
        arguments: json!({
            "queries": ["alpha", "needle"],
            "case_sensitive": false
        })
        .to_string(),
    };

    let output = tool
        .handle(ToolCall {
            call_id: "call-1".to_string(),
            tool_name: memory_tool_name(crate::SEARCH_TOOL_NAME),
            payload: payload.clone(),
        })
        .await
        .expect("search should succeed");

    assert_eq!(
        output.post_tool_use_response("call-1", &payload),
        Some(json!({
            "queries": ["alpha", "needle"],
            "match_mode": {
                "type": "any"
            },
            "path": null,
            "matches": [
                {
                    "path": "MEMORY.md",
                    "match_line_number": 1,
                    "content_start_line_number": 1,
                    "content": "alpha only",
                    "matched_queries": ["alpha"]
                },
                {
                    "path": "MEMORY.md",
                    "match_line_number": 2,
                    "content_start_line_number": 2,
                    "content": "needle only",
                    "matched_queries": ["needle"]
                },
                {
                    "path": "MEMORY.md",
                    "match_line_number": 3,
                    "content_start_line_number": 3,
                    "content": "alpha needle",
                    "matched_queries": ["alpha", "needle"]
                }
            ],
            "next_cursor": null,
            "truncated": false
        }))
    );
}

#[tokio::test]
async fn search_tool_accepts_windowed_all_match_mode() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let memory_root = tempdir.path().join("memories");
    tokio::fs::create_dir_all(&memory_root)
        .await
        .expect("create memories dir");
    tokio::fs::write(memory_root.join("MEMORY.md"), "alpha\nmiddle\nneedle\n")
        .await
        .expect("write memory");
    let tool = memory_tool(&memory_root, crate::SEARCH_TOOL_NAME);
    let payload = ToolPayload::Function {
        arguments: json!({
            "queries": ["alpha", "needle"],
            "match_mode": {
                "type": "all_within_lines",
                "line_count": 3
            }
        })
        .to_string(),
    };

    let output = tool
        .handle(ToolCall {
            call_id: "call-1".to_string(),
            tool_name: memory_tool_name(crate::SEARCH_TOOL_NAME),
            payload: payload.clone(),
        })
        .await
        .expect("search should succeed");

    assert_eq!(
        output.post_tool_use_response("call-1", &payload),
        Some(json!({
            "queries": ["alpha", "needle"],
            "match_mode": {
                "type": "all_within_lines",
                "line_count": 3
            },
            "path": null,
            "matches": [
                {
                    "path": "MEMORY.md",
                    "match_line_number": 1,
                    "content_start_line_number": 1,
                    "content": "alpha\nmiddle\nneedle",
                    "matched_queries": ["alpha", "needle"]
                }
            ],
            "next_cursor": null,
            "truncated": false
        }))
    );
}

#[tokio::test]
async fn search_tool_rejects_legacy_single_query() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let memory_root = tempdir.path().join("memories");
    tokio::fs::create_dir_all(&memory_root)
        .await
        .expect("create memories dir");
    let tool = memory_tool(&memory_root, crate::SEARCH_TOOL_NAME);
    let payload = ToolPayload::Function {
        arguments: json!({
            "query": "needle",
        })
        .to_string(),
    };

    let err = tool
        .handle(ToolCall {
            call_id: "call-1".to_string(),
            tool_name: memory_tool_name(crate::SEARCH_TOOL_NAME),
            payload,
        })
        .await
        .expect_err("legacy query field should be rejected");

    assert!(err.to_string().contains("unknown field"));
    assert!(err.to_string().contains("query"));
}

fn memory_tool(memory_root: &Path, tool_name: &str) -> Arc<dyn ExtensionToolExecutor> {
    let expected_tool_name = memory_tool_name(tool_name);
    crate::tools::memory_tools(LocalMemoriesBackend::from_memory_root(memory_root))
        .into_iter()
        .find(|tool| tool.tool_name() == expected_tool_name)
        .unwrap_or_else(|| panic!("{tool_name} tool should be registered"))
}

fn memory_tool_name(tool_name: &str) -> ToolName {
    ToolName::namespaced(crate::MEMORY_TOOLS_NAMESPACE, tool_name)
}
