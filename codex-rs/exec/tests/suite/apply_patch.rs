#![allow(clippy::expect_used, clippy::unwrap_used)]

use anyhow::Context;
use assert_cmd::prelude::*;
use codex_core::CODEX_APPLY_PATCH_ARG1;
use std::fs;
use std::process::Command;
use tempfile::tempdir;

/// While we may add an `apply-patch` subcommand to the `codex` CLI multitool
/// at some point, we must ensure that the smaller `codex-exec` CLI can still
/// emulate the `apply_patch` CLI.
#[test]
fn test_standalone_exec_cli_can_use_apply_patch() -> anyhow::Result<()> {
    let tmp = tempdir()?;
    let relative_path = "source.txt";
    let absolute_path = tmp.path().join(relative_path);
    fs::write(&absolute_path, "original content\n")?;

    Command::cargo_bin("codex-exec")
        .context("should find binary for codex-exec")?
        .arg(CODEX_APPLY_PATCH_ARG1)
        .arg(
            r#"*** Begin Patch
*** Update File: source.txt
@@
-original content
+modified by apply_patch
*** End Patch"#,
        )
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout("Success. Updated the following files:\nM source.txt\n")
        .stderr(predicates::str::is_empty());
    assert_eq!(
        fs::read_to_string(absolute_path)?,
        "modified by apply_patch\n"
    );
    Ok(())
}

#[cfg(not(target_os = "windows"))]
#[tokio::test]
async fn test_apply_patch_tool() -> anyhow::Result<()> {
    use core_test_support::load_sse_fixture_with_id_from_str;
    use tempfile::TempDir;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    const SSE_TOOL_CALL_ADD: &str = r#"[
  {
    "type": "response.output_item.done",
    "item": {
      "type": "function_call",
      "name": "apply_patch",
      "arguments": "{\n  \"input\": \"*** Begin Patch\\n*** Add File: test.md\\n+Hello world\\n*** End Patch\"\n}",
      "call_id": "__ID__"
    }
  },
  {
    "type": "response.completed",
    "response": {
      "id": "__ID__",
      "usage": {
        "input_tokens": 0,
        "input_tokens_details": null,
        "output_tokens": 0,
        "output_tokens_details": null,
        "total_tokens": 0
      },
      "output": []
    }
  }
]"#;

    const SSE_TOOL_CALL_UPDATE: &str = r#"[
  {
    "type": "response.output_item.done",
    "item": {
      "type": "function_call",
      "name": "apply_patch",
      "arguments": "{\n  \"input\": \"*** Begin Patch\\n*** Update File: test.md\\n@@\\n-Hello world\\n+Final text\\n*** End Patch\"\n}",
      "call_id": "__ID__"
    }
  },
  {
    "type": "response.completed",
    "response": {
      "id": "__ID__",
      "usage": {
        "input_tokens": 0,
        "input_tokens_details": null,
        "output_tokens": 0,
        "output_tokens_details": null,
        "total_tokens": 0
      },
      "output": []
    }
  }
]"#;

    const SSE_TOOL_CALL_COMPLETED: &str = r#"[
  {
    "type": "response.completed",
    "response": {
      "id": "__ID__",
      "usage": {
        "input_tokens": 0,
        "input_tokens_details": null,
        "output_tokens": 0,
        "output_tokens_details": null,
        "total_tokens": 0
      },
      "output": []
    }
  }
]"#;

    // Start a mock model server
    let server = MockServer::start().await;

    // First response: model calls apply_patch to create test.md
    let first = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(
            load_sse_fixture_with_id_from_str(SSE_TOOL_CALL_ADD, "call1"),
            "text/event-stream",
        );

    Mock::given(method("POST"))
        // .and(path("/v1/responses"))
        .respond_with(first)
        .up_to_n_times(1)
        .mount(&server)
        .await;

    // Second response: model calls apply_patch to update test.md
    let second = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(
            load_sse_fixture_with_id_from_str(SSE_TOOL_CALL_UPDATE, "call2"),
            "text/event-stream",
        );

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(second)
        .up_to_n_times(1)
        .mount(&server)
        .await;

    let final_completed = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(
            load_sse_fixture_with_id_from_str(SSE_TOOL_CALL_COMPLETED, "resp3"),
            "text/event-stream",
        );

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(final_completed)
        .expect(1)
        .mount(&server)
        .await;

    let tmp_cwd = TempDir::new().unwrap();
    Command::cargo_bin("codex-exec")
        .context("should find binary for codex-exec")?
        .current_dir(tmp_cwd.path())
        .env("CODEX_HOME", tmp_cwd.path())
        .env("OPENAI_API_KEY", "dummy")
        .env("OPENAI_BASE_URL", format!("{}/v1", server.uri()))
        .arg("--skip-git-repo-check")
        .arg("-s")
        .arg("workspace-write")
        .arg("foo")
        .assert()
        .success();

    // Verify final file contents
    let final_path = tmp_cwd.path().join("test.md");
    let contents = std::fs::read_to_string(&final_path)
        .unwrap_or_else(|e| panic!("failed reading {}: {e}", final_path.display()));
    assert_eq!(contents, "Final text\n");
    Ok(())
}

