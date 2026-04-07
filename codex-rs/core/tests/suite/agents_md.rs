use anyhow::Result;
use codex_exec_server::CreateDirectoryOptions;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::TestCodexBuilder;
use core_test_support::test_codex::test_codex;

async fn agents_instructions(mut builder: TestCodexBuilder) -> Result<String> {
    let server = start_mock_server().await;
    let resp_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp1"), ev_completed("resp1")]),
    )
    .await;

    let test = builder.build_remote_aware(&server).await?;
    test.submit_turn("hello").await?;

    let request = resp_mock.single_request();
    request
        .message_input_texts("user")
        .into_iter()
        .find(|text| text.starts_with("# AGENTS.md instructions for "))
        .ok_or_else(|| anyhow::anyhow!("instructions message not found"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn agents_override_is_preferred_over_agents_md() -> Result<()> {
    let instructions =
        agents_instructions(test_codex().with_workspace_setup(|cwd, fs| async move {
            let agents_md = cwd.join("AGENTS.md").expect("absolute AGENTS.md path");
            let override_md = cwd
                .join("AGENTS.override.md")
                .expect("absolute AGENTS.override.md path");
            fs.write_file(&agents_md, b"base doc".to_vec()).await?;
            fs.write_file(&override_md, b"override doc".to_vec())
                .await?;
            Ok::<(), anyhow::Error>(())
        }))
        .await?;

    assert!(
        instructions.contains("override doc"),
        "expected AGENTS.override.md contents: {instructions}"
    );
    assert!(
        !instructions.contains("base doc"),
        "expected AGENTS.md to be ignored when override exists: {instructions}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn configured_fallback_is_used_when_agents_candidate_is_directory() -> Result<()> {
    let instructions = agents_instructions(
        test_codex()
            .with_config(|config| {
                config.project_doc_fallback_filenames = vec!["WORKFLOW.md".to_string()];
            })
            .with_workspace_setup(|cwd, fs| async move {
                let agents_dir = cwd.join("AGENTS.md").expect("absolute AGENTS.md path");
                let fallback = cwd.join("WORKFLOW.md").expect("absolute WORKFLOW.md path");
                fs.create_directory(&agents_dir, CreateDirectoryOptions { recursive: true })
                    .await?;
                fs.write_file(&fallback, b"fallback doc".to_vec()).await?;
                Ok::<(), anyhow::Error>(())
            }),
    )
    .await?;

    assert!(
        instructions.contains("fallback doc"),
        "expected fallback doc contents: {instructions}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn agents_docs_are_concatenated_from_project_root_to_cwd() -> Result<()> {
    let instructions = agents_instructions(
        test_codex()
            .with_config(|config| {
                config.cwd = config
                    .cwd
                    .join("nested/workspace")
                    .expect("absolute nested workspace path");
            })
            .with_workspace_setup(|cwd, fs| async move {
                let nested = cwd.clone();
                let root = nested
                    .parent()
                    .and_then(|parent| parent.parent())
                    .expect("nested workspace should have a project root ancestor");
                let root_agents = root
                    .join("AGENTS.md")
                    .expect("absolute root AGENTS.md path");
                let git_marker = root.join(".git").expect("absolute .git path");
                let nested_agents = nested
                    .join("AGENTS.md")
                    .expect("absolute nested AGENTS.md path");

                fs.create_directory(&nested, CreateDirectoryOptions { recursive: true })
                    .await?;
                fs.write_file(&root_agents, b"root doc".to_vec()).await?;
                fs.write_file(&git_marker, b"gitdir: /tmp/mock-git-dir\n".to_vec())
                    .await?;
                fs.write_file(&nested_agents, b"child doc".to_vec()).await?;
                Ok::<(), anyhow::Error>(())
            }),
    )
    .await?;

    let root_pos = instructions
        .find("root doc")
        .expect("expected root doc in AGENTS instructions");
    let child_pos = instructions
        .find("child doc")
        .expect("expected child doc in AGENTS instructions");
    assert!(
        root_pos < child_pos,
        "expected root doc before child doc: {instructions}"
    );

    Ok(())
}
