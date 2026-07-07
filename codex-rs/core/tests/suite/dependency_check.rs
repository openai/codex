use anyhow::Result;
use codex_features::Feature;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::request_permissions::RequestPermissionProfile;
use core_test_support::responses::ev_apply_patch_custom_tool_call;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::test_codex::TestCodexBuilder;
use core_test_support::test_codex::TestCodexHarness;
use core_test_support::test_codex::test_codex;
use serde_json::json;

fn run_dependency_e2e<F, Fut>(test: F) -> Result<()>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<()>> + 'static,
{
    std::thread::Builder::new()
        .name("dependency-check-e2e".to_string())
        .stack_size(32 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?
                .block_on(test())
        })?
        .join()
        .map_err(|_| anyhow::anyhow!("dependency check test thread panicked"))?
}

fn dependency_check_builder() -> TestCodexBuilder {
    test_codex().with_model("gpt-5.4").with_config(|config| {
        config
            .features
            .enable(Feature::DependencyCheck)
            .expect("test config should enable dependency_check");
    })
}

async fn mount_function_call(
    harness: &TestCodexHarness,
    call_id: &str,
    tool_name: &str,
    arguments: serde_json::Value,
) -> Result<()> {
    mount_sse_sequence(
        harness.server(),
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(call_id, tool_name, &serde_json::to_string(&arguments)?),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;
    Ok(())
}

#[test]
fn shell_dependency_install_redirects_without_running_npm() -> Result<()> {
    run_dependency_e2e(|| async {
        let harness = TestCodexHarness::with_builder(dependency_check_builder()).await?;
        let call_id = "dependency-shell-redirect";
        mount_function_call(
            &harness,
            call_id,
            "shell_command",
            json!({"command": "npm install zod@3.23.8"}),
        )
        .await?;

        harness.submit("add zod").await?;

        let output = harness.function_call_stdout(call_id).await;
        assert!(output.contains("Dependency Check is enabled"));
        assert!(output.contains("dependency_check"));
        assert!(!harness.path("package-lock.json").exists());
        Ok(())
    })
}

#[test]
fn unified_exec_dependency_install_redirects_without_allocating_a_process() -> Result<()> {
    run_dependency_e2e(|| async {
        let builder = dependency_check_builder().with_config(|config| {
            config.use_experimental_unified_exec_tool = true;
            config
                .features
                .enable(Feature::UnifiedExec)
                .expect("test config should enable unified exec");
        });
        let harness = TestCodexHarness::with_builder(builder).await?;
        let call_id = "dependency-unified-redirect";
        mount_function_call(
            &harness,
            call_id,
            "exec_command",
            json!({"cmd": "npm install zod@3.23.8", "yield_time_ms": 1_000}),
        )
        .await?;

        harness.submit("add zod").await?;

        let output = harness.function_call_stdout(call_id).await;
        assert!(output.contains("Dependency Check is enabled"));
        assert!(!output.contains("Process running with session ID"));
        assert!(!harness.path("package-lock.json").exists());
        Ok(())
    })
}

#[test]
fn apply_patch_rejects_dependency_manifest_edits_without_mutation() -> Result<()> {
    run_dependency_e2e(|| async {
        let harness = TestCodexHarness::with_builder(dependency_check_builder()).await?;
        harness
            .write_file("package.json", br#"{"name":"fixture","version":"1.0.0"}"#)
            .await?;
        let before = std::fs::read_to_string(harness.path("package.json"))?;
        let call_id = "dependency-apply-patch-rejection";
        let patch = "*** Begin Patch\n*** Update File: package.json\n@@\n-{\"name\":\"fixture\",\"version\":\"1.0.0\"}\n+{\"name\":\"fixture\",\"version\":\"1.0.1\"}\n*** End Patch";
        mount_sse_sequence(
            harness.server(),
            vec![
                sse(vec![
                    ev_response_created("resp-1"),
                    ev_apply_patch_custom_tool_call(call_id, patch),
                    ev_completed("resp-1"),
                ]),
                sse(vec![
                    ev_assistant_message("msg-1", "done"),
                    ev_completed("resp-2"),
                ]),
            ],
        )
        .await;

        harness.submit("edit package.json").await?;

        let output = harness.apply_patch_output(call_id).await;
        assert!(output.contains("Do not edit JavaScript dependency manifests"));
        assert_eq!(
            std::fs::read_to_string(harness.path("package.json"))?,
            before
        );
        Ok(())
    })
}

#[test]
fn request_permissions_rejects_project_write_grants() -> Result<()> {
    run_dependency_e2e(|| async {
        let builder = dependency_check_builder().with_config(|config| {
            config
                .features
                .enable(Feature::RequestPermissionsTool)
                .expect("test config should enable request_permissions");
        });
        let harness = TestCodexHarness::with_builder(builder).await?;
        let permissions = RequestPermissionProfile {
            file_system: Some(FileSystemPermissions::from_read_write_roots(
                /*read*/ None,
                Some(vec![harness.path_abs("package.json")]),
            )),
            network: None,
        };
        let call_id = "dependency-request-permissions-rejection";
        mount_function_call(
            &harness,
            call_id,
            "request_permissions",
            json!({
                "reason": "make the project writable",
                "permissions": permissions,
            }),
        )
        .await?;

        harness.submit("request project write access").await?;

        let output = harness.function_call_stdout(call_id).await;
        assert!(
            output.contains("Generic tools cannot request permissions"),
            "unexpected request_permissions output: {output}"
        );
        Ok(())
    })
}