#[cfg(not(target_os = "windows"))]
#[tokio::test]
async fn test_apply_patch_freeform_tool() -> anyhow::Result<()> {
    use core_test_support::load_sse_fixture_with_id_from_str;
    use tempfile::TempDir;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    const SSE_TOOL_CALL_ADD: &str = r#"[
  {
    "type": "response.output_item.done",
    "item": {
      "type": "custom_tool_call",
      "name": "apply_patch",
      "input": "*** Begin Patch\n*** Add File: test.md\n+Hello world\n*** End Patch",
      "call_id": "__ID__"
    }
  },
  {
    "type": "response.completed",
    "response": {
      "id": "__ID__",
      "usage": {
        "input_tokens": 0,
        "input_tokens_details": null,
        "output_tokens": 0,
        "output_tokens_details": null,
        "total_tokens": 0
      },
      "output": []
    }
  }
]"#;

    const SSE_TOOL_CALL_UPDATE: &str = r#"[
  {
    "type": "response.output_item.done",
    "item": {
      "type": "custom_tool_call",
      "name": "apply_patch",
      "input": "*** Begin Patch\n*** Update File: test.md\n@@\n-Hello world\n+Final text\n*** End Patch",
      "call_id": "__ID__"
    }
  },
  {
    "type": "response.completed",
    "response": {
      "id": "__ID__",
      "usage": {
        "input_tokens": 0,
        "input_tokens_details": null,
        "output_tokens": 0,
        "output_tokens_details": null,
        "total_tokens": 0
      },
      "output": []
    }
  }
]"#;

    const SSE_TOOL_CALL_COMPLETED: &str = r#"[
  {
    "type": "response.completed",
    "response": {
      "id": "__ID__",
      "usage": {
        "input_tokens": 0,
        "input_tokens_details": null,
        "output_tokens": 0,
        "output_tokens_details": null,
        "total_tokens": 0
      },
      "output": []
    }
  }
]"#;

    // Start a mock model server
    let server = MockServer::start().await;

    // First response: model calls apply_patch to create test.md
    let first = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(
            load_sse_fixture_with_id_from_str(SSE_TOOL_CALL_ADD, "call1"),
            "text/event-stream",
        );

    Mock::given(method("POST"))
        // .and(path("/v1/responses"))
        .respond_with(first)
        .up_to_n_times(1)
        .mount(&server)
        .await;

    // Second response: model calls apply_patch to update test.md
    let second = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(
            load_sse_fixture_with_id_from_str(SSE_TOOL_CALL_UPDATE, "call2"),
            "text/event-stream",
        );

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(second)
        .up_to_n_times(1)
        .mount(&server)
        .await;

    let final_completed = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(
            load_sse_fixture_with_id_from_str(SSE_TOOL_CALL_COMPLETED, "resp3"),
            "text/event-stream",
        );

    Mock::given(method("POST"))
        // .and(path("/v1/responses"))
        .respond_with(final_completed)
        .expect(1)
        .mount(&server)
        .await;

    let tmp_cwd = TempDir::new().unwrap();
    Command::cargo_bin("codex-exec")
        .context("should find binary for codex-exec")?
        .current_dir(tmp_cwd.path())
        .env("CODEX_HOME", tmp_cwd.path())
        .env("OPENAI_API_KEY", "dummy")
        .env("OPENAI_BASE_URL", format!("{}/v1", server.uri()))
        .arg("--skip-git-repo-check")
        .arg("-s")
        .arg("workspace-write")
        .arg("foo")
        .assert()
        .success();

    // Verify final file contents
    let final_path = tmp_cwd.path().join("test.md");
    let contents = std::fs::read_to_string(&final_path)
        .unwrap_or_else(|e| panic!("failed reading {}: {e}", final_path.display()));
    assert_eq!(contents, "Final text\n");
    Ok(())
}
